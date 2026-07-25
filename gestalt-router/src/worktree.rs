use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;
use crate::run::RouterError;

#[derive(Debug, Clone)]
pub struct WorktreeManager {
    pub base_dir: PathBuf,
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Creates a git worktree for a specific agent/run at a given base SHA.
    pub fn create_worktree(&self, run_id: Uuid, agent_id: &str, base_sha: &str) -> Result<PathBuf, RouterError> {
        let wt_path = self.base_dir
            .join(run_id.to_string())
            .join("wts")
            .join(agent_id);

        // Ensure parent directory exists
        if let Some(parent) = wt_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| RouterError::GitError(format!("Failed to create parent directory for worktree: {}", e)))?;
        }

        let branch_name = format!("gestalt/{}/{}", run_id, agent_id);

        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &branch_name,
                wt_path.to_str().ok_or_else(|| RouterError::GitError("Invalid worktree path UTF-8".to_string()))?,
                base_sha,
            ])
            .output()
            .map_err(|e| RouterError::GitError(format!("Failed to spawn git worktree add: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RouterError::GitError(format!("git worktree add failed: {}", stderr)));
        }

        Ok(wt_path)
    }

    /// Removes a git worktree.
    pub fn cleanup_worktree(&self, path: &Path) -> Result<(), RouterError> {
        if !path.exists() {
            return Ok(());
        }

        let output = Command::new("git")
            .args([
                "worktree",
                "remove",
                "--force",
                path.to_str().ok_or_else(|| RouterError::GitError("Invalid worktree path UTF-8".to_string()))?,
            ])
            .output()
            .map_err(|e| RouterError::GitError(format!("Failed to spawn git worktree remove: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RouterError::GitError(format!("git worktree remove failed: {}", stderr)));
        }

        Ok(())
    }
}
