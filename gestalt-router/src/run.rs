use crate::run_state::AgentState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
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

fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    let mut is_absolute = path.is_absolute();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            },
            Component::CurDir => {},
            Component::Normal(c) => {
                components.push(c);
            },
            Component::RootDir => {
                is_absolute = true;
            },
            _ => {},
        }
    }
    let mut result = PathBuf::new();
    if is_absolute {
        result.push("/");
    }
    for c in components {
        result.push(c);
    }
    result
}

fn is_symlink_escape(worktree_root: &Path, file_path: &Path) -> bool {
    if let Ok(target) = std::fs::read_link(file_path) {
        let resolved = if target.is_absolute() {
            target
        } else {
            let parent = file_path.parent().unwrap_or(worktree_root);
            parent.join(target)
        };
        let norm_root = normalize_path(worktree_root);
        let norm_resolved = normalize_path(&resolved);
        if !norm_resolved.starts_with(&norm_root) {
            return true;
        }
    }
    false
}

fn scan_for_symlink_escapes(dir: &Path, current_dir: &Path, escaped_symlinks: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if metadata.file_type().is_symlink() {
                    if is_symlink_escape(dir, &path) {
                        escaped_symlinks.push(path.clone());
                    }
                } else if metadata.is_dir() {
                    scan_for_symlink_escapes(dir, &path, escaped_symlinks);
                }
            }
        }
    }
}
