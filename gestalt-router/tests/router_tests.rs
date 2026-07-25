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

use gestalt_router::timeline::{Event, EventLog, JsonlEventLog, VersionedEvent};
use std::fs::File;
use std::io::Write;

fn get_test_temp_dir() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("gestalt_test_{}", Uuid::new_v4()));
    path
}

#[test]
fn test_event_serialization_schema_and_adjacent_tagging() {
    let event = Event::AgentStateChanged {
        agent_id: "agent-1".to_string(),
        state: AgentState::Running,
    };
    let versioned = VersionedEvent { v: 1, event };

    let serialized = serde_json::to_string(&versioned).unwrap();
    // It must contain `"v":1`, `"type":"AgentStateChanged"`, and `"payload"`
    assert!(serialized.contains("\"v\":1"));
    assert!(serialized.contains("\"type\":\"AgentStateChanged\""));
    assert!(serialized.contains("\"payload\""));
    assert!(serialized.contains("\"agent_id\":\"agent-1\""));

    let deserialized: VersionedEvent = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.v, 1);
    match deserialized.event {
        Event::AgentStateChanged { agent_id, state } => {
            assert_eq!(agent_id, "agent-1");
            assert_eq!(state, AgentState::Running);
        }
        _ => panic!("Expected AgentStateChanged variant"),
    }
}

#[test]
fn test_jsonl_event_log_roundtrip_and_list_runs() {
    let temp_dir = get_test_temp_dir();
    let run_id = Uuid::new_v4();

    let log = JsonlEventLog::new_with_dir(run_id, temp_dir.clone()).unwrap();

    // Appending a few events
    log.append(Event::RunStarted).unwrap();
    log.append(Event::AgentStateChanged {
        agent_id: "agent-abc".to_string(),
        state: AgentState::Success,
    }).unwrap();
    log.append(Event::RunFinished).unwrap();

    // Read events back and verify
    let events = log.read_events(run_id).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], Event::RunStarted);
    assert!(matches!(events[1], Event::AgentStateChanged { .. }));
    assert_eq!(events[2], Event::RunFinished);

    // List runs and verify
    let runs = log.list_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0], run_id);

    // Clean up
    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn test_jsonl_event_log_truncation_recovery() {
    let temp_dir = get_test_temp_dir();
    let run_id = Uuid::new_v4();

    // Setup: manually write some valid events and a truncated/malformed last line
    let run_dir = temp_dir.join(run_id.to_string());
    std::fs::create_dir_all(&run_dir).unwrap();
    let file_path = run_dir.join("events.jsonl");

    let mut file = File::create(&file_path).unwrap();

    // Line 1: Valid event
    let ev1 = VersionedEvent { v: 1, event: Event::RunStarted };
    let ser1 = serde_json::to_string(&ev1).unwrap();
    writeln!(file, "{}", ser1).unwrap();

    // Line 2: Valid event
    let ev2 = VersionedEvent { v: 1, event: Event::SymlinkEscape { path: "/etc/passwd".to_string() } };
    let ser2 = serde_json::to_string(&ev2).unwrap();
    writeln!(file, "{}", ser2).unwrap();

    // Line 3: Truncated last line (not valid JSON)
    writeln!(file, "{{\"v\":1,\"type\":\"RunFinished\"").unwrap(); // missing closing brace

    // Initialize log and read events
    let log = JsonlEventLog::new_with_dir(run_id, temp_dir.clone()).unwrap();
    let events = log.read_events(run_id).unwrap();

    // It should tolerate the truncated last line, log a warning, and successfully return the first 2 events!
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], Event::RunStarted);
    assert_eq!(events[1], Event::SymlinkEscape { path: "/etc/passwd".to_string() });

    // Clean up
    let _ = std::fs::remove_dir_all(temp_dir);
}
