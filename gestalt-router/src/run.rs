use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub allowed_paths: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpec {
    pub base_ref: String,
    pub task: String,
    pub agents: Vec<AgentSpec>,
    pub max_parallel: usize,
    pub timeout: u64,
    pub push: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Success,
    Timeout,
    Crashed,
    NoChanges,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub status: AgentStatus,
    pub exit_code: Option<i32>,
    pub changed_files: Vec<String>,
    pub branch: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictKind {
    Overlap,
    MergeConflict,
    BinaryConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub path: String,
    pub agent_a: String,
    pub agent_b: String,
    pub kind: ConflictKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: Uuid,
    pub base_sha: String,
    pub agents: Vec<AgentResult>,
    pub integration_branch: String,
    pub conflicts: Vec<ConflictInfo>,
    pub events_path: String,
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouterErrorKind {
    GitError,
    AgentError,
    Timeout,
    InvalidSpec,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct RouterError {
    pub kind: RouterErrorKind,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid specification: {0}")]
    InvalidSpec(String),

    #[error("Timeline error: {0}")]
    TimelineError(String),
impl RouterError {
    pub fn new(
        kind: RouterErrorKind,
        message: impl Into<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source,
        }
    }

}