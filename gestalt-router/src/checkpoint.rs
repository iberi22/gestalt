use std::path::Path;
use std::process::Command;
use crate::run::RouterError;

/// Runs a git checkpoint inside the specified worktree.
/// Returns Ok(true) if changes were found and committed, or Ok(false) if no changes were found.
pub fn run_checkpoint(worktree_path: &Path, agent_id: &str) -> Result<bool, RouterError> {
    // 1. Check if there are any changes (staged, unstaged, untracked)
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to run git status: {}", e)))?;

    if !status_output.status.success() {
        let stderr = String::from_utf8_lossy(&status_output.stderr);
        return Err(RouterError::GitError(format!("git status failed: {}", stderr)));
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);
    if status_str.trim().is_empty() {
        return Ok(false);
    }

    // 2. Stage all changes
    let add_output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to run git add: {}", e)))?;

    if !add_output.status.success() {
        let stderr = String::from_utf8_lossy(&add_output.stderr);
        return Err(RouterError::GitError(format!("git add failed: {}", stderr)));
    }

    // 3. Commit the changes, bypassing hooks with --no-verify and setting hooksPath to /dev/null
    let commit_msg = format!("checkpoint(gestalt): saved state for agent {}", agent_id);
    let commit_output = Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "-m",
            &commit_msg,
            "--no-verify",
        ])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to run git commit: {}", e)))?;

    if !commit_output.status.success() {
        let stderr = String::from_utf8_lossy(&commit_output.stderr);
        return Err(RouterError::GitError(format!("git commit failed: {}", stderr)));
    }

    Ok(true)
}
