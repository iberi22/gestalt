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
use std::sync::Mutex;
use crate::run::RouterError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub sha: Option<String>,
    pub is_active: bool,
}

pub struct WorktreeManager {
    lock: Mutex<()>,
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }

    /// Verifies that git is installed and accessible.
    fn verify_git() -> Result<(), RouterError> {
        let output = std::process::Command::new("git")
            .arg("--version")
            .output()
            .map_err(|e| RouterError::GitError(format!("Git not found or failed to execute: {e}")))?;

        if !output.status.success() {
            return Err(RouterError::GitError("Git command returned non-zero exit code on --version".to_string()));
        }
        Ok(())
    }

    /// Helper to run a git command in the context of a repository path.
    fn run_git_command(
        &self,
        repo_path: &Path,
        args: &[&str],
    ) -> Result<String, RouterError> {
        Self::verify_git()?;

        let output = std::process::Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .map_err(|e| RouterError::GitError(format!("Failed to execute git command: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(RouterError::GitError(format!(
                "Git command failed (args: {args:?}): {stderr}"
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Creates a new worktree at the specified path and checks out a new branch.
    /// Maps to: git worktree add -b <branch> <path> <sha>
    pub fn create_worktree(
        &self,
        repo_path: &Path,
        base_sha: &str,
        branch: &str,
        worktree_path: &Path,
    ) -> Result<(), RouterError> {
        let _lock = self.lock.lock().unwrap();

        let path_str = worktree_path.to_str().ok_or_else(|| {
            RouterError::GitError("Invalid worktree path".to_string())
        })?;

        self.run_git_command(
            repo_path,
            &["worktree", "add", "-b", branch, path_str, base_sha],
        )?;

        Ok(())
    }

    /// Removes an existing worktree at the specified path.
    /// Maps to: git worktree remove <path>
    pub fn remove_worktree(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<(), RouterError> {
        let _lock = self.lock.lock().unwrap();

        let path_str = worktree_path.to_str().ok_or_else(|| {
            RouterError::GitError("Invalid worktree path".to_string())
        })?;

        self.run_git_command(
            repo_path,
            &["worktree", "remove", path_str],
        )?;

        Ok(())
    }

    /// Lists all worktrees in the given repository.
    /// Maps to: git worktree list --porcelain
    pub fn list_worktrees(
        &self,
        repo_path: &Path,
    ) -> Result<Vec<WorktreeInfo>, RouterError> {
        let output_str = self.run_git_command(
            repo_path,
            &["worktree", "list", "--porcelain"],
        )?;

        struct TempWorktreeInfo {
            path: PathBuf,
            branch: Option<String>,
            sha: Option<String>,
            is_prunable: bool,
        }

        let mut worktrees = Vec::new();
        let mut current: Option<TempWorktreeInfo> = None;

        for line in output_str.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("worktree ") {
                if let Some(wt) = current.take() {
                    worktrees.push(wt);
                }
                let path_str = &line["worktree ".len()..];
                current = Some(TempWorktreeInfo {
                    path: PathBuf::from(path_str),
                    branch: None,
                    sha: None,
                    is_prunable: false,
                });
            } else if let Some(ref mut wt) = current {
                if line.starts_with("HEAD ") {
                    wt.sha = Some(line["HEAD ".len()..].to_string());
                } else if line.starts_with("branch ") {
                    let branch_str = &line["branch ".len()..];
                    let clean_branch = if branch_str.starts_with("refs/heads/") {
                        branch_str["refs/heads/".len()..].to_string()
                    } else {
                        branch_str.to_string()
                    };
                    wt.branch = Some(clean_branch);
                } else if line.starts_with("prunable") {
                    wt.is_prunable = true;
                }
            }
        }
        if let Some(wt) = current {
            worktrees.push(wt);
        }

        let result: Vec<WorktreeInfo> = worktrees
            .into_iter()
            .map(|wt| {
                let is_active = wt.path.exists() && !wt.is_prunable;
                WorktreeInfo {
                    path: wt.path,
                    branch: wt.branch,
                    sha: wt.sha,
                    is_active,
                }
            })
            .collect();

        Ok(result)
    }

    /// Prunes stale worktree administrative files.
    /// Maps to: git worktree prune
    pub fn prune_worktrees(
        &self,
        repo_path: &Path,
    ) -> Result<(), RouterError> {
        let _lock = self.lock.lock().unwrap();

        self.run_git_command(
            repo_path,
            &["worktree", "prune"],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut path = std::env::temp_dir();
            let unique_id = uuid::Uuid::new_v4().to_string();
            path.push(format!("{}_{}", name, unique_id));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn run_git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Git command failed in test: {:?} -> stderr: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn test_worktree_manager_lifecycle() {
        let repo_dir = TempDir::new("gestalt_test_repo");

        // Initialize repository
        run_git(&repo_dir.path, &["init"]);
        run_git(&repo_dir.path, &["config", "user.name", "Gestalt Test"]);
        run_git(&repo_dir.path, &["config", "user.email", "test@gestalt.ai"]);

        // Initial commit
        fs::write(repo_dir.path.join("file.txt"), "hello").unwrap();
        run_git(&repo_dir.path, &["add", "file.txt"]);
        run_git(&repo_dir.path, &["commit", "-m", "initial commit"]);

        let base_sha = run_git(&repo_dir.path, &["rev-parse", "HEAD"]);

        // Worktree directory
        let wt_parent_dir = TempDir::new("gestalt_test_wt");
        let wt_path = wt_parent_dir.path.join("wt_subdir");

        let manager = WorktreeManager::new();

        // 1. Create worktree
        manager
            .create_worktree(&repo_dir.path, &base_sha, "test-branch", &wt_path)
            .expect("Failed to create worktree");

        // 2. List worktrees
        let list = manager
            .list_worktrees(&repo_dir.path)
            .expect("Failed to list worktrees");

        // We expect at least 2 worktrees: main repository and the new worktree.
        assert!(list.len() >= 2);

        let maybe_wt = list.iter().find(|wt| wt.path == wt_path);
        assert!(maybe_wt.is_some(), "New worktree not found in list");

        let wt = maybe_wt.unwrap();
        assert_eq!(wt.branch.as_deref(), Some("test-branch"));
        assert_eq!(wt.sha.as_ref(), Some(&base_sha));
        assert!(wt.is_active);

        // 3. Remove worktree
        manager
            .remove_worktree(&repo_dir.path, &wt_path)
            .expect("Failed to remove worktree");

        // 4. Verify removed from list
        let post_remove_list = manager
            .list_worktrees(&repo_dir.path)
            .expect("Failed to list worktrees post-removal");
        let maybe_removed_wt = post_remove_list.iter().find(|wt| wt.path == wt_path);
        assert!(maybe_removed_wt.is_none(), "Worktree should not be in the list");

        // 5. Prune
        manager
            .prune_worktrees(&repo_dir.path)
            .expect("Failed to prune worktrees");
    }
}

