use crate::run::RouterError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlapResult {
    pub shared_paths: Vec<PathBuf>,
    pub disjoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictKind {
    Content,
    BothModified,
    AddedByUs,
    AddedByThem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub path: PathBuf,
    pub kind: ConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlapInfo {
    pub agent_a: String,
    pub agent_b: String,
    pub files: Vec<PathBuf>,
}

/// Find overlaps between multiple agent branches.
pub fn find_overlaps(
    base_sha: &str,
    active_branches: &[(String, String)],
) -> Result<Vec<OverlapInfo>, RouterError> {
    find_overlaps_in_repo(Path::new("."), base_sha, active_branches)
}

/// Find overlaps between multiple agent branches inside a specific repository.
pub fn find_overlaps_in_repo(
    repo_path: &Path,
    base_sha: &str,
    active_branches: &[(String, String)],
) -> Result<Vec<OverlapInfo>, RouterError> {
    let mut overlaps = Vec::new();
    for i in 0..active_branches.len() {
        for j in (i + 1)..active_branches.len() {
            let (id_a, branch_a) = &active_branches[i];
            let (id_b, branch_b) = &active_branches[j];

            let files_a = get_modified_files(repo_path, base_sha, branch_a)?;
            let files_b = get_modified_files(repo_path, base_sha, branch_b)?;

            let result = detect_overlap(&files_a, &files_b);
            if !result.disjoint {
                overlaps.push(OverlapInfo {
                    agent_a: id_a.clone(),
                    agent_b: id_b.clone(),
                    files: result.shared_paths,
                });
            }
        }
    }
    Ok(overlaps)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeTestResult {
    Clean,
    Conflicts(Vec<ConflictInfo>),
}

/// Run git diff --name-only base_sha..branch to get the list of modified files.
pub fn get_modified_files(
    repo_path: &Path,
    base_sha: &str,
    branch: &str,
) -> Result<Vec<PathBuf>, RouterError> {
    let diff_spec = format!("{}..{}", base_sha, branch);
    let output = Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg(&diff_spec)
        .current_dir(repo_path)
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to execute git diff: {}", e)))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(RouterError::GitError(format!(
            "git diff failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            err_msg
        )));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let files = stdout_str
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();

    Ok(files)
}

/// Detect overlapping paths between two sets of files.
pub fn detect_overlap(files_a: &[PathBuf], files_b: &[PathBuf]) -> OverlapResult {
    let set_a: HashSet<&PathBuf> = files_a.iter().collect();
    let set_b: HashSet<&PathBuf> = files_b.iter().collect();

    let mut shared_paths: Vec<PathBuf> = set_a.intersection(&set_b).map(|&p| p.clone()).collect();

    // Ensure deterministic order for predictable tests
    shared_paths.sort();

    let disjoint = shared_paths.is_empty();

    OverlapResult {
        shared_paths,
        disjoint,
    }
}

/// Parse git version from "git version X.Y.Z"
fn get_git_version() -> Result<(u32, u32), RouterError> {
    let output = Command::new("git")
        .arg("--version")
        .output()
        .map_err(|e| RouterError::GitError(format!("Failed to execute git --version: {}", e)))?;
    let version_str = String::from_utf8_lossy(&output.stdout);
    let trimmed = version_str.trim();
    if let Some(pos) = trimmed.find("git version ") {
        let version_part = &trimmed[pos + "git version ".len()..];
        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() >= 2 {
            let major = parts[0].parse::<u32>().map_err(|_| {
                RouterError::GitError(format!("Invalid git major version: {}", parts[0]))
            })?;
            let minor = parts[1].parse::<u32>().map_err(|_| {
                RouterError::GitError(format!("Invalid git minor version: {}", parts[1]))
            })?;
            return Ok((major, minor));
        }
    }
    Err(RouterError::GitError(format!(
        "Could not parse git version from: {}",
        version_str
    )))
}

/// Map presence of stage 1, 2, 3 to a ConflictKind.
fn map_stages_to_kind(has_base: bool, has_our: bool, has_their: bool) -> ConflictKind {
    if has_base && has_our && has_their {
        ConflictKind::BothModified
    } else if has_our && has_their {
        ConflictKind::Content
    } else if has_our {
        ConflictKind::AddedByUs
    } else if has_their {
        ConflictKind::AddedByThem
    } else {
        ConflictKind::Content
    }
}

/// Helper to parse a line containing a 40-character SHA and extract the path after it.
fn extract_path_after_sha(line: &str) -> Option<PathBuf> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if let Some(sha_idx) = words
        .iter()
        .position(|w| w.len() == 40 && w.chars().all(|c| c.is_ascii_hexdigit()))
    {
        let sha = words[sha_idx];
        if let Some(pos) = line.find(sha) {
            let path_part = &line[pos + 40..];
            let trimmed = path_part.trim();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
    }
    None
}

/// Test mergeability of two branches sharing a base commit.
pub fn test_mergeability(
    repo_path: &Path,
    base_sha: &str,
    branch_a: &str,
    branch_b: &str,
) -> Result<MergeTestResult, RouterError> {
    let (major, minor) = get_git_version().unwrap_or((2, 38)); // Default to >= 2.38 if check fails

    if major > 2 || (major == 2 && minor >= 38) {
        // Git >= 2.38: use merge-tree with --write-tree
        let output = Command::new("git")
            .arg("merge-tree")
            .arg("--write-tree")
            .arg(format!("--merge-base={}", base_sha))
            .arg(branch_a)
            .arg(branch_b)
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                RouterError::GitError(format!("Failed to execute git merge-tree: {}", e))
            })?;

        if output.status.success() {
            return Ok(MergeTestResult::Clean);
        }

        let exit_code = output.status.code().unwrap_or(-1);
        if exit_code != 1 {
            // Some error other than a standard conflict
            let err_msg = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(RouterError::GitError(format!(
                "git merge-tree failed with exit code {}: {}",
                exit_code, err_msg
            )));
        }

        // Parse stdout of merge-tree for conflicts
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let mut path_stages: HashMap<PathBuf, HashSet<u32>> = HashMap::new();
        let mut content_conflict_paths: HashSet<PathBuf> = HashSet::new();

        for line in stdout_str.lines() {
            if let Some(path_str) = line.strip_prefix("CONFLICT (content): Merge conflict in ") {
                content_conflict_paths.insert(PathBuf::from(path_str.trim()));
                continue;
            }

            if let Some(tab_idx) = line.find('\t') {
                let metadata = &line[..tab_idx];
                let path_str = &line[tab_idx + 1..];
                let meta_parts: Vec<&str> = metadata.split_whitespace().collect();
                if meta_parts.len() == 3 {
                    let sha = meta_parts[1];
                    let stage_str = meta_parts[2];
                    if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
                        if let Ok(stage) = stage_str.parse::<u32>() {
                            if (1..=3).contains(&stage) {
                                path_stages
                                    .entry(PathBuf::from(path_str.trim()))
                                    .or_default()
                                    .insert(stage);
                            }
                        }
                    }
                }
            }
        }

        if path_stages.is_empty() && content_conflict_paths.is_empty() {
            // Fallback: if we didn't parse any stages but merge-tree exited 1, see if we have lines in a simple format
            return Ok(MergeTestResult::Clean);
        }

        // Build ConflictInfo list
        let mut conflicts = Vec::new();
        // Keep track of added paths to avoid duplicates
        let mut seen_paths = HashSet::new();

        for (path, stages) in path_stages {
            let has_base = stages.contains(&1);
            let has_our = stages.contains(&2);
            let has_their = stages.contains(&3);

            let kind = if content_conflict_paths.contains(&path) {
                ConflictKind::Content
            } else {
                map_stages_to_kind(has_base, has_our, has_their)
            };

            seen_paths.insert(path.clone());
            conflicts.push(ConflictInfo { path, kind });
        }

        // Any path explicitly mentioned in "CONFLICT (content)" but not captured in stage records
        for path in content_conflict_paths {
            if seen_paths.insert(path.clone()) {
                conflicts.push(ConflictInfo {
                    path,
                    kind: ConflictKind::Content,
                });
            }
        }

        conflicts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(MergeTestResult::Conflicts(conflicts))
    } else {
        // Fallback for Git < 2.38: use old merge-tree (tri-merge)
        // Command: git merge-tree base_sha branch_a branch_b
        let output = Command::new("git")
            .arg("merge-tree")
            .arg(base_sha)
            .arg(branch_a)
            .arg(branch_b)
            .current_dir(repo_path)
            .output()
            .map_err(|e| {
                RouterError::GitError(format!("Failed to execute git merge-tree fallback: {}", e))
            })?;

        // Note: old git merge-tree doesn't fail with exit code 1 on conflict.
        // It outputs clean merges as empty, and conflict blocks in stdout.
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let trimmed_stdout = stdout_str.trim();
        if trimmed_stdout.is_empty() {
            return Ok(MergeTestResult::Clean);
        }

        // Parse conflict blocks
        let mut path_stages: HashMap<PathBuf, HashSet<u32>> = HashMap::new();
        let mut path_block_kinds: HashMap<PathBuf, String> = HashMap::new();
        let mut current_block_kind: Option<String> = None;

        for line in stdout_str.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for conflict headers
            if line.starts_with("changed in both") {
                current_block_kind = Some("changed in both".to_string());
                continue;
            } else if line.starts_with("added in both") {
                current_block_kind = Some("added in both".to_string());
                continue;
            } else if line.starts_with("removed in both") {
                current_block_kind = Some("removed in both".to_string());
                continue;
            } else if !line.starts_with(' ') {
                // Other conflict header types or reset
                current_block_kind = None;
            }

            // Parse stage lines
            if line.starts_with("  ") {
                let stripped = line.trim();
                let words: Vec<&str> = stripped.split_whitespace().collect();
                if !words.is_empty() {
                    let role = words[0]; // base, our, or their
                    let stage_num = match role {
                        "base" => Some(1),
                        "our" => Some(2),
                        "their" => Some(3),
                        _ => None,
                    };

                    if let Some(stg) = stage_num {
                        if let Some(path) = extract_path_after_sha(line) {
                            path_stages.entry(path.clone()).or_default().insert(stg);
                            if let Some(ref blk) = current_block_kind {
                                path_block_kinds.insert(path, blk.clone());
                            }
                        }
                    }
                }
            }
        }

        if path_stages.is_empty() {
            return Ok(MergeTestResult::Clean);
        }

        let mut conflicts = Vec::new();
        for (path, stages) in path_stages {
            let has_base = stages.contains(&1);
            let has_our = stages.contains(&2);
            let has_their = stages.contains(&3);

            let kind = if let Some(blk) = path_block_kinds.get(&path) {
                if blk == "changed in both" {
                    ConflictKind::BothModified
                } else if blk == "added in both" {
                    ConflictKind::Content
                } else {
                    map_stages_to_kind(has_base, has_our, has_their)
                }
            } else {
                map_stages_to_kind(has_base, has_our, has_their)
            };

            conflicts.push(ConflictInfo { path, kind });
        }

        conflicts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(MergeTestResult::Conflicts(conflicts))
    }
}
