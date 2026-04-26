//! Agent Pool - Pre-warmed agent reuse for cold-start latency reduction
//!
//! Instead of creating fresh agents per task (expensive cold-start),
//! we maintain a pool of pre-warmed agents ready to execute immediately.

use crate::run_agent::RunAgentStats;
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Configuration for the agent pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Initial number of pre-warmed agents
    pub pre_warm: usize,
    /// Maximum pool size (agents are evicted when idle beyond max_age)
    pub max_size: usize,
    /// Maximum idle time before agent is considered stale (seconds)
    pub max_idle_secs: u64,
    /// Enable aggressive pre-warming on pool creation
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

/// Statistics about pool usage
#[derive(Debug, Default)]
pub struct PoolStats {
    pub checkouts: u64,
    pub checkins: u64,
    pub evictions: u64,
    pub pre_warm_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub avg_wait_time_ms: u64,
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

/// A pre-warmed agent ready for immediate use
struct PooledAgent {
    /// Agent ID within the pool
    id: usize,
    /// When this agent was last used
    last_used: Instant,
    /// Number of times this agent has been reused
    reuse_count: u64,
    /// Whether this agent is currently checked out
    checked_out: bool,
}

impl PooledAgent {
    fn new(id: usize) -> Self {
        Self {
            id,
            last_used: Instant::now(),
            reuse_count: 0,
            checked_out: false,
        }
    }

    fn checkout(&mut self) {
        self.checked_out = true;
        self.last_used = Instant::now();
        self.reuse_count += 1;
    }

    fn checkin(&mut self) {
        self.checked_out = false;
        self.last_used = Instant::now();
    }

    fn is_stale(&self, max_idle: Duration) -> bool {
        !self.checked_out && self.last_used.elapsed() > max_idle
    }

    fn is_idle(&self) -> bool {
        !self.checked_out
    }
}

/// Thread-safe agent pool with pre-warming support
pub struct AgentPool {
    /// Pool configuration
    config: PoolConfig,
    /// Available agents (not checked out)
    available: Arc<RwLock<VecDeque<usize>>>,
    /// All agents (metadata)
    agents: Arc<RwLock<Vec<PooledAgent>>>,
    /// Statistics
    stats: Arc<RwLock<PoolStats>>,
    /// Total agents created (for IDs)
    next_id: Arc<RwLock<usize>>,
    /// Wait time tracking for stats
    wait_times: Arc<RwLock<Vec<u64>>>,
}

impl AgentPool {
    /// Create a new agent pool
    pub fn new(config: PoolConfig) -> Self {
        Self {
            config,
            available: Arc::new(RwLock::new(VecDeque::new())),
            agents: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(PoolStats::default())),
            next_id: Arc::new(RwLock::new(0)),
            wait_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get an available agent ID (must call pre_warm_agent separately)
    pub async fn checkout(&self) -> Option<usize> {
        let start = Instant::now();
        
        let agent_id = {
            let mut available = self.available.write().await;
            available.pop_front()
        };

        let wait_time_ms = start.elapsed().as_millis() as u64;
        
        if let Some(id) = agent_id {
            // Mark as checked out
            let mut agents = self.agents.write().await;
            if let Some(agent) = agents.get_mut(id) {
                agent.checkout();
            }
            
            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.checkouts += 1;
                stats.hits += 1;
            }
            {
                let mut times = self.wait_times.write().await;
                times.push_back(wait_time_ms);
                // Keep only last 100 samples
                if times.len() > 100 {
                    times.pop_front();
                }
            }

            debug!("Pool checkout: agent {} (wait: {}ms, reuse: {})", 
                id, wait_time_ms, agents.get(id).map(|a| a.reuse_count).unwrap_or(0));
            
            Some(id)
        } else {
            // No available agent
            {
                let mut stats = self.stats.write().await;
                stats.checkouts += 1;
                stats.misses += 1;
            }
            {
                let mut times = self.wait_times.write().await;
                times.push_back(wait_time_ms);
                if times.len() > 100 {
                    times.pop_front();
                }
            }
            None
        }
    }

    /// Return an agent to the pool
    pub async fn checkin(&self, agent_id: usize) {
        let mut agents = self.agents.write().await;
        
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.checkin();
            
            // Check if we should evict (pool over max_size)
            let pool_size = agents.iter().filter(|a| !a.checked_out || a.id == agent_id).count();
            
            if pool_size > self.config.max_size && agent.is_idle() {
                // Evict this agent
                drop(agents);
                self.evict(agent_id).await;
                return;
            }
            
            // Return to available queue
            drop(agents);
            let mut available = self.available.write().await;
            available.push_back(agent_id);
            
            let mut stats = self.stats.write().await;
            stats.checkins += 1;
        }
    }

    /// Register a new pre-warmed agent
    pub async fn register(&self) -> usize {
        let id = {
            let mut next = self.next_id.write().await;
            let id = *next;
            *next += 1;
            id
        };

        let mut agents = self.agents.write().await;
        agents.push(PooledAgent::new(id));
        
        let mut available = self.available.write().await;
        available.push_back(id);

        let mut stats = self.stats.write().await;
        stats.pre_warm_requests += 1;

        debug!("Pool registered agent {}", id);
        id
    }

    /// Pre-warm agents up to config.pre_warm count
    /// Returns the number of agents pre-warmed
    pub async fn pre_warm(&self) -> usize {
        let current_size = {
            let agents = self.agents.read().await;
            agents.len()
        };

        let to_create = if self.config.eager_pre_warm {
            self.config.pre_warm.saturating_sub(current_size)
        } else {
            // Lazy: just ensure we have at least one warm agent
            if current_size == 0 { 1 } else { 0 }
        };

        for _ in 0..to_create {
            self.register().await;
        }

        let new_size = {
            let agents = self.agents.read().await;
            agents.len()
        };

        info!(
            "Pool pre-warmed: {} -> {} agents (target: {})",
            current_size, new_size, self.config.pre_warm
        );

        new_size
    }

    /// Evict a specific agent from the pool
    async fn evict(&self, agent_id: usize) {
        // Remove from available queue
        {
            let mut available = self.available.write().await;
            available.retain(|&id| id != agent_id);
        }

        // Remove agent
        {
            let mut agents = self.agents.write().await;
            agents.retain(|a| a.id != agent_id);
            // Re-index remaining agents (update their IDs)
            // Actually, we just mark eviction in stats
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

    /// Get current pool statistics
    pub async fn stats(&self) -> PoolStats {
        let stats = self.stats.read().await;
        let mut result = stats.clone();

        // Calculate avg wait time
        let times = self.wait_times.read().await;
        if !times.is_empty() {
            result.avg_wait_time_ms = times.iter().sum::<u64>() / times.len() as u64;
        }

        result
    }

    /// Get pool size (total registered agents)
    pub async fn size(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Get number of available (idle) agents
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

/// Pool-aware agent handle - wraps agent ID with automatic return to pool
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
        // Return agent to pool on drop
        let pool = self.pool.clone();
        let agent_id = self.agent_id;
        tokio::spawn(async move {
            pool.checkin(agent_id).await;
        });
    }
}

// Need to add Clone to AgentPool for PooledAgentGuard
impl Clone for AgentPool {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            available: self.available.clone(),
            agents: self.agents.clone(),
            stats: self.stats.clone(),
            next_id: self.next_id.clone(),
            wait_times: self.wait_times.clone(),
        }
    }
}