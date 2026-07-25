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

// ==========================================
// INTEGRATION PIPELINE TESTS (GESTALT-06)
// ==========================================

use gestalt_router::integrate::{integrate, AgentIntegrationSpec};
use std::path::Path;
use std::process::Command;

struct TempTestRepo {
    path: std::path::PathBuf,
}

impl TempTestRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gestalt_test_repo_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempTestRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_git_sync(args: &[&str], dir: &Path) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute git");
    assert!(
        output.status.success(),
        "Git command failed: {:?} -> {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn setup_temp_git_repo(repo_path: &Path) -> String {
    run_git_sync(&["init"], repo_path);
    run_git_sync(&["config", "user.name", "Test User"], repo_path);
    run_git_sync(&["config", "user.email", "test@example.com"], repo_path);

    // Create base files
    std::fs::write(repo_path.join("file1.txt"), "Line 1\nLine 2\n").expect("write file1");
    std::fs::write(repo_path.join("file2.txt"), "Apple\nBanana\n").expect("write file2");
    std::fs::write(repo_path.join("file3.txt"), "Cat\nDog\n").expect("write file3");

    run_git_sync(&["add", "file1.txt", "file2.txt", "file3.txt"], repo_path);
    run_git_sync(&["commit", "-m", "initial commit"], repo_path);

    run_git_sync(&["rev-parse", "HEAD"], repo_path)
}

#[tokio::test]
async fn test_integrate_no_conflicts() {
    let repo = TempTestRepo::new();
    let base_sha = setup_temp_git_repo(&repo.path);

    // Create Agent A branch - modifies file1.txt
    run_git_sync(&["checkout", "-b", "branch_agent_a"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent A\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent A changes"], &repo.path);

    // Go back to master, create Agent B branch - modifies file2.txt
    run_git_sync(&["checkout", "master"], &repo.path);
    run_git_sync(&["checkout", "-b", "branch_agent_b"], &repo.path);
    std::fs::write(repo.path.join("file2.txt"), "Apple\nBanana\nModified by Agent B\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent B changes"], &repo.path);

    // We can also have an Agent C branch with NO changes, which should sort first
    run_git_sync(&["checkout", "master"], &repo.path);
    run_git_sync(&["checkout", "-b", "branch_agent_c"], &repo.path);
    run_git_sync(&["commit", "--allow-empty", "-m", "Agent C empty changes"], &repo.path);

    let run_id = Uuid::new_v4();
    let agents = vec![
        AgentIntegrationSpec {
            id: "agent_b".to_string(),
            branch: "branch_agent_b".to_string(),
        },
        AgentIntegrationSpec {
            id: "agent_a".to_string(),
            branch: "branch_agent_a".to_string(),
        },
        AgentIntegrationSpec {
            id: "agent_c".to_string(),
            branch: "branch_agent_c".to_string(),
        },
    ];

    // Run integrate using the fast in-memory path (force_fallback: false)
    let result = integrate(&base_sha, &agents, run_id, false, Some(&repo.path))
        .await
        .unwrap();

    // Verify results
    assert!(!result.merge_sha.is_empty());
    assert_eq!(result.merged_branches.len(), 3);
    // Because Agent C has 0 changes, Agent A and B have 1 change each.
    // They are sorted as: agent_c (0 changes) -> agent_a (1 change) -> agent_b (1 change)
    // (agent_a comes before agent_b due to alphabetical tie-breaking on id)
    assert_eq!(result.merged_branches[0], "branch_agent_c");
    assert_eq!(result.merged_branches[1], "branch_agent_a");
    assert_eq!(result.merged_branches[2], "branch_agent_b");
    assert!(result.conflicts.is_empty());

    // Verify that the integration ref was created and has the correct parents
    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let resolved_sha = run_git_sync(&["rev-parse", &ref_name], &repo.path);
    assert_eq!(resolved_sha, result.merge_sha);

    // Parents of the octopus merge commit: base_sha, branch_agent_c, branch_agent_a, branch_agent_b
    let parents_stdout = run_git_sync(&["rev-list", "--parents", "-n", "1", &resolved_sha], &repo.path);
    let parents: Vec<&str> = parents_stdout.split_whitespace().collect();
    // Format is "commit parent1 parent2 parent3 ..."
    assert_eq!(parents.len(), 5); // commit + 4 parents

    // Verify file contents on the merge commit
    // Checkout user working tree should remain untouched (we were on branch_agent_c before integrate,
    // let's verify we're still on branch_agent_c and index/working tree is untouched)
    let current_branch = run_git_sync(&["rev-parse", "--abbrev-ref", "HEAD"], &repo.path);
    assert_eq!(current_branch, "branch_agent_c");

    // Let's verify files on the generated merge tree
    let file1_content = run_git_sync(&["show", &format!("{}:file1.txt", result.merge_sha)], &repo.path);
    assert_eq!(file1_content, "Line 1\nLine 2\nModified by Agent A");

    let file2_content = run_git_sync(&["show", &format!("{}:file2.txt", result.merge_sha)], &repo.path);
    assert_eq!(file2_content, "Apple\nBanana\nModified by Agent B");
}

#[tokio::test]
async fn test_integrate_with_conflicts() {
    let repo = TempTestRepo::new();
    let base_sha = setup_temp_git_repo(&repo.path);

    // Create Agent A branch - modifies file1.txt
    run_git_sync(&["checkout", "-b", "branch_agent_a"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent A\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent A changes"], &repo.path);

    // Go back to master, create Agent B branch - modifies file1.txt in a conflicting way
    run_git_sync(&["checkout", "master"], &repo.path);
    run_git_sync(&["checkout", "-b", "branch_agent_b"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent B with conflicting content\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent B conflicting changes"], &repo.path);

    let run_id = Uuid::new_v4();
    let agents = vec![
        AgentIntegrationSpec {
            id: "agent_b".to_string(),
            branch: "branch_agent_b".to_string(),
        },
        AgentIntegrationSpec {
            id: "agent_a".to_string(),
            branch: "branch_agent_a".to_string(),
        },
    ];

    // Run integrate
    let result = integrate(&base_sha, &agents, run_id, false, Some(&repo.path))
        .await
        .unwrap();

    // Verify results
    assert!(!result.merge_sha.is_empty());
    // agent_a was merged successfully. agent_b conflicted and was skipped.
    assert_eq!(result.merged_branches.len(), 1);
    assert_eq!(result.merged_branches[0], "branch_agent_a");

    // Conflict should be reported for agent_b on file1.txt
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].agent_id, "agent_b");
    assert_eq!(result.conflicts[0].path, "file1.txt");

    // Verify that the integration ref was created and points to the merge_sha
    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let resolved_sha = run_git_sync(&["rev-parse", &ref_name], &repo.path);
    assert_eq!(resolved_sha, result.merge_sha);

    // Parents of the octopus merge commit: base_sha, branch_agent_a (since agent_b failed/was skipped)
    let parents_stdout = run_git_sync(&["rev-list", "--parents", "-n", "1", &resolved_sha], &repo.path);
    let parents: Vec<&str> = parents_stdout.split_whitespace().collect();
    assert_eq!(parents.len(), 3); // commit + 2 parents (base_sha and agent_a)

    // User's working tree current branch should still be branch_agent_b and untouched
    let current_branch = run_git_sync(&["rev-parse", "--abbrev-ref", "HEAD"], &repo.path);
    assert_eq!(current_branch, "branch_agent_b");
}

#[tokio::test]
async fn test_integrate_fallback_no_conflicts() {
    let repo = TempTestRepo::new();
    let base_sha = setup_temp_git_repo(&repo.path);

    // Create Agent A branch - modifies file1.txt
    run_git_sync(&["checkout", "-b", "branch_agent_a"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent A\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent A changes"], &repo.path);

    // Go back to master, create Agent B branch - modifies file2.txt
    run_git_sync(&["checkout", "master"], &repo.path);
    run_git_sync(&["checkout", "-b", "branch_agent_b"], &repo.path);
    std::fs::write(repo.path.join("file2.txt"), "Apple\nBanana\nModified by Agent B\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent B changes"], &repo.path);

    let run_id = Uuid::new_v4();
    let agents = vec![
        AgentIntegrationSpec {
            id: "agent_b".to_string(),
            branch: "branch_agent_b".to_string(),
        },
        AgentIntegrationSpec {
            id: "agent_a".to_string(),
            branch: "branch_agent_a".to_string(),
        },
    ];

    // Run integrate using the FALLBACK path (force_fallback: true)
    let result = integrate(&base_sha, &agents, run_id, true, Some(&repo.path))
        .await
        .unwrap();

    // Verify results
    assert!(!result.merge_sha.is_empty());
    assert_eq!(result.merged_branches.len(), 2);
    assert_eq!(result.merged_branches[0], "branch_agent_a");
    assert_eq!(result.merged_branches[1], "branch_agent_b");
    assert!(result.conflicts.is_empty());

    // Verify integration ref
    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let resolved_sha = run_git_sync(&["rev-parse", &ref_name], &repo.path);
    assert_eq!(resolved_sha, result.merge_sha);

    // Verify file contents on the merge commit
    let file1_content = run_git_sync(&["show", &format!("{}:file1.txt", result.merge_sha)], &repo.path);
    assert_eq!(file1_content, "Line 1\nLine 2\nModified by Agent A");

    let file2_content = run_git_sync(&["show", &format!("{}:file2.txt", result.merge_sha)], &repo.path);
    assert_eq!(file2_content, "Apple\nBanana\nModified by Agent B");
}

#[tokio::test]
async fn test_integrate_fallback_with_conflicts() {
    let repo = TempTestRepo::new();
    let base_sha = setup_temp_git_repo(&repo.path);

    // Create Agent A branch - modifies file1.txt
    run_git_sync(&["checkout", "-b", "branch_agent_a"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent A\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent A changes"], &repo.path);

    // Go back to master, create Agent B branch - modifies file1.txt in a conflicting way
    run_git_sync(&["checkout", "master"], &repo.path);
    run_git_sync(&["checkout", "-b", "branch_agent_b"], &repo.path);
    std::fs::write(repo.path.join("file1.txt"), "Line 1\nLine 2\nModified by Agent B with conflicting content\n").unwrap();
    run_git_sync(&["commit", "-am", "Agent B conflicting changes"], &repo.path);

    let run_id = Uuid::new_v4();
    let agents = vec![
        AgentIntegrationSpec {
            id: "agent_b".to_string(),
            branch: "branch_agent_b".to_string(),
        },
        AgentIntegrationSpec {
            id: "agent_a".to_string(),
            branch: "branch_agent_a".to_string(),
        },
    ];

    // Run integrate using the FALLBACK path (force_fallback: true)
    let result = integrate(&base_sha, &agents, run_id, true, Some(&repo.path))
        .await
        .unwrap();

    // Verify results
    assert!(!result.merge_sha.is_empty());
    assert_eq!(result.merged_branches.len(), 1);
    assert_eq!(result.merged_branches[0], "branch_agent_a");

    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].agent_id, "agent_b");
    assert_eq!(result.conflicts[0].path, "file1.txt");

    // Verify integration ref
    let ref_name = format!("refs/heads/gestalt/run_{}", run_id);
    let resolved_sha = run_git_sync(&["rev-parse", &ref_name], &repo.path);
    assert_eq!(resolved_sha, result.merge_sha);
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
