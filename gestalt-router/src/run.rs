use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use uuid::Uuid;
use crate::run_state::AgentState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub allowed_paths: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpec {
    pub base_ref: String,
    pub task: String,
    pub agents: Vec<AgentSpec>,
    pub max_parallel: usize,
    pub timeout: u64,
    pub push: bool,
    pub integration_branch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Success,
    Timeout,
    Crashed,
    NoChanges,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub state: AgentState,
    pub output: Option<String>,
    pub error: Option<String>,
    pub branch: Option<String>,
    pub changed_files: Vec<String>,
    pub duration_ms: u64,
    pub run_id: Option<uuid::Uuid>,
    pub worktree_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictKind {
    Overlap,
    MergeConflict,
    BinaryConflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub path: String,
    pub agent_a: String,
    pub agent_b: String,
    pub kind: ConflictKind,

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: Uuid,
    pub agents: Vec<AgentResult>,
    pub merged_branches: Vec<String>,
    pub conflicts: Vec<ConflictInfo>,
    pub events_path: String,
    pub success: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouterErrorKind {
    GitError,
    AgentError,
    Timeout,
    InvalidSpec,
    TimelineError,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct RouterError {
    pub kind: RouterErrorKind,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
}

impl RouterError {
    pub fn new(
        kind: RouterErrorKind,
        message: impl Into<String>,
        source: Option<Box<dyn std::error::Error + Send + Sync + 'static>>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source,
        }
    }

    pub fn TimelineError(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::TimelineError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn GitError(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::GitError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn AgentError(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::AgentError,
            message: msg.into(),
            source: None,
        }
    }

    pub fn InvalidSpec(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::InvalidSpec,
            message: msg.into(),
            source: None,
        }
    }

    pub fn Timeout(msg: impl Into<String>) -> Self {
        Self {
            kind: RouterErrorKind::Timeout,
            message: msg.into(),
            source: None,
        }
    }
}

pub struct Router {
    pub repo_path: PathBuf,
}

impl Router {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub async fn execute(&self, spec: RunSpec) -> Result<RunReport, RouterError> {
        let run_id = Uuid::new_v4();
        let semaphore = Arc::new(Semaphore::new(spec.max_parallel));
        let mut join_set = tokio::task::JoinSet::new();

        for agent in &spec.agents {
            let sem = semaphore.clone();
            let agent = agent.clone();
            let repo_path = self.repo_path.clone();
            let run_id = run_id;
            let base_ref = spec.base_ref.clone();
            let timeout_sec = spec.timeout;

            join_set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let branch_name = format!("gestalt/run-{}/{}", run_id, agent.id);
                let worktree_dir_name = format!("gestalt-worktree-{}-{}", run_id, agent.id);
                let worktree_path = std::env::temp_dir().join(worktree_dir_name);

                // Add Git worktree
                let worktree_add_res = run_git_cmd(&repo_path, &[
                    "worktree",
                    "add",
                    "-b",
                    &branch_name,
                    &worktree_path.to_string_lossy(),
                    &base_ref,
                ]);

                if let Err(e) = worktree_add_res {
                    return AgentResult {
                        agent_id: agent.id.clone(),
                        state: AgentState::Crashed,
                        output: None,
                        error: Some(format!("Failed to create git worktree: {}", e)),
                        branch: None,
                        changed_files: vec![],
                        duration_ms: 0,
                        run_id: None,
                        worktree_path: None,
                    };
                }

                // Execute agent process
                let mut cmd = tokio::process::Command::new(&agent.command);
                cmd.args(&agent.args)
                    .current_dir(&worktree_path);
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = run_git_cmd(&repo_path, &[
                            "worktree",
                            "remove",
                            "--force",
                            &worktree_path.to_string_lossy(),
                        ]);
                        let _ = std::fs::remove_dir_all(&worktree_path);
                        return AgentResult {
                            agent_id: agent.id.clone(),
                            state: AgentState::Crashed,
                            output: None,
                            error: Some(format!("Failed to spawn agent process: {}", e)),
                        };
                    }
                };

                let pid = child.id();

                // Wait with timeout using select!
                let timeout_duration = std::time::Duration::from_secs(timeout_sec);

                let (mut state, output, error) = tokio::select! {
                    wait_res = child.wait_with_output() => {
                        match wait_res {
                            Ok(output_val) => {
                                let stdout = String::from_utf8_lossy(&output_val.stdout).into_owned();
                                let stderr = String::from_utf8_lossy(&output_val.stderr).into_owned();
                                let full_output = format!("{}\n{}", stdout, stderr);
                                if output_val.status.success() {
                                    (AgentState::Success, Some(full_output), None)
                                } else {
                                    let code = output_val.status.code().unwrap_or(-1);
                                    (
                                        AgentState::Crashed,
                                        Some(full_output),
                                        Some(format!("Agent process exited with code {}", code)),
                                    )
                                }
                            }
                            Err(e) => {
                                (AgentState::Crashed, None, Some(format!("Failed to wait for agent process: {}", e)))
                            }
                        }
                    }
                    _ = tokio::time::sleep(timeout_duration) => {
                        if let Some(p) = pid {
                            let _ = std::process::Command::new("kill")
                                .arg("-9")
                                .arg(p.to_string())
                                .status();
                        }
                        (
                            AgentState::Timeout,
                            None,
                            Some("Agent process timed out and was killed".to_string()),
                        )
                    }
                };

                let mut has_changes = false;
                if state == AgentState::Success {
                    let mut escaped_symlinks = Vec::new();
                    scan_for_symlink_escapes(&worktree_path, &worktree_path, &mut escaped_symlinks);
                    if !escaped_symlinks.is_empty() {
                        state = AgentState::Quarantined;
                        for esc in escaped_symlinks {
                            let _ = std::fs::remove_file(esc);
                        }
                    }

                    if let Ok(status_out) = run_git_cmd(&worktree_path, &["status", "--porcelain"]) {
                        if !status_out.trim().is_empty() {
                            has_changes = true;
                            let _ = run_git_cmd(&worktree_path, &["add", "-A"]);
                            let _ = run_git_cmd(&worktree_path, &[
                                "commit",
                                "-m",
                                &format!("feat(agent-{}): updates", agent.id),
                            ]);
                        }
                    }

                    if !has_changes && state == AgentState::Success {
                        state = AgentState::NoChanges;
                    }
                }

                // Cleanup worktree
                let _ = run_git_cmd(&repo_path, &[
                    "worktree",
                    "remove",
                    "--force",
                    &worktree_path.to_string_lossy(),
                ]);
                if worktree_path.exists() {
                    let _ = std::fs::remove_dir_all(&worktree_path);
                }

                AgentResult {
                    agent_id: agent.id,
                    state,
                    output,
                    error,
                }
            });
        }

        let mut agent_results = Vec::new();
        while let Some(res) = join_set.join_next().await {
            if let Ok(agent_res) = res {
                agent_results.push(agent_res);
            }
        }

        // Sort agent_results to match the original order of spec.agents
        let agent_order: HashMap<String, usize> = spec
            .agents
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.clone(), i))
            .collect();
        agent_results.sort_by_key(|r| agent_order.get(&r.agent_id).copied().unwrap_or(usize::MAX));

        let mut merged_branches = Vec::new();
        let mut conflicts = Vec::new();

        // Checkout integration branch from base_ref
        if let Err(e) = run_git_cmd(&self.repo_path, &[
            "checkout",
            "-B",
            &spec.integration_branch,
            &spec.base_ref,
        ]) {
            return Err(RouterError::GitError(format!(
                "Failed to checkout integration branch: {}",
                e
            )));
        }

        // Sequentially merge each successful/quarantined agent branch
        for result in &agent_results {
            if result.state == AgentState::Success || result.state == AgentState::Quarantined {
                let branch_name = format!("gestalt/run-{}/{}", run_id, result.agent_id);

                match run_git_cmd(&self.repo_path, &[
                    "merge",
                    "--no-ff",
                    "-m",
                    &format!("Merge agent-{}", result.agent_id),
                    &branch_name,
                ]) {
                    Ok(_) => {
                        merged_branches.push(branch_name);
                    }
                    Err(_) => {
                        if let Ok(conflicted_out) = run_git_cmd(&self.repo_path, &[
                            "diff",
                            "--name-only",
                            "--diff-filter=U",
                        ]) {
                            for line in conflicted_out.lines() {
                                let path_str = line.trim().to_string();
                                if !path_str.is_empty() && !conflicts.contains(&path_str) {
                                    conflicts.push(path_str);
                                }
                            }
                        }
                        let _ = run_git_cmd(&self.repo_path, &["merge", "--abort"]);
                    }
                }
            }
        }

        // Checkout back to base_ref to restore state
        let _ = run_git_cmd(&self.repo_path, &["checkout", &spec.base_ref]);

        let events_path = std::env::temp_dir()
            .join(format!("run-{}.jsonl", run_id))
            .to_string_lossy()
            .into_owned();

        Ok(RunReport {
            run_id,
            agents: agent_results,
            merged_branches,
            conflicts,
            events_path,
            success: true,
        })
    }
}

fn run_git_cmd(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    let mut is_absolute = path.is_absolute();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            Component::Normal(c) => {
                components.push(c);
            }
            Component::RootDir => {
                is_absolute = true;
            }
            _ => {}
        }
    }
    let mut result = PathBuf::new();
    if is_absolute {
        result.push("/");
    }
    for c in components {
        result.push(c);
    }
    result
}

fn is_symlink_escape(worktree_root: &Path, file_path: &Path) -> bool {
    if let Ok(target) = std::fs::read_link(file_path) {
        let resolved = if target.is_absolute() {
            target
        } else {
            let parent = file_path.parent().unwrap_or(worktree_root);
            parent.join(target)
        };
        let norm_root = normalize_path(worktree_root);
        let norm_resolved = normalize_path(&resolved);
        if !norm_resolved.starts_with(&norm_root) {
            return true;
        }
    }
    false
}

fn scan_for_symlink_escapes(dir: &Path, current_dir: &Path, escaped_symlinks: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(metadata) = std::fs::symlink_metadata(&path) {
                if metadata.file_type().is_symlink() {
                    if is_symlink_escape(dir, &path) {
                        escaped_symlinks.push(path.clone());
                    }
                } else if metadata.is_dir() {
                    scan_for_symlink_escapes(dir, &path, escaped_symlinks);
                }
            }
        }
    }
}