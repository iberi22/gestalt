//! Ingest module - Handles metric ingestion and feedback loop integration
//!
//! Provides CLI commands for:
//! - Ingesting agent execution results (from swarm bridge)
//! - Viewing agent priorities
//! - Viewing recommended next steps

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use gestalt_core::agent::{AgentMetrics, TokenUsage, MetricsStore, MetricsAggregator};

/// Lightweight agent result from Python swarm bridge JSON
#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
struct SwarmBridgeResponse {
    goal: String,
    #[serde(rename = "duration_ms")]
    duration_ms: u64,
    stats: SwarmStats,
    agents: Vec<SwarmBridgeResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SwarmStats {
    total: usize,
    successful: usize,
    warnings: usize,
    errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStep {
    pub agent_type: String,
    pub confidence: f64,
    pub action: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityUpdate {
    pub agent_type: String,
    pub old_priority: u32,
    pub new_priority: u32,
    pub reason: String,
}

/// Read JSON from stdin or file
async fn read_json(input: Option<PathBuf>) -> Result<serde_json::Value> {
    let content = if let Some(path) = input {
        tokio::fs::read_to_string(&path).await?
    } else {
        use tokio::io::AsyncReadExt;
        let mut stdin = tokio::io::stdin();
        let mut buf = String::new();
        stdin.read_to_string(&mut buf).await?;
        buf
    };

    let json: serde_json::Value = serde_json::from_str(&content)?;
    Ok(json)
}

/// Convert swarm bridge JSON to AgentMetrics
fn convert_result(result: &SwarmBridgeResult) -> AgentMetrics {
    let success = result.status == "success";
    let provider = "SwarmBridge".to_string();
    let model = "unknown".to_string();

    // Simple estimated token usage
    let prompt_tokens = 120;
    let completion_tokens = if success { 250 } else { 0 };
    let token_usage = Some(TokenUsage::new(prompt_tokens, completion_tokens));

    let cost_estimate = MetricsAggregator::calculate_cost(
        &provider,
        &model,
        prompt_tokens,
        completion_tokens,
    );

    AgentMetrics {
        agent_id: result.id.clone(),
        agent_type: result.name.clone(),
        duration_ms: result.duration_ms,
        token_usage,
        model,
        provider,
        tools_used: 1,
        success,
        cost_estimate,
        cold_start: false,
    }
}

fn get_db_path() -> PathBuf {
    std::path::Path::new("data/metrics_store.json").to_path_buf()
}

/// Handle `swarm ingest --run-id <id> [--file <path>]`
pub async fn handle_ingest(run_id: &str, input_file: Option<PathBuf>) -> Result<()> {
    println!("\n📥 INGEST MODE");
    println!("   Run ID: {}", run_id);
    println!("   Source: {:?}\n", input_file);

    // Read JSON
    let json = read_json(input_file).await?;
    let response: SwarmBridgeResponse = serde_json::from_value(json)?;

    println!("  📊 Swarm Stats: {} total, {} success, {} errors",
        response.stats.total, response.stats.successful, response.stats.errors);

    let db_path = get_db_path();
    let mut stored_metrics = MetricsStore::load_metrics(&db_path).unwrap_or_default();

    // Convert and record metrics
    let mut success_count = 0;
    let mut fail_count = 0;

    for agent_result in &response.agents {
        let metrics = convert_result(agent_result);
        if metrics.success {
            success_count += 1;
        } else {
            fail_count += 1;
        }
        stored_metrics.push(metrics);
    }

    MetricsStore::save_metrics(&db_path, &stored_metrics)?;

    println!("\n  ✅ Recorded {} successful, ❌ {} failed metrics", success_count, fail_count);

    // Analyze and update priorities
    println!("\n🔄 Running feedback loop analysis...");

    // Group and calculate priority updates
    let mut old_priorities = HashMap::new();
    let mut new_priorities = HashMap::new();
    let mut reasons = HashMap::new();

    for m in &stored_metrics {
        let old_p = old_priorities.entry(m.agent_type.clone()).or_insert(5u32);
        let current_p = new_priorities.entry(m.agent_type.clone()).or_insert(5u32);
        if !m.success {
            *current_p = (*current_p + 1).min(10);
            reasons.insert(m.agent_type.clone(), "Increasing priority due to task failure".to_string());
        } else if m.duration_ms > 8000 {
            *current_p = (*current_p + 1).min(8);
            reasons.insert(m.agent_type.clone(), "Increasing priority due to high latency (> 8s)".to_string());
        }
    }

    let mut updates = Vec::new();
    for (agent_type, new_p) in new_priorities {
        let old_p = *old_priorities.get(&agent_type).unwrap_or(&5);
        if new_p != old_p {
            updates.push(PriorityUpdate {
                agent_type: agent_type.clone(),
                old_priority: old_p,
                new_priority: new_p,
                reason: reasons.get(&agent_type).cloned().unwrap_or_default(),
            });
        }
    }

    if updates.is_empty() {
        println!("  ℹ️  No priority changes (insufficient data or no change)");
    } else {
        println!("  📈 Priority Updates:");
        for update in &updates {
            println!(
                "     {}: {} -> {} ({})",
                update.agent_type, update.old_priority, update.new_priority, update.reason
            );
        }
    }

    // Generate next steps
    let next_steps = calculate_next_steps(&stored_metrics);

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

fn calculate_next_steps(metrics: &[AgentMetrics]) -> Vec<NextStep> {
    let mut next_steps = Vec::new();
    let mut stats_by_type: HashMap<String, (usize, usize, u64)> = HashMap::new(); // (total, failed, total_duration)

    for m in metrics {
        let entry = stats_by_type.entry(m.agent_type.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        if !m.success {
            entry.1 += 1;
        }
        entry.2 += m.duration_ms;
    }

    for (agent_type, (total, failed, total_duration)) in stats_by_type {
        let fail_rate = failed as f64 / total as f64;
        let avg_latency = total_duration as f64 / total as f64;

        if fail_rate > 0.20 {
            next_steps.push(NextStep {
                agent_type: agent_type.clone(),
                confidence: 0.90,
                action: "Review prompt instructions and input validation schemas".to_string(),
                reason: format!("Failure rate is too high ({:.1}%)", fail_rate * 100.0),
            });
        }
        if avg_latency > 5000.0 {
            next_steps.push(NextStep {
                agent_type: agent_type.clone(),
                confidence: 0.80,
                action: "Refactor tooling and reduce context window sizes".to_string(),
                reason: format!("Average execution latency exceeds 5s ({:.1}ms)", avg_latency),
            });
        }
    }

    next_steps
}

/// Handle `swarm priorities [--agent-type <type>]`
pub async fn show_priorities(agent_type: Option<&str>) -> Result<()> {
    let db_path = get_db_path();
    let stored_metrics = MetricsStore::load_metrics(&db_path).unwrap_or_default();

    if let Some(at) = agent_type {
        let mut priority = 5;
        for m in &stored_metrics {
            if m.agent_type == at {
                if !m.success {
                    priority = (priority + 1).min(10);
                } else if m.duration_ms > 8000 {
                    priority = (priority + 1).min(8);
                }
            }
        }
        println!("\n🔢 Priority for '{}': {}\n", at, priority);
    } else {
        println!("\n🔢 AGENT PRIORITIES");
        println!("{}", "=".repeat(50));

        let mut stats_by_type: HashMap<String, (usize, usize)> = HashMap::new(); // (total, failed)
        for m in &stored_metrics {
            let entry = stats_by_type.entry(m.agent_type.clone()).or_insert((0, 0));
            entry.0 += 1;
            if !m.success {
                entry.1 += 1;
            }
        }

        if stats_by_type.is_empty() {
            println!("  ℹ️  No priorities recorded yet. Run `swarm ingest` first.");
        } else {
            for (at, (total, failed)) in &stats_by_type {
                let fail_rate = *failed as f64 / *total as f64;
                let mut priority = 5;
                for m in &stored_metrics {
                    if &m.agent_type == at {
                        if !m.success {
                            priority = (priority + 1).min(10);
                        } else if m.duration_ms > 8000 {
                            priority = (priority + 1).min(8);
                        }
                    }
                }

                println!(
                    "  {:20} | priority: {} | failure rate: {:.1}%",
                    at,
                    priority,
                    fail_rate * 100.0
                );
            }
        }

        println!();
    }

    Ok(())
}

/// Handle `swarm next-steps [--agent-type <type>]`
pub async fn show_next_steps(agent_type: Option<&str>) -> Result<()> {
    let db_path = get_db_path();
    let stored_metrics = MetricsStore::load_metrics(&db_path).unwrap_or_default();

    let all_steps = calculate_next_steps(&stored_metrics);
    let steps: Vec<NextStep> = if let Some(at) = agent_type {
        all_steps.into_iter().filter(|s| s.agent_type == at).collect()
    } else {
        all_steps
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
        }
    }

    println!();
    Ok(())
}
