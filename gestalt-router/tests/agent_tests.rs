use gestalt_router::agent::{SubprocessRunner, AgentOutcome};
use gestalt_router::run::{AgentResult, AgentSpec, RunSpec};
use gestalt_router::run_state::AgentState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_agent_spec_construction() {
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
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin".to_string());
    let spec = AgentSpec {
        id: "env-agent".to_string(),
        command: "printenv".to_string(),
        args: vec![],
        allowed_paths: Some(vec!["/tmp".to_string()]),
        env: Some(env),
    };
    assert!(spec.env.is_some());
    assert_eq!(spec.env.as_ref().unwrap().len(), 1);
}

#[test]
fn test_agent_result_success() {
    let result = AgentResult {
        agent_id: "success-agent".to_string(),
        state: AgentState::Success,
        output: Some("done".to_string()),
        error: None,
        branch: Some("feat/test".to_string()),
        changed_files: vec!["src/lib.rs".to_string()],
        duration_ms: 1500,
        run_id: None,
        worktree_path: None,
    };
    assert_eq!(result.state, AgentState::Success);
    assert_eq!(result.changed_files.len(), 1);
}

#[test]
fn test_agent_result_crashed() {
    let result = AgentResult {
        agent_id: "fail-agent".to_string(),
        state: AgentState::Crashed,
        output: None,
        error: Some("exit code 1".to_string()),
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
fn test_agent_result_timeout() {
    let result = AgentResult {
        agent_id: "slow".to_string(),
        state: AgentState::Timeout,
        output: None,
        error: Some("timeout".to_string()),
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
        agent_id: "noop".to_string(),
        state: AgentState::NoChanges,
        output: Some("nothing".to_string()),
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
        agent_id: "ser-test".to_string(),
        state: AgentState::Success,
        output: Some("data".to_string()),
        error: None,
        branch: Some("feat/x".to_string()),
        changed_files: vec!["main.rs".to_string()],
        duration_ms: 1000,
        run_id: None,
        worktree_path: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("ser-test"));
    let deser: AgentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.agent_id, "ser-test");
}

#[test]
fn test_run_spec_with_integration_branch() {
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test".to_string(),
        agents: vec![],
        max_parallel: 2,
        timeout: 60,
        push: false,
        integration_branch: Some("develop".to_string()),
    };
    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("\"integration_branch\":\"develop\""));
    assert!(json.contains("\"push\":false"));
}

#[test]
fn test_run_spec_without_integration_branch() {
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test".to_string(),
        agents: vec![],
        max_parallel: 1,
        timeout: 30,
        push: true,
        integration_branch: None,
    };
    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("\"integration_branch\":null"));
    assert!(json.contains("\"push\":true"));
}

#[test]
fn test_subprocess_runner_creation() {
    let runner = SubprocessRunner::new(Duration::from_secs(60));
    assert_eq!(runner.timeout, Duration::from_secs(60));
}

#[test]
fn test_subprocess_runner_default() {
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
}

#[test]
fn test_agent_outcome_with_error() {
    let outcome = AgentOutcome {
        state: AgentState::Crashed,
        error: Some("process failed".to_string()),
        exit_code: Some(1),
        stdout_path: PathBuf::from("/tmp/out.log"),
        stderr_path: PathBuf::from("/tmp/err.log"),
        duration: Duration::from_secs(2),
        files_changed: vec![],
    };
    assert_eq!(outcome.exit_code, Some(1));
    assert!(outcome.error.is_some());
}

#[test]
fn test_all_agent_states() {
    use AgentState::*;
    let states = vec![Pending, Running, Success, NoChanges, Timeout, Crashed, Quarantined];
    assert_eq!(states.len(), 7);
    assert_ne!(Pending, Running);
    assert_ne!(Running, Success);
}

#[test]
fn test_multi_agent_specs() {
    let agents: Vec<AgentSpec> = (0..3)
        .map(|i| AgentSpec {
            id: format!("agent-{}", i),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            allowed_paths: None,
            env: None,
        })
        .collect();
    assert_eq!(agents.len(), 3);
    assert_eq!(agents[0].id, "agent-0");
}
