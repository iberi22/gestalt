use std::path::Path;
use std::process::Command;
use crate::run::RouterError;

#[derive(Debug, Clone)]
pub struct IntegrationResult {
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<String>,
}

/// Integrates agent branches sequentially into the target integration branch.
pub fn integrate_branches(
    integration_wt_path: &Path,
    base_sha: &str,
    integration_branch: &str,
    branches_to_merge: &[(String, String)],
) -> Result<IntegrationResult, RouterError> {
    // 1. Checkout/reset the integration branch to the base SHA
    let checkout_output = Command::new("git")
        .args(["checkout", "-B", integration_branch, base_sha])
        .current_dir(integration_wt_path)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to checkout integration branch: {}", e)))?;

    if !checkout_output.status.success() {
        let stderr = String::from_utf8_lossy(&checkout_output.stderr);
        return Err(RouterError::GitError(format!("git checkout -B failed: {}", stderr)));
    }

    let mut merged_branches = Vec::new();
    let mut conflicts = Vec::new();

    // 2. Sequentially merge each agent's branch
    for (agent_id, branch_name) in branches_to_merge {
        let merge_output = Command::new("git")
            .args(["merge", "--no-ff", "-m", &format!("Merge agent {}", agent_id), branch_name])
            .current_dir(integration_wt_path)
            .output()
            .map_err(|e| RouterError::GitError(format!("Failed to run git merge: {}", e)))?;

        if merge_output.status.success() {
            merged_branches.push(branch_name.clone());
        } else {
            // Retrieve conflicted files
            let diff_output = Command::new("git")
                .args(["diff", "--name-only", "--diff-filter=U"])
                .current_dir(integration_wt_path)
                .output()
                .map_err(|e| RouterError::GitError(format!("Failed to get conflicted files: {}", e)))?;

            let diff_str = String::from_utf8_lossy(&diff_output.stdout);
            let mut found_conflicts = false;
            for line in diff_str.lines() {
                let file = line.trim();
                if !file.is_empty() {
                    conflicts.push(format!("agent {}: {}", agent_id, file));
                    found_conflicts = true;
                }
            }

            if !found_conflicts {
                // Fallback conflict name if porcelain diff is empty but merge still failed
                conflicts.push(format!("agent {}: generic merge conflict", agent_id));
            }

            // Abort the failed merge to clean up the worktree
            let abort_output = Command::new("git")
                .args(["merge", "--abort"])
                .current_dir(integration_wt_path)
                .output()
                .map_err(|e| RouterError::GitError(format!("Failed to abort git merge: {}", e)))?;

            if !abort_output.status.success() {
                let stderr = String::from_utf8_lossy(&abort_output.stderr);
                return Err(RouterError::GitError(format!("git merge --abort failed: {}", stderr)));
            }
        }
    }

    Ok(IntegrationResult {
        merged_branches,
        conflicts,
    })
}
