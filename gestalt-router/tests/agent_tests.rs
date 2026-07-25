use gestalt_router::agent::{AgentRunner, SubprocessRunner};
use gestalt_router::run::{AgentSpec, RunSpec, RouterError};
use gestalt_router::run_state::AgentState;
use gestalt_router::timeline::{Event, EventLog};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn init_temp_git_repo() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();

    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();
    assert!(status.success());

    std::process::Command::new("git")
        .arg("config")
        .arg("user.name")
        .arg("Gestalt Test")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    std::process::Command::new("git")
        .arg("config")
        .arg("user.email")
        .arg("test@gestalt.local")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    std::fs::write(temp_dir.path().join("README.md"), "# Test Repo").unwrap();

    std::process::Command::new("git")
        .arg("add")
        .arg("README.md")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    std::process::Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("Initial commit")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    temp_dir
}

// ---------------------------------------------------------------------------
// 1. AgentSpec serialization round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_agent_spec_serialization() {
    // Full spec with all optional fields populated
    let spec = AgentSpec {
        id: "ser-agent".into(),
        command: "echo".into(),
        args: vec!["hello".into()],
        allowed_paths: Some(vec!["/tmp".into(), "/var".into()]),
        env: Some({
            let mut m = HashMap::new();
            m.insert("MY_VAR".into(), "my_value".into());
            m
        }),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deserialized: AgentSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, "ser-agent");
    assert_eq!(deserialized.command, "echo");
    assert_eq!(deserialized.args, vec!["hello"]);
    assert_eq!(
        deserialized.allowed_paths,
        Some(vec!["/tmp".into(), "/var".into()])
    );
    assert_eq!(
        deserialized.env.as_ref().unwrap().get("MY_VAR"),
        Some(&"my_value".into())
    );

    // Round-trip when optional fields are None
    let minimal = AgentSpec {
        id: "minimal".into(),
        command: "true".into(),
        args: vec![],
        allowed_paths: None,
        env: None,
    };
    let json_min = serde_json::to_string(&minimal).unwrap();
    let deser_min: AgentSpec = serde_json::from_str(&json_min).unwrap();
    assert!(deser_min.allowed_paths.is_none());
    assert!(deser_min.env.is_none());
}

// ---------------------------------------------------------------------------
// 2. RunSpec serialization — push + integration_branch fields
// ---------------------------------------------------------------------------

#[test]
fn test_run_spec_fields() {
    // With integration_branch set and push = true
    let spec = RunSpec {
        base_ref: "main".into(),
        task: "add feature".into(),
        agents: vec![],
        max_parallel: 2,
        timeout: 120,
        push: true,
        integration_branch: Some("develop".into()),
    };
    assert!(spec.push);
    assert_eq!(spec.integration_branch.as_deref(), Some("develop"));

    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("\"push\":true"));
    assert!(json.contains("\"integration_branch\":\"develop\""));

    // Without integration_branch (null)
    let spec2 = RunSpec {
        integration_branch: None,
        ..spec
    };
    let json2 = serde_json::to_string(&spec2).unwrap();
    assert!(json2.contains("\"integration_branch\":null"));

    // push = false
    let spec3 = RunSpec {
        push: false,
        ..spec
    };
    let json3 = serde_json::to_string(&spec3).unwrap();
    assert!(json3.contains("\"push\":false"));
}

// ---------------------------------------------------------------------------
// 3. SubprocessRunner returns correct AgentResult for a successful run
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_subprocess_runner_agent_result() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let spec = AgentSpec {
        id: "result-test".into(),
        command: "sh".into(),
        args: vec![
            "-c".into(),
            "echo 'hello world'; echo 'new line' >> README.md".into(),
        ],
        allowed_paths: None,
        env: None,
    };

    let result = runner
        .run(&spec, temp_git.path(), "test result", Duration::from_secs(10))
        .await
        .unwrap();

    // Core AgentResult fields
    assert_eq!(result.agent_id, "result-test");
    assert_eq!(result.state, AgentState::Success);
    assert!(result.error.is_none(), "Expected no error: {:?}", result.error);
    assert!(
        result.duration_ms > 0,
        "Duration should be > 0, got {}",
        result.duration_ms
    );
    assert_eq!(
        result.worktree_path,
        Some(temp_git.path().to_string_lossy().to_string())
    );

    // The script appended to README.md → should be in changed_files
    assert!(
        !result.changed_files.is_empty(),
        "Expected at least one changed file"
    );
    assert!(
        result.changed_files.iter().any(|f| f.contains("README.md")),
        "README.md should be in changed_files"
    );

    // SubprocessRunner does not populate these fields
    assert!(result.output.is_none(), "output should be None");
    assert!(result.branch.is_none(), "branch should be None");
    assert!(result.run_id.is_none(), "run_id should be None");
}

// ---------------------------------------------------------------------------
// 4. AgentRunner trait impl — dispatch through trait object
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_agent_runner_trait_impl() {
    let temp_git = init_temp_git_repo();
    let runner: Box<dyn AgentRunner> = Box::new(SubprocessRunner::new(Duration::from_secs(10)));

    let spec = AgentSpec {
        id: "trait-test".into(),
        command: "echo".into(),
        args: vec!["trait object works".into()],
        allowed_paths: None,
        env: None,
    };

    let result = runner
        .run(&spec, temp_git.path(), "test trait obj", Duration::from_secs(10))
        .await
        .unwrap();

    assert_eq!(result.agent_id, "trait-test");
    assert_eq!(result.state, AgentState::Success);

    // Also dispatch through a &dyn AgentRunner reference
    let runner_ref: &dyn AgentRunner = &*runner;
    let result2 = runner_ref
        .run(&spec, temp_git.path(), "test via ref", Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(result2.agent_id, "trait-test");
    assert_eq!(result2.state, AgentState::Success);
}

// ---------------------------------------------------------------------------
// 5. Mock EventLog for testing — log, append, read_events, list_runs
// ---------------------------------------------------------------------------

struct MockEventLog {
    events: Mutex<Vec<Event>>,
}

impl MockEventLog {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl EventLog for MockEventLog {
    fn log(&self, event: Event) -> Result<(), RouterError> {
        self.append(event)
    }

    fn append(&self, event: Event) -> Result<(), RouterError> {
        let mut guard = self.events.lock().map_err(|e| {
            RouterError::TimelineError(format!("MockEventLog mutex poisoned: {}", e))
        })?;
        guard.push(event);
        Ok(())
    }

    fn read_events(&self, _run_id: Uuid) -> Result<Vec<Event>, RouterError> {
        let guard = self.events.lock().map_err(|e| {
            RouterError::TimelineError(format!("MockEventLog mutex poisoned: {}", e))
        })?;
        Ok(guard.clone())
    }

    fn list_runs(&self) -> Result<Vec<Uuid>, RouterError> {
        // Extract unique run_ids from RunStarted events
        let guard = self.events.lock().map_err(|e| {
            RouterError::TimelineError(format!("MockEventLog mutex poisoned: {}", e))
        })?;
        let mut ids = Vec::new();
        for event in guard.iter() {
            if let Event::RunStarted { run_id, .. } = event {
                if !ids.contains(run_id) {
                    ids.push(*run_id);
                }
            }
        }
        Ok(ids)
    }
}

#[tokio::test]
async fn test_mock_event_log() {
    let log = MockEventLog::new();

    // Initially empty
    let run_id = Uuid::new_v4();
    let events = log.read_events(run_id).unwrap();
    assert!(events.is_empty());

    // Log a RunStarted event (uses the EventLog::log shorthand)
    let event1 = Event::RunStarted {
        run_id,
        task: "mock log test".into(),
        agents: vec!["agent-1".into()],
        sha_base: "deadbeef".into(),
    };
    log.log(event1.clone()).unwrap();
    let events = log.read_events(run_id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], event1);

    // Append an AgentStateChanged event
    let event2 = Event::AgentStateChanged {
        run_id,
        agent_id: "agent-1".into(),
        from: AgentState::Pending,
        to: AgentState::Running,
    };
    log.append(event2.clone()).unwrap();
    let events = log.read_events(run_id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1], event2);

    // Verify list_runs returns the run_id we used
    let run_ids = log.list_runs().unwrap();
    assert_eq!(run_ids, vec![run_id]);

    // A second run produces an additional entry in list_runs
    let run_id2 = Uuid::new_v4();
    let event3 = Event::RunStarted {
        run_id: run_id2,
        task: "second run".into(),
        agents: vec!["agent-2".into()],
        sha_base: "cafebabe".into(),
    };
    log.log(event3).unwrap();
    let run_ids = log.list_runs().unwrap();
    assert_eq!(run_ids.len(), 2);
    assert!(run_ids.contains(&run_id));
    assert!(run_ids.contains(&run_id2));
}

// ---------------------------------------------------------------------------
// 6. Agent timeout handling — process killed after Duration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_agent_timeout_handling() {
    let temp_git = init_temp_git_repo();
    // Very short timeout to force the timeout path
    let runner = SubprocessRunner::new(Duration::from_millis(500));

    let spec = AgentSpec {
        id: "timeout-test".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "sleep 999".into()],
        allowed_paths: None,
        env: None,
    };

    let start = Instant::now();
    let result = runner
        .run(&spec, temp_git.path(), "test timeout", Duration::from_secs(10))
        .await
        .unwrap();
    let elapsed = start.elapsed();

    // The implementation uses self.timeout (500ms) + up to 5 s grace (SIGTERM
    // wait) + possible SIGKILL.  Total should be well under 8 s.
    assert!(
        elapsed < Duration::from_secs(8),
        "Timeout should complete within ~6 s (500 ms + 5 s grace), took {:?}",
        elapsed
    );

    // Current behaviour: SubprocessRunner always returns Success state
    // regardless of timeout.  Error is set because exit code forced to -1.
    assert_eq!(result.state, AgentState::Success);
    assert!(
        result.error.is_some(),
        "Expected error string for timed-out process"
    );
}

// ---------------------------------------------------------------------------
// 7. Process group creation (setsid) — grandchild cleanup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_process_group_creation() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(1));

    let pid_file = temp_git.path().join("child.pid");
    let script = format!(
        "sleep 999 & echo $! > {}; sleep 999",
        pid_file.to_str().unwrap()
    );

    let spec = AgentSpec {
        id: "pg-test".into(),
        command: "sh".into(),
        args: vec!["-c".into(), script],
        allowed_paths: None,
        env: None,
    };

    let result = runner
        .run(&spec, temp_git.path(), "test process group", Duration::from_secs(10))
        .await
        .unwrap();

    assert_eq!(result.state, AgentState::Success);
    assert!(result.error.is_some(), "Timed-out agent should report error");

    // The pid file must exist (the backgrounded sleep wrote its PID)
    assert!(
        pid_file.exists(),
        "Child PID file should have been written"
    );

    let pid_str = std::fs::read_to_string(&pid_file).unwrap();
    let grandchild_pid: i32 = pid_str.trim().parse().unwrap();

    #[cfg(unix)]
    {
        // Small delay for signal propagation
        tokio::time::sleep(Duration::from_millis(500)).await;

        // kill with signal 0 checks existence without sending a signal
        let is_alive = unsafe { libc::kill(grandchild_pid, 0) == 0 };
        assert!(
            !is_alive,
            "Grandchild sleep (pid={}) should have been killed via process-group death",
            grandchild_pid
        );
    }
}

// ---------------------------------------------------------------------------
// 8. Output capture — stdout and stderr side-effects in worktree
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_output_capture() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let output_file = temp_git.path().join("output.txt");
    let marker_file = temp_git.path().join("done.flag");

    let spec = AgentSpec {
        id: "capture-test".into(),
        command: "sh".into(),
        args: vec![
            "-c".into(),
            format!(
                "echo 'stdout line' && echo 'stderr line' >&2 && \
                 touch '{}' && touch '{}'",
                output_file.display(),
                marker_file.display()
            ),
        ],
        allowed_paths: None,
        env: None,
    };

    let result = runner
        .run(&spec, temp_git.path(), "test output capture", Duration::from_secs(10))
        .await
        .unwrap();

    assert_eq!(result.state, AgentState::Success);

    // Side-channel: the agent created these files in the worktree
    assert!(output_file.exists(), "output.txt should exist");
    assert!(marker_file.exists(), "done.flag should exist");

    // Both files should appear in changed_files
    assert!(
        result.changed_files.iter().any(|f| f.contains("output.txt")),
        "output.txt in changed_files"
    );
    assert!(
        result.changed_files.iter().any(|f| f.contains("done.flag")),
        "done.flag in changed_files"
    );
}

// ---------------------------------------------------------------------------
// 9. Environment sanitization — custom env passed, dangerous vars cleared
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_environment_sanitization() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let env_file = temp_git.path().join("env_dump.txt");

    // Set a custom variable that should survive sanitization
    let mut env = HashMap::new();
    env.insert("GESTALT_CUSTOM_VAR".into(), "custom_value".into());

    let spec = AgentSpec {
        id: "env-test".into(),
        command: "sh".into(),
        args: vec![
            "-c".into(),
            format!(
                "echo \"CUSTOM=$GESTALT_CUSTOM_VAR\" > '{}'; \
                 echo \"TASK=$GESTALT_TASK\" >> '{}'; \
                 echo \"PATH_HAS_CONTENT=${{PATH:+yes}}\" >> '{}'",
                env_file.display(),
                env_file.display(),
                env_file.display()
            ),
        ],
        allowed_paths: None,
        env: Some(env),
    };

    let result = runner
        .run(&spec, temp_git.path(), "test env", Duration::from_secs(10))
        .await
        .unwrap();

    assert_eq!(result.state, AgentState::Success);
    assert!(result.error.is_none(), "Expected no error: {:?}", result.error);

    // Read back the env dump the agent wrote to the worktree
    let contents = std::fs::read_to_string(&env_file).unwrap();

    // GESTALT_CUSTOM_VAR from spec.env must be present
    assert!(
        contents.contains("CUSTOM=custom_value"),
        "Custom env var should be passed through, got: {}",
        contents
    );

    // GESTALT_TASK should be set by the runner
    assert!(
        contents.contains("TASK=test env"),
        "GESTALT_TASK should be set by runner, got: {}",
        contents
    );

    // PATH should still be available (safe var restored by env_clear + restore)
    assert!(
        contents.contains("PATH_HAS_CONTENT=yes"),
        "PATH should be present after sanitization, got: {}",
        contents
    );
}

// ---------------------------------------------------------------------------
// 10. Agent failure — non-zero exit code
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_agent_failure_result() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let spec = AgentSpec {
        id: "fail-test".into(),
        command: "sh".into(),
        args: vec!["-c".into(), "exit 42".into()],
        allowed_paths: None,
        env: None,
    };

    let result = runner
        .run(&spec, temp_git.path(), "test failure", Duration::from_secs(10))
        .await
        .unwrap();

    // Current behaviour: state is always Success. Error is set when
    // exit code != 0.
    assert_eq!(result.state, AgentState::Success);
    assert!(
        result.error.is_some(),
        "Expected error for non-zero exit code"
    );
    assert!(
        result.error.as_deref().unwrap().contains("non-zero"),
        "Error should mention non-zero exit, got: {:?}",
        result.error
    );

    // No files should have changed (exit 42 doesn't modify the worktree)
    assert!(
        result.changed_files.is_empty(),
        "Expected no changed files on failure"
    );
}
