use crate::checkpoint::run_git_cmd;
use crate::run::RouterError;
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

/// Integration implementation.
/// Integrates changes from multiple agent branches sequentially using in-memory git merge-tree.
/// Specifically detects if two or more agents have modified the same binary file, in which case
/// it aborts, preserves both branches, and reports a HardConflict.
pub fn integrate(
    repo_dir: &Path,
    base_sha: &str,
    branches: &[(String, String)], // Vec of (agent_id, branch_or_sha)
) -> Result<MergeResult, RouterError> {
    // 1. Detect binary files modified by each agent and check for binary conflicts.
    // binary_mods tracks which agents modified which binary files: HashMap<binary_path, Vec<agent_id>>
    let mut binary_mods: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for (agent_id, branch_or_sha) in branches {
        // Run git diff --numstat base_sha..branch_or_sha
        let args = ["diff", "--numstat", base_sha, branch_or_sha];
        let (status, stdout, _stderr) = run_git_cmd(repo_dir, &args).map_err(|e| {
            RouterError::GitError(format!("Failed to run git diff --numstat: {}", e))
        })?;

        if status != 0 {
            return Err(RouterError::GitError(format!(
                "git diff --numstat returned error code {}: {}",
                status, _stderr
            )));
        }

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 && parts[0] == "-" && parts[1] == "-" {
                let path = parts[2].to_string();
                binary_mods.entry(path).or_default().push(agent_id.clone());
            }
        }
    }

    // Check if any binary file was modified by more than one agent.
    let mut conflicted_binaries = Vec::new();
    for (path, agents) in &binary_mods {
        if agents.len() > 1 {
            conflicted_binaries.push(path.clone());
        }
    }

    if !conflicted_binaries.is_empty() {
        conflicted_binaries.sort();
        let branch_names: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();
        return Ok(MergeResult::HardConflict {
            conflicted_files: conflicted_binaries,
            branches_preserved: branch_names,
        });
    }

    // 2. Perform sequential merge using git merge-tree
    let mut current_tree = base_sha.to_string();
    let mut conflicted_files = Vec::new();

    for (_agent_id, branch_or_sha) in branches {
        let args = ["merge-tree", "--write-tree", &current_tree, branch_or_sha];
        let (status, stdout, _stderr) = run_git_cmd(repo_dir, &args)
            .map_err(|e| RouterError::GitError(format!("Failed to run git merge-tree: {}", e)))?;

        if status != 0 {
            // There is a merge conflict! Parse conflicted files from stdout.
            let mut files = Vec::new();
            for line in stdout.lines() {
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
        } else {
            current_tree = stdout.trim().to_string();
        }
    }

    if !conflicted_files.is_empty() {
        conflicted_files.sort();
        conflicted_files.dedup();
        let branch_names: Vec<String> = branches.iter().map(|(_, b)| b.clone()).collect();
        return Ok(MergeResult::HardConflict {
            conflicted_files,
            branches_preserved: branch_names,
        });
    }

    // 3. Create integration commit using git commit-tree
    // We bypass hooks by running git with "-c core.hooksPath=/dev/null".
    let mut commit_args = vec![
        "-c",
        "core.hooksPath=/dev/null",
        "commit-tree",
        &current_tree,
        "-p",
        base_sha,
    ];

    for (_, branch_or_sha) in branches {
        commit_args.push("-p");
        commit_args.push(branch_or_sha);
    }

    commit_args.push("-m");
    commit_args.push("gestalt integration merge [bypassed hooks]");

    let (status, stdout, stderr) = run_git_cmd(repo_dir, &commit_args)
        .map_err(|e| RouterError::GitError(format!("Failed to run git commit-tree: {}", e)))?;

    if status != 0 {
        return Err(RouterError::GitError(format!(
            "git commit-tree failed with code {}: {}",
            status, stderr
        )));
    }

    let merged_commit_sha = stdout.trim().to_string();

    Ok(MergeResult::Success { merged_commit_sha })
}
