//! Ingest module - Handles metric ingestion and feedback loop integration
//!
//! Provides CLI commands for:
//! - Ingesting agent execution results (from swarm bridge)
//! - Viewing agent priorities
//! - Viewing recommended next steps

use anyhow::Result;
use std::path::PathBuf;
use tracing::warn;

use gestalt_timeline::models::execution_metrics::{
    categorize_error, ExecutionMetrics, NextStep,
};
use gestalt_timeline::models::FlexibleTimestamp;
use gestalt_timeline::services::FeedbackLoopService;
use gestalt_timeline::db::SurrealClient;
use gestalt_timeline::config::Settings;

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
        let mut buf = String::new();
        stdin.read_to_string(&mut buf).await?;
        buf
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
        timestamp: FlexibleTimestamp::now(),
        project_id: None,
        output_lines: result.lines.as_ref().map(|l| l.len() as u32),
        metadata: Default::default(),
    }
}

/// Connect to the timeline database
async fn connect_db() -> Result<SurrealClient> {
    let settings = Settings::new()
        .map_err(|e| anyhow::anyhow!("Failed to load settings: {}", e))?;

    let db = SurrealClient::connect(&settings.database)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to database: {}", e))?;

    Ok(db)
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

    // Connect to database
    let db = connect_db().await?;
    let feedback = FeedbackLoopService::new(db);

    // Convert and record metrics
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
            }
            Err(e) => {
                warn!("Failed to record metrics for {}: {}", agent_result.id, e);
            }
        }
    }

    println!("\n  ✅ Recorded {} successful, ❌ {} failed metrics", success_count, fail_count);

    // Analyze and update priorities
    println!("\n🔄 Running feedback loop analysis...");
    let updates = feedback.analyze_and_update_priorities(run_id).await?;

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
    let db = connect_db().await?;
    let feedback = FeedbackLoopService::new(db);

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
    let db = connect_db().await?;
    let feedback = FeedbackLoopService::new(db);

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
