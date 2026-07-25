use crate::run::RouterError;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckpointResult {
    pub success: bool,
    pub commit_sha: Option<String>,
    pub symlink_escapes: Vec<SymlinkEscape>,
    pub excluded_files: Vec<ExcludedFile>,
    pub files_committed: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymlinkEscape {
    pub path: String,
    pub target: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExcludedFile {
    pub path: String,
    pub reason: String,
}

/// Lexically cleans/normalizes a path by resolving '.' and '..' components.
pub fn clean_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            Component::Normal(c) => {
                components.push(c);
            }
            Component::Prefix(p) => {
                components.push(p.as_os_str());
            }
            Component::RootDir => {
                components.clear();
                components.push(component.as_os_str());
            }
        }
    }
    components.iter().collect()
}

/// Execute a git command and return stdout as String on success.
pub(crate) fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to execute git command: {}", e)))?;

    if !output.status.success() {
        return Err(RouterError::GitError(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Execute a git commit command with custom error handling.
fn run_git_commit_cmd(repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to execute git commit: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(RouterError::GitError(format!(
            "git commit failed: stdout: {}, stderr: {}",
            stdout,
            stderr
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Checks if a symlink target points to a location outside the worktree.
pub fn is_symlink_escape(worktree_root: &Path, symlink_rel_path: &Path, target: &str) -> bool {
    let worktree_root_abs =
        std::fs::canonicalize(worktree_root).unwrap_or_else(|_| worktree_root.to_path_buf());

    let symlink_abs = worktree_root_abs.join(symlink_rel_path);
    let parent_abs = symlink_abs.parent().unwrap_or(&worktree_root_abs);

    let target_path = Path::new(target);
    let resolved_abs = if target_path.is_absolute() {
        target_path.to_path_buf()
    } else {
        parent_abs.join(target_path)
    };

    let cleaned_resolved = clean_path(&resolved_abs);

    !cleaned_resolved.starts_with(&worktree_root_abs)
}

/// Checkpoint implementation.
/// Processes modified/untracked files, filtering ignored files and symlink escapes,
/// and commits the allowed changes while bypassing any hooks.
pub fn checkpoint(
    worktree_dir: &Path,
    commit_message: &str,
) -> Result<CheckpointResult, RouterError> {
    // 1. Run git status --porcelain --ignored -uall to identify modified and untracked files.
    let stdout = run_git_cmd(worktree_dir, &["status", "--porcelain", "--ignored", "-uall"])
        .map_err(|e| RouterError::GitError(format!("Failed to check git status: {}", e)))?;

    let mut excluded_files = Vec::new();
    let mut files_to_add = Vec::new();

    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let status_code = &line[0..2];
        let raw_path = line[3..].trim();
        // Handle double-quoted paths if git status outputs them.
        let path_str = if raw_path.starts_with('"') && raw_path.ends_with('"') {
            &raw_path[1..raw_path.len() - 1]
        } else {
            raw_path
        };

        // If status code is "!!" (ignored) or if git check-ignore returns 0, it's ignored.
        let mut is_ignored = status_code == "!!";
        if !is_ignored {
            if let Ok(ignore_stdout) = run_git_cmd(worktree_dir, &["check-ignore", "-q", path_str])
            {
                if ignore_stdout.is_empty() {
                    is_ignored = true;
                }
            }
        }

        if is_ignored {
            excluded_files.push(ExcludedFile {
                path: path_str.to_string(),
                reason: "ExcludedFile: matches a .gitignore pattern".to_string(),
            });
        } else {
            files_to_add.push(path_str.to_string());
        }
    }

    // 2. Stage the valid files individually.
    for path_str in &files_to_add {
        run_git_cmd(
            worktree_dir,
            &["-c", "core.hooksPath=/dev/null", "add", path_str],
        )
        .map_err(|e| RouterError::GitError(format!("Failed to stage file: {}", e)))?;
    }

    // 3. Scan staged files for symlink escapes using git ls-files -s.
    let ls_stdout = run_git_cmd(worktree_dir, &["ls-files", "-s"])
        .map_err(|e| RouterError::GitError(format!("Failed to run git ls-files: {}", e)))?;

    let mut symlink_escapes = Vec::new();
    let mut files_committed = Vec::new();

    for line in ls_stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 2 {
            let metadata = parts[0];
            let path_str = parts[1];
            let meta_parts: Vec<&str> = metadata.split_whitespace().collect();
            if !meta_parts.is_empty() && meta_parts[0] == "120000" {
                // Read symlink target
                let abs_symlink_path = worktree_dir.join(path_str);
                let target = if let Ok(target_path) = std::fs::read_link(&abs_symlink_path) {
                    target_path.to_string_lossy().into_owned()
                } else if let Ok(content) = std::fs::read_to_string(&abs_symlink_path) {
                    content.trim().to_string()
                } else {
                    String::new()
                };

                if is_symlink_escape(worktree_dir, Path::new(path_str), &target) {
                    symlink_escapes.push(SymlinkEscape {
                        path: path_str.to_string(),
                        target,
                    });

                    // Unstage the escaped symlink so it is not in the commit.
                    let reset_result = run_git_cmd(
                        worktree_dir,
                        &[
                            "-c",
                            "core.hooksPath=/dev/null",
                            "reset",
                            "HEAD",
                            "--",
                            path_str,
                        ],
                    );

                    if reset_result.is_err() {
                        // If HEAD doesn't exist (empty repo), reset HEAD -- <file> might fail.
                        // Use git rm --cached <file> as a fallback.
                        let _ = run_git_cmd(worktree_dir, &["rm", "--cached", path_str]);
                    }
                } else {
                    files_committed.push(path_str.to_string());
                }
            } else {
                files_committed.push(path_str.to_string());
            }
        }
    }

    // Filter out files_committed that were actually unstaged or not in files_to_add.
    files_committed.retain(|f| files_to_add.contains(f) && !symlink_escapes.iter().any(|se| &se.path == f));

    // 4. Commit changes with hooks bypassed.
    let commit_args = &[
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "-m",
        commit_message,
        "--no-verify",
    ];

    let commit_result = run_git_commit_cmd(worktree_dir, commit_args);

    let commit_sha = match commit_result {
        Ok(_) => match run_git_cmd(worktree_dir, &["rev-parse", "HEAD"]) {
            Ok(sha) => Some(sha.trim().to_string()),
            Err(_) => None,
        },
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("nothing to commit") || err_msg.contains("nothing added to commit")
            {
                None
            } else {
                return Err(e);
            }
        }
    };

    Ok(CheckpointResult {
        success: true,
        commit_sha,
        symlink_escapes,
        excluded_files,
        files_committed,
    })
}

/// Run checkpoint and return a boolean indicating whether changes were committed.
pub fn run_checkpoint(worktree_dir: &Path, agent_id: &str) -> Result<bool, RouterError> {
    let commit_msg = format!("gestalt: checkpoint {}", agent_id);
    match checkpoint(worktree_dir, &commit_msg) {
        Ok(result) => {
            if result.commit_sha.is_some() {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        Err(e) => Err(e),
    }
}
