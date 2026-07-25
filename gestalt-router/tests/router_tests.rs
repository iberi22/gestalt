use gestalt_router::run::{AgentResult, AgentSpec, RouterError, RunReport, RunSpec};
use gestalt_router::run_state::{AgentState, RunManifest};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn test_agent_state_serialization() {
    let state = AgentState::Pending;
    let serialized = serde_json::to_string(&state).unwrap();
    assert_eq!(serialized, "\"Pending\"");

    let deserialized: AgentState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, AgentState::Pending);
}

#[test]
fn test_run_spec_serialization() {
    let agent = AgentSpec {
        id: "agent-1".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        allowed_paths: vec!["/tmp".to_string()],
        env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test task".to_string(),
        agents: vec![agent],
        integration_branch: "integration-1".to_string(),
        timeout: 3600,
        max_parallel: 2,
    };

    let serialized = serde_json::to_string(&spec).unwrap();
    let deserialized: RunSpec = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.base_ref, "main");
    assert_eq!(deserialized.task, "test task");
    assert_eq!(deserialized.agents.len(), 1);
    assert_eq!(deserialized.agents[0].id, "agent-1");
    assert_eq!(deserialized.agents[0].command, "echo");
    assert_eq!(deserialized.agents[0].args[0], "hello");
    assert_eq!(deserialized.agents[0].allowed_paths[0], "/tmp");
    assert_eq!(deserialized.agents[0].env.get("KEY").unwrap(), "VALUE");
    assert_eq!(deserialized.integration_branch, "integration-1");
    assert_eq!(deserialized.timeout, 3600);
    assert_eq!(deserialized.max_parallel, 2);
}

#[test]
fn test_run_manifest() {
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "task".to_string(),
        agents: vec![],
        integration_branch: "int".to_string(),
        timeout: 100,
        max_parallel: 1,
    };

    let run_id = Uuid::new_v4();
    let mut agent_states = HashMap::new();
    agent_states.insert("agent-1".to_string(), AgentState::Running);

    let manifest = RunManifest {
        run_id,
        spec,
        agent_states,
    };

    let serialized = serde_json::to_string(&manifest).unwrap();
    let deserialized: RunManifest = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.run_id, run_id);
    assert_eq!(
        deserialized.agent_states.get("agent-1").unwrap(),
        &AgentState::Running
    );
}

#[test]
fn test_router_error_display() {
    let err = RouterError::GitError("failed to merge".to_string());
    assert_eq!(err.to_string(), "Git error: failed to merge");

    let err2 = RouterError::AgentError("agent crashed".to_string());
    assert_eq!(err2.to_string(), "Agent error: agent crashed");

    let err3 = RouterError::Timeout;
    assert_eq!(err3.to_string(), "Timeout error");

    let err4 = RouterError::InvalidSpec("missing command".to_string());
    assert_eq!(err4.to_string(), "Invalid specification: missing command");
}

#[test]
fn test_agent_result_and_report() {
    let result = AgentResult {
        agent_id: "agent-1".to_string(),
        state: AgentState::Success,
        output: Some("done".to_string()),
        error: None,
    };

    let report = RunReport {
        run_id: Uuid::new_v4(),
        agents: vec![result],
        merged_branches: vec!["feature-1".to_string()],
        conflicts: vec![],
        events_path: "/tmp/events".to_string(),
    };

    assert_eq!(report.agents[0].agent_id, "agent-1");
    assert_eq!(report.agents[0].state, AgentState::Success);
    assert_eq!(report.merged_branches[0], "feature-1");
    assert_eq!(report.events_path, "/tmp/events");
}

use gestalt_router::process::{ProcessManager, RunStatus};

#[tokio::test]
async fn test_process_manager_mock_success() {
    let pm = ProcessManager::new();

    let agent1 = AgentSpec {
        id: "agent-1".to_string(),
        command: "mock_success".to_string(),
        args: vec!["Hello from agent 1".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let agent2 = AgentSpec {
        id: "agent-2".to_string(),
        command: "mock_success".to_string(),
        args: vec!["Hello from agent 2".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "successful task".to_string(),
        agents: vec![agent1, agent2],
        integration_branch: "int-branch".to_string(),
        timeout: 10,
        max_parallel: 2,
    };

    let handle = pm.start_run(spec);

    // Initial status should be Running (since tokio spawns it instantly) or Pending
    let current_status = handle.status();
    assert!(
        current_status == RunStatus::Pending || current_status == RunStatus::Running,
        "Status should be Pending or Running, got {:?}",
        current_status
    );

    let report = handle.await_completion().await.unwrap();

    assert_eq!(handle.status(), RunStatus::Completed);
    assert_eq!(report.agents.len(), 2);

    let agent1_res = report.agents.iter().find(|a| a.agent_id == "agent-1").unwrap();
    assert_eq!(agent1_res.state, AgentState::Success);
    assert_eq!(agent1_res.output.as_deref(), Some("Hello from agent 1"));

    let agent2_res = report.agents.iter().find(|a| a.agent_id == "agent-2").unwrap();
    assert_eq!(agent2_res.state, AgentState::Success);
    assert_eq!(agent2_res.output.as_deref(), Some("Hello from agent 2"));

    // Check manifest too
    let manifest = handle.manifest();
    assert_eq!(manifest.agent_states.get("agent-1").unwrap(), &AgentState::Success);
    assert_eq!(manifest.agent_states.get("agent-2").unwrap(), &AgentState::Success);
}

#[tokio::test]
async fn test_process_manager_cancel() {
    let pm = ProcessManager::new();

    let agent = AgentSpec {
        id: "sleepy-agent".to_string(),
        command: "mock_sleep".to_string(),
        args: vec!["5000".to_string()], // Sleep for 5 seconds
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "cancellable task".to_string(),
        agents: vec![agent],
        integration_branch: "int-branch".to_string(),
        timeout: 10,
        max_parallel: 1,
    };

    let handle = pm.start_run(spec);

    // Yield control to let background tasks start running
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(handle.status(), RunStatus::Running);

    // Cancel the run
    handle.cancel();

    // Awaiting completion should fail or return cancelled error quickly
    let result = handle.await_completion().await;
    assert!(result.is_err());
    assert_eq!(handle.status(), RunStatus::Cancelled);

    // Agent state in the manifest should be Crashed (due to cancellation)
    let manifest = handle.manifest();
    assert_eq!(manifest.agent_states.get("sleepy-agent").unwrap(), &AgentState::Crashed);
}

#[tokio::test]
async fn test_process_manager_global_timeout() {
    let pm = ProcessManager::new();

    let agent = AgentSpec {
        id: "sleepy-agent".to_string(),
        command: "mock_sleep".to_string(),
        args: vec!["5000".to_string()], // Sleep for 5 seconds
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "timeout task".to_string(),
        agents: vec![agent],
        integration_branch: "int-branch".to_string(),
        timeout: 1, // Global timeout of 1 second
        max_parallel: 1,
    };

    let handle = pm.start_run(spec);

    // Awaiting completion should return timeout error after ~1 second
    let result = handle.await_completion().await;
    assert!(result.is_err(), "Expected timeout error, got Ok");

    match result.unwrap_err() {
        RouterError::Timeout => {}
        err => panic!("Expected RouterError::Timeout, got {:?}", err),
    }

    assert_eq!(handle.status(), RunStatus::Failed);

    // Agent state in the manifest should be Timeout
    let manifest = handle.manifest();
    assert_eq!(manifest.agent_states.get("sleepy-agent").unwrap(), &AgentState::Timeout);
}

#[tokio::test]
async fn test_process_manager_max_parallel() {
    let pm = ProcessManager::new();

    let agent1 = AgentSpec {
        id: "agent-1".to_string(),
        command: "mock_sleep".to_string(),
        args: vec!["100".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let agent2 = AgentSpec {
        id: "agent-2".to_string(),
        command: "mock_sleep".to_string(),
        args: vec!["100".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "parallel task".to_string(),
        agents: vec![agent1, agent2],
        integration_branch: "int-branch".to_string(),
        timeout: 10,
        max_parallel: 1, // Sequential execution
    };

    let start_time = std::time::Instant::now();
    let handle = pm.start_run(spec);
    let report = handle.await_completion().await.unwrap();
    let duration = start_time.elapsed();

    assert_eq!(handle.status(), RunStatus::Completed);
    assert_eq!(report.agents.len(), 2);
    // Since each sleeps 100ms and max_parallel is 1, they must run sequentially and take >= 200ms
    assert!(
        duration >= std::time::Duration::from_millis(200),
        "Expected sequential execution to take at least 200ms, took {:?}",
        duration
    );
}
