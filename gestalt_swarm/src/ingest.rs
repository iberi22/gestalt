//! Ingest module - Handles metric ingestion and feedback loop integration
//!
//! Provides CLI commands for:
//! - Ingesting agent execution results (from swarm bridge)
//! - Viewing agent priorities
//! - Viewing recommended next steps

use anyhow::Result;
use std::path::PathBuf;
use tracing::warn;

use gestalt_core::models::{
    categorize_error, AgentStats, ExecutionMetrics, NextStep, PriorityUpdate,
};


// ============================================================================
// Swarm Bridge Parsing & Handling
// ============================================================================

/// Lightweight agent result from Python swarm bridge JSON
#[derive(Debug, serde::Deserialize)]
struct SwarmBridgeResult {
    id: String,
    name: String,
    status: String,
    #[serde(rename = "duration_ms")]
    duration_ms: u64,
    #[serde(rename = "returncode")]
    returncode: Option<i32>,
    stderr: Option<String>,
    lines: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
struct SwarmBridgeResponse {
    goal: String,
    #[serde(rename = "duration_ms")]
    duration_ms: u64,
    stats: SwarmStats,
    agents: Vec<SwarmBridgeResult>,
}

#[derive(Debug, serde::Deserialize)]
struct SwarmStats {
    total: usize,
    successful: usize,
    warnings: usize,
    errors: usize,
}

/// Read JSON from stdin or file
async fn read_json(input: Option<PathBuf>) -> Result<serde_json::Value> {
    let content = if let Some(path) = input {
        tokio::fs::read_to_string(&path).await?
    } else {
        use tokio::io::AsyncReadExt;
        let mut stdin = tokio::io::stdin();
        let mut _buf = String::new();
        stdin.read_to_string(&mut _buf).await?;
        _buf
    };

    let json: serde_json::Value = serde_json::from_str(&content)?;
    Ok(json)
}

/// Convert swarm bridge JSON to ExecutionMetrics
fn convert_result(run_id: &str, result: &SwarmBridgeResult) -> ExecutionMetrics {
    let success = result.status == "success";
    let error_msg = result.stderr.clone().filter(|s| !s.is_empty());
    let error_category = error_msg.as_ref().and_then(|e| categorize_error(e));

    ExecutionMetrics {
        id: None,
        run_id: run_id.to_string(),
        agent_id: result.id.clone(),
        agent_type: result.name.clone(),
        success,
        duration_ms: result.duration_ms,
        tools_used: 1,
        return_code: result.returncode,
        error_category,
        error_message: error_msg.map(|e| e.chars().take(200).collect()),
        timestamp: chrono::Utc::now().to_rfc3339(),
        project_id: None,
        output_lines: result.lines.as_ref().map(|l| l.len() as u64),
        metadata: Default::default(),
    }
}

// ============================================================================
// Core Models and Feedback Loop Data Structures
// ============================================================================

/// ExecutionHistory: tracks per-agent success/fail/timeout rates
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionHistory {
    pub agent_type: String,
    pub success_count: usize,
    pub fail_count: usize,
    pub timeout_count: usize,
    pub total_duration_ms: u64,
    pub success_rate: f64,
    pub current_priority: u64,
}

/// MetricDrivenPriorities: represents priority adjustments driven by historical execution metrics
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricDrivenPriorities {
    pub agent_type: String,
    pub priority: u64,
    pub reason: String,
}

/// SelfTuning: routes tasks to the most reliable agents based on historical success rates
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelfTuning {
    pub best_agent_type: Option<String>,
    pub reliability_score: f64,
    pub routing_map: std::collections::HashMap<String, String>,
}

impl SelfTuning {
    pub fn tune_routing(history: &[ExecutionMetrics]) -> Self {
        let mut agent_success_counts = std::collections::HashMap::new();
        let mut agent_total_counts = std::collections::HashMap::new();

        for m in history {
            *agent_total_counts.entry(m.agent_type.clone()).or_insert(0) += 1;
            if m.success {
                *agent_success_counts
                    .entry(m.agent_type.clone())
                    .or_insert(0) += 1;
            }
        }

        let mut best_agent_type = None;
        let mut best_rate = 0.0;
        let mut routing_map = std::collections::HashMap::new();

        for (at, total) in &agent_total_counts {
            let success = *agent_success_counts.get(at).unwrap_or(&0);
            let rate = success as f64 / *total as f64;
            if rate > best_rate {
                best_rate = rate;
                best_agent_type = Some(at.clone());
            }
            // Route poorly performing agent tasks to more reliable agents if possible
            if rate < 0.6 && best_rate > 0.8 {
                if let Some(ref best) = best_agent_type {
                    routing_map.insert(at.clone(), best.clone());
                }
            } else {
                routing_map.insert(at.clone(), at.clone());
            }
        }

        Self {
            best_agent_type,
            reliability_score: best_rate,
            routing_map,
        }
    }
}

/// FeedbackReport: compiles dynamic periodic feedback reports summarizing agent history and recommendations
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackReport {
    pub timestamp: String,
    pub total_executions: usize,
    pub success_rate: f64,
    pub agent_histories: Vec<ExecutionHistory>,
    pub self_tuning_routing: SelfTuning,
}

impl FeedbackReport {
    pub fn generate(
        history: &[ExecutionMetrics],
        priorities: &std::collections::HashMap<String, u64>,
    ) -> Self {
        let mut agent_types = std::collections::HashSet::new();
        for m in history {
            agent_types.insert(m.agent_type.clone());
        }

        let mut agent_histories = Vec::new();
        let total_executions = history.len();
        let successful_executions = history.iter().filter(|m| m.success).count();
        let overall_success_rate = if total_executions > 0 {
            successful_executions as f64 / total_executions as f64
        } else {
            1.0
        };

        for at in agent_types {
            let agent_runs: Vec<&ExecutionMetrics> =
                history.iter().filter(|m| m.agent_type == at).collect();
            let total = agent_runs.len();
            let success = agent_runs.iter().filter(|m| m.success).count();
            let fail = agent_runs
                .iter()
                .filter(|m| !m.success && m.error_category.as_deref() != Some("timeout"))
                .count();
            let timeout = agent_runs
                .iter()
                .filter(|m| m.error_category.as_deref() == Some("timeout"))
                .count();
            let duration: u64 = agent_runs.iter().map(|m| m.duration_ms).sum();

            agent_histories.push(ExecutionHistory {
                agent_type: at.clone(),
                success_count: success,
                fail_count: fail,
                timeout_count: timeout,
                total_duration_ms: duration,
                success_rate: if total > 0 {
                    success as f64 / total as f64
                } else {
                    0.0
                },
                current_priority: *priorities.get(&at).unwrap_or(&100u64),
            });
        }

        let self_tuning_routing = SelfTuning::tune_routing(history);

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_executions,
            success_rate: overall_success_rate,
            agent_histories,
            self_tuning_routing,
        }
    }

    pub fn print_report(&self) {
        println!("\n==================================================");
        println!("📝 PERIODIC FEEDBACK REPORT");
        println!("==================================================");
        println!("Report Generated: {}", self.timestamp);
        println!("Total Executions: {}", self.total_executions);
        println!("Overall Success Rate: {:.1}%", self.success_rate * 100.0);
        println!("\nAgent Breakdown:");
        for ah in &self.agent_histories {
            println!(
                "  Agent: {:15} | Success: {} | Fail: {} | Timeout: {} | Priority: {} | Success Rate: {:.1}%",
                ah.agent_type,
                ah.success_count,
                ah.fail_count,
                ah.timeout_count,
                ah.current_priority,
                ah.success_rate * 100.0
            );
        }
        if let Some(ref best) = self.self_tuning_routing.best_agent_type {
            println!("\nSelf-Tuning Routing Recommendation:");
            println!(
                "  Most Reliable Agent: {} ({:.1}% success rate)",
                best,
                self.self_tuning_routing.reliability_score * 100.0
            );
            println!("  Routing overrides:");
            for (from, to) in &self.self_tuning_routing.routing_map {
                if from != to {
                    println!(
                        "    Route tasks for '{}' -> '{}' (due to poor reliability)",
                        from, to
                    );
                }
            }
        }
        println!("==================================================\n");
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SwarmMetricsDb {
    pub metrics: Vec<ExecutionMetrics>,
    pub priorities: std::collections::HashMap<String, u64>,
}

// ============================================================================
// Feedback Loop Service
// ============================================================================

pub struct FeedbackLoopService {
    pub db_path: PathBuf,
}

impl FeedbackLoopService {
    pub fn new() -> Self {
        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push("swarm_metrics.json");
        Self { db_path: path }
    }

    fn get_lock_file(&self) -> Result<std::fs::File> {
        let lock_path = self.db_path.with_extension("lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)?;
        Ok(file)
    }

    fn read_db_unlocked(&self) -> SwarmMetricsDb {
        if let Ok(content) = std::fs::read_to_string(&self.db_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            SwarmMetricsDb::default()
        }
    }

    fn write_db_unlocked(&self, db: &SwarmMetricsDb) -> Result<()> {
        let content = serde_json::to_string_pretty(db)?;
        let _ = std::fs::write(&self.db_path, &content);
        Ok(())
    }

    fn read_db(&self) -> SwarmMetricsDb {
        match self.get_lock_file() {
            Ok(file) => {
                let lock = fd_lock::RwLock::new(file);
                let _guard = lock.read().ok();
                self.read_db_unlocked()
            }
            Err(_) => self.read_db_unlocked()
        }
    }

    fn update_db<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut SwarmMetricsDb) -> R,
    {
        let file = self.get_lock_file()?;
        let mut lock = fd_lock::RwLock::new(file);
        let _guard = lock.write()?; // exclusive write lock

        let mut db = self.read_db_unlocked();
        let result = f(&mut db);
        self.write_db_unlocked(&db)?;

        Ok(result)
    }

    pub async fn record_metrics(&self, metrics: ExecutionMetrics) -> Result<()> {
        self.update_db(|db| {
            db.metrics.push(metrics);
        })?;
        Ok(())
    }

    pub async fn get_priority(&self, agent_type: &str) -> u64 {
        let db = self.read_db();
        *db.priorities.get(agent_type).unwrap_or(&100u64)
    }

    pub async fn get_all_priorities(&self) -> Vec<(String, u64)> {
        let db = self.read_db();
        let mut priorities: Vec<(String, u64)> = db.priorities.clone().into_iter().collect();
        priorities.sort_by(|a, b| b.1.cmp(&a.1));
        priorities
    }

    pub async fn get_stats(&self) -> Result<Vec<AgentStats>> {
        let db = self.read_db();
        let mut agent_types = std::collections::HashSet::new();
        for m in &db.metrics {
            agent_types.insert(m.agent_type.clone());
        }

        let mut stats = Vec::new();
        for at in agent_types {
            let total = db.metrics.iter().filter(|m| m.agent_type == at).count();
            if total > 0 {
                let failed = db
                    .metrics
                    .iter()
                    .filter(|m| m.agent_type == at && !m.success)
                    .count();
                stats.push(AgentStats {
                    agent_type: at,
                    failure_rate: failed as f64 / total as f64,
                });
            }
        }
        Ok(stats)
    }

    pub async fn analyze_and_update_priorities(
        &self,
        _run_id: &str,
    ) -> Result<Vec<PriorityUpdate>> {
        self.update_db(|db| {
            let mut updates = Vec::new();

            let mut agent_types = std::collections::HashSet::new();
            for m in &db.metrics {
                agent_types.insert(m.agent_type.clone());
            }

            for at in agent_types {
                let agent_metrics: Vec<&ExecutionMetrics> =
                    db.metrics.iter().filter(|m| m.agent_type == at).collect();
                let total = agent_metrics.len();
                if total > 0 {
                    let successful = agent_metrics.iter().filter(|m| m.success).count();
                    let success_rate = successful as f64 / total as f64;

                    let old_priority = *db.priorities.get(&at).unwrap_or(&100u64);

                    // Adjust priority dynamically based on success rate
                    let new_priority = if success_rate >= 0.90 {
                        150
                    } else if success_rate >= 0.70 {
                        100
                    } else {
                        50
                    };

                    if old_priority != new_priority {
                        let reason = format!(
                            "MetricDrivenPriorities: success rate is {:.1}% based on {} executions",
                            success_rate * 100.0,
                            total
                        );
                        updates.push(PriorityUpdate {
                            agent_type: at.clone(),
                            old_priority,
                            new_priority,
                            reason,
                        });
                        db.priorities.insert(at, new_priority);
                    }
                }
            }
            updates
        })
    }

    pub async fn generate_next_steps(&self) -> Result<Vec<NextStep>> {
        let db = self.read_db();
        let mut next_steps = Vec::new();

        let mut agent_types = std::collections::HashSet::new();
        for m in &db.metrics {
            agent_types.insert(m.agent_type.clone());
        }

        for at in agent_types {
            let agent_metrics: Vec<&ExecutionMetrics> =
                db.metrics.iter().filter(|m| m.agent_type == at).collect();
            let failed_metrics: Vec<&ExecutionMetrics> = agent_metrics
                .iter()
                .filter(|m| !m.success)
                .cloned()
                .collect();

            if !failed_metrics.is_empty() {
                let mut categories = std::collections::HashMap::new();
                for fm in &failed_metrics {
                    let cat = fm
                        .error_category
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    *categories.entry(cat).or_insert(0) += 1;
                }

                let most_common_cat = categories
                    .into_iter()
                    .max_by_key(|&(_, count)| count)
                    .map(|(cat, _)| cat)
                    .unwrap_or_else(|| "unknown".to_string());

                let confidence = failed_metrics.len() as f64 / agent_metrics.len() as f64;

                let (action, reason) = match most_common_cat.as_str() {
                    "timeout" => (
                        "Increase timeout limits or optimize model prompt complexity".to_string(),
                        format!(
                            "Agent {} had {} timeouts out of {} runs.",
                            at,
                            failed_metrics.len(),
                            agent_metrics.len()
                        ),
                    ),
                    "rate_limit" => (
                        "Implement exponential backoff or rotate LLM providers".to_string(),
                        format!(
                            "Agent {} experienced rate limit errors in {} runs.",
                            at,
                            failed_metrics.len()
                        ),
                    ),
                    "authentication" => (
                        "Check API keys and credentials configuration".to_string(),
                        format!(
                            "Agent {} failed due to credential errors in {} runs.",
                            at,
                            failed_metrics.len()
                        ),
                    ),
                    _ => (
                        "Inspect agent prompt and error logs for code-level bugs".to_string(),
                        format!(
                            "Agent {} failed in {} out of {} runs.",
                            at,
                            failed_metrics.len(),
                            agent_metrics.len()
                        ),
                    ),
                };

                next_steps.push(NextStep {
                    agent_type: at,
                    confidence,
                    action,
                    reason,
                    error_category: Some(most_common_cat),
                });
            }
        }

        next_steps.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(next_steps)
    }

    pub async fn get_next_steps_for_agent(&self, agent_type: &str) -> Result<Vec<NextStep>> {
        let steps = self.generate_next_steps().await?;
        Ok(steps
            .into_iter()
            .filter(|s| s.agent_type == agent_type)
            .collect())
    }

    pub async fn generate_report(&self) -> Result<FeedbackReport> {
        let db = self.read_db();
        Ok(FeedbackReport::generate(&db.metrics, &db.priorities))
    }
}

// ============================================================================
// CLI Commands Implementations
// ============================================================================

/// Handle `swarm ingest --run-id <id> [--file <path>]`
pub async fn handle_ingest(run_id: &str, input_file: Option<PathBuf>) -> Result<()> {
    println!("\n📥 INGEST MODE");
    println!("   Run ID: {}", run_id);
    println!("   Source: {:?}\n", input_file);

    let json = read_json(input_file).await?;
    let response: SwarmBridgeResponse = serde_json::from_value(json)?;

    println!(
        "  📊 Swarm Stats: {} total, {} success, {} errors",
        response.stats.total, response.stats.successful, response.stats.errors
    );

    let feedback = FeedbackLoopService::new();

    let mut success_count = 0;
    let mut fail_count = 0;

    for agent_result in &response.agents {
        let metrics = convert_result(run_id, agent_result);

        match feedback.record_metrics(metrics).await {
            Ok(_) => {
                if agent_result.status == "success" {
                    success_count += 1;
                } else {
                    fail_count += 1;
                }
            },
            Err(e) => {
                warn!("Failed to record metrics for {}: {}", agent_result.id, e);
            },
        }
    }

    println!(
        "\n  ✅ Recorded {} successful, ❌ {} failed metrics",
        success_count, fail_count
    );

    println!("\n🔄 Running feedback loop analysis...");
    let updates = feedback.analyze_and_update_priorities(run_id).await?;

    if updates.is_empty() {
        println!("  ℹ️  No priority changes (insufficient data or no change)");
    } else {
        println!("  📈 Priority Updates (MetricDrivenPriorities):");
        for update in &updates {
            println!(
                "     {}: {} -> {} ({})",
                update.agent_type, update.old_priority, update.new_priority, update.reason
            );
        }
    }

    let report = feedback.generate_report().await?;
    report.print_report();

    let next_steps = feedback.generate_next_steps().await?;

    if !next_steps.is_empty() {
        println!("\n📋 RECOMMENDED NEXT STEPS:");
        println!("{}", "-".repeat(50));
        for (i, step) in next_steps.iter().take(10).enumerate() {
            println!(
                "  {}. [{}] {:.0}% - {}\n     └─ {}\n",
                i + 1,
                step.agent_type,
                step.confidence * 100.0,
                step.action,
                step.reason
            );
        }
    } else {
        println!("\n  ℹ️  No next steps (no failures detected)");
    }

    Ok(())
}

/// Handle `swarm priorities [--agent-type <type>]`
pub async fn show_priorities(agent_type: Option<&str>) -> Result<()> {
    let feedback = FeedbackLoopService::new();

    if let Some(at) = agent_type {
        let priority = feedback.get_priority(at).await;
        println!("\n🔢 Priority for '{}': {}\n", at, priority);
    } else {
        let priorities = feedback.get_all_priorities().await;
        let stats = feedback.get_stats().await?;

        println!("\n🔢 AGENT PRIORITIES");
        println!("{}", "=".repeat(50));

        if priorities.is_empty() {
            println!("  ℹ️  No priorities recorded yet. Run `swarm ingest` first.");
        } else {
            for (at, priority) in &priorities {
                let failure_rate = stats
                    .iter()
                    .find(|s| &s.agent_type == at)
                    .map(|s| s.failure_rate)
                    .unwrap_or(0.0);

                println!(
                    "  {:20} | priority: {} | failure rate: {:.1}%",
                    at,
                    priority,
                    failure_rate * 100.0
                );
            }
        }

        println!();
    }

    Ok(())
}

/// Handle `swarm next-steps [--agent-type <type>]`
pub async fn show_next_steps(agent_type: Option<&str>) -> Result<()> {
    let feedback = FeedbackLoopService::new();

    let steps: Vec<NextStep> = if let Some(at) = agent_type {
        feedback.get_next_steps_for_agent(at).await?
    } else {
        feedback.generate_next_steps().await?
    };

    println!("\n📋 RECOMMENDED NEXT STEPS");
    println!("{}", "=".repeat(50));

    if steps.is_empty() {
        println!("  ℹ️  No next steps available (no failures detected)");
    } else {
        for (i, step) in steps.iter().enumerate() {
            println!(
                "\n{}. [{}] {:.0}% confidence",
                i + 1,
                step.agent_type,
                step.confidence * 100.0
            );
            println!("   Action: {}", step.action);
            println!("   Reason: {}", step.reason);
            if let Some(cat) = &step.error_category {
                println!("   Category: {}", cat);
            }
        }
    }

    println!();
    Ok(())
}

// ============================================================================
// Automated Verification Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_driven_priorities_success_rate() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_swarm_metrics_unique.json");
        let _ = std::fs::remove_file(&db_path);

        let service = FeedbackLoopService {
            db_path: db_path.clone(),
        };

        // Track 10 executions for Agent A with 90% success (9 success, 1 failure)
        for i in 0..10 {
            let success = i < 9;
            let metrics = ExecutionMetrics {
                id: None,
                run_id: "test-run-1".to_string(),
                agent_id: format!("agent-a-{}", i),
                agent_type: "AgentA".to_string(),
                success,
                duration_ms: 200,
                tools_used: 1,
                return_code: if success { Some(0) } else { Some(1) },
                error_category: if success {
                    None
                } else {
                    Some("unknown".to_string())
                },
                error_message: if success {
                    None
                } else {
                    Some("some error".to_string())
                },
                timestamp: "2026-03-31T00:00:00Z".to_string(),
                project_id: None,
                output_lines: Some(10),
                metadata: serde_json::Value::Null,
            };
            service.record_metrics(metrics).await.unwrap();
        }

        // Track 10 executions for Agent B with 50% success (5 success, 5 failure) for comparison
        for i in 0..10 {
            let success = i < 5;
            let metrics = ExecutionMetrics {
                id: None,
                run_id: "test-run-1".to_string(),
                agent_id: format!("agent-b-{}", i),
                agent_type: "AgentB".to_string(),
                success,
                duration_ms: 300,
                tools_used: 1,
                return_code: if success { Some(0) } else { Some(1) },
                error_category: if success {
                    None
                } else {
                    Some("unknown".to_string())
                },
                error_message: if success {
                    None
                } else {
                    Some("some error".to_string())
                },
                timestamp: "2026-03-31T00:00:00Z".to_string(),
                project_id: None,
                output_lines: Some(15),
                metadata: serde_json::Value::Null,
            };
            service.record_metrics(metrics).await.unwrap();
        }

        // Run feedback loop analysis to auto-adjust priorities based on success rate
        let updates = service
            .analyze_and_update_priorities("test-run-1")
            .await
            .unwrap();
        assert!(!updates.is_empty(), "Priorities should have been adjusted");

        let priority_a = service.get_priority("AgentA").await;
        let priority_b = service.get_priority("AgentB").await;

        println!(
            "AgentA priority: {}, AgentB priority: {}",
            priority_a, priority_b
        );

        // Agent A has 90% success -> should get high priority (150)
        // Agent B has 50% success -> should get low priority (50)
        assert_eq!(
            priority_a, 150,
            "Agent A (90% success) should get high priority (150)"
        );
        assert_eq!(
            priority_b, 50,
            "Agent B (50% success) should get low priority (50)"
        );

        // Verify SelfTuning recommends routing tasks to most reliable agent (AgentA)
        let report = service.generate_report().await.unwrap();
        assert_eq!(
            report.self_tuning_routing.best_agent_type.as_deref(),
            Some("AgentA"),
            "SelfTuning should recommend AgentA as the best/most reliable agent"
        );

        // Verify FeedbackReport compiles and contains both agent histories
        assert_eq!(report.total_executions, 20);
        assert_eq!(report.agent_histories.len(), 2);

        // Clean up test database file
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_concurrent_writes_no_clobber() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_swarm_metrics_concurrent.json");
        // Clean up both JSON and lock file
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("lock"));

        let service = std::sync::Arc::new(FeedbackLoopService {
            db_path: db_path.clone(),
        });

        // Spawn 40 concurrent tasks, each writing one unique metric
        let num_tasks = 40;
        let mut handles = vec![];

        for i in 0..num_tasks {
            let s = service.clone();
            handles.push(tokio::spawn(async move {
                let metrics = ExecutionMetrics {
                    id: None,
                    run_id: "concurrent-run".to_string(),
                    agent_id: format!("agent-{}", i),
                    agent_type: "TestAgent".to_string(),
                    success: true,
                    duration_ms: 100,
                    tools_used: 1,
                    return_code: Some(0),
                    error_category: None,
                    error_message: None,
                    timestamp: "2026-03-31T00:00:00Z".to_string(),
                    project_id: None,
                    output_lines: Some(5),
                    metadata: serde_json::Value::Null,
                };
                s.record_metrics(metrics).await.unwrap();
            }));
        }

        // Wait for all tasks to complete
        for h in handles {
            h.await.unwrap();
        }

        // Generate report or read db directly to count records
        let report = service.generate_report().await.unwrap();
        assert_eq!(
            report.total_executions, num_tasks,
            "Expected exactly {} executions recorded without clobbering!",
            num_tasks
        );

        // Clean up
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("lock"));
    }
}
