use crate::run_state::AgentState;
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
    pub integration_branch: Option<String>,
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
    pub state: AgentState,
    pub output: Option<String>,
    pub error: Option<String>,
    pub branch: Option<String>,
    pub changed_files: Vec<String>,
    pub duration_ms: u64,
    pub run_id: Option<uuid::Uuid>,
    pub worktree_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictKind {
    Overlap,
    MergeConflict,
    BinaryConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub agent_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: Uuid,
    pub task: String,
    pub agents: Vec<AgentResult>,
    pub duration_ms: u64,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
    pub events_path: String,
    pub success: bool,
}

impl RunReport {
    /// Serialize this report as a JSON Value suitable for Xavier archival.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "run_id": self.run_id.to_string(),
            "task": self.task,
            "agents": self.agents.iter().map(|r| {
                serde_json::json!({
                    "agent_id": r.agent_id,
                    "state": format!("{:?}", r.state),
                    "error": r.error,
                    "changed_files": r.changed_files,
                    "duration_ms": r.duration_ms,
                })
            }).collect::<Vec<_>>(),
            "merged_branches": self.merged_branches,
            "conflicts": self.conflicts.iter().map(|c| {
                serde_json::json!({
                    "agent_id": c.agent_id,
                    "path": c.path,
                })
            }).collect::<Vec<_>>(),
            "duration_ms": self.duration_ms,
            "success": self.success,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouterErrorKind {
    GitError,
    AgentError,
    Timeout,
    InvalidSpec,
    TimelineError,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct RouterError {
    pub kind: RouterErrorKind,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

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

    pub fn timeline_error(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::TimelineError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn git_error(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::GitError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn agent_error(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::AgentError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn invalid_spec(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::InvalidSpec,
            message: msg.into(),
            source: None,
        }
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::Timeout,
            message: msg.into(),
            source: None,
        }
    }

    #[allow(non_snake_case)]
    pub fn TimelineError(msg: impl Into<String>) -> Self {
        Self::timeline_error(msg)
    }

    #[allow(non_snake_case)]
    pub fn GitError(msg: impl Into<String>) -> Self {
        Self::git_error(msg)
    }

    #[allow(non_snake_case)]
    pub fn AgentError(msg: impl Into<String>) -> Self {
        Self::agent_error(msg)
    }

    #[allow(non_snake_case)]
    pub fn InvalidSpec(msg: impl Into<String>) -> Self {
        Self::invalid_spec(msg)
    }

    #[allow(non_snake_case)]
    pub fn Timeout(msg: impl Into<String>) -> Self {
        Self::timeout(msg)
    }
}
