use gestalt_router::agent::{AgentRunner, SubprocessRunner, AgentOutcome};
use gestalt_router::run::{AgentResult, AgentSpec, RouterError};
use gestalt_router::run_state::AgentState;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_agent_spec_default_construction() {
    let spec = AgentSpec {
        id: "test-agent".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        allowed_paths: None,
        env: None,
    };
    assert_eq!(spec.id, "test-agent");
    assert_eq!(spec.command, "echo");
}

#[test]
fn test_agent_spec_with_env() {
    let mut env = std::collections::HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin".to_string());
    env.insert("HOME".to_string(), "/home/user".to_string());

    let spec = AgentSpec {
        id: "env-agent".to_string(),
        command: "printenv".to_string(),
        args: vec![],
        allowed_paths: Some(vec!["/tmp".to_string()]),
        env: Some(env),
    };
    assert!(spec.env.is_some());
    assert_eq!(spec.env.as_ref().unwrap().len(), 2);
}

#[test]
fn test_agent_result_construction() {
    let result = AgentResult {
        agent_id: "agent-1".to_string(),
        state: AgentState::Success,
        output: Some("task completed".to_string()),
        error: None,
        branch: Some("feat/test".to_string()),
        changed_files: vec!["src/lib.rs".to_string()],
        duration_ms: 2500,
        run_id: None,
        worktree_path: None,
    };
    assert_eq!(result.state, AgentState::Success);
    assert!(result.duration_ms > 0);
}

#[test]
fn test_agent_result_error_state() {
    let result = AgentResult {
        agent_id: "failing-agent".to_string(),
        state: AgentState::Crashed,
        output: None,
        error: Some("Process exited with code 1".to_string()),
        branch: None,
        changed_files: vec![],
        duration_ms: 500,
        run_id: None,
        worktree_path: None,
    };
    assert_eq!(result.state, AgentState::Crashed);
    assert!(result.error.is_some());
}

#[test]
fn test_agent_state_transitions() {
    let states = vec![
        AgentState::Pending,
        AgentState::Running,
        AgentState::Success,
        AgentState::NoChanges,
        AgentState::Timeout,
        AgentState::Crashed,
        AgentState::Quarantined,
    ];
    assert_eq!(states.len(), 7);
    assert_ne!(states[0], states[1]);
    assert_ne!(states[2], states[4]);
}

#[test]
fn test_agent_result_timeout_state() {
    let result = AgentResult {
        agent_id: "slow-agent".to_string(),
        state: AgentState::Timeout,
        output: None,
        error: Some("Timeout after 30s".to_string()),
        branch: None,
        changed_files: vec![],
        duration_ms: 30000,
        run_id: None,
        worktree_path: None,
    };
    assert_eq!(result.state, AgentState::Timeout);
}

#[test]
fn test_agent_result_no_changes() {
    let result = AgentResult {
        agent_id: "noop-agent".to_string(),
        state: AgentState::NoChanges,
        output: Some("nothing to do".to_string()),
        error: None,
        branch: None,
        changed_files: vec![],
        duration_ms: 100,
        run_id: None,
        worktree_path: None,
    };
    assert_eq!(result.state, AgentState::NoChanges);
    assert!(result.changed_files.is_empty());
}

#[test]
fn test_agent_result_serialization() {
    let result = AgentResult {
        agent_id: "serialize-test".to_string(),
        state: AgentState::Success,
        output: Some("output".to_string()),
        error: None,
        branch: Some("feat/test".to_string()),
        changed_files: vec!["src/main.rs".to_string()],
        duration_ms: 1000,
        run_id: None,
        worktree_path: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("serialize-test"));
    assert!(json.contains("Success"));

    let deserialized: AgentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.agent_id, "serialize-test");
    assert_eq!(deserialized.state, AgentState::Success);
}

#[test]
fn test_subprocess_runner_creation() {
    let runner = SubprocessRunner::new(Duration::from_secs(60));
    assert_eq!(runner.timeout, Duration::from_secs(60));
}

#[test]
fn test_subprocess_runner_default_timeout() {
    let runner = SubprocessRunner::new(Duration::from_secs(300));
    assert!(runner.timeout.as_secs() >= 60);
}

#[test]
fn test_agent_outcome_construction() {
    let outcome = AgentOutcome {
        state: AgentState::Success,
        error: None,
        exit_code: Some(0),
        stdout_path: PathBuf::from("/tmp/stdout.log"),
        stderr_path: PathBuf::from("/tmp/stderr.log"),
        duration: Duration::from_secs(5),
        files_changed: vec![PathBuf::from("src/lib.rs")],
    };
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.duration > Duration::ZERO);
}
