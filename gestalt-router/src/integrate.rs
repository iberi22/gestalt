use crate::checkpoint;
use crate::run::ConflictInfo;
use crate::run::{self, RouterError};
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

/// Integration implementation.
pub fn integrate_branches(
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
                    agent_id: binary_mods.get(f).and_then(|v| v.first().cloned()).unwrap_or_default(),
                    path: f.clone(),
                })
                .collect(),
        });
    }

    // 2. Perform sequential merge using git merge-tree
    let mut current_tree = base_sha.to_string();
    let mut conflicted_files = Vec::new();

    for (_agent_id, branch_or_sha) in branches {
        let args = ["merge-tree", "--write-tree", "--merge-base", base_sha, &current_tree, branch_or_sha];
        let result = checkpoint::run_git_cmd(repo_dir, &args);

        match result {
            Ok(stdout) => {
                current_tree = stdout.trim().to_string();
            }
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
            }
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

    // 3. Create integration commit using git commit-tree
    let commit_args = vec![
        "-c",
        "core.hooksPath=/dev/null",
        "commit-tree",
        &current_tree,
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
                    &current_tree,
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
        }
    };

    let merged_branches: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();

    Ok(IntegrateResult {
        merge_sha: final_sha,
        merged_branches,
        conflicts: vec![],
    })
}
