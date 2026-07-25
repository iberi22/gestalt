use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub allowed_paths: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpec {
    pub base_ref: String,
    pub task: String,
    pub agents: Vec<AgentSpec>,
    pub integration_branch: String,
    pub timeout: u64,
    pub max_parallel: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub state: crate::run_state::AgentState,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: Uuid,
    pub agents: Vec<AgentResult>,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<String>,
    pub events_path: String,
}

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("Git error: {0}")]
    GitError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid specification: {0}")]
    InvalidSpec(String),

    #[error("Timeline error: {0}")]
    TimelineError(String),
}
