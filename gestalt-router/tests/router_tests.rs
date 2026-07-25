use gestalt_router::run::{
    AgentResult, AgentSpec, AgentStatus, ConflictInfo, ConflictKind, RouterError, RouterErrorKind,
    RunReport, RunSpec,
};
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
        allowed_paths: Some(vec!["/tmp".to_string()]),
        env: Some(HashMap::from([("KEY".to_string(), "VALUE".to_string())])),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test task".to_string(),
        agents: vec![agent],
        timeout: 3600,
        max_parallel: 2,
        push: true,
    };

    let serialized = serde_json::to_string(&spec).unwrap();
    let deserialized: RunSpec = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.base_ref, "main");
    assert_eq!(deserialized.task, "test task");
    assert_eq!(deserialized.agents.len(), 1);
    assert_eq!(deserialized.agents[0].id, "agent-1");
    assert_eq!(deserialized.agents[0].command, "echo");
    assert_eq!(deserialized.agents[0].args[0], "hello");
    assert_eq!(
        deserialized.agents[0].allowed_paths.as_ref().unwrap()[0],
        "/tmp"
    );
    assert_eq!(
        deserialized.agents[0]
            .env
            .as_ref()
            .unwrap()
            .get("KEY")
            .unwrap(),
        "VALUE"
    );
    assert_eq!(deserialized.timeout, 3600);
    assert_eq!(deserialized.max_parallel, 2);
    assert!(deserialized.push);
}

#[test]
fn test_run_manifest() {
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "task".to_string(),
        agents: vec![],
        timeout: 100,
        max_parallel: 1,
        push: false,
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
    let err = RouterError::new(RouterErrorKind::GitError, "Git error: failed to merge", None);
    assert_eq!(err.to_string(), "Git error: failed to merge");

    let err2 = RouterError::new(RouterErrorKind::AgentError, "Agent error: agent crashed", None);
    assert_eq!(err2.to_string(), "Agent error: agent crashed");

    let err3 = RouterError::new(RouterErrorKind::Timeout, "Timeout error", None);
    assert_eq!(err3.to_string(), "Timeout error");

    let err4 = RouterError::new(
        RouterErrorKind::InvalidSpec,
        "Invalid specification: missing command",
        None,
    );
    assert_eq!(err4.to_string(), "Invalid specification: missing command");
}

#[test]
fn test_agent_result_and_report() {
    let result = AgentResult {
        agent_id: "agent-1".to_string(),
        status: AgentStatus::Success,
        exit_code: Some(0),
        changed_files: vec!["src/main.rs".to_string()],
        branch: "agent-branch-1".to_string(),
        duration_ms: 125,
    };

    let conflict = ConflictInfo {
        path: "src/main.rs".to_string(),
        agent_a: "agent-1".to_string(),
        agent_b: "agent-2".to_string(),
        kind: ConflictKind::Overlap,
    };

    let report = RunReport {
        run_id: Uuid::new_v4(),
        base_sha: "abcdef123456".to_string(),
        agents: vec![result],
        integration_branch: "feature-1".to_string(),
        conflicts: vec![conflict],
        events_path: "/tmp/events".to_string(),
        success: true,
    };

    let serialized = serde_json::to_string(&report).unwrap();
    let deserialized: RunReport = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.run_id, report.run_id);
    assert_eq!(deserialized.base_sha, report.base_sha);
    assert_eq!(deserialized.agents[0].agent_id, "agent-1");
    assert_eq!(deserialized.agents[0].status, AgentStatus::Success);
    assert_eq!(deserialized.agents[0].exit_code, Some(0));
    assert_eq!(deserialized.agents[0].changed_files[0], "src/main.rs");
    assert_eq!(deserialized.agents[0].branch, "agent-branch-1");
    assert_eq!(deserialized.agents[0].duration_ms, 125);

    assert_eq!(deserialized.integration_branch, "feature-1");
    assert_eq!(deserialized.conflicts[0].path, "src/main.rs");
    assert_eq!(deserialized.conflicts[0].agent_a, "agent-1");
    assert_eq!(deserialized.conflicts[0].agent_b, "agent-2");
    assert_eq!(deserialized.conflicts[0].kind, ConflictKind::Overlap);
    assert_eq!(deserialized.events_path, "/tmp/events");
    assert!(deserialized.success);
}
