//! Automated Feedback Loop Service
//!
//! Analyzes execution metrics post-run to:
//! - Generate next steps based on error patterns
//! - Update priorities automatically
//! - Provide actionable insights for agent improvement

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::db::SurrealClient;
use crate::models::execution_metrics::{
    AgentStats, ErrorCategory, ExecutionMetrics, NextStep, PriorityLevel, PriorityUpdate,
};
use crate::models::EventType;

/// Minimum runs before computing stable stats
const MIN_RUNS_FOR_STATS: u32 = 3;

/// Minimum runs before auto-updating priority
const MIN_RUNS_FOR_PRIORITY_UPDATE: u32 = 5;

/// Failure rate thresholds for priority escalation
const CRITICAL_FAILURE_RATE: f64 = 0.5;
const HIGH_FAILURE_RATE: f64 = 0.25;
const MEDIUM_FAILURE_RATE: f64 = 0.1;

/// Tracks current priorities per agent type (in-memory cache).
type PriorityCache = HashMap<String, u8>;

/// The automated feedback loop service.
#[derive(Clone)]
pub struct FeedbackLoopService {
    db: SurrealClient,
    /// In-memory cache of current priorities per agent type
    priorities: Arc<RwLock<PriorityCache>>,
    /// Previously computed stats (for delta detection)
    previous_stats: Arc<RwLock<HashMap<String, AgentStats>>>,
}

impl FeedbackLoopService {
    /// Create a new FeedbackLoopService.
    pub fn new(db: SurrealClient) -> Self {
        Self {
            db,
            priorities: Arc::new(RwLock::new(HashMap::new())),
            previous_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ------------------------------------------------------------------------
    // Core API
    // ------------------------------------------------------------------------

    /// Record execution metrics from a completed agent run.
    ///
    /// This is called post-run to collect metrics and trigger the feedback loop.
    pub async fn record_metrics(&self, metrics: ExecutionMetrics) -> anyhow::Result<()> {
        debug!(
            "Recording metrics for agent {} (run={}, success={})",
            metrics.agent_id, metrics.run_id, metrics.success
        );

        // Store in database
        let stored: ExecutionMetrics = self.db.create("execution_metrics", &metrics).await?;

        // Emit timeline event
        let event_type = if metrics.success {
            EventType::SubAgentCompleted(metrics.agent_type.clone())
        } else {
            EventType::SubAgentFailed(metrics.agent_type.clone())
        };

        let timeline_event = crate::models::TimelineEvent::new(&metrics.agent_id, event_type)
            .with_payload(serde_json::json!({
                "run_id": metrics.run_id,
                "agent_type": metrics.agent_type,
                "success": metrics.success,
                "duration_ms": metrics.duration_ms,
                "error_category": metrics.error_category.map(|e| e.to_string()),
            }));

        let _ = self.db.create("timeline_events", &timeline_event).await;

        info!(
            "Recorded metrics for agent {} (run={}): {:?}",
            stored.agent_id, stored.run_id, stored.error_category
        );

        Ok(())
    }

    /// Record multiple metrics from a swarm run.
    pub async fn record_swarm_metrics(
        &self,
        run_id: &str,
        results: &[SwarmAgentResult],
    ) -> anyhow::Result<()> {
        for result in results {
            let metrics = ExecutionMetrics::from_agent_result(
                run_id,
                &result.agent_id,
                &result.agent_type,
                result.success,
                result.duration_ms,
                result.return_code,
                result.error.clone(),
                crate::models::FlexibleTimestamp::now(),
            )
            .with_project(&result.project_id)
            .with_output_lines(result.output_lines.unwrap_or(0));

            if let Err(e) = self.record_metrics(metrics).await {
                warn!("Failed to record metrics for {}: {}", result.agent_id, e);
            }
        }

        // After recording all metrics, analyze and potentially update priorities
        self.analyze_and_update_priorities(run_id).await?;

        Ok(())
    }

    /// Analyze current metrics and generate next steps.
    ///
    /// Returns next steps for all agent types that have failures.
    pub async fn generate_next_steps(&self) -> anyhow::Result<Vec<NextStep>> {
        let stats = self.compute_agent_stats(None).await?;
        let mut all_steps = Vec::new();

        for (_, agent_stats) in stats {
            if agent_stats.failed_runs == 0 {
                continue;
            }

            let steps = NextStep::generate_from_patterns(
                &agent_stats.agent_type,
                &agent_stats.error_patterns,
            );
            all_steps.extend(steps);
        }

        // Sort by confidence descending
        all_steps.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        Ok(all_steps)
    }

    /// Get next steps for a specific agent type.
    pub async fn get_next_steps_for_agent(
        &self,
        agent_type: &str,
    ) -> anyhow::Result<Vec<NextStep>> {
        let stats = self.compute_agent_stats(Some(agent_type)).await?;
        let mut all_steps = Vec::new();

        if let Some(agent_stats) = stats.get(agent_type) {
            if agent_stats.failed_runs > 0 {
                all_steps = NextStep::generate_from_patterns(
                    agent_type,
                    &agent_stats.error_patterns,
                );
            }
        }

        Ok(all_steps)
    }

    /// Get current priority for an agent type.
    pub async fn get_priority(&self, agent_type: &str) -> u8 {
        let cache = self.priorities.read().await;
        *cache.get(agent_type).unwrap_or(&1)
    }

    /// Get all current priorities.
    pub async fn get_all_priorities(&self) -> HashMap<String, u8> {
        let cache = self.priorities.read().await;
        cache.clone()
    }

    /// Analyze metrics and auto-update priorities.
    ///
    /// This is the core feedback loop: after enough data is collected,
    /// priorities are adjusted based on failure patterns.
    pub async fn analyze_and_update_priorities(
        &self,
        triggered_by_run_id: &str,
    ) -> anyhow::Result<Vec<PriorityUpdate>> {
        let stats_map = self.compute_agent_stats(None).await?;
        let mut updates = Vec::new();

        for (agent_type, new_stats) in stats_map {
            // Only update if we have enough runs
            if new_stats.total_runs < MIN_RUNS_FOR_PRIORITY_UPDATE {
                debug!(
                    "Skipping priority update for {}: only {} runs (need {})",
                    agent_type, new_stats.total_runs, MIN_RUNS_FOR_PRIORITY_UPDATE
                );
                continue;
            }

            let old_priority = self.get_priority(&agent_type).await;
            let new_priority_level =
                PriorityLevel::from_failure_rate(new_stats.failure_rate, false);
            let new_priority = new_priority_level.as_u8();

            if new_priority != old_priority {
                let reason = format!(
                    "Failure rate changed from {}% to {}% ({}/{} failed runs)",
                    (old_priority as f64 / 10.0 * 100.0) as u32,
                    (new_priority as f64 / 10.0 * 100.0) as u32,
                    new_stats.failed_runs,
                    new_stats.total_runs
                );

                let update = PriorityUpdate::new(
                    &agent_type,
                    old_priority,
                    new_priority,
                    &reason,
                    triggered_by_run_id,
                );

                // Persist the update
                let _: PriorityUpdate = self.db.create("priority_updates", &update).await?;

                // Update in-memory cache
                {
                    let mut cache = self.priorities.write().await;
                    cache.insert(agent_type.clone(), new_priority);
                }

                info!(
                    "Priority updated for {}: {} -> {} ({})",
                    agent_type, old_priority, new_priority, reason
                );

                updates.push(update);
            }
        }

        Ok(updates)
    }

    /// Get aggregated stats for all agent types.
    pub async fn get_stats(&self) -> anyhow::Result<Vec<AgentStats>> {
        let stats_map = self.compute_agent_stats(None).await?;
        let mut stats: Vec<_> = stats_map.into_values().collect();
        stats.sort_by(|a, b| {
            b.failed_runs
                .cmp(&a.failed_runs)
                .then_with(|| b.failure_rate().partial_cmp(&a.failure_rate()).unwrap())
        });
        Ok(stats)
    }

    /// Get priority update history.
    pub async fn get_priority_history(&self) -> anyhow::Result<Vec<PriorityUpdate>> {
        let query = r#"
            SELECT * FROM priority_updates
            ORDER BY timestamp DESC
            LIMIT 50
        "#;
        let mut response = self.db.query(query).await?;
        let updates: Vec<PriorityUpdate> = response.take(0)?;
        Ok(updates)
    }

    // ------------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------------

    /// Compute aggregated stats per agent type.
    async fn compute_agent_stats(
        &self,
        filter_agent_type: Option<&str>,
    ) -> anyhow::Result<HashMap<String, AgentStats>> {
        let where_clause = match filter_agent_type {
            Some(at) => format!("WHERE agent_type = '{}'", at),
            None => String::new(),
        };

        let query = format!(
            r#"
            SELECT
                agent_type,
                count() as total_runs,
                math::sum(success) as successful_runs,
                math::sum(1 - success) as failed_runs,
                math::mean(duration_ms) as avg_duration_ms,
                math::min(duration_ms) as min_duration_ms,
                math::max(duration_ms) as max_duration_ms
            FROM execution_metrics
            {}
            GROUP BY agent_type
            "#,
            where_clause
        );

        #[derive(Debug, serde::Deserialize)]
        struct RawStats {
            agent_type: String,
            total_runs: u32,
            successful_runs: Option<f64>,
            failed_runs: Option<f64>,
            avg_duration_ms: Option<f64>,
            min_duration_ms: Option<u64>,
            max_duration_ms: Option<u64>,
        }

        let mut raw_stats_response = self.db.query(&query).await?;
        let raw_stats: Vec<RawStats> = raw_stats_response.take(0)?;
        let mut stats_map = HashMap::new();

        for raw in raw_stats {
            if raw.total_runs < MIN_RUNS_FOR_STATS {
                continue;
            }

            let successful = raw.successful_runs.unwrap_or(0.0) as u32;
            let failed = raw.failed_runs.unwrap_or(0.0) as u32;
            let failure_rate = if raw.total_runs > 0 {
                failed as f64 / raw.total_runs as f64
            } else {
                0.0
            };

            // Get error patterns per category
            let error_patterns = self.get_error_patterns(&raw.agent_type).await?;

            // Generate next steps
            let recommended_next_steps: Vec<String> = NextStep::generate_from_patterns(
                &raw.agent_type,
                &error_patterns,
            )
            .into_iter()
            .map(|s| s.action)
            .take(3)
            .collect();

            let priority_level =
                PriorityLevel::from_failure_rate(failure_rate, false);

            let stats = AgentStats {
                agent_type: raw.agent_type.clone(),
                total_runs: raw.total_runs,
                successful_runs: successful,
                failed_runs: failed,
                avg_duration_ms: raw.avg_duration_ms.unwrap_or(0.0),
                min_duration_ms: raw.min_duration_ms.unwrap_or(0),
                max_duration_ms: raw.max_duration_ms.unwrap_or(0),
                failure_rate,
                error_patterns,
                recommended_next_steps,
                current_priority: priority_level.as_u8(),
            };

            stats_map.insert(raw.agent_type, stats);
        }

        Ok(stats_map)
    }

    /// Get error pattern counts per category for an agent type.
    async fn get_error_patterns(&self, agent_type: &str) -> anyhow::Result<HashMap<String, u32>> {
        let query = r#"
            SELECT error_category, count() as count
            FROM execution_metrics
            WHERE agent_type = $agent_type AND success = false
            GROUP BY error_category
        "#;

        #[derive(Debug, serde::Deserialize)]
        struct ErrorPatternRow {
            error_category: Option<String>,
            count: u32,
        }

        let rows: Vec<ErrorPatternRow> = self
            .db
            .query_with(query, ("agent_type", agent_type))
            .await?;

        let mut patterns = HashMap::new();
        for row in rows {
            let category = row
                .error_category
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            patterns.insert(category, row.count);
        }

        Ok(patterns)
    }
}

/// Lightweight agent result from swarm bridge (used for ingestion).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SwarmAgentResult {
    pub agent_id: String,
    pub agent_type: String,
    pub success: bool,
    pub duration_ms: u64,
    pub return_code: Option<i32>,
    pub error: Option<String>,
    pub output_lines: Option<u32>,
    pub project_id: String,
}

impl From<serde_json::Value> for SwarmAgentResult {
    fn from(json: serde_json::Value) -> Self {
        Self {
            agent_id: json["id"].as_str().unwrap_or("unknown").to_string(),
            agent_type: json["name"].as_str().unwrap_or("unknown").to_string(),
            success: json["status"].as_str().map(|s| s == "success").unwrap_or(false),
            duration_ms: json["duration_ms"].as_u64().unwrap_or(0),
            return_code: json["returncode"].as_i64().map(|c| c as i32),
            error: json["stderr"].as_str().map(|s| s.to_string()),
            output_lines: json["lines"]
                .as_array()
                .map(|arr| arr.len() as u32),
            project_id: "default".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::execution_metrics::categorize_error;

    #[test]
    fn test_categorize_error_timeout() {
        let cat = categorize_error("Operation timed out after 30s").unwrap();
        assert_eq!(cat, ErrorCategory::Timeout);
    }

    #[test]
    fn test_categorize_error_command_not_found() {
        let cat = categorize_error("cargo: command not found").unwrap();
        assert_eq!(cat, ErrorCategory::CommandNotFound);
    }

    #[test]
    fn test_categorize_error_compile() {
        let cat = categorize_error("error: compilation failed\n  --> src/lib.rs:10:5").unwrap();
        assert_eq!(cat, ErrorCategory::RustCompileError);
    }

    #[test]
    fn test_priority_level_from_rate() {
        assert_eq!(
            PriorityLevel::from_failure_rate(0.6, false),
            PriorityLevel::Critical
        );
        assert_eq!(
            PriorityLevel::from_failure_rate(0.3, false),
            PriorityLevel::High
        );
        assert_eq!(
            PriorityLevel::from_failure_rate(0.15, false),
            PriorityLevel::Medium
        );
        assert_eq!(
            PriorityLevel::from_failure_rate(0.05, false),
            PriorityLevel::Low
        );
        assert_eq!(PriorityLevel::from_failure_rate(0.0, false), PriorityLevel::Healthy);
    }

    #[test]
    fn test_next_step_generation() {
        let mut patterns = HashMap::new();
        patterns.insert("timeout".to_string(), 5);
        patterns.insert("command_not_found".to_string(), 2);

        let steps = NextStep::generate_from_patterns("test_agent", &patterns);
        assert!(!steps.is_empty());
        assert_eq!(steps[0].agent_type, "test_agent");
    }
}
