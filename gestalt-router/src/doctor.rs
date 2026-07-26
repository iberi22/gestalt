use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

fn get_runs_dir() -> PathBuf {
    if let Some(gestalt_home) = std::env::var_os("GESTALT_HOME") {
        PathBuf::from(gestalt_home).join("runs")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".gestalt").join("runs")
    } else {
        PathBuf::from(".gestalt").join("runs")
    }
}

#[derive(Debug, Error)]
pub enum DoctorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, DoctorError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanedRun {
    pub run_id: String,
    pub worktrees: Vec<PathBuf>,
    pub branches: Vec<String>,
    pub manifest_exists: bool,
    pub status: String, // "Active" or "Orphaned"
}

// Struct to parse ~/.gestalt/runs/{run_id}/manifest.json if it exists.
// Based on "Run manifest: ~/.gestalt/runs/{run_id}/manifest.json con run_id, sha_base, worktrees, branches, created_at"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestJson {
    pub run_id: String,
    pub sha_base: Option<String>,
    pub worktrees: Option<Vec<String>>,
    pub branches: Option<Vec<String>>,
    pub created_at: Option<String>,
}

pub struct Doctor {
    pub force: bool,
    pub push: bool,
}

impl Doctor {
    pub fn new(force: bool, push: bool) -> Self {
        Self { force, push }
    }

    /// List all runs (both active and orphaned).
    pub fn list_orphaned(&self, log: &dyn Fn(&str), repo_path: &Path) -> Vec<OrphanedRun> {
        let runs_dir = get_runs_dir();

        let runs_dir_canonical = runs_dir.canonicalize().unwrap_or_else(|_| runs_dir.clone());
        let _repo_path_canonical = repo_path
            .canonicalize()
            .unwrap_or_else(|_| repo_path.to_path_buf());

        log(&format!(
            "Scanning for runs. runs_dir: {}, repo_path: {}",
            runs_dir.display(),
            repo_path.display()
        ));

        // 1. Parse git worktree list --porcelain
        let mut physical_worktrees_by_run: HashMap<String, Vec<PathBuf>> = HashMap::new();
        let mut physical_branches_by_run: HashMap<String, Vec<String>> = HashMap::new();

        match std::process::Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(repo_path)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut current_worktree = None;
                for line in stdout.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        current_worktree = None;
                    } else if let Some(path_str) = line.strip_prefix("worktree ") {
                        let path = PathBuf::from(path_str.trim());
                        current_worktree = Some(path);
                    } else if let Some(branch_ref) = line.strip_prefix("branch ") {
                        let branch_ref = branch_ref.trim();
                        let branch_name =
                            if let Some(short) = branch_ref.strip_prefix("refs/heads/") {
                                short.to_string()
                            } else {
                                branch_ref.to_string()
                            };

                        if let Some(ref path) = current_worktree {
                            // Try to extract run ID from path or branch name
                            let mut extracted_run_id =
                                extract_run_id_from_path(path, &runs_dir_canonical);
                            if extracted_run_id.is_none() {
                                extracted_run_id = extract_run_id_from_branch(&branch_name);
                            }

                            if let Some(run_id) = extracted_run_id {
                                physical_worktrees_by_run
                                    .entry(run_id.clone())
                                    .or_default()
                                    .push(path.clone());
                                physical_branches_by_run
                                    .entry(run_id)
                                    .or_default()
                                    .push(branch_name);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log(&format!("Failed to run git worktree list: {:?}", e));
            }
        }

        // 2. Discover run IDs from ~/.gestalt/runs/ subdirectories
        let mut manifest_runs: HashMap<String, ManifestJson> = HashMap::new();
        if runs_dir.exists() {
            if let Ok(entries) = fs::read_dir(&runs_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let dir_name = entry.file_name().to_string_lossy().to_string();
                            // If this directory name is a potential run ID, look for manifest.json
                            let manifest_file = entry.path().join("manifest.json");
                            if manifest_file.exists() {
                                if let Ok(content) = fs::read_to_string(&manifest_file) {
                                    if let Ok(manifest) =
                                        serde_json::from_str::<ManifestJson>(&content)
                                    {
                                        manifest_runs.insert(manifest.run_id.clone(), manifest);
                                    } else {
                                        // Malformed JSON: insert a placeholder manifest so we know it exists
                                        manifest_runs.insert(
                                            dir_name.clone(),
                                            ManifestJson {
                                                run_id: dir_name.clone(),
                                                sha_base: None,
                                                worktrees: None,
                                                branches: None,
                                                created_at: None,
                                            },
                                        );
                                    }
                                }
                            } else {
                                // Directory exists but no manifest.json file inside it
                                // This is a manifest/dir candidate without manifest.json
                                manifest_runs.insert(
                                    dir_name.clone(),
                                    ManifestJson {
                                        run_id: dir_name.clone(),
                                        sha_base: None,
                                        worktrees: None,
                                        branches: None,
                                        created_at: None,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        // 3. Query all local/remote branches starting with "gestalt/"
        let mut branches_by_run: HashMap<String, Vec<String>> = HashMap::new();
        match std::process::Command::new("git")
            .args([
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/",
                "refs/remotes/",
            ])
            .current_dir(repo_path)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let refname = line.trim();
                    let branch_name = if let Some(local) = refname.strip_prefix("refs/heads/") {
                        Some(local.to_string())
                    } else if let Some(remote) = refname.strip_prefix("refs/remotes/") {
                        // Strip the remote prefix (e.g. origin/)
                        if let Some(slash_idx) = remote.find('/') {
                            Some(remote[slash_idx + 1..].to_string())
                        } else {
                            Some(remote.to_string())
                        }
                    } else {
                        None
                    };

                    if let Some(ref name) = branch_name {
                        if let Some(run_id) = extract_run_id_from_branch(name) {
                            branches_by_run
                                .entry(run_id)
                                .or_default()
                                .push(name.clone());
                        }
                    }
                }
            }
            Err(e) => {
                log(&format!("Failed to run git for-each-ref: {:?}", e));
            }
        }

        // Deduplicate and gather all unique run IDs
        let mut all_run_ids: HashSet<String> = HashSet::new();
        for r_id in physical_worktrees_by_run.keys() {
            all_run_ids.insert(r_id.clone());
        }
        for r_id in manifest_runs.keys() {
            all_run_ids.insert(r_id.clone());
        }
        for r_id in branches_by_run.keys() {
            all_run_ids.insert(r_id.clone());
        }

        let mut orphaned_runs = Vec::new();

        for run_id in all_run_ids {
            // Determine if manifest file actually exists
            let manifest_file_path = runs_dir.join(&run_id).join("manifest.json");
            let manifest_exists = manifest_file_path.exists();

            // Gather all worktree paths from manifest and physical list
            let mut worktrees_set: HashSet<PathBuf> = HashSet::new();
            if let Some(phys_wts) = physical_worktrees_by_run.get(&run_id) {
                for wt in phys_wts {
                    worktrees_set.insert(wt.clone());
                }
            }
            if let Some(m_run) = manifest_runs.get(&run_id) {
                if let Some(ref m_wts) = m_run.worktrees {
                    for wt_str in m_wts {
                        worktrees_set.insert(PathBuf::from(wt_str));
                    }
                }
            }

            // Gather all branch names from manifest, physical list, and git refs
            let mut branches_set: HashSet<String> = HashSet::new();
            if let Some(phys_brs) = physical_branches_by_run.get(&run_id) {
                for br in phys_brs {
                    branches_set.insert(br.clone());
                }
            }
            if let Some(ref_brs) = branches_by_run.get(&run_id) {
                for br in ref_brs {
                    branches_set.insert(br.clone());
                }
            }
            if let Some(m_run) = manifest_runs.get(&run_id) {
                if let Some(ref m_brs) = m_run.branches {
                    for br in m_brs {
                        branches_set.insert(br.clone());
                    }
                }
            }

            // Convert to sorted Vecs for predictability
            let mut worktrees: Vec<PathBuf> = worktrees_set.into_iter().collect();
            worktrees.sort();
            let mut branches: Vec<String> = branches_set.into_iter().collect();
            branches.sort();

            // Check if there is any physical worktree
            let physical_worktree_exists = worktrees.iter().any(|wt| wt.exists());

            // A run is Active if the manifest exists AND at least one physical worktree exists
            // A run is Orphaned if:
            // - worktree exists but NO manifest (worktree huérfano)
            // - manifest exists but NO physical worktree (manifest huérfano)
            // - neither exists but branches exist
            let status = if manifest_exists && physical_worktree_exists {
                "Active".to_string()
            } else {
                "Orphaned".to_string()
            };

            orphaned_runs.push(OrphanedRun {
                run_id,
                worktrees,
                branches,
                manifest_exists,
                status,
            });
        }

        orphaned_runs.sort_by(|a, b| a.run_id.cmp(&b.run_id));
        orphaned_runs
    }

    /// List only orphaned runs.
    pub fn find_orphaned_runs(&self, log: &dyn Fn(&str), repo_path: &Path) -> Vec<OrphanedRun> {
        self.list_orphaned(log, repo_path)
            .into_iter()
            .filter(|r| r.status == "Orphaned")
            .collect()
    }

    /// Prune a specific run resources.
    pub fn prune_run(&self, run_id: &str, log: &dyn Fn(&str), repo_path: &Path) -> Result<()> {
        let runs_dir = get_runs_dir();
        let runs_dir_canonical = runs_dir.canonicalize().unwrap_or_else(|_| runs_dir.clone());
        let repo_path_canonical = repo_path
            .canonicalize()
            .unwrap_or_else(|_| repo_path.to_path_buf());

        log(&format!("Pruning run_id: {}", run_id));

        // Gather resources associated with run_id
        let runs = self.list_orphaned(log, repo_path);
        let run_info = runs.iter().find(|r| r.run_id == run_id);

        if let Some(info) = run_info {
            // Anti-Hallucination Guard: Active runs cannot be deleted without --force
            if info.status == "Active" && !self.force {
                log(&format!(
                    "Skipping active run {} (requires --force)",
                    run_id
                ));
                return Err(DoctorError::Other(format!(
                    "Cannot prune active run {} without force flag",
                    run_id
                )));
            }

            // Prune worktrees
            for wt in &info.worktrees {
                if is_safe_to_delete_worktree(wt, &runs_dir_canonical, &repo_path_canonical) {
                    if wt.exists() {
                        let wt_str = wt.to_string_lossy();
                        log(&format!("Removing worktree: {}", wt_str));

                        let res = std::process::Command::new("git")
                            .args(["worktree", "remove", &wt_str])
                            .current_dir(repo_path)
                            .output();

                        let success = match res {
                            Ok(out) => out.status.success(),
                            Err(_) => false,
                        };

                        if !success {
                            log(&format!(
                                "git worktree remove failed for {}, trying --force",
                                wt_str
                            ));
                            let _ = std::process::Command::new("git")
                                .args(["worktree", "remove", "--force", &wt_str])
                                .current_dir(repo_path)
                                .status();
                        }
                    } else {
                        log(&format!(
                            "Worktree path {} does not exist physically, skipping git removal",
                            wt.display()
                        ));
                    }
                } else {
                    log(&format!(
                        "DANGEROUS: Worktree path {} is NOT safe to delete! Skipping.",
                        wt.display()
                    ));
                }
            }

            // Prune branches
            for branch in &info.branches {
                log(&format!("Deleting local branch: {}", branch));
                let res = std::process::Command::new("git")
                    .args(["branch", "-D", branch])
                    .current_dir(repo_path)
                    .status();

                match res {
                    Ok(status) if status.success() => {
                        log(&format!("Deleted local branch {}", branch));
                    }
                    _ => {
                        log(&format!(
                            "Failed to delete local branch {} (might not exist)",
                            branch
                        ));
                    }
                }

                if self.push {
                    log(&format!("Deleting remote branch: {} on origin", branch));
                    let res = std::process::Command::new("git")
                        .args(["push", "origin", "--delete", branch])
                        .current_dir(repo_path)
                        .status();

                    match res {
                        Ok(status) if status.success() => {
                            log(&format!("Deleted remote branch {} on origin", branch));
                        }
                        _ => {
                            log(&format!("Remote branch {} does not exist on origin or push failed (skipped)", branch));
                        }
                    }
                }
            }
        } else {
            log(&format!("No registry found in list_orphaned for run_id {}, cleaning up directories directly", run_id));
        }

        // Archive events
        let run_dir = runs_dir.join(run_id);
        if run_dir.exists() {
            let archive_dir = match dirs::home_dir() {
                Some(home) => home
                    .join(".gestalt")
                    .join("archive")
                    .join("runs")
                    .join(run_id),
                None => PathBuf::from(".gestalt")
                    .join("archive")
                    .join("runs")
                    .join(run_id),
            };

            // Look for events file (e.g., events.jsonl)
            let events_file = run_dir.join("events.jsonl");
            let events_dir = run_dir.join("events");

            let mut archived = false;

            if events_file.exists() {
                log(&format!("Archiving events file: {}", events_file.display()));
                if let Err(e) = fs::create_dir_all(&archive_dir) {
                    log(&format!("Failed to create archive directory: {:?}", e));
                } else {
                    let dest = archive_dir.join("events.jsonl");
                    if let Err(e) = fs::copy(&events_file, &dest) {
                        log(&format!("Failed to copy events file: {:?}", e));
                    } else {
                        archived = true;
                    }
                }
            }

            if events_dir.exists() {
                log(&format!(
                    "Archiving events directory: {}",
                    events_dir.display()
                ));
                if let Err(e) = fs::create_dir_all(&archive_dir) {
                    log(&format!("Failed to create archive directory: {:?}", e));
                } else {
                    let dest = archive_dir.join("events");
                    if let Err(e) = copy_dir_all(&events_dir, &dest) {
                        log(&format!("Failed to copy events directory: {:?}", e));
                    } else {
                        archived = true;
                    }
                }
            }

            if archived {
                log(&format!(
                    "Events successfully archived to {}",
                    archive_dir.display()
                ));
            } else {
                log("No events found to archive.");
            }

            // Cleanup the run folder completely
            log(&format!("Removing run directory: {}", run_dir.display()));
            if let Err(e) = fs::remove_dir_all(&run_dir) {
                log(&format!("Failed to remove run directory: {:?}", e));
            }
        }

        Ok(())
    }

    /// Prune all orphaned runs.
    pub fn prune_all(&self, log: &dyn Fn(&str), repo_path: &Path) -> Result<Vec<String>> {
        let runs = self.list_orphaned(log, repo_path);
        let mut pruned_run_ids = Vec::new();

        for run in runs {
            // Prune if status is "Orphaned", or if force is true (which allows pruning active runs as well)
            if run.status == "Orphaned" || self.force {
                match self.prune_run(&run.run_id, log, repo_path) {
                    Ok(_) => {
                        pruned_run_ids.push(run.run_id.clone());
                    }
                    Err(e) => {
                        log(&format!("Error pruning run {}: {:?}", run.run_id, e));
                    }
                }
            } else {
                log(&format!(
                    "Run {} is Active, skipping in prune_all (requires --force)",
                    run.run_id
                ));
            }
        }

        Ok(pruned_run_ids)
    }
}

// Helpers

fn extract_run_id_from_path(path: &Path, runs_dir_canonical: &Path) -> Option<String> {
    let path_canonical = path
        .canonicalize()
        .ok()
        .unwrap_or_else(|| path.to_path_buf());
    if path_canonical.starts_with(runs_dir_canonical) {
        let relative = path_canonical.strip_prefix(runs_dir_canonical).ok()?;
        let mut components = relative.components();
        if let Some(std::path::Component::Normal(first)) = components.next() {
            return Some(first.to_string_lossy().to_string());
        }
    }
    None
}

fn extract_run_id_from_branch(branch: &str) -> Option<String> {
    if branch.starts_with("gestalt/") {
        let parts: Vec<&str> = branch.split('/').collect();
        if parts.len() >= 2 {
            return Some(parts[1].to_string());
        }
    }
    None
}

fn is_safe_to_delete_worktree(
    path: &Path,
    runs_dir_canonical: &Path,
    repo_path_canonical: &Path,
) -> bool {
    let path_canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(), // If it doesn't exist, we might not canonicalize it, but let's check prefix
    };

    // Must be under ~/.gestalt/runs/ (runs_dir_canonical)
    if !path_canonical.starts_with(runs_dir_canonical) {
        return false;
    }
    // Must NOT be the repository path or its parent
    if path_canonical == repo_path_canonical || repo_path_canonical.starts_with(&path_canonical) {
        return false;
    }
    true
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
