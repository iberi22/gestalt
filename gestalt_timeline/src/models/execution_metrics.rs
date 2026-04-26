//! Execution Metrics - Core data structures for the feedback loop
//!
//! Tracks agent execution metrics to enable automated improvement
//! through pattern analysis and priority adjustment.

use super::FlexibleTimestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::sql::Thing;

/// Aggregated execution metrics for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Thing>,

    /// Run identifier (matches swarm run ID)
    pub run_id: String,

    /// Agent identifier
    pub agent_id: String,

    /// Agent type (code_analyzer, git_analyzer, etc.)
    pub agent_type: String,

    /// Whether the agent succeeded
    pub success: bool,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Number of tools used
    pub tools_used: u32,

    /// Return code (0 = success, non-zero = failure)
    pub return_code: Option<i32>,

    /// Error category if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_category: Option<ErrorCategory>,

    /// Error message (truncated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// Timestamp of the run
    pub timestamp: FlexibleTimestamp,

    /// Associated project ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,

    /// Lines of output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_lines: Option<u32>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl ExecutionMetrics {
    /// Create metrics from swarm agent result.
    pub fn from_agent_result(
        run_id: &str,
        agent_id: &str,
        agent_type: &str,
        success: bool,
        duration_ms: u64,
        return_code: Option<i32>,
        error_message: Option<String>,
        timestamp: FlexibleTimestamp,
    ) -> Self {
        let error_category = error_message.as_ref().and_then(|e| categorize_error(e));
        Self {
            id: None,
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            success,
            duration_ms,
            tools_used: 1,
            return_code,
            error_category,
            error_message: error_message.map(|e| e.chars().take(200).collect()),
            timestamp,
            project_id: None,
            output_lines: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the project ID.
    pub fn with_project(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }

    /// Set output lines count.
    pub fn with_output_lines(mut self, lines: u32) -> Self {
        self.output_lines = Some(lines);
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Categories of errors for pattern analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Command not found or not installed
    CommandNotFound,
    /// Operation timed out
    Timeout,
    /// Non-zero exit code
    NonZeroExit,
    /// Permission denied
    PermissionDenied,
    /// File not found
    FileNotFound,
    /// Network/connectivity issue
    NetworkError,
    /// Rust compilation error
    RustCompileError,
    /// JSON parse error
    ParseError,
    /// Resource exhausted (memory, disk, etc.)
    ResourceExhausted,
    /// Unknown error
    Unknown,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCategory::CommandNotFound => write!(f, "command_not_found"),
            ErrorCategory::Timeout => write!(f, "timeout"),
            ErrorCategory::NonZeroExit => write!(f, "non_zero_exit"),
            ErrorCategory::PermissionDenied => write!(f, "permission_denied"),
            ErrorCategory::FileNotFound => write!(f, "file_not_found"),
            ErrorCategory::NetworkError => write!(f, "network_error"),
            ErrorCategory::RustCompileError => write!(f, "rust_compile_error"),
            ErrorCategory::ParseError => write!(f, "parse_error"),
            ErrorCategory::ResourceExhausted => write!(f, "resource_exhausted"),
            ErrorCategory::Unknown => write!(f, "unknown"),
        }
    }
}

/// Categorize an error message into an ErrorCategory.
pub fn categorize_error(error: &str) -> Option<ErrorCategory> {
    let lower = error.to_lowercase();

    if lower.contains("not found") || lower.contains("enoent") {
        if lower.contains("command") || lower.contains("cargo") || lower.contains("rg") {
            return Some(ErrorCategory::CommandNotFound);
        }
        return Some(ErrorCategory::FileNotFound);
    }

    if lower.contains("timeout") || lower.contains("timed out") {
        return Some(ErrorCategory::Timeout);
    }

    if lower.contains("permission denied") || lower.contains("access denied") {
        return Some(ErrorCategory::PermissionDenied);
    }

    if lower.contains("couldn") || lower.contains("connection")
        || lower.contains("network") || lower.contains("dns")
    {
        return Some(ErrorCategory::NetworkError);
    }

    if lower.contains("error") && (lower.contains("compilation") || lower.contains("cargo")
        || lower.contains("rustc") || lower.contains("^"))
    {
        return Some(ErrorCategory::RustCompileError);
    }

    if lower.contains("json") && lower.contains("parse") {
        return Some(ErrorCategory::ParseError);
    }

    if lower.contains("out of memory") || lower.contains("disk full")
        || lower.contains("no space left")
    {
        return Some(ErrorCategory::ResourceExhausted);
    }

    if lower.contains("exit code") && lower.contains("non") {
        return Some(ErrorCategory::NonZeroExit);
    }

    // Check for non-zero exit code pattern
    if lower.contains("exit code")
        || lower.contains("returncode")
        || lower.contains("return code")
    {
        return Some(ErrorCategory::NonZeroExit);
    }

    Some(ErrorCategory::Unknown)
}

/// Aggregated statistics per agent type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStats {
    pub agent_type: String,
    pub total_runs: u32,
    pub successful_runs: u32,
    pub failed_runs: u32,
    pub avg_duration_ms: f64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub failure_rate: f64,
    /// Map of error category -> count
    pub error_patterns: HashMap<String, u32>,
    /// Recommended next steps based on failure patterns
    pub recommended_next_steps: Vec<String>,
    /// Current priority (1-10, higher = more important to fix)
    pub current_priority: u8,
}

impl AgentStats {
    /// Calculate failure rate.
    pub fn failure_rate(&self) -> f64 {
        if self.total_runs == 0 {
            return 0.0;
        }
        self.failed_runs as f64 / self.total_runs as f64
    }

    /// Calculate average duration.
    pub fn avg_duration_ms(&self) -> f64 {
        if self.total_runs == 0 {
            return 0.0;
        }
        self.avg_duration_ms
    }
}

/// Priority level for agent types based on failure analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorityLevel {
    Critical = 9,  // >50% failure rate or security-related
    High = 7,     // 25-50% failure rate
    Medium = 5,   // 10-25% failure rate
    Low = 3,      // <10% failure rate
    Healthy = 1,  // 0% failure rate
}

impl PriorityLevel {
    pub fn from_failure_rate(rate: f64, is_security: bool) -> Self {
        if rate > 0.5 || is_security {
            PriorityLevel::Critical
        } else if rate > 0.25 {
            PriorityLevel::High
        } else if rate > 0.1 {
            PriorityLevel::Medium
        } else if rate > 0.0 {
            PriorityLevel::Low
        } else {
            PriorityLevel::Healthy
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Recommended next action based on error patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStep {
    pub action: String,
    pub reason: String,
    pub confidence: f32,
    pub agent_type: String,
    pub error_category: Option<String>,
}

impl NextStep {
    /// Generate next steps based on error patterns.
    pub fn generate_from_patterns(
        agent_type: &str,
        error_patterns: &HashMap<String, u32>,
    ) -> Vec<NextStep> {
        let mut steps = Vec::new();

        for (category, count) in error_patterns {
            if *count == 0 {
                continue;
            }

            let (action, reason, confidence) = match category.as_str() {
                "command_not_found" => (
                    format!("Ensure {} is installed and in PATH", agent_type),
                    format!("Failed {} times due to missing command", count),
                    0.95,
                ),
                "timeout" => (
                    format!("Increase timeout for {} or optimize execution", agent_type),
                    format!("Failed {} times due to timeout", count),
                    0.9,
                ),
                "permission_denied" => (
                    format!("Fix permissions for {} execution", agent_type),
                    format!("Failed {} times due to permission issues", count),
                    0.95,
                ),
                "file_not_found" => (
                    format!("Verify file paths for {} or run setup", agent_type),
                    format!("Failed {} times due to missing files", count),
                    0.85,
                ),
                "network_error" => (
                    format!("Check network connectivity for {}", agent_type),
                    format!("Failed {} times due to network issues", count),
                    0.9,
                ),
                "rust_compile_error" => (
                    format!("Run cargo check to fix compilation errors in {}", agent_type),
                    format!("Failed {} times due to Rust compilation errors", count),
                    0.9,
                ),
                "parse_error" => (
                    format!("Fix JSON/data parsing in {}", agent_type),
                    format!("Failed {} times due to parse errors", count),
                    0.85,
                ),
                "resource_exhausted" => (
                    format!("Allocate more resources or optimize {}", agent_type),
                    format!("Failed {} times due to resource exhaustion", count),
                    0.9,
                ),
                "non_zero_exit" => (
                    format!("Debug {} - unexpected non-zero exit", agent_type),
                    format!("Failed {} times with non-zero exit", count),
                    0.7,
                ),
                _ => (
                    format!("Investigate and fix {}", agent_type),
                    format!("Failed {} times due to unknown errors", count),
                    0.5,
                ),
            };

            steps.push(NextStep {
                action,
                reason,
                confidence,
                agent_type: agent_type.to_string(),
                error_category: Some(category.clone()),
            });
        }

        // Sort by confidence descending
        steps.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        steps
    }
}

/// Priority update record for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityUpdate {
    pub agent_type: String,
    pub old_priority: u8,
    pub new_priority: u8,
    pub reason: String,
    pub triggered_by_run_id: String,
    pub timestamp: FlexibleTimestamp,
}

impl PriorityUpdate {
    pub fn new(
        agent_type: &str,
        old_priority: u8,
        new_priority: u8,
        reason: &str,
        run_id: &str,
    ) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            old_priority,
            new_priority,
            reason: reason.to_string(),
            triggered_by_run_id: run_id.to_string(),
            timestamp: FlexibleTimestamp::now(),
        }
    }
}
