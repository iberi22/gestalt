//! Agent Pool - Pre-warmed agent reuse for cold-start latency reduction
//!
//! Instead of creating fresh agents per task (expensive cold-start),
//! we maintain a pre_warm pool (PreWarmPool / warm_pool) of pre-warmed agents
//! ready to execute immediately. It implements robust lifecycle management.

use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Lifecycle state of a pooled agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentState {
    /// Agent is newly created or currently initializing
    Pending,
    /// Agent is fully pre-warmed and ready for immediate task execution
    Warm,
    /// Agent is checked out and currently running a task
    Running,
    /// Agent completed its task and is cooling down/cleaning up resources
    Cooling,
    /// Agent has completed cooling and is idle in the pool
    Idle,
}

/// Configuration for the agent pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Initial number of pre_warm/pre-warmed agents to keep ready
    pub pre_warm: usize,
    /// Maximum pool size (agents are evicted when idle beyond max_idle_secs)
    pub max_size: usize,
    /// Maximum idle time before agent is considered stale (seconds)
    pub max_idle_secs: u64,
    /// Enable aggressive eager pre_warm / pre-warming on pool creation
    pub eager_pre_warm: bool,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            pre_warm: 2,
            max_size: 8,
            max_idle_secs: 300, // 5 minutes
            eager_pre_warm: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_keeps_n_agents_warm_after_initialization() {
        // Initialize pool with pre_warm target of 3 agents
        let config = PoolConfig::new(3, 5).with_eager_pre_warm(true);
        let pool = AgentPool::new(config);

        // Give the background lifecycle worker thread a tiny bit of time to initialize and pre-warm
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Verify that pool size has reached 3, and all 3 are in the available (Warm) queue
        assert_eq!(pool.size().await, 3);
        assert_eq!(pool.available_count().await, 3);

        let stats = pool.stats().await;
        assert_eq!(stats.warm_count, 3);
    }

    #[tokio::test]
    async fn test_pool_checkout_and_checkin_lifecycle() {
        // Use max_size of 2 equal to pre_warm so pool cannot spawn extra agents,
        // forcing it to reuse the agent and transition it back to Warm.
        let config = PoolConfig::new(2, 2);
        let pool = AgentPool::new(config);

        tokio::time::sleep(Duration::from_millis(150)).await;

        assert_eq!(pool.available_count().await, 2);

        // Checkout an agent
        let agent_id_1 = pool.checkout().await;
        assert!(agent_id_1.is_some());
        let id1 = agent_id_1.unwrap();

        // Verify available count goes down to 1
        assert_eq!(pool.available_count().await, 1);

        // Verify checked out agent has state = Running
        {
            let agents = pool.agents.read().await;
            let agent = agents.iter().find(|a| a.id == id1).unwrap();
            assert_eq!(agent.state, AgentState::Running);
        }

        // Return agent to pool
        pool.checkin(id1).await;

        // Immediately after checkin, agent is in Cooling state
        {
            let agents = pool.agents.read().await;
            let agent = agents.iter().find(|a| a.id == id1).unwrap();
            assert_eq!(agent.state, AgentState::Cooling);
        }

        // Wait for cooldown to transition to Idle, and then background loop transitions Idle -> Warm
        tokio::time::sleep(Duration::from_millis(100)).await;

        {
            let agents = pool.agents.read().await;
            let agent = agents.iter().find(|a| a.id == id1).unwrap();
            assert_eq!(agent.state, AgentState::Warm);
        }

        assert_eq!(pool.available_count().await, 2);
    }

    #[tokio::test]
    async fn test_pooled_agent_guard_automatic_checkin() {
        // Use max_size of 1 equal to pre_warm so pool cannot spawn extra agents,
        // forcing it to reuse the agent and transition it back to Warm.
        let config = PoolConfig::new(1, 1);
        let pool = AgentPool::new(config);

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(pool.available_count().await, 1);

        let agent_id = {
            let id = pool.checkout().await.unwrap();
            let _guard = PooledAgentGuard::new(&pool, id);
            assert_eq!(pool.available_count().await, 0);
            id
            // guard goes out of scope here, triggering drop and async checkin
        };

        // Give checkin task a moment to run and transition agent state
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify that agent was checked back in and is back to Warm state
        assert_eq!(pool.available_count().await, 1);
        {
            let agents = pool.agents.read().await;
            let agent = agents.iter().find(|a| a.id == agent_id).unwrap();
            assert_eq!(agent.state, AgentState::Warm);
        }
    }

    #[tokio::test]
    async fn test_pool_stats_calculation() {
        let config = PoolConfig::new(2, 4);
        let pool = AgentPool::new(config);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Perform checkouts and checkins
        let id1 = pool.checkout().await.unwrap();
        let id2 = pool.checkout().await.unwrap();
        let id3 = pool.checkout().await; // None (cache miss)

        assert!(id3.is_none());

        pool.checkin(id1).await;
        pool.checkin(id2).await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        let stats = pool.stats().await;
        assert_eq!(stats.checkouts, 3);
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 2.0 / 3.0);
        assert!(stats.avg_warm_time_ms > 0);
    }
}

/// Statistics about pool usage, including warm_pool and lifecycle metrics
#[derive(Debug, Default, Clone)]
pub struct PoolStats {
    pub checkouts: u64,
    pub checkins: u64,
    pub evictions: u64,
    pub pre_warm_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub avg_wait_time_ms: u64,
    // --- Lifecycle and pre_warm/warm_pool Metrics ---
    /// Number of agents currently in Warm state
    pub warm_count: usize,
    /// Ratio of cache hits to total checkouts requested
    pub hit_rate: f64,
    /// Average time spent in Warm state before checkout (milliseconds)
    pub avg_warm_time_ms: u64,
}

impl PoolConfig {
    pub fn new(pre_warm: usize, max_size: usize) -> Self {
        Self {
            pre_warm,
            max_size,
            ..Default::default()
        }
    }

    pub fn with_idle_timeout(mut self, secs: u64) -> Self {
        self.max_idle_secs = secs;
        self
    }

    pub fn with_eager_pre_warm(mut self, eager: bool) -> Self {
        self.eager_pre_warm = eager;
        self
    }
}

/// A pre-warmed agent ready for immediate use, tracked via the lifecycle state machine
#[derive(Debug, Clone)]
pub struct PooledAgent {
    /// Agent ID within the pool
    pub id: usize,
    /// When this agent was last used/modified
    pub last_used: Instant,
    /// Number of times this agent has been reused
    pub reuse_count: u64,
    /// The current lifecycle state of this agent
    pub state: AgentState,
    /// Instant when the agent entered the Warm state
    pub warm_since: Option<Instant>,
    /// Instant when the agent entered the Cooling state
    pub cooling_since: Option<Instant>,
}

impl PooledAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            last_used: Instant::now(),
            reuse_count: 0,
            state: AgentState::Pending,
            warm_since: None,
            cooling_since: None,
        }
    }

    /// Transition agent to Warm state
    pub fn to_warm(&mut self) {
        self.state = AgentState::Warm;
        self.warm_since = Some(Instant::now());
        self.cooling_since = None;
        self.last_used = Instant::now();
    }

    /// Checkout the agent, transitioning to Running. Returns elapsed warm time.
    pub fn checkout(&mut self) -> Option<Duration> {
        self.state = AgentState::Running;
        let elapsed = self.warm_since.take().map(|i| i.elapsed());
        self.last_used = Instant::now();
        self.reuse_count += 1;
        elapsed
    }

    /// Checkin the agent, transitioning to Cooling.
    pub fn checkin(&mut self) {
        self.state = AgentState::Cooling;
        self.cooling_since = Some(Instant::now());
        self.last_used = Instant::now();
    }

    /// Check if the agent is idle and has exceeded maximum allowed idle time
    pub fn is_stale(&self, max_idle: Duration) -> bool {
        matches!(self.state, AgentState::Warm | AgentState::Idle)
            && self.last_used.elapsed() > max_idle
    }

    /// Check if the agent is idle
    pub fn is_idle(&self) -> bool {
        matches!(self.state, AgentState::Idle)
    }
}

/// Thread-safe agent pool with pre_warm and lifecycle management support
pub struct AgentPool {
    /// Pool configuration
    config: PoolConfig,
    /// Available agents (in Warm state, not checked out)
    available: Arc<RwLock<VecDeque<usize>>>,
    /// All agents (metadata and state)
    agents: Arc<RwLock<Vec<PooledAgent>>>,
    /// Statistics
    stats: Arc<RwLock<PoolStats>>,
    /// Total agents created (for unique IDs)
    next_id: Arc<RwLock<usize>>,
    /// Wait time tracking for stats
    wait_times: Arc<RwLock<Vec<u64>>>,
    /// Warm time tracking for stats (duration in Warm state before checkout)
    warm_times: Arc<RwLock<Vec<u64>>>,
}

impl AgentPool {
    /// Create a new agent pool and automatically start its lifecycle / pre_warm management
    pub fn new(config: PoolConfig) -> Self {
        let pool = Self {
            config: config.clone(),
            available: Arc::new(RwLock::new(VecDeque::new())),
            agents: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(PoolStats::default())),
            next_id: Arc::new(RwLock::new(0)),
            wait_times: Arc::new(RwLock::new(Vec::new())),
            warm_times: Arc::new(RwLock::new(Vec::new())),
        };

        // Spawn the background lifecycle / PreWarmPool worker
        let config_clone = config;
        let agents_weak = Arc::downgrade(&pool.agents);
        let available_weak = Arc::downgrade(&pool.available);
        let stats_weak = Arc::downgrade(&pool.stats);
        let next_id_weak = Arc::downgrade(&pool.next_id);
        let wait_times_weak = Arc::downgrade(&pool.wait_times);
        let warm_times_weak = Arc::downgrade(&pool.warm_times);

        tokio::spawn(async move {
            Self::manage_lifecycle_loop(
                config_clone,
                agents_weak,
                available_weak,
                stats_weak,
                next_id_weak,
                wait_times_weak,
                warm_times_weak,
            )
            .await;
        });

        pool
    }

    /// Asynchronous background worker that maintains the pre_warm target and coordinates agent lifecycle transitions
    async fn manage_lifecycle_loop(
        config: PoolConfig,
        agents_weak: std::sync::Weak<RwLock<Vec<PooledAgent>>>,
        available_weak: std::sync::Weak<RwLock<VecDeque<usize>>>,
        stats_weak: std::sync::Weak<RwLock<PoolStats>>,
        next_id_weak: std::sync::Weak<RwLock<usize>>,
        _wait_times_weak: std::sync::Weak<RwLock<Vec<u64>>>,
        _warm_times_weak: std::sync::Weak<RwLock<Vec<u64>>>,
    ) {
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Upgrade weak references. If upgrade fails, all pool handles are dropped; shutdown the task.
            let agents_arc = match agents_weak.upgrade() {
                Some(arc) => arc,
                None => break,
            };
            let available_arc = match available_weak.upgrade() {
                Some(arc) => arc,
                None => break,
            };
            let stats_arc = match stats_weak.upgrade() {
                Some(arc) => arc,
                None => break,
            };
            let next_id_arc = match next_id_weak.upgrade() {
                Some(arc) => arc,
                None => break,
            };

            let mut agents = agents_arc.write().await;
            let mut available = available_arc.write().await;

            let mut to_add_to_available = Vec::new();
            let mut to_evict = Vec::new();

            // Calculate active count beforehand to avoid borrow checker conflicts
            let mut active_count = agents
                .iter()
                .filter(|a| matches!(a.state, AgentState::Warm | AgentState::Pending))
                .count();

            // 1. Advance states of existing agents (using sequential transitions to allow multi-step transitions in a single tick)
            for agent in agents.iter_mut() {
                if agent.state == AgentState::Pending {
                    // Pending -> Warm
                    agent.to_warm();
                    to_add_to_available.push(agent.id);
                }

                if agent.state == AgentState::Cooling {
                    // Cooling -> Idle (cooldown complete after 20ms)
                    if let Some(cooling_since) = agent.cooling_since {
                        if cooling_since.elapsed() >= Duration::from_millis(20) {
                            agent.state = AgentState::Idle;
                            agent.cooling_since = None;
                        }
                    } else {
                        agent.state = AgentState::Idle;
                    }
                }

                if agent.state == AgentState::Idle {
                    // Idle -> Warm (if we need to maintain the pre_warm count)
                    if active_count < config.pre_warm {
                        agent.to_warm();
                        to_add_to_available.push(agent.id);
                        active_count += 1;
                    }
                }

                // Identify stale agents (Warm or Idle beyond max_idle_secs)
                if agent.is_stale(Duration::from_secs(config.max_idle_secs)) {
                    to_evict.push(agent.id);
                }
            }

            // Evict stale agents
            for id in to_evict {
                if let Some(pos) = agents.iter().position(|a| a.id == id) {
                    agents.remove(pos);
                    available.retain(|&x| x != id);
                    let mut stats = stats_arc.write().await;
                    stats.evictions += 1;
                    debug!("Pool lifecycle manager: evicted stale agent {}", id);
                }
            }

            // Add newly warmed agents to the available queue
            for id in to_add_to_available {
                if !available.contains(&id) {
                    available.push_back(id);
                }
            }

            // 2. Replenish pool to ensure the pre_warm targets are met
            let active_count = agents
                .iter()
                .filter(|a| matches!(a.state, AgentState::Warm | AgentState::Pending))
                .count();

            if active_count < config.pre_warm && agents.len() < config.max_size {
                let to_create = config
                    .pre_warm
                    .saturating_sub(active_count)
                    .min(config.max_size.saturating_sub(agents.len()));

                for _ in 0..to_create {
                    let id = {
                        let mut next = next_id_arc.write().await;
                        let id = *next;
                        *next += 1;
                        id
                    };
                    agents.push(PooledAgent::new(id));
                    let mut stats = stats_arc.write().await;
                    stats.pre_warm_requests += 1;
                    debug!("Pool lifecycle manager: registered new agent {}", id);
                }
            }
        }
    }

    /// Get an available agent ID from the warm_pool
    pub async fn checkout(&self) -> Option<usize> {
        let start = Instant::now();

        let agent_id = {
            let mut available = self.available.write().await;
            available.pop_front()
        };

        let wait_time_ms = start.elapsed().as_millis() as u64;

        if let Some(id) = agent_id {
            // Mark agent as Running (and record how long it was pre-warmed)
            let mut agents = self.agents.write().await;
            if let Some(agent) = agents.iter_mut().find(|a| a.id == id) {
                if let Some(warm_duration) = agent.checkout() {
                    let mut warm_times = self.warm_times.write().await;
                    warm_times.push(warm_duration.as_millis() as u64);
                    if warm_times.len() > 100 {
                        warm_times.remove(0);
                    }
                }
            }

            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.checkouts += 1;
                stats.hits += 1;
            }
            {
                let mut times = self.wait_times.write().await;
                times.push(wait_time_ms);
                if times.len() > 100 {
                    times.remove(0);
                }
            }

            debug!(
                "Pool checkout: agent {} (wait: {}ms, reuse: {})",
                id,
                wait_time_ms,
                agents
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| a.reuse_count)
                    .unwrap_or(0)
            );

            Some(id)
        } else {
            // No pre-warmed agent available (miss)
            {
                let mut stats = self.stats.write().await;
                stats.checkouts += 1;
                stats.misses += 1;
            }
            {
                let mut times = self.wait_times.write().await;
                times.push(wait_time_ms);
                if times.len() > 100 {
                    times.remove(0);
                }
            }
            None
        }
    }

    /// Return an agent to the pool, entering the Cooling state
    pub async fn checkin(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.iter_mut().find(|a| a.id == agent_id) {
            agent.checkin(); // Transition to Cooling

            // Check if we should evict immediately because total exceeds max_size
            let pool_size = agents
                .iter()
                .filter(|a| !matches!(a.state, AgentState::Running) || a.id == agent_id)
                .count();

            if pool_size > self.config.max_size {
                drop(agents);
                self.evict(agent_id).await;
                return;
            }

            let mut stats = self.stats.write().await;
            stats.checkins += 1;
        }
    }

    /// Register a new pre-warmed agent manually
    pub async fn register(&self) -> usize {
        let id = {
            let mut next = self.next_id.write().await;
            let id = *next;
            *next += 1;
            id
        };

        let mut agents = self.agents.write().await;
        agents.push(PooledAgent::new(id));

        let mut stats = self.stats.write().await;
        stats.pre_warm_requests += 1;

        debug!("Pool registered agent manually {}", id);
        id
    }

    /// Explicitly trigger pre_warm to ensure target is met
    pub async fn pre_warm(&self) -> usize {
        let current_size = {
            let agents = self.agents.read().await;
            agents.len()
        };

        let to_create = if self.config.eager_pre_warm {
            self.config.pre_warm.saturating_sub(current_size)
        } else if current_size == 0 {
            1
        } else {
            0
        };

        for _ in 0..to_create {
            self.register().await;
        }

        let new_size = {
            let agents = self.agents.read().await;
            agents.len()
        };

        info!(
            "Pool pre-warmed manually: {} -> {} agents (target: {})",
            current_size, new_size, self.config.pre_warm
        );

        new_size
    }

    /// Evict a specific agent from the pool immediately
    async fn evict(&self, agent_id: usize) {
        {
            let mut available = self.available.write().await;
            available.retain(|&id| id != agent_id);
        }

        {
            let mut agents = self.agents.write().await;
            agents.retain(|a| a.id != agent_id);
        }

        let mut stats = self.stats.write().await;
        stats.evictions += 1;

        debug!("Pool evicted agent {}", agent_id);
    }

    /// Evict all stale agents (idle beyond max_idle_secs)
    pub async fn evict_stale(&self) -> usize {
        let max_idle = Duration::from_secs(self.config.max_idle_secs);
        let mut evictions = 0;

        let stale_ids: Vec<usize> = {
            let agents = self.agents.read().await;
            agents
                .iter()
                .filter(|a| a.is_stale(max_idle))
                .map(|a| a.id)
                .collect()
        };

        for id in stale_ids {
            self.evict(id).await;
            evictions += 1;
        }

        if evictions > 0 {
            info!("Pool evicted {} stale agents", evictions);
        }

        evictions
    }

    /// Get current pool statistics, computing warm_count, hit_rate, and avg_warm_time_ms
    pub async fn stats(&self) -> PoolStats {
        let mut result = {
            let stats = self.stats.read().await;
            stats.clone()
        };

        // Calculate avg wait time
        let times = self.wait_times.read().await;
        if !times.is_empty() {
            result.avg_wait_time_ms = times.iter().sum::<u64>() / times.len() as u64;
        }

        // Calculate warm count
        let agents = self.agents.read().await;
        result.warm_count = agents
            .iter()
            .filter(|a| a.state == AgentState::Warm)
            .count();

        // Calculate hit rate
        let total_requests = result.hits + result.misses;
        if total_requests > 0 {
            result.hit_rate = result.hits as f64 / total_requests as f64;
        } else {
            result.hit_rate = 0.0;
        }

        // Calculate avg warm time
        let warm_times = self.warm_times.read().await;
        if !warm_times.is_empty() {
            result.avg_warm_time_ms = warm_times.iter().sum::<u64>() / warm_times.len() as u64;
        } else {
            result.avg_warm_time_ms = 0;
        }

        result
    }

    /// Get pool size (total registered agents in any state)
    pub async fn size(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Get number of available (ready/warm) agents
    pub async fn available_count(&self) -> usize {
        let available = self.available.read().await;
        available.len()
    }
}

impl Default for AgentPool {
    fn default() -> Self {
        Self::new(PoolConfig::default())
    }
}

/// Pool-aware agent guard - wraps agent ID with automatic checkin on drop
pub struct PooledAgentGuard<'a> {
    pool: &'a AgentPool,
    agent_id: usize,
}

impl<'a> PooledAgentGuard<'a> {
    pub fn new(pool: &'a AgentPool, agent_id: usize) -> Self {
        Self { pool, agent_id }
    }

    pub fn agent_id(&self) -> usize {
        self.agent_id
    }
}

impl<'a> Drop for PooledAgentGuard<'a> {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let agent_id = self.agent_id;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                pool.checkin(agent_id).await;
            });
        }
    }
}

impl Clone for AgentPool {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            available: self.available.clone(),
            agents: self.agents.clone(),
            stats: self.stats.clone(),
            next_id: self.next_id.clone(),
            wait_times: self.wait_times.clone(),
            warm_times: self.warm_times.clone(),
        }
    }
}
