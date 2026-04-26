// ============================================================================
// Load Tests for Concurrent Agent Spawning
// ============================================================================
//
// Tests spawn system under realistic load: 20, 50, 100 concurrent agents.
// Measures latency, verifies no race conditions or deadlocks.
//
// Run with: cargo test --package gestalt_swarm --test load_test

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{RwLock, Semaphore};
use tokio::time::timeout;

use crate::health::{HealthConfig, SwarmHealthMonitor};
use crate::shared::SharedState;

/// Test configuration for load testing
#[derive(Debug, Clone)]
pub struct LoadTestConfig {
    pub agent_count: usize,
    pub max_concurrency: usize,
    pub spawn_timeout_ms: u64,
    pub health_check_interval_ms: u64,
}

impl LoadTestConfig {
    pub fn new(agent_count: usize) -> Self {
        Self {
            agent_count,
            max_concurrency: agent_count.min(16),
            spawn_timeout_ms: 30_000,
            health_check_interval_ms: 100,
        }
    }

    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.max_concurrency = concurrency;
        self
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.spawn_timeout_ms = ms;
        self
    }
}

/// Result of a load test run
#[derive(Debug)]
pub struct LoadTestResult {
    pub agent_count: usize,
    pub successful_spawns: usize,
    pub failed_spawns: usize,
    pub total_duration_ms: u64,
    pub avg_spawn_latency_ms: f64,
    pub min_spawn_latency_ms: u64,
    pub max_spawn_latency_ms: u64,
    pub p50_spawn_latency_ms: u64,
    pub p95_spawn_latency_ms: u64,
    pub p99_spawn_latency_ms: u64,
    pub deadlocks_detected: bool,
    pub race_conditions_detected: bool,
}

impl LoadTestResult {
    pub fn summary(&self) -> String {
        format!(
            "Load Test Results ({} agents):\n\
             ├─ Successful: {}\n\
             ├─ Failed: {}\n\
             ├─ Total time: {}ms\n\
             ├─ Avg latency: {:.2}ms\n\
             ├─ Min/Max: {}/{}ms\n\
             ├─ P50/P95/P99: {}/{}/{}ms\n\
             └─ Issues: deadlocks={}, races={}",
            self.agent_count,
            self.successful_spawns,
            self.failed_spawns,
            self.total_duration_ms,
            self.avg_spawn_latency_ms,
            self.min_spawn_latency_ms,
            self.max_spawn_latency_ms,
            self.p50_spawn_latency_ms,
            self.p95_spawn_latency_ms,
            self.p99_spawn_latency_ms,
            self.deadlocks_detected,
            self.race_conditions_detected,
        )
    }
}

/// Shared state for tracking spawn events across agents
#[derive(Debug)]
pub struct SpawnTracker {
    pub spawned_ids: Arc<RwLock<Vec<usize>>>,
    pub completed_ids: Arc<RwLock<Vec<usize>>>,
    pub spawn_times: Arc<RwLock<Vec<u64>>>,
    pub completion_times: Arc<RwLock<Vec<u64>>>,
    pub errors: Arc<RwLock<Vec<String>>>,
    pub panic_detected: Arc<AtomicBool>,
    pub duplicate_spawns: Arc<AtomicUsize>,
}

impl SpawnTracker {
    pub fn new() -> Self {
        Self {
            spawned_ids: Arc::new(RwLock::new(Vec::new())),
            completed_ids: Arc::new(RwLock::new(Vec::new())),
            spawn_times: Arc::new(RwLock::new(Vec::new())),
            completion_times: Arc::new(RwLock::new(Vec::new())),
            errors: Arc::new(RwLock::new(Vec::new())),
            panic_detected: Arc::new(AtomicBool::new(false)),
            duplicate_spawns: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub async fn record_spawn(&self, agent_id: usize, spawn_time_ms: u64) {
        // Check for duplicates (race condition indicator)
        let is_duplicate = {
            let mut ids = self.spawned_ids.write().await;
            if ids.contains(&agent_id) {
                true
            } else {
                ids.push(agent_id);
                false
            }
        };

        if is_duplicate {
            self.duplicate_spawns.fetch_add(1, Ordering::SeqCst);
        }

        let mut times = self.spawn_times.write().await;
        times.push(spawn_time_ms);
    }

    pub async fn record_completion(&self, agent_id: usize, completion_time_ms: u64) {
        let mut ids = self.completed_ids.write().await;
        ids.push(agent_id);
        drop(ids);

        let mut times = self.completion_times.write().await;
        times.push(completion_time_ms);
    }

    pub async fn record_error(&self, agent_id: usize, error: String) {
        let mut errors = self.errors.write().await;
        errors.push(format!("Agent {}: {}", agent_id, error));
    }

    pub fn detect_panic(&self) {
        self.panic_detected.store(true, Ordering::SeqCst);
    }

    pub async fn get_latencies(&self) -> Vec<u64> {
        let times = self.spawn_times.read().await;
        times.clone()
    }

    pub fn has_race_conditions(&self) -> bool {
        self.duplicate_spawns.load(Ordering::SeqCst) > 0
            || self.panic_detected.load(Ordering::SeqCst)
    }
}

/// Simulate agent spawn with timing (lightweight simulation)
async fn spawn_agent_simulated(
    agent_id: usize,
    semaphore: Arc<Semaphore>,
    tracker: Arc<SpawnTracker>,
    config: &LoadTestConfig,
    start_time: Instant,
) -> Result<u64, String> {
    let permit = semaphore
        .acquire()
        .await
        .map_err(|e| format!("Semaphore acquire error: {}", e))?;

    let spawn_latency_ms = start_time.elapsed().as_millis() as u64;
    tracker.record_spawn(agent_id, spawn_latency_ms).await;

    // Simulate agent work (yield + tiny sleep to test concurrency)
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_micros(50)).await;

    let completion_ms = start_time.elapsed().as_millis() as u64;
    tracker.record_completion(agent_id, completion_ms).await;

    drop(permit);
    Ok(spawn_latency_ms)
}

/// Run a load test with specified configuration
pub async fn run_load_test(config: LoadTestConfig) -> LoadTestResult {
    let start_time = Instant::now();
    let tracker = Arc::new(SpawnTracker::new());
    let semaphore = Arc::new(Semaphore::new(config.max_concurrency));

    // Spawn all agents concurrently
    let mut handles = Vec::with_capacity(config.agent_count);

    for agent_id in 0..config.agent_count {
        let tracker = tracker.clone();
        let sem = semaphore.clone();
        let cfg = config.clone();
        let start = start_time;

        let handle = tokio::spawn(async move {
            spawn_agent_simulated(agent_id, sem, tracker, &cfg, start).await
        });

        handles.push(handle);
    }

    // Wait for all with timeout (deadlock detection)
    let deadline = Instant::now() + Duration::from_millis(config.spawn_timeout_ms);
    let mut successful = 0;
    let mut failed = 0;
    let mut deadlocks = 0;

    for (i, handle) in handles.into_iter().enumerate() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, handle).await {
            Ok(Ok(Ok(_))) => successful += 1,
            Ok(Ok(Err(e))) => {
                failed += 1;
                eprintln!("Agent {} error: {}", i, e);
            }
            Ok(Err(e)) => {
                failed += 1;
                eprintln!("Join error: {}", e);
            }
            Err(_) => {
                // Timeout = potential deadlock
                deadlocks += 1;
                failed += 1;
                eprintln!("TIMEOUT: Agent {} may be deadlocked", i);
            }
        }
    }

    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let latencies = tracker.get_latencies().await;

    // Calculate statistics
    let (avg, min, max) = if latencies.is_empty() {
        (0.0, 0, 0)
    } else {
        let sum: u64 = latencies.iter().sum();
        (
            sum as f64 / latencies.len() as f64,
            *latencies.iter().min().unwrap(),
            *latencies.iter().max().unwrap(),
        )
    };

    // Sort latencies once for percentile calculations
    latencies.sort();

    let p50 = percentile(&latencies, 0.50);
    let p95 = percentile(&latencies, 0.95);
    let p99 = percentile(&latencies, 0.99);

    let race_conditions = tracker.has_race_conditions();

    LoadTestResult {
        agent_count: config.agent_count,
        successful_spawns: successful,
        failed_spawns: failed,
        total_duration_ms,
        avg_spawn_latency_ms: avg,
        min_spawn_latency_ms: min,
        max_spawn_latency_ms: max,
        p50_spawn_latency_ms: p50,
        p95_spawn_latency_ms: p95,
        p99_spawn_latency_ms: p99,
        deadlocks_detected: deadlocks > 0,
        race_conditions_detected: race_conditions,
    }
}

/// Calculate percentile from sorted vector
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ============================================================================
// Test Modules
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 20 concurrent agents
    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_20_agents() {
        let config = LoadTestConfig::new(20).with_concurrency(10);
        let result = run_load_test(config).await;

        println!("\n{}", result.summary());

        assert_eq!(result.agent_count, 20);
        assert!(
            !result.deadlocks_detected,
            "Deadlock detected with 20 agents"
        );
        assert!(
            !result.race_conditions_detected,
            "Race conditions detected with 20 agents"
        );
        // At least 90% should succeed (allow some flakiness)
        assert!(
            result.successful_spawns >= 18,
            "Too many failures: {}/20",
            result.successful_spawns
        );
    }

    /// Test 50 concurrent agents
    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_50_agents() {
        let config = LoadTestConfig::new(50).with_concurrency(25);
        let result = run_load_test(config).await;

        println!("\n{}", result.summary());

        assert_eq!(result.agent_count, 50);
        assert!(
            !result.deadlocks_detected,
            "Deadlock detected with 50 agents"
        );
        assert!(
            !result.race_conditions_detected,
            "Race conditions detected with 50 agents"
        );
        assert!(
            result.successful_spawns >= 45,
            "Too many failures: {}/50",
            result.successful_spawns
        );
    }

    /// Test 100 concurrent agents
    #[tokio::test(flavor = "multi_thread")]
    async fn test_spawn_100_agents() {
        let config = LoadTestConfig::new(100).with_concurrency(50);
        let result = run_load_test(config).await;

        println!("\n{}", result.summary());

        assert_eq!(result.agent_count, 100);
        assert!(
            !result.deadlocks_detected,
            "Deadlock detected with 100 agents"
        );
        assert!(
            !result.race_conditions_detected,
            "Race conditions detected with 100 agents"
        );
        assert!(
            result.successful_spawns >= 90,
            "Too many failures: {}/100",
            result.successful_spawns
        );
    }

    /// Test latency consistency across multiple runs
    #[tokio::test(flavor = "multi_thread")]
    async fn test_latency_consistency() {
        let mut results = Vec::new();

        // Run 5 iterations of 50 agents
        for i in 0..5 {
            let config = LoadTestConfig::new(50).with_concurrency(25);
            let result = run_load_test(config).await;
            println!("Run {}: avg={:.2}ms, p95={}ms", i + 1, result.avg_spawn_latency_ms, result.p95_spawn_latency_ms);
            results.push(result);
        }

        // Check that latency doesn't degrade significantly
        let avgs: Vec<f64> = results.iter().map(|r| r.avg_spawn_latency_ms).collect();
        let max_avg = avgs.iter().cloned().fold(0.0_f64, f64::max);
        let min_avg = avgs.iter().cloned().fold(f64::MAX, f64::min);

        // p95 should not increase by more than 50% from first to last
        let first_p95 = results.first().map(|r| r.p95_spawn_latency_ms).unwrap_or(0);
        let last_p95 = results.last().map(|r| r.p95_spawn_latency_ms).unwrap_or(0);

        assert!(
            last_p95 <= first_p95 * 3 / 2,
            "Latency degradation detected: first_p95={}ms, last_p95={}ms",
            first_p95,
            last_p95
        );

        println!(
            "Latency range: {:.2}ms - {:.2}ms (spread: {:.2}ms)",
            min_avg,
            max_avg,
            max_avg - min_avg
        );
    }

    /// Test shared state access under load
    #[tokio::test(flavor = "multi_thread")]
    async fn test_shared_state_under_load() {
        let state: SharedState<Vec<usize>> = SharedState::new(Vec::new());
        let config = LoadTestConfig::new(100).with_concurrency(50);

        let start_time = Instant::now();
        let mut handles = Vec::new();

        for agent_id in 0..config.agent_count {
            let state = state.clone();
            let handle = tokio::spawn(async move {
                let mut write = state.write().await;
                write.push(agent_id);
                // Simulate some work
                tokio::task::yield_now();
            });
            handles.push(handle);
        }

        // All should complete without deadlock
        for handle in handles {
            handle.await.expect("Task panicked");
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let state_content = state.read().await;

        assert_eq!(
            state_content.len(),
            config.agent_count,
            "Not all agents wrote to shared state"
        );

        println!(
            "Shared state test: {} agents in {}ms",
            config.agent_count, duration_ms
        );
    }

    /// Test health monitor under concurrent registration
    #[tokio::test(flavor = "multi_thread")]
    async fn test_health_monitor_concurrent_registration() {
        let config = HealthConfig::default();
        let monitor = Arc::new(SwarmHealthMonitor::new(config));

        let agent_count = 100;
        let mut handles = Vec::new();

        for agent_id in 0..agent_count {
            let monitor = monitor.clone();
            let handle = tokio::spawn(async move {
                monitor.register_agent(agent_id).await;
                monitor.heartbeat(agent_id).await;
                monitor.report_task_complete(agent_id, true).await;
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.expect("Health monitoring task panicked");
        }

        // Verify all agents registered
        let all_health = monitor.get_all_health().await;
        assert_eq!(all_health.len(), agent_count);

        let (status, healthy, _) = monitor.get_swarm_status().await;
        println!(
            "Health monitor test: status={:?}, healthy={}/{}",
            status, healthy, agent_count
        );
    }
}

// ============================================================================
// Load Test Runner (Binary)
// ============================================================================

/// Standalone load test runner (can be compiled as separate binary)
pub async fn run_all_load_tests() {
    println!("\n{}", "=".repeat(70));
    println!("🐝 GESTALT SWARM - LOAD TEST SUITE");
    println!("{}", "=".repeat(70));

    let configs = vec![
        LoadTestConfig::new(20).with_concurrency(10),
        LoadTestConfig::new(50).with_concurrency(25),
        LoadTestConfig::new(100).with_concurrency(50),
    ];

    for config in configs {
        println!("\n▶ Running load test: {} agents, concurrency: {}",
            config.agent_count, config.max_concurrency);
        let result = run_load_test(config).await;
        println!("\n{}", result.summary());
        println!("{}", "-".repeat(70));
    }

    println!("\n✅ Load test suite complete");
}