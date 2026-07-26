use crate::run::RouterError;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub sha: Option<String>,
    pub is_active: bool,
}

pub struct WorktreeManager {
    lock: Mutex<()>,
    pub base_dir: PathBuf,
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/gestalt"))
    }
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            lock: Mutex::new(()),
            base_dir,
        }
    }

    /// High-level create_worktree: creates a worktree named by run_id + agent_id.
    pub fn create_worktree(
        &self,
        run_id: Uuid,
        agent_id: &str,
        base_sha: &str,
    ) -> Result<PathBuf, RouterError> {
        let _lock = self.lock.lock().unwrap();
        let branch = format!("gestalt/{}/{}", run_id, agent_id);
        let wt_path = self.base_dir.join(format!("{}-{}", run_id, agent_id));

        let repo_dir = std::env::current_dir()
            .map_err(|e| RouterError::GitError(format!("Failed to get current dir: {}", e)))?;

        // Idempotency: cleanup existing worktree at wt_path if registered
        if let Ok(list) = self.list_worktrees(&repo_dir) {
            if list.iter().any(|wt| wt.path == wt_path) {
                let _ = self.remove_worktree_locked(&repo_dir, &wt_path);
            }
        }
        let _ = self.run_git_command_locked(&repo_dir, &["worktree", "prune"]);

        // Idempotency: delete branch if it already exists
        let _ = self.run_git_command_locked(&repo_dir, &["branch", "-D", &branch]);

        let path_str = wt_path
            .to_str()
            .ok_or_else(|| RouterError::GitError("Invalid worktree path".to_string()))?;

        self.run_git_command_locked(
            &repo_dir,
            &["worktree", "add", "-b", &branch, path_str, base_sha],
        )?;

        Ok(wt_path)
    }

    /// Cleanup a worktree by path.
    pub fn cleanup_worktree(&self, path: &Path) -> Result<(), RouterError> {
        let repo_dir = std::env::current_dir()
            .map_err(|e| RouterError::GitError(format!("Failed to get current dir: {}", e)))?;
        self.remove_worktree(&repo_dir, path)
    }

    /// Verifies that git is installed and accessible.
    fn verify_git() -> Result<(), RouterError> {
        let output = std::process::Command::new("git")
            .arg("--version")
            .output()
            .map_err(|e| {
                RouterError::GitError(format!("Git not found or failed to execute: {e}"))
            })?;

        if !output.status.success() {
            return Err(RouterError::GitError(
                "Git command returned non-zero exit code on --version".to_string(),
            ));
        }
        Ok(())
    }

    /// Helper to run a git command in the context of a repository path with locks.
    pub fn run_git_command(&self, repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
        let _lock = self.lock.lock().unwrap();
        self.run_git_command_locked(repo_path, args)
    }

    /// Internal git executor with automatic retry for lock/concurrency conflicts.
    fn run_git_command_locked(&self, repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
        Self::verify_git()?;

        let mut retries = 5;
        let mut delay = std::time::Duration::from_millis(50);

        loop {
            let output = std::process::Command::new("git")
                .current_dir(repo_path)
                .args(args)
                .output()
                .map_err(|e| RouterError::GitError(format!("Failed to execute git command: {e}")))?;

            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }

            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let is_lock_error = stderr.contains("index.lock")
                || stderr.contains("another git process is running")
                || stderr.contains("Lock exists")
                || stderr.contains("lock");

            if is_lock_error && retries > 0 {
                retries -= 1;
                std::thread::sleep(delay);
                delay *= 2;
                continue;
            }

            return Err(RouterError::GitError(format!(
                "Git command failed (args: {args:?}): {stderr}"
            )));
        }
    }

    /// Low-level create_worktree: creates a worktree at a specific path.
    pub fn create_worktree_at(
        &self,
        repo_path: &Path,
        base_sha: &str,
        branch: &str,
        worktree_path: &Path,
    ) -> Result<(), RouterError> {
        let _lock = self.lock.lock().unwrap();

        // Idempotency: cleanup existing worktree at worktree_path if registered
        if let Ok(list) = self.list_worktrees_locked(repo_path) {
            if list.iter().any(|wt| wt.path == worktree_path) {
                let _ = self.remove_worktree_locked(repo_path, worktree_path);
            }
        }
        let _ = self.run_git_command_locked(repo_path, &["worktree", "prune"]);

        // Idempotency: delete branch if it already exists
        let _ = self.run_git_command_locked(repo_path, &["branch", "-D", branch]);

        let path_str = worktree_path
            .to_str()
            .ok_or_else(|| RouterError::GitError("Invalid worktree path".to_string()))?;

        self.run_git_command_locked(
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
        self.remove_worktree_locked(repo_path, worktree_path)
    }

    fn remove_worktree_locked(
        &self,
        repo_path: &Path,
        worktree_path: &Path,
    ) -> Result<(), RouterError> {
        let path_str = worktree_path
            .to_str()
            .ok_or_else(|| RouterError::GitError("Invalid worktree path".to_string()))?;

        // Use --force to remove even with untracked/modified files or deleted path
        let res = self.run_git_command_locked(repo_path, &["worktree", "remove", "--force", path_str]);

        match res {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("not a valid worktree")
                    || err_str.contains("is not a worktree")
                    || err_str.contains("not a working tree")
                    || err_str.contains("is not a working tree")
                {
                    // Already removed, treat as success (idempotent)
                } else {
                    return Err(e);
                }
            }
        }

        let _ = self.run_git_command_locked(repo_path, &["worktree", "prune"]);

        Ok(())
    }

    /// Lists all worktrees in the given repository.
    /// Maps to: git worktree list --porcelain
    pub fn list_worktrees(&self, repo_path: &Path) -> Result<Vec<WorktreeInfo>, RouterError> {
        let _lock = self.lock.lock().unwrap();
        self.list_worktrees_locked(repo_path)
    }

    fn list_worktrees_locked(&self, repo_path: &Path) -> Result<Vec<WorktreeInfo>, RouterError> {
        let output_str = self.run_git_command_locked(repo_path, &["worktree", "list", "--porcelain"])?;

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
            if let Some(path_str) = line.strip_prefix("worktree ") {
                if let Some(wt) = current.take() {
                    worktrees.push(wt);
                }
                current = Some(TempWorktreeInfo {
                    path: PathBuf::from(path_str),
                    branch: None,
                    sha: None,
                    is_prunable: false,
                });
            } else if let Some(ref mut wt) = current {
                if let Some(sha) = line.strip_prefix("HEAD ") {
                    wt.sha = Some(sha.to_string());
                } else if let Some(branch_str) = line.strip_prefix("branch ") {
                    let clean_branch = if let Some(stripped) = branch_str.strip_prefix("refs/heads/") {
                        stripped.to_string()
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
    pub fn prune_worktrees(&self, repo_path: &Path) -> Result<(), RouterError> {
        let _lock = self.lock.lock().unwrap();

        self.run_git_command_locked(repo_path, &["worktree", "prune"])?;

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

        let manager = WorktreeManager::new(PathBuf::from("/tmp"));

        // 1. Create worktree
        manager
            .create_worktree_at(&repo_dir.path, &base_sha, "test-branch", &wt_path)
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
        assert!(
            maybe_removed_wt.is_none(),
            "Worktree should not be in the list"
        );

        // 5. Prune
        manager
            .prune_worktrees(&repo_dir.path)
            .expect("Failed to prune worktrees");
    }
}
