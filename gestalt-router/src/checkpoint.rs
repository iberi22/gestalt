use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use crate::run::RouterError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub sha: String,
    pub files_changed: Vec<String>,
    pub warnings: Vec<String>,
}

pub struct Checkpointer;

impl Checkpointer {
    pub fn checkpoint(
        repo_path: impl AsRef<Path>,
        agent_id: &str,
        run_id: uuid::Uuid,
    ) -> Result<CheckpointResult, RouterError> {
        let repo_path = repo_path.as_ref();
        let canonical_repo_path = repo_path.canonicalize().map_err(|e| {
            RouterError::GitError(format!("Failed to canonicalize repo_path: {}", e))
        })?;

        // 1. Run git status --porcelain --ignored to list modified, created, deleted, or ignored files
        let status_output = run_git_cmd(&canonical_repo_path, &["status", "--porcelain", "--ignored"])?;

        let mut files_to_stage = Vec::new();
        let mut files_changed = Vec::new();
        let mut warnings = Vec::new();

        for line in status_output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // In porcelain v1, first 2 characters are status codes, then a space, then the path.
            if line.len() < 4 {
                continue;
            }

            let status_code = &line[0..2];
            let raw_path = &line[3..];

            // Handle rename: "R  orig -> new"
            let path_str = if raw_path.contains(" -> ") {
                raw_path.split(" -> ").last().unwrap_or(raw_path)
            } else {
                raw_path
            };

            let path_str = unquote_path(path_str);
            if path_str.is_empty() {
                continue;
            }

            if status_code == "!!" {
                // Ignored file
                warnings.push(format!("ExcludedFile: {}", path_str));
                continue;
            }

            let full_path = canonical_repo_path.join(path_str);

            // Check if it's deleted (status contains 'D' or 'D' in X or Y)
            let is_deleted = status_code.contains('D');

            if is_deleted {
                files_to_stage.push(path_str.to_string());
                files_changed.push(path_str.to_string());
                continue;
            }

            // Check if it is a symlink on the filesystem
            let is_symlink = match std::fs::symlink_metadata(&full_path) {
                Ok(meta) => meta.is_symlink(),
                Err(_) => false,
            };

            if is_symlink {
                // Read symlink target
                match std::fs::read_link(&full_path) {
                    Ok(target) => {
                        let symlink_dir = full_path.parent().unwrap_or(&canonical_repo_path);
                        let absolute_target = if target.is_absolute() {
                            target.clone()
                        } else {
                            symlink_dir.join(&target)
                        };
                        let normalized_target = normalize_path(&absolute_target);

                        // Symlink escape check: target outside worktree
                        if !normalized_target.starts_with(&canonical_repo_path) {
                            warnings.push(format!("SymlinkEscape: {}", path_str));
                            continue;
                        }
                    }
                    Err(e) => {
                        warnings.push(format!("Failed to read symlink {}: {}", path_str, e));
                        continue;
                    }
                }
            }

            files_to_stage.push(path_str.to_string());
            files_changed.push(path_str.to_string());
        }

        // 2. git add INDIVIDUAL of each file (NO git add -A)
        for file in &files_to_stage {
            run_git_cmd(&canonical_repo_path, &["add", file])?;
        }

        // 3. Double-check for symlink escapes in git index via git ls-files -s
        let ls_files_output = run_git_cmd(&canonical_repo_path, &["ls-files", "-s"])?;
        for line in ls_files_output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Format: <mode> <sha> <stage>\t<path>
            if line.starts_with("120000 ") {
                if let Some(tab_idx) = line.find('\t') {
                    let path_str = unquote_path(&line[tab_idx + 1..]);
                    let full_path = canonical_repo_path.join(path_str);
                    if let Ok(target) = std::fs::read_link(&full_path) {
                        let symlink_dir = full_path.parent().unwrap_or(&canonical_repo_path);
                        let absolute_target = if target.is_absolute() {
                            target.clone()
                        } else {
                            symlink_dir.join(&target)
                        };
                        let normalized_target = normalize_path(&absolute_target);
                        if !normalized_target.starts_with(&canonical_repo_path) {
                            // Escape! Unstage it
                            let _ = run_git_cmd(&canonical_repo_path, &["reset", "HEAD", path_str]);
                            // Ensure it's not in files_changed
                            files_changed.retain(|f| f != path_str);
                            if !warnings.iter().any(|w| w.contains("SymlinkEscape") && w.contains(path_str)) {
                                warnings.push(format!("SymlinkEscape: {}", path_str));
                            }
                        }
                    }
                }
            }
        }

        // 4. git commit -m "gestalt: checkpoint {agent} {run_id}" --no-verify -c core.hooksPath=/dev/null
        if files_changed.is_empty() {
            warnings.push("NoChanges".to_string());
            return Ok(CheckpointResult {
                sha: String::new(),
                files_changed: Vec::new(),
                warnings,
            });
        }

        let commit_msg = format!("gestalt: checkpoint {} {}", agent_id, run_id);

        let commit_res = run_git_commit_cmd(
            &canonical_repo_path,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                &commit_msg,
                "--no-verify",
            ]
        );

        match commit_res {
            Ok(_) => {
                let sha_output = run_git_cmd(&canonical_repo_path, &["rev-parse", "HEAD"])?;
                let sha = sha_output.trim().to_string();
                Ok(CheckpointResult {
                    sha,
                    files_changed,
                    warnings,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("nothing to commit") || err_msg.contains("no changes added to commit") || err_msg.contains("clean") {
                    warnings.push("NoChanges".to_string());
                    Ok(CheckpointResult {
                        sha: String::new(),
                        files_changed: Vec::new(),
                        warnings,
                    })
                } else {
                    Err(e)
                }
            }
        }
    }
}

fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
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

fn run_git_commit_cmd(repo_path: &Path, args: &[&str]) -> Result<String, RouterError> {
    let output = std::process::Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to execute git commit: {}", e)))?;

    if !output.status.success() {
        return Err(RouterError::GitError(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn unquote_path(path_str: &str) -> &str {
    let mut s = path_str.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    }
    s
}

fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::Normal(c) => {
                components.push(c);
            }
            Component::CurDir => {}
            Component::RootDir => {
                components.clear();
                components.push(component.as_os_str());
            }
            Component::Prefix(p) => {
                components.clear();
                components.push(p.as_os_str());
            }
        }
    }
    components.iter().collect()
}
