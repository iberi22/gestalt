use crate::run::RouterError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConflictInfo {
    pub agent_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrateResult {
    pub merge_sha: String,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
}

#[derive(Debug, Clone)]
pub struct AgentIntegrationSpec {
    pub id: String,
    pub branch: String,
}

async fn run_git_cmd(
    args: &[&str],
    current_dir: Option<&std::path::Path>,
) -> Result<(std::process::ExitStatus, String, String), RouterError> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args);
    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }
    let output = cmd.output().await.map_err(|e| {
        RouterError::GitError(format!("Failed to execute git command {:?}: {}", args, e))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    Ok((output.status, stdout, stderr))
}

pub async fn git_version_supports_merge_tree(repo_path: Option<&std::path::Path>) -> bool {
    let (_status, stdout, _stderr) = match run_git_cmd(&["--version"], repo_path).await {
        Ok(res) => res,
        Err(_) => return false,
    };
    let words: Vec<&str> = stdout.split_whitespace().collect();
    if let Some(pos) = words.iter().position(|&w| w == "version") {
        if pos + 1 < words.len() {
            let ver_str = words[pos + 1];
            let parts: Vec<&str> = ver_str.split('.').collect();
            if !parts.is_empty() {
                if let Ok(major) = parts[0].parse::<u32>() {
                    if major > 2 {
                        return true;
                    } else if major == 2 {
                        if parts.len() > 1 {
                            if let Ok(minor) = parts[1].parse::<u32>() {
                                return minor >= 38;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

fn parse_conflicted_paths(output: &str) -> Vec<String> {
    let mut paths = std::collections::BTreeSet::new();
    for line in output.lines() {
        if line.starts_with("CONFLICT") {
            if let Some(pos) = line.find("conflict in ") {
                let path = line[pos + "conflict in ".len()..].trim().to_string();
                if !path.is_empty() {
                    paths.insert(path);
                }
            }
        } else if let Some(tab_index) = line.find('\t') {
            let left = &line[..tab_index];
            let right = &line[tab_index + 1..];
            let parts: Vec<&str> = left.split_whitespace().collect();
            if parts.len() == 3 && (parts[2] == "1" || parts[2] == "2" || parts[2] == "3") {
                paths.insert(right.trim().to_string());
            }
        }
    }
    paths.into_iter().collect()
}

async fn integrate_fallback(
    base_sha: &str,
    agent_changes: &[(AgentIntegrationSpec, usize)],
    run_id: uuid::Uuid,
    repo_path: Option<&std::path::Path>,
) -> Result<IntegrateResult, RouterError> {
    let temp_dir = std::env::temp_dir().join(format!("gestalt_integrate_{}", uuid::Uuid::new_v4()));
    let temp_path_str = temp_dir.to_string_lossy().to_string();

    let (_status, _stdout, stderr) = run_git_cmd(
        &["worktree", "add", "--detach", &temp_path_str, base_sha],
        repo_path,
    ).await?;
    if !_status.success() {
        return Err(RouterError::GitError(format!(
            "Fallback failed to add worktree: {}",
            stderr
        )));
    }

    let mut merged_branches = Vec::new();
    let mut conflicts = Vec::new();

    let cleanup = || async {
        let _ = run_git_cmd(&["worktree", "remove", "--force", &temp_path_str], repo_path).await;
    };

    for (agent, _count) in agent_changes {
        let merge_msg = format!("Merge branch {} into integration", agent.branch);
        let (status, _stdout, _stderr) = run_git_cmd(
            &["merge", "--no-ff", &agent.branch, "-m", &merge_msg],
            Some(&temp_dir),
        ).await?;

        if status.success() {
            merged_branches.push(agent.branch.clone());
        } else {
            let (_diff_status, diff_stdout, _diff_stderr) = run_git_cmd(
                &["diff", "--name-only", "--diff-filter=U"],
                Some(&temp_dir),
            ).await?;

            for line in diff_stdout.lines() {
                let path = line.trim().to_string();
                if !path.is_empty() {
                    conflicts.push(ConflictInfo {
                        agent_id: agent.id.clone(),
                        path,
                    });
                }
            }

            if conflicts.iter().all(|c| c.agent_id != agent.id) {
                conflicts.push(ConflictInfo {
                    agent_id: agent.id.clone(),
                    path: "unknown_conflict_path".to_string(),
                });
            }

            let _ = run_git_cmd(&["merge", "--abort"], Some(&temp_dir)).await;
        }
    }

    let (_status, stdout, stderr) = run_git_cmd(&["rev-parse", "HEAD^{tree}"], Some(&temp_dir)).await?;
    if !_status.success() {
        cleanup().await;
        return Err(RouterError::GitError(format!(
            "Fallback failed to get final tree SHA: {}",
            stderr
        )));
    }
    let final_tree = stdout.trim().to_string();

    let commit_msg = format!(
        "gestalt: integrate run {} ({} agents)",
        run_id,
        merged_branches.len()
    );

    let mut commit_args = vec!["commit-tree", &final_tree];
    commit_args.push("-p");
    commit_args.push(base_sha);
    for branch in &merged_branches {
        commit_args.push("-p");
        commit_args.push(branch);
    }
    commit_args.push("-m");
    commit_args.push(&commit_msg);

    let (_status, stdout, stderr) = run_git_cmd(&commit_args, repo_path).await?;
    if !_status.success() {
        cleanup().await;
        return Err(RouterError::GitError(format!(
            "Fallback failed to create integration commit: {}",
            stderr
        )));
    }
    let final_sha = stdout.trim().to_string();

    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let (_status, _stdout, stderr) = run_git_cmd(&["update-ref", &ref_name, &final_sha], repo_path).await?;
    if !_status.success() {
        cleanup().await;
        return Err(RouterError::GitError(format!(
            "Fallback failed to update ref {}: {}",
            ref_name, stderr
        )));
    }

    cleanup().await;

    Ok(IntegrateResult {
        merge_sha: final_sha,
        merged_branches,
        conflicts,
    })
}

pub async fn integrate(
    base_sha: &str,
    agents: &[AgentIntegrationSpec],
    run_id: uuid::Uuid,
    force_fallback: bool,
    repo_path: Option<&std::path::Path>,
) -> Result<IntegrateResult, RouterError> {
    let mut agent_changes = Vec::new();
    for agent in agents {
        let (_status, stdout, stderr) = run_git_cmd(
            &["diff", "--name-only", base_sha, &agent.branch],
            repo_path,
        ).await?;

        if !_status.success() {
            return Err(RouterError::GitError(format!(
                "Failed to diff branch {} with {}: {}",
                agent.branch, base_sha, stderr
            )));
        }

        let count = stdout.lines().filter(|line| !line.trim().is_empty()).count();
        agent_changes.push((agent.clone(), count));
    }

    agent_changes.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id))
    });

    if force_fallback || !git_version_supports_merge_tree(repo_path).await {
        return integrate_fallback(base_sha, &agent_changes, run_id, repo_path).await;
    }

    let (_status, stdout, stderr) = run_git_cmd(
        &["rev-parse", &format!("{}^{{tree}}", base_sha)],
        repo_path,
    ).await?;
    if !_status.success() {
        return Err(RouterError::GitError(format!(
            "Failed to resolve base tree for {}: {}",
            base_sha, stderr
        )));
    }
    let mut current_tree = stdout.trim().to_string();

    let mut merged_branches = Vec::new();
    let mut conflicts = Vec::new();

    for (agent, _count) in &agent_changes {
        let (status, stdout, stderr) = run_git_cmd(
            &[
                "merge-tree",
                "--write-tree",
                &format!("--merge-base={}", base_sha),
                &current_tree,
                &agent.branch,
            ],
            repo_path,
        ).await?;

        let mut lines = stdout.lines();
        let merged_tree = lines.next().map(|s| s.trim().to_string()).unwrap_or_default();

        if status.success() && !merged_tree.is_empty() {
            current_tree = merged_tree;
            merged_branches.push(agent.branch.clone());
        } else {
            let full_output = format!("{}\n{}", stdout, stderr);
            let conflicted_paths = parse_conflicted_paths(&full_output);

            for path in conflicted_paths {
                conflicts.push(ConflictInfo {
                    agent_id: agent.id.clone(),
                    path,
                });
            }

            if conflicts.iter().all(|c| c.agent_id != agent.id) {
                conflicts.push(ConflictInfo {
                    agent_id: agent.id.clone(),
                    path: "unknown_conflict_path".to_string(),
                });
            }
        }
    }

    let commit_msg = format!(
        "gestalt: integrate run {} ({} agents)",
        run_id,
        merged_branches.len()
    );

    let mut commit_args = vec!["commit-tree", &current_tree];
    commit_args.push("-p");
    commit_args.push(base_sha);

    for branch in &merged_branches {
        commit_args.push("-p");
        commit_args.push(branch);
    }

    commit_args.push("-m");
    commit_args.push(&commit_msg);

    let (_status, stdout, stderr) = run_git_cmd(&commit_args, repo_path).await?;
    if !_status.success() {
        return Err(RouterError::GitError(format!(
            "Failed to create integration commit: {}",
            stderr
        )));
    }
    let final_sha = stdout.trim().to_string();

    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let (_status, _stdout, stderr) = run_git_cmd(&["update-ref", &ref_name, &final_sha], repo_path).await?;
    if !_status.success() {
        return Err(RouterError::GitError(format!(
            "Failed to update ref {}: {}",
            ref_name, stderr
        )));
    }

    Ok(IntegrateResult {
        merge_sha: final_sha,
        merged_branches,
        conflicts,
    })
}
