use crate::run::{AgentResult, AgentSpec, RouterError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub state: crate::run_state::AgentState,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    pub duration: Duration,
    pub files_changed: Vec<PathBuf>,
}

#[async_trait]
pub trait AgentRunner: Send + Sync {
    async fn run(
        &self,
        spec: &AgentSpec,
        worktree: &Path,
        task: &str,
        timeout: Duration,
    ) -> Result<AgentResult, RouterError>;
}

pub struct SubprocessRunner {
    pub timeout: Duration,
}

impl SubprocessRunner {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

/// Helper function to check if a process with a given PID is alive.
/// Under Unix, `kill(pid, 0)` is used to query the process's existence.
///
/// # Safety
/// // SAFETY: This helper function encapsulates the unsafe `libc::kill` call. It ensures the PID is validated
/// // before executing.
pub fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        if pid <= 1 {
            return false;
        }
        // SAFETY: We validate that `pid > 1` (since system processes/groups are not targeted),
        // and calling `kill` with signal 0 is standard, safe, and does not alter process state.
        unsafe {
            libc::kill(pid as libc::pid_t, 0) == 0
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

// Helpers for process tree and cgroup tracking
#[cfg(unix)]
fn get_all_pids() -> Vec<u32> {
    let mut pids = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if let Ok(pid) = name.parse::<u32>() {
                            pids.push(pid);
                        }
                    }
                }
            }
        }
    }
    pids
}

#[cfg(unix)]
fn get_ppid(pid: u32) -> Option<u32> {
    let stat_path = format!("/proc/{}/stat", pid);
    if let Ok(content) = std::fs::read_to_string(&stat_path) {
        if let Some(last_paren) = content.rfind(')') {
            let rest = &content[last_paren + 1..];
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(ppid) = parts[1].parse::<u32>() {
                    return Some(ppid);
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn get_descendants(parent: u32) -> Vec<u32> {
    let pids = get_all_pids();
    let mut parent_to_children: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();
    for pid in pids {
        if let Some(ppid) = get_ppid(pid) {
            parent_to_children.entry(ppid).or_default().push(pid);
        }
    }

    let mut descendants = Vec::new();
    let mut queue = vec![parent];
    while let Some(current) = queue.pop() {
        if let Some(children) = parent_to_children.get(&current) {
            for &child in children {
                descendants.push(child);
                queue.push(child);
            }
        }
    }
    descendants
}

#[cfg(unix)]
fn find_writable_cgroup_base() -> Option<PathBuf> {
    // SAFETY: libc::getuid() is a standard system call that does not dereference raw pointers or modify any process/system state.
    let uid = unsafe { libc::getuid() };
    let mut candidates = vec![
        PathBuf::from(format!(
            "/sys/fs/cgroup/user.slice/user-{}.slice/user@{}.service/app.slice",
            uid, uid
        )),
        PathBuf::from(format!(
            "/sys/fs/cgroup/user.slice/user-{}.slice/user@{}.service",
            uid, uid
        )),
    ];

    // Parse from self cgroup
    if let Ok(content) = std::fs::read_to_string("/proc/self/cgroup") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                let path = parts[2].trim_start_matches('/');
                if !path.is_empty() {
                    candidates.push(PathBuf::from("/sys/fs/cgroup").join(path));
                }
            }
        }
    }

    candidates.push(PathBuf::from("/sys/fs/cgroup"));

    for path in candidates {
        if path.exists() {
            let temp_dir = path.join(format!("gestalt-probe-{}", uuid::Uuid::new_v4()));
            if std::fs::create_dir(&temp_dir).is_ok() {
                let _ = std::fs::remove_dir(&temp_dir);
                return Some(path);
            }
        }
    }
    None
}

#[derive(Debug)]
pub struct ProcessReaper {
    pub pid: Option<u32>,
    pub cgroup_path: Option<PathBuf>,
}

impl ProcessReaper {
    pub fn new(pid: Option<u32>, cgroup_path: Option<PathBuf>) -> Self {
        Self { pid, cgroup_path }
    }

    pub fn kill_gracefully(&self) {
        #[cfg(unix)]
        {
            let mut descendants = Vec::new();
            if let Some(p) = self.pid {
                descendants = get_descendants(p);
                descendants.push(p);
            }

            // Kill process group
            if let Some(p) = self.pid {
                if p > 1 {
                    // SAFETY: We validate that `p > 1` so that we do not kill system processes (PID <= 1) or target unexpected process groups.
                    unsafe {
                        libc::kill(-(p as libc::pid_t), libc::SIGTERM);
                    }
                }
            }

            // Kill descendants individually
            for &pid in &descendants {
                if pid > 1 {
                    // SAFETY: We validate that `pid > 1` so that we do not kill system processes (PID <= 1).
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGTERM);
                    }
                }
            }
        }
    }

    pub fn kill_forcefully(&self) {
        #[cfg(unix)]
        {
            let _ = self.kill_cgroup();

            let mut descendants = Vec::new();
            if let Some(p) = self.pid {
                descendants = get_descendants(p);
                descendants.push(p);
            }

            // Kill process group
            if let Some(p) = self.pid {
                if p > 1 {
                    // SAFETY: We validate that `p > 1` so that we do not kill system processes (PID <= 1) or target unexpected process groups.
                    unsafe {
                        libc::kill(-(p as libc::pid_t), libc::SIGKILL);
                    }
                }
            }

            // Kill descendants individually
            for &pid in &descendants {
                if pid > 1 {
                    // SAFETY: We validate that `pid > 1` so that we do not kill system processes (PID <= 1).
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGKILL);
                    }
                }
            }
        }
    }

    fn kill_cgroup(&self) -> bool {
        if let Some(ref cg) = self.cgroup_path {
            // Try cgroup.kill
            let kill_file = cg.join("cgroup.kill");
            if std::fs::write(&kill_file, "1").is_ok() {
                return true;
            }
            // Fallback: read cgroup.procs and kill each process
            let procs_file = cg.join("cgroup.procs");
            if let Ok(content) = std::fs::read_to_string(&procs_file) {
                for line in content.lines() {
                    if let Ok(pid) = line.trim().parse::<i32>() {
                        if pid > 1 {
                            // SAFETY: We validate that `pid > 1` to ensure we do not target system processes.
                            unsafe {
                                libc::kill(pid, libc::SIGKILL);
                            }
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    pub fn cleanup(&self) {
        self.kill_forcefully();

        if let Some(ref cg) = self.cgroup_path {
            let cg = cg.clone();
            tokio::spawn(async move {
                for _ in 0..10 {
                    if std::fs::remove_dir(&cg).is_ok() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            });
        }
    }
}

impl Drop for ProcessReaper {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[async_trait]
impl AgentRunner for SubprocessRunner {
    async fn run(
        &self,
        spec: &AgentSpec,
        worktree: &Path,
        task: &str,
        timeout: Duration,
    ) -> Result<AgentResult, RouterError> {
        let start_time = Instant::now();

        // 1. Generate unique stdout and stderr file paths
        let run_id = uuid::Uuid::new_v4();
        let stdout_path =
            std::env::temp_dir().join(format!("agent_stdout_{}_{}.log", spec.id, run_id));
        let stderr_path =
            std::env::temp_dir().join(format!("agent_stderr_{}_{}.log", spec.id, run_id));

        // Create the files
        let mut stdout_file = tokio::fs::File::create(&stdout_path)
            .await
            .map_err(|e| RouterError::AgentError(format!("Failed to create stdout file: {}", e)))?;
        let mut stderr_file = tokio::fs::File::create(&stderr_path)
            .await
            .map_err(|e| RouterError::AgentError(format!("Failed to create stderr file: {}", e)))?;

        // 2. Build the Command
        let mut cmd = tokio::process::Command::new(&spec.command);
        cmd.args(&spec.args);
        cmd.current_dir(worktree);

        // Sanitize environment variables: clear everything, then add safe essential variables plus user-specified ones.
        cmd.env_clear();
        let mut has_path = false;
        for var in &["PATH", "HOME", "USER", "TERM", "LANG", "LC_ALL"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
                if *var == "PATH" {
                    has_path = true;
                }
            }
        }
        if !has_path {
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        }

        if let Some(ref env) = spec.env {
            for (k, v) in env.iter() {
                cmd.env(k, v);
            }
        }

        // Add the task itself as an environment variable or context if needed, but the main thing is sanitizing env
        cmd.env("GESTALT_TASK", task);

        // Configure process group on Unix (safe, no unsafe)
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        // Attempt to find a writable cgroup base and create a sub-cgroup
        let mut cgroup_path = None;
        #[cfg(unix)]
        {
            if let Some(base) = find_writable_cgroup_base() {
                let cg_dir = base.join(format!("gestalt-agent-{}", uuid::Uuid::new_v4()));
                if std::fs::create_dir(&cg_dir).is_ok() {
                    cgroup_path = Some(cg_dir);
                }
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // 3. Spawn the child process
        let mut child = cmd.spawn().map_err(|e| {
            RouterError::AgentError(format!("Failed to spawn agent process: {}", e))
        })?;

        // Extract pid immediately to avoid borrow issues later
        let pid = child.id();

        // Write PID to cgroup.procs from parent safely (no pre_exec race condition or cgroup write issues)
        #[cfg(unix)]
        if let (Some(p), Some(ref cg)) = (pid, &cgroup_path) {
            let _ = std::fs::write(cg.join("cgroup.procs"), p.to_string());
        }

        let reaper = ProcessReaper::new(pid, cgroup_path);

        // Extract stdout/stderr pipes
        let mut child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| RouterError::AgentError("Failed to open stdout pipe".to_string()))?;
        let mut child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| RouterError::AgentError("Failed to open stderr pipe".to_string()))?;

        // Copy stdout/stderr to files concurrently
        let stdout_handle =
            tokio::spawn(async move { tokio::io::copy(&mut child_stdout, &mut stdout_file).await });
        let stderr_handle =
            tokio::spawn(async move { tokio::io::copy(&mut child_stderr, &mut stderr_file).await });

        // 4. Wait with timeout and graceful termination/SIGKILL pattern
        let wait_fut = child.wait();
        tokio::pin!(wait_fut);

        #[allow(unused_assignments)]
        let mut exit_code = None;
        let mut is_timeout = false;

        match tokio::time::timeout(timeout, &mut wait_fut).await {
            Ok(Ok(status)) => {
                exit_code = status.code();
            },
            Ok(Err(e)) => {
                return Err(RouterError::AgentError(format!(
                    "Process wait error: {}",
                    e
                )));
            },
            Err(_) => {
                // Timeout occurred!
                is_timeout = true;

                // Send SIGTERM to the process group and all descendants gracefully
                reaper.kill_gracefully();

                // Wait up to 5 seconds for the process to exit gracefully
                let grace_duration = Duration::from_secs(5);
                match tokio::time::timeout(grace_duration, &mut wait_fut).await {
                    Ok(Ok(status)) => {
                        exit_code = status.code();
                    },
                    Ok(Err(e)) => {
                        return Err(RouterError::AgentError(format!(
                            "Process wait error after SIGTERM: {}",
                            e
                        )));
                    },
                    Err(_) => {
                        // Grace period expired! Send SIGKILL to the process group and descendants forcefully
                        reaper.kill_forcefully();

                        // Final wait to reap the process
                        match wait_fut.await {
                            Ok(status) => {
                                exit_code = status.code();
                            },
                            Err(e) => {
                                return Err(RouterError::AgentError(format!(
                                    "Process reap error after SIGKILL: {}",
                                    e
                                )));
                            },
                        }
                    },
                }
            },
        }

        // Wait for stdout and stderr copying tasks to finish
        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        let duration = start_time.elapsed();

        if is_timeout {
            exit_code = Some(-1);
        }

        // Read captured output and delete temporary files to avoid disk leaks
        let stdout_content = tokio::fs::read_to_string(&stdout_path)
            .await
            .unwrap_or_default();
        let stderr_content = tokio::fs::read_to_string(&stderr_path)
            .await
            .unwrap_or_default();
        let _ = tokio::fs::remove_file(&stdout_path).await;
        let _ = tokio::fs::remove_file(&stderr_path).await;

        let mut full_output = String::new();
        if !stdout_content.is_empty() {
            full_output.push_str(&stdout_content);
        }
        if !stderr_content.is_empty() {
            if !full_output.is_empty() {
                full_output.push('\n');
            }
            full_output.push_str(&stderr_content);
        }

        let output_opt = if full_output.is_empty() {
            None
        } else {
            Some(full_output)
        };

        // 5. Gather files changed via git status --porcelain
        let mut files_changed = Vec::new();
        let mut git_cmd = tokio::process::Command::new("git");
        git_cmd
            .arg("status")
            .arg("--porcelain")
            .current_dir(worktree);
        if let Ok(output) = git_cmd.output().await {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.len() > 3 {
                        let file_path = line[3..].trim_matches('"');
                        files_changed.push(worktree.join(file_path));
                    }
                }
            }
        }

        let state = if is_timeout {
            crate::run_state::AgentState::Timeout
        } else if exit_code.is_some_and(|c| c != 0) {
            crate::run_state::AgentState::Crashed
        } else {
            crate::run_state::AgentState::Success
        };

        let error = if is_timeout {
            Some("Agent process timed out".to_string())
        } else if exit_code.is_some_and(|c| c != 0) {
            Some(format!(
                "Process exited with non-zero code: {:?}",
                exit_code
            ))
        } else {
            None
        };

        Ok(crate::run::AgentResult {
            agent_id: spec.id.clone(),
            state,
            output: output_opt,
            error,
            branch: None,
            changed_files: files_changed
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            duration_ms: duration.as_millis() as u64,
            run_id: None,
            worktree_path: Some(worktree.to_string_lossy().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_state::AgentState;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("gestalt_reaper_test_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[tokio::test]
    async fn test_process_reaper_timeout_kills_tree() {
        #[cfg(unix)]
        {
            let temp_dir = TestTempDir::new();
            let pid_file = temp_dir.path().join("child.pid");

            let runner = SubprocessRunner::new(Duration::from_millis(500));
            // Spawn a script that starts a background sleep process, writes its pid to pid_file, and waits.
            // If the script is killed, the background process should also be killed by the ProcessReaper.
            let spec = AgentSpec {
                id: "tree-killer-agent".to_string(),
                command: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    format!("sleep 100 & echo $! > {}; wait", pid_file.to_string_lossy()),
                ],
                allowed_paths: None,
                env: None,
            };

            let result = runner
                .run(
                    &spec,
                    temp_dir.path(),
                    "test cgroup / process tree kill",
                    Duration::from_millis(500),
                )
                .await
                .unwrap();

            assert_eq!(result.state, AgentState::Timeout);

            // Read the pid of the background process
            assert!(
                pid_file.exists(),
                "PID file was not written by background process"
            );
            let pid_str = std::fs::read_to_string(&pid_file).unwrap();
            let bg_pid: i32 = pid_str.trim().parse().unwrap();

            // Check if the background sleep process is still running.
            // Under Unix, kill(pid, 0) returns -1 with ESRCH if the process does not exist.
            // // SAFETY: Checks if process is alive safely.
            let is_alive = is_process_alive(bg_pid as u32);

            assert!(
                !is_alive,
                "The background descendant process (PID {}) is still alive!",
                bg_pid
            );
        }
    }
}
