use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, RwLock};
use tokio::time::interval;

// ============================================================================
// Health Status Enum
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    Recovering,
    Dead,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Starting => write!(f, "STARTING"),
            HealthStatus::Healthy => write!(f, "HEALTHY"),
            HealthStatus::Degraded => write!(f, "DEGRADED"),
            HealthStatus::Unhealthy => write!(f, "UNHEALTHY"),
            HealthStatus::Recovering => write!(f, "RECOVERING"),
            HealthStatus::Dead => write!(f, "DEAD"),
        }
    }
}

// ============================================================================
// Agent Health Record
// ============================================================================

#[derive(Debug, Clone)]
pub struct AgentHealth {
    pub agent_id: usize,
    pub status: HealthStatus,
    pub last_heartbeat: Instant,
    pub start_time: Instant,
    pub restart_count: u64,
    pub consecutive_failures: u64,
    pub total_tasks_executed: u64,
    pub last_error: Option<String>,
}

impl AgentHealth {
    pub fn new(agent_id: usize) -> Self {
        Self {
            agent_id,
            status: HealthStatus::Starting,
            last_heartbeat: Instant::now(),
            start_time: Instant::now(),
            restart_count: 0,
            consecutive_failures: 0,
            total_tasks_executed: 0,
            last_error: None,
        }
    }

    pub fn is_alive(&self) -> bool {
        matches!(
            self.status,
            HealthStatus::Starting | HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Recovering
        )
    }

    pub fn time_since_heartbeat(&self) -> Duration {
        self.last_heartbeat.elapsed()
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

// ============================================================================
// Health Monitoring Config
// ============================================================================

#[derive(Debug, Clone)]
pub struct HealthConfig {
    pub heartbeat_interval_ms: u64,
    pub health_check_interval_ms: u64,
    pub max_heartbeat_delay_ms: u64,
    pub max_restart_attempts: u64,
    pub recovery_delay_ms: u64,
    pub enable_auto_recovery: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: 5000,
            health_check_interval_ms: 2000,
            max_heartbeat_delay_ms: 15000,
            max_restart_attempts: 3,
            recovery_delay_ms: 1000,
            enable_auto_recovery: true,
        }
    }
}

// ============================================================================
// Swarm Health Monitor
// ============================================================================

#[derive(Debug)]
pub struct SwarmHealthMonitor {
    agents: Arc<RwLock<std::collections::HashMap<usize, AgentHealth>>>,
    config: HealthConfig,
    shutdown_flag: Arc<AtomicBool>,
    events_tx: watch::Sender<HealthEvent>,
}

#[derive(Debug, Clone)]
pub enum HealthEvent {
    AgentDied { agent_id: usize, reason: String },
    AgentRecovered { agent_id: usize },
    AgentRestarted { agent_id: usize, attempt: u64 },
    AgentUnhealthy { agent_id: usize, reason: String },
    SwarmDegraded { healthy_count: usize, total_count: usize },
    SwarmHealthy,
}

impl SwarmHealthMonitor {
    pub fn new(config: HealthConfig) -> Self {
        let (events_tx, _) = watch::channel(HealthEvent::SwarmHealthy);
        Self {
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            events_tx,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<HealthEvent> {
        self.events_tx.subscribe()
    }

    pub async fn register_agent(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;
        agents.insert(agent_id, AgentHealth::new(agent_id));
        let _ = self.events_tx.send(HealthEvent::SwarmHealthy);
    }

    pub async fn unregister_agent(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;
        if let Some(health) = agents.get_mut(&agent_id) {
            health.status = HealthStatus::Dead;
        }
    }

    pub async fn heartbeat(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;
        if let Some(health) = agents.get_mut(&agent_id) {
            health.last_heartbeat = Instant::now();
            if health.status == HealthStatus::Unhealthy {
                health.status = HealthStatus::Recovering;
            }
        }
    }

    pub async fn report_task_complete(&self, agent_id: usize, success: bool) {
        let mut agents = self.agents.write().await;
        if let Some(health) = agents.get_mut(&agent_id) {
            health.total_tasks_executed += 1;
            if success {
                health.consecutive_failures = 0;
                health.status = HealthStatus::Healthy;
            } else {
                health.consecutive_failures += 1;
                if health.consecutive_failures >= 3 {
                    health.status = HealthStatus::Degraded;
                }
            }
        }
    }

    pub async fn report_error(&self, agent_id: usize, error: String) {
        let mut agents = self.agents.write().await;
        if let Some(health) = agents.get_mut(&agent_id) {
            health.last_error = Some(error.clone());
            health.consecutive_failures += 1;
            if health.consecutive_failures >= 3 {
                health.status = HealthStatus::Unhealthy;
                let _ = self.events_tx.send(HealthEvent::AgentUnhealthy {
                    agent_id,
                    reason: error,
                });
            }
        }
    }

    pub async fn get_agent_health(&self, agent_id: usize) -> Option<AgentHealth> {
        let agents = self.agents.read().await;
        agents.get(&agent_id).cloned()
    }

    pub async fn get_all_health(&self) -> Vec<AgentHealth> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    pub async fn get_swarm_status(&self) -> (HealthStatus, usize, usize) {
        let agents = self.agents.read().await;
        let total = agents.len();
        let healthy = agents
            .values()
            .filter(|h| h.status == HealthStatus::Healthy || h.status == HealthStatus::Degraded)
            .count();

        let overall_status = if healthy == total {
            HealthStatus::Healthy
        } else if healthy > 0 {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        (overall_status, healthy, total)
    }

    pub async fn check_stale_agents(&self) -> Vec<usize> {
        let mut stale = Vec::new();
        let mut agents = self.agents.write().await;
        let max_delay = Duration::from_millis(self.config.max_heartbeat_delay_ms);

        for (agent_id, health) in agents.iter_mut() {
            if health.is_alive() && health.time_since_heartbeat() > max_delay {
                health.status = HealthStatus::Unhealthy;
                stale.push(*agent_id);
            }
        }
        stale
    }

    pub fn should_restart(&self, agent_id: usize) -> bool {
        if !self.config.enable_auto_recovery {
            return false;
        }
        // This is called after checking health - rely on agents lock for actual count
        true
    }

    pub async fn mark_dead(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;
        if let Some(health) = agents.get_mut(&agent_id) {
            health.status = HealthStatus::Dead;
        }
    }

    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown_flag.load(Ordering::SeqCst)
    }
}

// ============================================================================
// Background Health Checker
// ============================================================================

pub struct HealthChecker {
    monitor: Arc<SwarmHealthMonitor>,
    config: HealthConfig,
}

impl HealthChecker {
    pub fn new(monitor: Arc<SwarmHealthMonitor>, config: HealthConfig) -> Self {
        Self { monitor, config }
    }

    pub async fn run(&self) {
        let mut check_interval = interval(Duration::from_millis(self.config.health_check_interval_ms));

        loop {
            check_interval.tick().await;

            if self.monitor.is_shutdown() {
                tracing::info!("Health checker shutting down");
                break;
            }

            let stale = self.monitor.check_stale_agents().await;

            if !stale.is_empty() {
                tracing::warn!("Detected {} stale agents: {:?}", stale.len(), stale);
                for agent_id in stale {
                    let _ = self.monitor.events_tx.send(HealthEvent::AgentUnhealthy {
                        agent_id,
                        reason: "Missed heartbeat".to_string(),
                    });
                }
            }

            let (status, healthy, total) = self.monitor.get_swarm_status().await;
            if status == HealthStatus::Unhealthy && total > 0 && healthy == 0 {
                let _ = self.monitor.events_tx.send(HealthEvent::SwarmDegraded { healthy_count: healthy, total_count: total });
            }
        }
    }
}

// ============================================================================
// Recovery Manager
// ============================================================================

#[derive(Debug, Clone)]
pub struct RecoveryAction {
    pub agent_id: usize,
    pub action: RecoveryActionType,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum RecoveryActionType {
    Restart,
    Reset,
    Kill,
}

pub struct RecoveryManager {
    monitor: Arc<SwarmHealthMonitor>,
    config: HealthConfig,
    recovery_tx: tokio::sync::mpsc::Sender<RecoveryAction>,
}

impl RecoveryManager {
    pub fn new(
        monitor: Arc<SwarmHealthMonitor>,
        config: HealthConfig,
        recovery_tx: tokio::sync::mpsc::Sender<RecoveryAction>,
    ) -> Self {
        Self {
            monitor,
            config,
            recovery_tx,
        }
    }

    pub async fn run(&mut self) {
        let mut health_stream = self.monitor.subscribe();

        loop {
            health_stream.changed().await.ok();
            let event = (*health_stream.borrow()).clone();
            self.handle_event(&event).await;
        }
    }

    async fn handle_event(&self, event: &HealthEvent) {
        match event {
            HealthEvent::AgentUnhealthy { agent_id, reason } => {
                if self.monitor.config.enable_auto_recovery {
                    tracing::warn!(
                        "Agent {} is unhealthy: {}. Initiating recovery...",
                        agent_id,
                        reason
                    );

                    let agents = self.monitor.agents.read().await;
                    let restart_count = agents
                        .get(agent_id)
                        .map(|h| h.restart_count)
                        .unwrap_or(0);

                    if restart_count < self.config.max_restart_attempts {
                        let _ = self.recovery_tx.send(RecoveryAction {
                            agent_id: *agent_id,
                            action: RecoveryActionType::Restart,
                            reason: reason.clone(),
                        }).await;
                    } else {
                        tracing::error!(
                            "Agent {} exceeded max restart attempts ({}). Marking as dead.",
                            agent_id,
                            self.config.max_restart_attempts
                        );
                        self.monitor.mark_dead(*agent_id).await;
                    }
                }
            }
            _ => {}
        }
    }
}