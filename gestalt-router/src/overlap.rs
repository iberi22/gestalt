use crate::run::RouterError;
use gestalt_state::memstate::MemState;
use gestalt_ws::WsEvent;
use gestalt_ws::WsServer;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A lock conflict detected in real time between two agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockConflict {
    /// The file path that both agents tried to lock.
    pub path: String,
    /// The agent that already holds the lock.
    pub agent_a: String,
    /// The agent that failed to acquire the lock.
    pub agent_b: String,
    /// The run in which the conflict occurred.
    pub run_id: String,
}

/// Real-time overlap detector using MemState's timeline broadcast channel.
///
/// Monitors lock acquisition events broadcast by [`MemState`] and detects
/// when two agents compete for the same file. When a conflict is found,
/// it emits a `conflict_detected` event back through the broadcast channel
/// so that subscribers (including the WebSocket bridge) can observe it.
///
/// The legacy `find_overlaps` / `detect_overlap` functions are retained for
/// post-hoc (git-based) overlap analysis after all agents have finished.
#[derive(Clone)]
pub struct OverlapDetector {
    mem_state: MemState,
}

impl OverlapDetector {
    /// Create a new `OverlapDetector` that monitors the given [`MemState`].
    pub fn new(mem_state: MemState) -> Self {
        Self { mem_state }
    }

    /// Spawn a background task that subscribes to MemState events and
    /// detects real-time lock conflicts.
    ///
    /// The task listens for `lock_acquired` events. When a new lock is
    /// acquired, it checks whether another agent already holds a lock on
    /// the same path. If so, it emits a `conflict_detected` event via
    /// [`MemState::push_event`] so that both the timeline log and
    /// WebSocket bridge are notified.
    ///
    /// Also listens for `lock_conflict` events emitted by
    /// [`MemState::try_lock`] when a lock attempt fails due to an existing
    /// holder — these are forwarded as `conflict_detected` so the system
    /// has a single, canonical event type for conflicts.
    pub fn spawn_monitor(&self) {
        let mem_state = self.mem_state.clone();
        let mut rx = self.mem_state.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => match event.event_type.as_str() {
                        "lock_acquired" => {
                            // A new lock was acquired — check if another agent
                            // already holds a lock on the same path
                            if let Ok(payload) =
                                serde_json::from_str::<serde_json::Value>(&event.payload)
                            {
                                if let Some(path) =
                                    payload.get("path").and_then(|v| v.as_str())
                                {
                                    if let Some(ref agent_id) = event.agent_id {
                                        if let Some(holder) =
                                            Self::check_lock(&mem_state, path, agent_id)
                                        {
                                            let conflict_payload = serde_json::json!({
                                                "path": path,
                                                "agent_a": holder,
                                                "agent_b": agent_id,
                                                "message": format!(
                                                    "Conflict: agent {holder} already holds lock on {path}"
                                                ),
                                            })
                                            .to_string();
                                            mem_state.push_event(
                                                &event.run_id,
                                                Some(agent_id),
                                                "conflict_detected",
                                                &conflict_payload,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        "lock_conflict" => {
                            // A lock attempt failed — forward as conflict_detected
                            // so downstream consumers only need to listen for one type
                            if let Ok(payload) =
                                serde_json::from_str::<serde_json::Value>(&event.payload)
                            {
                                let conflict_payload = serde_json::json!({
                                    "path": payload.get("path"),
                                    "agent_a": payload.get("held_by"),
                                    "agent_b": payload.get("agent_b"),
                                    "message": format!(
                                        "Lock conflict on {} between {} and {}",
                                        payload.get("path").and_then(|v| v.as_str()).unwrap_or("?"),
                                        payload.get("held_by").and_then(|v| v.as_str()).unwrap_or("?"),
                                        payload.get("agent_b").and_then(|v| v.as_str()).unwrap_or("?"),
                                    ),
                                })
                                .to_string();
                                mem_state.push_event(
                                    &event.run_id,
                                    event.agent_id.as_deref(),
                                    "conflict_detected",
                                    &conflict_payload,
                                );
                            }
                        }
                        _ => {
                            // Ignore other event types
                        }
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("OverlapDetector lagged by {n} events");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::debug!("OverlapDetector: MemState broadcast closed");
                        break;
                    }
                }
            }
        });
    }

    /// Check whether a lock on `path` is currently held by an agent other than
    /// `agent_id`.
    ///
    /// Returns `Some(holder_agent_id)` if another agent holds the lock, or
    /// `None` if the path is free or locked by the same agent.
    pub fn check_lock(mem_state: &MemState, path: &str, agent_id: &str) -> Option<String> {
        for lock in mem_state.get_locks() {
            if lock.path == path && lock.agent_id != agent_id {
                return Some(lock.agent_id);
            }
        }
        None
    }

    /// Statically check all current locks and return any conflicts for the
    /// given agent. Returns all locks held by other agents.
    pub fn check_all_locks_for_agent(
        mem_state: &MemState,
        agent_id: &str,
    ) -> Vec<(String, String)> {
        mem_state
            .get_locks()
            .iter()
            .filter(|lock| lock.agent_id != agent_id)
            .map(|lock| (lock.path.clone(), lock.agent_id.clone()))
            .collect()
    }

    /// Get a reference to the inner MemState.
    pub fn mem_state(&self) -> &MemState {
        &self.mem_state
    }
}

/// Real-time conflict detector that subscribes to MemState events and
/// broadcasts [`WsEvent::ConflictDetected`] directly through the WebSocket
/// server whenever a lock conflict is detected.
///
/// Unlike [`OverlapDetector`], which pushes events back through
/// [`MemState::push_event`], this detector emits structured WebSocket
/// events so that connected clients receive real-time conflict notifications.
pub struct LiveConflictDetector {
    /// Shared in-memory state with broadcast channel.
    state: MemState,
    /// Optional WebSocket server for broadcasting events.
    ws: Option<WsServer>,
}

impl LiveConflictDetector {
    /// Create a new `LiveConflictDetector`.
    pub fn new(state: MemState, ws: Option<WsServer>) -> Self {
        Self { state, ws }
    }

    /// Subscribe to MemState events and detect conflicts in real time.
    ///
    /// Listens for `lock_acquired` and `lock_conflict` events. When a
    /// conflict is found, it broadcasts a [`WsEvent::ConflictDetected`]
    /// through the WebSocket server (if configured).
    pub async fn run(self) {
        let mut rx = self.state.subscribe();
        let ws = self.ws.clone();

        loop {
            match rx.recv().await {
                Ok(event) => match event.event_type.as_str() {
                    "lock_acquired" => {
                        // A new lock was acquired — check if another agent
                        // already holds a lock on the same path
                        if let Ok(payload) =
                            serde_json::from_str::<serde_json::Value>(&event.payload)
                        {
                            if let Some(path) =
                                payload.get("path").and_then(|v| v.as_str())
                            {
                                if let Some(ref agent_id) = event.agent_id {
                                    if let Some(holder) =
                                        Self::check_lock(&self.state, path, agent_id)
                                    {
                                        Self::broadcast_conflict(
                                            &ws,
                                            &event.run_id,
                                            &holder,
                                            agent_id,
                                            path,
                                            &format!(
                                                "Conflict: agent {holder} already holds lock on {path}"
                                            ),
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                    }
                    "lock_conflict" => {
                        // A lock attempt failed — forward as ConflictDetected
                        if let Ok(payload) =
                            serde_json::from_str::<serde_json::Value>(&event.payload)
                        {
                            let path = payload
                                .get("path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let holder = payload
                                .get("held_by")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let agent_b = payload
                                .get("agent_b")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            Self::broadcast_conflict(
                                &ws,
                                &event.run_id,
                                holder,
                                agent_b,
                                path,
                                &format!(
                                    "Lock conflict on {path} between {holder} and {agent_b}"
                                ),
                            )
                            .await;
                        }
                    }
                    _ => {
                        // Ignore other event types
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("LiveConflictDetector lagged by {n} events");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::debug!("LiveConflictDetector: MemState broadcast closed");
                    break;
                }
            }
        }
    }

    /// Check whether a lock on `path` is currently held by an agent other than
    /// `agent_id`.
    fn check_lock(state: &MemState, path: &str, agent_id: &str) -> Option<String> {
        for lock in state.get_locks() {
            if lock.path == path && lock.agent_id != agent_id {
                return Some(lock.agent_id);
            }
        }
        None
    }

    /// Broadcast a [`WsEvent::ConflictDetected`] through the WebSocket server
    /// (if one is configured).
    async fn broadcast_conflict(
        ws: &Option<WsServer>,
        run_id: &str,
        agent_a: &str,
        agent_b: &str,
        path: &str,
        message: &str,
    ) {
        if let Some(ref ws_server) = ws {
            ws_server
                .broadcast(&WsEvent::ConflictDetected {
                    run_id: run_id.to_string(),
                    agent_a: agent_a.to_string(),
                    agent_b: agent_b.to_string(),
                    path: path.to_string(),
                    message: message.to_string(),
                })
                .await;
        }
    }
}

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
    repo_path: &Path,
    base_sha: &str,
    active_branches: &[(String, String)],
) -> Result<Vec<OverlapInfo>, RouterError> {
    find_overlaps_in_repo(repo_path, base_sha, active_branches)
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

            let kind = map_stages_to_kind(has_base, has_our, has_their);

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
