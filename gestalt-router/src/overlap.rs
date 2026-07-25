use std::collections::HashSet;
use std::process::Command;
use crate::run::RouterError;

#[derive(Debug, Clone)]
pub struct OverlapInfo {
    pub agent_a: String,
    pub agent_b: String,
    pub files: Vec<String>,
}

/// Helper to get the list of files modified in a branch compared to the base SHA.
pub fn get_changed_files(base_sha: &str, branch_name: &str) -> Result<HashSet<String>, RouterError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", base_sha, branch_name])
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to run git diff: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RouterError::GitError(format!("git diff failed for branch {}: {}", branch_name, stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: HashSet<String> = stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    Ok(files)
}

/// Detects overlapping file paths modified by different agents.
pub fn find_overlaps(
    base_sha: &str,
    agents_branches: &[(String, String)],
) -> Result<Vec<OverlapInfo>, RouterError> {
    let mut agent_changes = Vec::new();
    for (agent_id, branch_name) in agents_branches {
        let files = get_changed_files(base_sha, branch_name)?;
        agent_changes.push((agent_id, files));
    }

    let mut overlaps = Vec::new();
    for i in 0..agent_changes.len() {
        for j in (i + 1)..agent_changes.len() {
            let (agent_a, files_a) = &agent_changes[i];
            let (agent_b, files_b) = &agent_changes[j];

            let intersection: Vec<String> = files_a
                .intersection(files_b)
                .cloned()
                .collect();

            if !intersection.is_empty() {
                overlaps.push(OverlapInfo {
                    agent_a: (*agent_a).clone(),
                    agent_b: (*agent_b).clone(),
                    files: intersection,
                });
            }
        }
    }

    Ok(overlaps)
}
