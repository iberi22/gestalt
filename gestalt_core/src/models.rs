//! Shared domain models used across Gestalt crates.
//!
//! These types are canonical — all consumers should import them from here
//! rather than redefining them locally.

// ============================================================================
// Core Data Structures for Agent Execution Metrics
// ============================================================================

/// Metrics collected from a single agent execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionMetrics {
    pub id: Option<String>,
    pub run_id: String,
    pub agent_id: String,
    pub agent_type: String,
    pub success: bool,
    pub duration_ms: u64,
    pub tools_used: u64,
    pub return_code: Option<i32>,
    pub error_category: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: String,
    pub project_id: Option<String>,
    pub output_lines: Option<u64>,
    pub metadata: serde_json::Value,
}

/// A recommended next action for a given agent type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NextStep {
    pub agent_type: String,
    pub confidence: f64,
    pub action: String,
    pub reason: String,
    pub error_category: Option<String>,
}

/// A priority adjustment for an agent type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PriorityUpdate {
    pub agent_type: String,
    pub old_priority: u64,
    pub new_priority: u64,
    pub reason: String,
}

/// Aggregate statistics for a given agent type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentStats {
    pub agent_type: String,
    pub failure_rate: f64,
}

// ============================================================================
// Error Categorisation
// ============================================================================

/// Categorise a stderr string into a high-level error bucket.
pub fn categorize_error(stderr: &str) -> Option<String> {
    let lower = stderr.to_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        Some("timeout".to_string())
    } else if lower.contains("rate limit") || lower.contains("429") {
        Some("rate_limit".to_string())
    } else if lower.contains("auth") || lower.contains("key") {
        Some("authentication".to_string())
    } else {
        Some("unknown".to_string())
    }
}
