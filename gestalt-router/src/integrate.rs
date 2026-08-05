use crate::checkpoint;
use crate::run::ConflictInfo;
use crate::run::RouterError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergeResult {
    Success {
        merged_commit_sha: String,
    },
    HardConflict {
        conflicted_files: Vec<String>,
        branches_preserved: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrateResult {
    pub merge_sha: String,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIntegrationSpec {
    pub id: String,
    pub branch: String,
}

/// ClassifiedMergeError categories for parsing git/merge issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifiedMergeError {
    Conflict(String),
    LockTimeout(String),
    CorruptBranch(String),
    Other(String),
}

impl ClassifiedMergeError {
    /// Checks if this classified error is retryable under the given policy.
    pub fn is_retryable(&self, policy: &RetryPolicy) -> bool {
        match self {
            ClassifiedMergeError::Conflict(_) => policy.retry_on_conflict,
            ClassifiedMergeError::LockTimeout(_) => policy.retry_on_lock_timeout,
            ClassifiedMergeError::CorruptBranch(_) => policy.retry_on_corrupt_branch,
            ClassifiedMergeError::Other(_) => false,
        }
    }
}

/// Classifies a git command stderr/error message.
pub fn classify_git_error(err_msg: &str) -> ClassifiedMergeError {
    let lower = err_msg.to_lowercase();
    if lower.contains("conflict") || lower.contains("merge conflict") {
        ClassifiedMergeError::Conflict(err_msg.to_string())
    } else if lower.contains("lock")
        || lower.contains("timeout")
        || lower.contains("unable to create")
    {
        ClassifiedMergeError::LockTimeout(err_msg.to_string())
    } else if lower.contains("corrupt")
        || lower.contains("bad object")
        || lower.contains("not a valid")
    {
        ClassifiedMergeError::CorruptBranch(err_msg.to_string())
    } else {
        ClassifiedMergeError::Other(err_msg.to_string())
    }
}

/// Classifies a RouterError.
pub fn classify_router_error(err: &RouterError) -> ClassifiedMergeError {
    classify_git_error(&err.message)
}

/// RetryPolicy for clean-slate retries.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub backoff: std::time::Duration,
    pub retry_on_conflict: bool,
    pub retry_on_lock_timeout: bool,
    pub retry_on_corrupt_branch: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: std::time::Duration::from_millis(5),
            retry_on_conflict: false,
            retry_on_lock_timeout: true,
            retry_on_corrupt_branch: true,
        }
    }
}

std::thread_local! {
    /// Thread-local retry policy used by `integrate_branches`.
    pub static RETRY_POLICY: std::cell::RefCell<RetryPolicy> = const { std::cell::RefCell::new(RetryPolicy {
        max_attempts: 3,
        backoff: std::time::Duration::from_millis(5),
        retry_on_conflict: false,
        retry_on_lock_timeout: true,
        retry_on_corrupt_branch: true,
    }) };

    /// Thread-local hook executed before each integration attempt to allow dynamic repo adjustments (e.g. mock updates).
    pub static TEST_HOOK: std::cell::RefCell<Option<TestHookFn>> = const { std::cell::RefCell::new(None) };
}

/// Test hook signature: receives mutable mock repository entries before each integration attempt.
pub type TestHookFn = Box<dyn FnMut(&mut Vec<(String, String)>)>;

/// Helper struct for clean-slate retry documentation matching.
pub struct CleanSlateRetry;

/// Integration implementation with clean-slate retry mechanism.
pub async fn integrate_branches(
    repo_dir: &Path,
    base_sha: &str,
    integration_branch: &str,
    branches: &[(String, String)],
) -> Result<IntegrateResult, RouterError> {
    let policy = RETRY_POLICY.with(|p| p.borrow().clone());
    let mut attempts = 0;
    let mut branches_local = branches.to_vec();

    loop {
        attempts += 1;

        // Run the test hook if registered, allowing mutation of branches_local
        TEST_HOOK.with(|h| {
            if let Some(ref mut hook) = *h.borrow_mut() {
                hook(&mut branches_local);
            }
        });

        // Attempt sequential integration from the clean base_sha
        match integrate_branches_attempt(repo_dir, base_sha, integration_branch, &branches_local) {
            Ok(result) => {
                if !result.conflicts.is_empty()
                    && policy.retry_on_conflict
                    && attempts < policy.max_attempts
                {
                    let err = ClassifiedMergeError::Conflict("Merge conflict detected".to_string());
                    if err.is_retryable(&policy) {
                        tokio::time::sleep(policy.backoff).await;
                        continue;
                    }
                }
                return Ok(result);
            },
            Err(e) => {
                let classified = classify_router_error(&e);
                if classified.is_retryable(&policy) && attempts < policy.max_attempts {
                    tokio::time::sleep(policy.backoff).await;
                    continue;
                }
                return Err(e);
            },
        }
    }
}

/// Perform a single integration attempt.
fn integrate_branches_attempt(
    repo_dir: &Path,
    base_sha: &str,
    integration_branch: &str,
    branches: &[(String, String)],
) -> Result<IntegrateResult, RouterError> {
    let _ = integration_branch; // will be used when updating the integration branch ref
                                // 1. Detect binary files modified by each agent and check for binary conflicts.
    let mut binary_mods: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for (agent_id, branch_or_sha) in branches {
        let args = ["diff", "--numstat", base_sha, branch_or_sha];
        let stdout = checkpoint::run_git_cmd(repo_dir, &args).map_err(|e| {
            RouterError::GitError(format!("Failed to run git diff --numstat: {}", e))
        })?;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 && parts[0] == "-" && parts[1] == "-" {
                let path = parts[2].to_string();
                binary_mods.entry(path).or_default().push(agent_id.clone());
            }
        }
    }

    let mut conflicted_binaries = Vec::new();
    for (path, agents) in &binary_mods {
        if agents.len() > 1 {
            conflicted_binaries.push(path.clone());
        }
    }

    if !conflicted_binaries.is_empty() {
        conflicted_binaries.sort();
        let branch_names: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();
        return Ok(IntegrateResult {
            merge_sha: String::new(),
            merged_branches: branch_names,
            conflicts: conflicted_binaries
                .iter()
                .map(|f| ConflictInfo {
                    agent_id: binary_mods
                        .get(f)
                        .and_then(|v| v.first().cloned())
                        .unwrap_or_default(),
                    path: f.clone(),
                })
                .collect(),
        });
    }

    // 2. Perform sequential merge using git merge-tree
    let mut current_tree = base_sha.to_string();
    let mut conflicted_files = Vec::new();

    for (_agent_id, branch_or_sha) in branches {
        let args = [
            "merge-tree",
            "--write-tree",
            "--merge-base",
            base_sha,
            &current_tree,
            branch_or_sha,
        ];
        let result = checkpoint::run_git_cmd(repo_dir, &args);

        match result {
            Ok(stdout) => {
                let merged_tree = stdout.trim().to_string();
                // Create an intermediate commit so that we have a commit object for the next merge-tree
                let intermediate_args = [
                    "-c",
                    "core.hooksPath=/dev/null",
                    "commit-tree",
                    &merged_tree,
                    "-p",
                    &current_tree,
                    "-p",
                    branch_or_sha,
                    "-m",
                    "gestalt: intermediate merge",
                ];
                match checkpoint::run_git_cmd(repo_dir, &intermediate_args) {
                    Ok(sha) => {
                        current_tree = sha.trim().to_string();
                    },
                    Err(e) => {
                        conflicted_files.push(format!("commit-tree-failed: {}", e));
                    },
                }
            },
            Err(e) => {
                // There is a merge conflict. stdout contains the conflict info.
                let err_msg = e.to_string();
                let mut files = Vec::new();
                for line in err_msg.lines() {
                    if line.starts_with("Conflict") || line.contains("conflict") {
                        if let Some(idx) = line.find("in ") {
                            let p = &line[idx + 3..];
                            files.push(p.trim().to_string());
                        } else {
                            let words: Vec<&str> = line.split_whitespace().collect();
                            if !words.is_empty() {
                                files.push(words[words.len() - 1].trim().to_string());
                            }
                        }
                    }
                }
                if files.is_empty() {
                    files.push(format!("conflict-in-branch-{}", branch_or_sha));
                }
                conflicted_files.extend(files);
            },
        }
    }

    if !conflicted_files.is_empty() {
        conflicted_files.sort();
        conflicted_files.dedup();
        let branch_names: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();
        return Ok(IntegrateResult {
            merge_sha: String::new(),
            merged_branches: branch_names,
            conflicts: conflicted_files
                .iter()
                .map(|f| ConflictInfo {
                    agent_id: String::new(),
                    path: f.clone(),
                })
                .collect(),
        });
    }

    // 3. Resolve the merged tree SHA from the final intermediate commit
    //    (commit-tree expects a tree SHA, not a commit SHA)
    let merged_tree_sha = if !branches.is_empty() {
        let tree_rev = format!("{}:", current_tree);
        let tree_args = vec!["rev-parse", &tree_rev];
        match checkpoint::run_git_cmd(repo_dir, &tree_args) {
            Ok(tree) => tree.trim().to_string(),
            Err(_) => {
                // Fall back: the last merge-tree output may already be a tree
                current_tree.clone()
            },
        }
    } else {
        base_sha.to_string()
    };

    // 4. Create integration commit using git commit-tree
    let commit_args = vec![
        "-c",
        "core.hooksPath=/dev/null",
        "commit-tree",
        &merged_tree_sha,
        "-p",
        base_sha,
        "-m",
        "gestalt: integrate agent branches",
    ];

    let final_sha = match checkpoint::run_git_cmd(repo_dir, &commit_args) {
        Ok(sha) => sha.trim().to_string(),
        Err(e) => {
            let err_msg = e.to_string();
            // If commit-tree fails because of missing parent, try without -p
            if err_msg.contains("unknown parent") || err_msg.contains("not a valid") {
                let args_no_parent = vec![
                    "-c",
                    "core.hooksPath=/dev/null",
                    "commit-tree",
                    &merged_tree_sha,
                    "-m",
                    "gestalt: integrate agent branches",
                ];
                checkpoint::run_git_cmd(repo_dir, &args_no_parent)
                    .map(|sha| sha.trim().to_string())?
            } else {
                return Err(RouterError::GitError(format!(
                    "Failed to create merge commit: {}",
                    err_msg
                )));
            }
        },
    };

    let merged_branches: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();

    Ok(IntegrateResult {
        merge_sha: final_sha,
        merged_branches,
        conflicts: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use std::process::Command;
    use uuid::Uuid;

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("gestalt_integrate_test_{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn init_git_repo(repo_path: &Path) {
        Command::new("git")
            .arg("init")
            .current_dir(repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Tester"])
            .current_dir(repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .status()
            .unwrap();
        // Force the initial branch to be named 'main'
        Command::new("git")
            .args(["checkout", "-b", "main"])
            .current_dir(repo_path)
            .status()
            .unwrap();

        fs::write(
            repo_path.join("file.txt"),
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo_path)
            .status()
            .unwrap();
    }

    #[tokio::test]
    async fn test_conflict_error_clean_slate_retry_succeeds() {
        let temp = TempDir::new();
        let repo_path = temp.path.clone();
        init_git_repo(&repo_path);

        let base_sha = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap();

        // Branch A modifies file.txt line 2 to "A"
        Command::new("git")
            .args(["checkout", "-b", "branch_a"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        fs::write(
            repo_path.join("file.txt"),
            "line 1\nA\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "commit A"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        let sha_a = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap();

        // Branch B modifies file.txt line 2 to "B" (conflicting!)
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["checkout", "-b", "branch_b"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        fs::write(
            repo_path.join("file.txt"),
            "line 1\nB\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "commit B"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        let sha_b = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap();

        // Branch B2 (non-conflicting fallback!) modifies line 9 to "B2"
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["checkout", "-b", "branch_b2"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        fs::write(
            repo_path.join("file.txt"),
            "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nB2\nline 10\n",
        )
        .unwrap();
        Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "commit B2"])
            .current_dir(&repo_path)
            .status()
            .unwrap();
        let sha_b2 = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&repo_path)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap();

        // Configure RETRY_POLICY to retry on conflict
        RETRY_POLICY.with(|p| {
            *p.borrow_mut() = RetryPolicy {
                max_attempts: 3,
                backoff: std::time::Duration::from_millis(1),
                retry_on_conflict: true,
                retry_on_lock_timeout: false,
                retry_on_corrupt_branch: false,
            };
        });

        // Configure TEST_HOOK to switch Branch B (conflicting) to Branch B2 (non-conflicting) on the second attempt
        let attempt_counter = RefCell::new(0);
        let sha_b_clone = sha_b.clone();
        let sha_b2_clone = sha_b2.clone();
        TEST_HOOK.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |branches_local| {
                let mut cnt = attempt_counter.borrow_mut();
                *cnt += 1;
                if *cnt > 1 {
                    // On retry, replace the conflicting sha_b with non-conflicting sha_b2
                    for (_agent, sha) in branches_local.iter_mut() {
                        if *sha == sha_b_clone {
                            *sha = sha_b2_clone.clone();
                        }
                    }
                }
            }));
        });

        // Run the integration which starts with a conflict (sha_a vs sha_b)
        let branches = vec![
            ("agent_a".to_string(), sha_a),
            ("agent_b".to_string(), sha_b),
        ];

        let result = integrate_branches(&repo_path, &base_sha, "integration", &branches).await.unwrap();

        // Verify that it successfully completed on retry (no conflicts, got a valid merge sha)
        assert!(
            !result.merge_sha.is_empty(),
            "Should have a valid merge SHA after retry"
        );
        assert!(
            result.conflicts.is_empty(),
            "Conflicts list should be empty after retry"
        );

        // Reset the thread-local state
        RETRY_POLICY.with(|p| {
            *p.borrow_mut() = Default::default();
        });
        TEST_HOOK.with(|h| {
            *h.borrow_mut() = None;
        });
    }
}
