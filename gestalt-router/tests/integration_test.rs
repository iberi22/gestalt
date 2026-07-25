use gestalt_router::integrate::{AgentIntegrationSpec, IntegrateResult};
use gestalt_router::overlap::OverlapInfo;
use gestalt_router::run::{AgentResult, AgentSpec, RunReport, RunSpec};
use gestalt_router::run_state::{AgentState, RunManifest};
use gestalt_router::timeline::{Event, EventLog, JsonlEventLog, VersionedEvent};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn test_full_pipeline_agent_spec_construction() {
    let agent = AgentSpec {
        id: "test-agent".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        allowed_paths: Some(vec!["/tmp".to_string()]),
        env: Some({
            let mut map = HashMap::new();
            map.insert("PATH".to_string(), "/usr/bin".to_string());
            map
        }),
    };
    assert_eq!(agent.id, "test-agent");
    assert_eq!(agent.command, "echo");
}

#[test]
fn test_run_spec_construction() {
    let agent = AgentSpec {
        id: "agent-1".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        allowed_paths: None,
        env: None,
    };
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test orchestration".to_string(),
        agents: vec![agent],
        max_parallel: 2,
        timeout: 60,
        push: false,
        integration_branch: Some("integration".to_string()),
    };
    assert_eq!(spec.task, "test orchestration");
    assert_eq!(spec.agents.len(), 1);
    assert_eq!(spec.max_parallel, 2);
    assert_eq!(spec.integration_branch, Some("integration".to_string()));
}

#[test]
fn test_multi_agent_specs() {
    let agents: Vec<AgentSpec> = (0..3)
        .map(|i| AgentSpec {
            id: format!("agent-{}", i),
            command: "echo".to_string(),
            args: vec![format!("hello {}", i)],
            allowed_paths: None,
            env: None,
        })
        .collect();
    assert_eq!(agents.len(), 3);
    assert_eq!(agents[1].id, "agent-1");
}

#[test]
fn test_agent_result_builder() {
    let result = AgentResult {
        agent_id: "builder".to_string(),
        state: AgentState::Success,
        output: Some("task completed".to_string()),
        error: None,
        branch: Some("feat/builder".to_string()),
        changed_files: vec!["src/lib.rs".to_string()],
        duration_ms: 5000,
        run_id: Some(Uuid::new_v4()),
        worktree_path: Some("/tmp/wt".to_string()),
    };
    assert_eq!(result.state, AgentState::Success);
    assert!(!result.changed_files.is_empty());
}

#[test]
fn test_run_manifest_with_states() {
    let mut agent_states = HashMap::new();
    agent_states.insert("a1".to_string(), AgentState::Pending);
    agent_states.insert("a2".to_string(), AgentState::Running);

    let manifest = RunManifest {
        run_id: Uuid::new_v4(),
        spec: RunSpec {
            base_ref: "main".to_string(),
            task: "t".to_string(),
            agents: vec![],
            max_parallel: 1,
            timeout: 30,
            push: false,
            integration_branch: None,
        },
        agent_states,
    };
    assert_eq!(manifest.agent_states.len(), 2);
    assert_eq!(manifest.agent_states["a1"], AgentState::Pending);
}

#[test]
fn test_event_serialization_roundtrip() {
    let run_id = Uuid::new_v4();
    let event = Event::RunStarted {
        run_id,
        task: "test".to_string(),
        agents: vec!["agent-1".to_string()],
        sha_base: "abc123".to_string(),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: Event = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}

#[test]
fn test_versioned_event_wrapper() {
    let event = Event::RunFinished {
        run_id: Uuid::new_v4(),
        summary: "completed".to_string(),
    };
    let versioned = VersionedEvent { v: 1, event };
    assert_eq!(versioned.v, 1);
    let json = serde_json::to_string(&versioned).unwrap();
    assert!(json.contains("RunFinished"));
}

#[test]
fn test_event_log_creation() {
    let run_id = Uuid::new_v4();
    let log = JsonlEventLog::new(run_id);
    assert!(log.is_ok(), "JsonlEventLog should be creatable");
}

#[test]
fn test_event_log_read_events() {
    let run_id = Uuid::new_v4();
    let log = JsonlEventLog::new(run_id).unwrap();
    let events = log.read_events(run_id);
    assert!(events.is_ok(), "Should be able to read events");
}

#[test]
fn test_integrate_result_construction() {
    let result = IntegrateResult {
        merge_sha: "abc123def".to_string(),
        merged_branches: vec!["feat/a".to_string(), "feat/b".to_string()],
        conflicts: vec![],
    };
    assert_eq!(result.merged_branches.len(), 2);
    assert_eq!(result.merge_sha.len(), 9);
}

#[test]
fn test_integrate_result_with_conflicts() {
    let result = IntegrateResult {
        merge_sha: String::new(),
        merged_branches: vec![],
        conflicts: vec![gestalt_router::run::ConflictInfo {
            agent_id: "agent-a".to_string(),
            path: "Cargo.toml".to_string(),
        }],
    };
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].path, "Cargo.toml");
}

#[test]
fn test_agent_integration_spec() {
    let spec = AgentIntegrationSpec {
        id: "test-agent".to_string(),
        branch: "feat/test".to_string(),
    };
    assert_eq!(spec.id, "test-agent");
    assert_eq!(spec.branch, "feat/test");
}

#[test]
fn test_overlap_info_with_multiple_files() {
    let info = OverlapInfo {
        agent_a: "agent-1".to_string(),
        agent_b: "agent-2".to_string(),
        files: vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")],
    };
    assert_eq!(info.files.len(), 2);
    assert!(info.files.contains(&PathBuf::from("Cargo.toml")));
}

#[test]
fn test_worktree_manager_creation() {
    let wt = gestalt_router::worktree::WorktreeManager::new(PathBuf::from("/tmp/gestalt"));
    assert_eq!(wt.base_dir, PathBuf::from("/tmp/gestalt"));
}

#[test]
fn test_run_report_with_agents_and_conflicts() {
    let report = RunReport {
        run_id: Uuid::new_v4(),
        agents: vec![AgentResult {
            agent_id: "a1".to_string(),
            state: AgentState::Success,
            output: None,
            error: None,
            branch: None,
            changed_files: vec![],
            duration_ms: 100,
            run_id: None,
            worktree_path: None,
        }],
        merged_branches: vec!["feat/a1".to_string()],
        conflicts: vec![],
        events_path: "/tmp/run.jsonl".to_string(),
        success: true,
    };
    assert_eq!(report.agents.len(), 1);
    assert!(report.success);
}
