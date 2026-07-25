use crate::run::RouterError;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

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

/// Helper function to execute git commands within a given directory.
pub fn run_git_cmd(dir: &Path, args: &[&str]) -> Result<(i32, String, String), String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute git command: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let status = output.status.code().unwrap_or(-1);

    Ok((status, stdout, stderr))
}

/// Lexically cleans/normalizes a path by resolving '.' and '..' components.
pub fn clean_path(path: &Path) -> PathBuf {
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
            Component::CurDir => {}
            Component::Normal(c) => {
                components.push(c);
            }
            Component::Prefix(p) => {
                components.push(p.as_os_str());
            }
            Component::RootDir => {
                components.push(component.as_os_str());
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
    // 1. Run git status --porcelain --ignored to identify modified and untracked files.
    let (status, stdout, stderr) =
        run_git_cmd(worktree_dir, &["status", "--porcelain", "--ignored"])
            .map_err(|e| RouterError::GitError(format!("Failed to check git status: {}", e)))?;

    if status != 0 {
        return Err(RouterError::GitError(format!(
            "git status returned {}: {}",
            status, stderr
        )));
    }

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
            if let Ok((ignore_status, _, _)) =
                run_git_cmd(worktree_dir, &["check-ignore", "-q", path_str])
            {
                if ignore_status == 0 {
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
        // Run git add with core.hooksPath=/dev/null (not strictly needed for add, but safe)
        let (add_status, _, add_stderr) = run_git_cmd(
            worktree_dir,
            &["-c", "core.hooksPath=/dev/null", "add", path_str],
        )
        .map_err(|e| RouterError::GitError(format!("Failed to stage file: {}", e)))?;

        if add_status != 0 {
            return Err(RouterError::GitError(format!(
                "Failed to stage file {}: {}",
                path_str, add_stderr
            )));
        }
    }

    // 3. Scan staged files for symlink escapes using git ls-files -s.
    let (ls_status, ls_stdout, ls_stderr) = run_git_cmd(worktree_dir, &["ls-files", "-s"])
        .map_err(|e| RouterError::GitError(format!("Failed to run git ls-files: {}", e)))?;

    if ls_status != 0 {
        return Err(RouterError::GitError(format!(
            "git ls-files failed with code {}: {}",
            ls_status, ls_stderr
        )));
    }

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
                    let (reset_status, _, _reset_stderr) = run_git_cmd(
                        worktree_dir,
                        &[
                            "-c",
                            "core.hooksPath=/dev/null",
                            "reset",
                            "HEAD",
                            "--",
                            path_str,
                        ],
                    )
                    .map_err(|e| {
                        RouterError::GitError(format!("Failed to unstage escaped symlink: {}", e))
                    })?;

                    if reset_status != 0 {
                        // If HEAD doesn't exist (empty repo), reset HEAD -- <file> might fail.
                        // We can use git rm --cached <file> as a fallback.
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

    // Filter out files_committed that were actually unstaged.
    files_committed.retain(|f| !symlink_escapes.iter().any(|se| &se.path == f));

    // 4. Commit changes with hooks bypassed.
    let commit_args = &[
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "-m",
        commit_message,
        "--no-verify",
    ];

    let (commit_status, _, commit_stderr) = run_git_cmd(worktree_dir, commit_args)
        .map_err(|e| RouterError::GitError(format!("Failed to commit: {}", e)))?;

    let commit_sha = if commit_status == 0 {
        // Get the commit SHA
        let (rev_status, rev_stdout, _) = run_git_cmd(worktree_dir, &["rev-parse", "HEAD"])
            .map_err(|e| RouterError::GitError(format!("Failed to resolve commit SHA: {}", e)))?;
        if rev_status == 0 {
            Some(rev_stdout.trim().to_string())
        } else {
            None
        }
    } else {
        // If the commit failed because there is nothing to commit, that's fine.
        // Check if working tree is clean.
        if commit_stderr.contains("nothing to commit")
            || commit_stderr.contains("nothing added to commit")
        {
            None
        } else {
            return Err(RouterError::GitError(format!(
                "git commit failed with code {}: {}",
                commit_status, commit_stderr
            )));
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

