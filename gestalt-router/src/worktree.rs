use crate::run::RouterError;
use async_trait::async_trait;
use gestalt_core::ports::outbound::vfs::{BlockEdit, FileVersion, VfsError, VirtualFS};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub sha: Option<String>,
    pub is_active: bool,
}

fn normalize_path_simple(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    let mut is_absolute = path.is_absolute();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            Component::Normal(c) => {
                components.push(c);
            }
            Component::RootDir => {
                is_absolute = true;
            }
            _ => {}
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

/// Validator to enforce that file writes occur only within declared allowed paths.
pub struct WriteSetValidator {
    pub allowed_paths: Option<Vec<String>>,
}

impl WriteSetValidator {
    /// Create a new validator with the given declared allowed paths.
    pub fn new(allowed_paths: Option<Vec<String>>) -> Self {
        Self { allowed_paths }
    }

    /// Validates if a target path is allowed.
    /// Returns Ok(()) if allowed, or an error if the path lies outside the declared paths.
    pub fn validate(&self, path: &str) -> Result<(), String> {
        let allowed = match &self.allowed_paths {
            None => {
                tracing::warn!("No allowed write-set declared for the agent. Allowing all writes by default but warning.");
                return Ok(());
            }
            Some(paths) => paths,
        };

        let target = Path::new(path);
        let norm_target = normalize_path_simple(target);

        for allowed_raw in allowed {
            let allowed_path = Path::new(allowed_raw);
            let norm_allowed = normalize_path_simple(allowed_path);

            // 1. Direct path/prefix match (e.g. if both are relative, or both are absolute)
            if norm_target == norm_allowed || norm_target.starts_with(&norm_allowed) {
                return Ok(());
            }

            // 2. Relative suffix match: if allowed_path is relative (e.g. "src")
            // and target_path is absolute (e.g. "/tmp/repo/src/main.rs"),
            // we check if target_path contains the components of allowed_path.
            if let Ok(cwd) = std::env::current_dir() {
                let norm_cwd = normalize_path_simple(&cwd);
                if let Ok(rel_target) = norm_target.strip_prefix(&norm_cwd) {
                    if rel_target == norm_allowed || rel_target.starts_with(&norm_allowed) {
                        return Ok(());
                    }
                }
            }

            // 3. Simple component-based subsequence match
            let target_comps: Vec<_> = norm_target.components().collect();
            let allowed_comps: Vec<_> = norm_allowed.components().collect();
            if target_comps.len() >= allowed_comps.len() {
                for window in target_comps.windows(allowed_comps.len()) {
                    if window == allowed_comps.as_slice() {
                        return Ok(());
                    }
                }
            }
        }

        Err(format!(
            "Write set violation: path '{}' is outside declared allowed paths: {:?}",
            path, allowed
        ))
    }
}

pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub write_set_allowed_paths: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>>,
}

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl Default for WorktreeManager {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/gestalt"))
    }
}

impl WorktreeManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            write_set_allowed_paths: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Declare / register the allowed write paths (write-set) for a specific agent.
    pub fn register_allowed_paths(&self, agent_id: &str, paths: Vec<String>) {
        if let Ok(mut map) = self.write_set_allowed_paths.lock() {
            map.insert(agent_id.to_string(), paths);
        }
    }

    /// Retrieve the declared allowed write paths for a specific agent.
    pub fn get_allowed_paths(&self, agent_id: &str) -> Option<Vec<String>> {
        if let Ok(map) = self.write_set_allowed_paths.lock() {
            map.get(agent_id).cloned()
        } else {
            None
        }
    }

    /// High-level create_worktree: creates a worktree named by run_id + agent_id.
    pub fn create_worktree(
        &self,
        run_id: Uuid,
        agent_id: &str,
        base_sha: &str,
    ) -> Result<PathBuf, RouterError> {
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
        self.run_git_command_locked(repo_path, args)
    }

    /// Internal git executor with automatic retry for lock/concurrency conflicts.
    fn run_git_command_locked(
        &self,
        repo_path: &Path,
        args: &[&str],
    ) -> Result<String, RouterError> {
        Self::verify_git()?;

        let mut retries = 5;
        let mut delay = std::time::Duration::from_millis(50);

        loop {
            let output = std::process::Command::new("git")
                .current_dir(repo_path)
                .args(args)
                .output()
                .map_err(|e| {
                    RouterError::GitError(format!("Failed to execute git command: {e}"))
                })?;

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
        let res =
            self.run_git_command_locked(repo_path, &["worktree", "remove", "--force", path_str]);

        match res {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("not a valid worktree")
                    || err_str.contains("is not a worktree")
                    || err_str.contains("not a working tree")
                    || err_str.contains("is not a working tree")
                    || err_str.contains("no es un árbol de trabajo")
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
        self.list_worktrees_locked(repo_path)
    }

    fn list_worktrees_locked(&self, repo_path: &Path) -> Result<Vec<WorktreeInfo>, RouterError> {
        let output_str =
            self.run_git_command_locked(repo_path, &["worktree", "list", "--porcelain"])?;

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
                    let clean_branch =
                        if let Some(stripped) = branch_str.strip_prefix("refs/heads/") {
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
        self.run_git_command_locked(repo_path, &["worktree", "prune"])?;

        Ok(())
    }
}

// ── VirtualFS implementation (legacy git-backed) ───────────────────────────

fn run_git_show(repo_path: &Path, path: &str) -> Result<String, VfsError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["show", format!("HEAD:{}", path).as_str()])
        .output()
        .map_err(|e| VfsError::Internal(format!("git show failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("fatal: bad revision") || stderr.contains("fatal: Path") {
            return Err(VfsError::NotFound(format!("path not found: {path}")));
        }
        return Err(VfsError::Internal(format!(
            "git show error: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_log(repo_path: &Path, path: &str) -> Result<Vec<FileVersion>, VfsError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args([
            "log",
            "--format=%H|%ai|%an",
            "--follow",
            "--",
            path,
        ])
        .output()
        .map_err(|e| VfsError::Internal(format!("git log failed: {e}")))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut versions = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() == 3 {
            let git_sha = parts[0].to_string();
            let timestamp = parts[1].to_string();
            let agent_id = parts[2].to_string();

            // Get content at this revision to compute a content hash
            let content = match run_git_show_at(repo_path, path, &git_sha) {
                Ok(c) => c,
                Err(_) => continue,
            };

            versions.push(FileVersion {
                hash: sha256_hex(&content),
                content,
                timestamp,
                agent_id,
            });
        }
    }

    Ok(versions)
}

fn run_git_show_at(repo_path: &Path, path: &str, sha: &str) -> Result<String, VfsError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["show", format!("{}:{}", sha, path).as_str()])
        .output()
        .map_err(|e| VfsError::Internal(format!("git show at {sha} failed: {e}")))?;

    if !output.status.success() {
        return Err(VfsError::NotFound(format!(
            "path not found at {sha}: {path}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_diff(repo_path: &Path, path: &str, from: &str, to: &str) -> Result<String, VfsError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(["diff", &format!("{}..{}", from, to), "--", path])
        .output()
        .map_err(|e| VfsError::Internal(format!("git diff failed: {e}")))?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn get_repo_path() -> Result<PathBuf, VfsError> {
    std::env::current_dir().map_err(|e| VfsError::Internal(format!("Failed to get cwd: {e}")))
}

#[async_trait]
impl VirtualFS for WorktreeManager {
    async fn read_file(&self, path: &str) -> Result<(String, String), VfsError> {
        let repo_path = get_repo_path()?;
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let content = run_git_show(&repo_path, &path)?;
            let hash = sha256_hex(&content);
            Ok((content, hash))
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn write_block(&self, path: &str, block: BlockEdit) -> Result<String, VfsError> {
        // Enforce write-set via WriteSetValidator
        let allowed_paths = self.get_allowed_paths(&block.agent_id);
        let validator = WriteSetValidator::new(allowed_paths);
        if let Err(e) = validator.validate(path) {
            tracing::error!("WriteSetValidator rejected write: {}", e);
            return Err(VfsError::Internal(e));
        }

        let repo_path = get_repo_path()?;
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            // Read current content (from git or filesystem)
            let current_content: String = match run_git_show(&repo_path, &path) {
                Ok(c) => c,
                Err(VfsError::NotFound(_)) => String::new(),
                Err(e) => return Err(e),
            };

            // Apply block edit
            let new_content = if block.old_string.is_empty() && block.new_string.is_empty() {
                current_content.clone()
            } else if current_content.contains(&block.old_string) {
                current_content.replace(&block.old_string, &block.new_string)
            } else if !block.context.is_empty() && current_content.contains(&block.context) {
                current_content.replace(&block.old_string, &block.new_string)
            } else if current_content.is_empty() {
                block.new_string.clone()
            } else {
                format!("{}{}", current_content, block.new_string)
            };

            // Write to filesystem
            let disk_path = std::path::Path::new(&path);
            if let Some(parent) = disk_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| VfsError::Internal(format!("failed to create dirs: {e}")))?;
            }
            std::fs::write(disk_path, &new_content)
                .map_err(|e| VfsError::Internal(format!("failed to write file: {e}")))?;

            let hash = sha256_hex(&new_content);
            Ok(hash)
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn list_versions(&self, path: &str) -> Result<Vec<FileVersion>, VfsError> {
        let repo_path = get_repo_path()?;
        let path = path.to_string();
        tokio::task::spawn_blocking(move || run_git_log(&repo_path, &path))
            .await
            .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn get_diff(&self, path: &str, from: &str, to: &str) -> Result<String, VfsError> {
        let repo_path = get_repo_path()?;
        let path = path.to_string();
        let from = from.to_string();
        let to = to.to_string();
        tokio::task::spawn_blocking(move || run_git_diff(&repo_path, &path, &from, &to))
            .await
            .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn lock(&self, _path: &str, _agent: &str) -> Result<bool, VfsError> {
        // Legacy WorktreeManager — file-level locking is handled by MemState.
        Ok(true)
    }

    async fn unlock(&self, _path: &str, _agent: &str) -> Result<bool, VfsError> {
        Ok(true)
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

    #[tokio::test]
    async fn test_write_set_validator_enforcement() {
        let temp_dir = TempDir::new("gestalt_vfs_test");

        // Initialize repository
        run_git(&temp_dir.path, &["init"]);
        run_git(&temp_dir.path, &["config", "user.name", "Gestalt Test"]);
        run_git(&temp_dir.path, &["config", "user.email", "test@gestalt.ai"]);

        // Initial commit so HEAD:file.txt exists for git show / read_file
        let test_file = temp_dir.path.join("file.txt");
        fs::write(&test_file, "initial").unwrap();
        run_git(&temp_dir.path, &["add", "file.txt"]);
        run_git(&temp_dir.path, &["commit", "-m", "initial commit"]);

        let manager = WorktreeManager::new(PathBuf::from("/tmp"));

        // Register allowed paths for "agent-test"
        manager.register_allowed_paths("agent-test", vec!["file.txt".to_string(), "src/".to_string()]);

        // Change current directory to our test repo so that run_git_show can find it
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir.path).unwrap();

        // 1. Write within declared path (exact match) should succeed
        let block1 = BlockEdit {
            agent_id: "agent-test".to_string(),
            run_id: "run-test".to_string(),
            old_string: "initial".to_string(),
            new_string: "updated".to_string(),
            context: "".to_string(),
        };

        let res1 = manager.write_block("file.txt", block1).await;
        std::env::set_current_dir(&original_dir).unwrap(); // Restore directory before assertions

        assert!(res1.is_ok(), "Expected write to succeed inside allowed path. Got error: {:?}", res1);

        // Restore dir again for the second check
        std::env::set_current_dir(&temp_dir.path).unwrap();

        // 2. Write outside declared path should fail
        let block2 = BlockEdit {
            agent_id: "agent-test".to_string(),
            run_id: "run-test".to_string(),
            old_string: "".to_string(),
            new_string: "malicious code".to_string(),
            context: "".to_string(),
        };

        let res2 = manager.write_block("forbidden.txt", block2).await;
        std::env::set_current_dir(&original_dir).unwrap(); // Restore directory

        assert!(res2.is_err(), "Expected write to be rejected outside allowed path");
        let err_str = res2.unwrap_err().to_string();
        assert!(err_str.contains("Write set violation"), "Expected Write set violation, got: {}", err_str);
    }
}
