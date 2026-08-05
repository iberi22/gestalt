use gestalt_router::integrate::{integrate_branches, AgentIntegrationSpec, IntegrateResult};
use gestalt_router::overlap::OverlapInfo;
use gestalt_router::run::{AgentResult, AgentSpec, RunReport, RunSpec};
use gestalt_router::run_state::{AgentState, MemState};
use gestalt_router::timeline::{Event, EventLog, StateDbEventLog, VersionedEvent};
use gestalt_state::StateDb;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use uuid::Uuid;

fn init_test_repo() -> TempDir {
    let dir = TempDir::new();
    let repo_path = dir.path().to_path_buf();

    Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Gestalt Tester"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "tester@gestalt.local"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    std::fs::write(repo_path.join("README.md"), "# Test").unwrap();
    Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["branch", "-m", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    dir
}

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
fn test_memstate_agent_state_tracking() {
    let mem = MemState::new();
    let run_id = Uuid::new_v4().to_string();
    mem.set_agent_state(&run_id, "a1", "Pending");
    mem.set_agent_state(&run_id, "a2", "Running");

    assert_eq!(
        mem.get_agent_state(&run_id, "a1").as_deref(),
        Some("Pending")
    );
    assert_eq!(
        mem.get_agent_state(&run_id, "a2").as_deref(),
        Some("Running")
    );
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
fn test_state_db_event_log_creation() {
    let tmp = TempDir::new();
    let db_path = tmp.path().join("test.db");
    let db = Arc::new(StateDb::open(&db_path).unwrap());
    let run_id = Uuid::new_v4();
    let _log = StateDbEventLog::new(db, run_id);
    // Construction succeeds without panic/error
    assert!(db_path.exists());
}

#[test]
fn test_state_db_event_log_read_events() {
    let tmp = TempDir::new();
    let db_path = tmp.path().join("test.db");
    let db = Arc::new(StateDb::open(&db_path).unwrap());
    let run_id = Uuid::new_v4();
    let log = StateDbEventLog::new(db, run_id);
    let events = log.read_events(run_id);
    assert!(events.is_ok(), "Should be able to read events");
    assert!(
        events.unwrap().is_empty(),
        "Expected empty events for untouched run"
    );
}

#[test]
fn test_integrate_result_construction() {
    let result = IntegrateResult {
        merge_sha: "abc123de".to_string(),
        merged_branches: vec!["feat/a".to_string(), "feat/b".to_string()],
        conflicts: vec![],
    };
    assert_eq!(result.merged_branches.len(), 2);
    assert_eq!(result.merge_sha.len(), 8);
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
        task: "test task".into(),
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
        duration_ms: 100,
        merged_branches: vec!["feat/a1".to_string()],
        conflicts: vec![],
        events_path: "/tmp/run.jsonl".to_string(),
        success: true,
    };
    assert_eq!(report.agents.len(), 1);
    assert!(report.success);
}

#[tokio::test]
async fn test_integrate_branches_success_and_conflict() {
    // 1. Initialize git repo
    let dir = init_test_repo();
    let repo_path = dir.path().to_path_buf();

    // 2. Create branch_a modifying file_a.txt
    Command::new("git")
        .args(["checkout", "-b", "branch_a"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::fs::write(repo_path.join("file_a.txt"), "content a").unwrap();
    Command::new("git")
        .args(["add", "file_a.txt"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "commit a"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // Get the SHA of branch_a
    let sha_a = Command::new("git")
        .args(["rev-parse", "branch_a"])
        .current_dir(&repo_path)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap();

    // Go back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // 3. Create branch_b modifying file_b.txt
    Command::new("git")
        .args(["checkout", "-b", "branch_b"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::fs::write(repo_path.join("file_b.txt"), "content b").unwrap();
    Command::new("git")
        .args(["add", "file_b.txt"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "commit b"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    let sha_b = Command::new("git")
        .args(["rev-parse", "branch_b"])
        .current_dir(&repo_path)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap();

    // Go back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    let base_sha = Command::new("git")
        .args(["rev-parse", "main"])
        .current_dir(&repo_path)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap();

    // 4. Run integrate_branches for branch_a and branch_b
    let branches = vec![
        ("agent_a".to_string(), sha_a.clone()),
        ("agent_b".to_string(), sha_b.clone()),
    ];

    let result = integrate_branches(&repo_path, &base_sha, "integration", &branches).await.unwrap();
    assert!(
        !result.merge_sha.is_empty(),
        "Should produce a merge SHA, but got: {:?}",
        result
    );
    assert_eq!(result.merged_branches.len(), 2);
    assert!(result.conflicts.is_empty(), "Should have no conflicts");

    // 5. Test binary conflict detection
    // Go back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // Create binary file with null bytes in branch_c
    Command::new("git")
        .args(["checkout", "-b", "branch_c"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::fs::write(repo_path.join("binary.bin"), b"binary\0content\0c").unwrap();
    Command::new("git")
        .args(["add", "binary.bin"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "commit c"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    let sha_c = Command::new("git")
        .args(["rev-parse", "branch_c"])
        .current_dir(&repo_path)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap();

    // Go back to main
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // Create conflicting binary file with null bytes in branch_d
    Command::new("git")
        .args(["checkout", "-b", "branch_d"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::fs::write(
        repo_path.join("binary.bin"),
        b"different\0binary\0content\0d",
    )
    .unwrap();
    Command::new("git")
        .args(["add", "binary.bin"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "commit d"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    let sha_d = Command::new("git")
        .args(["rev-parse", "branch_d"])
        .current_dir(&repo_path)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap();

    let binary_branches = vec![
        ("agent_c".to_string(), sha_c),
        ("agent_d".to_string(), sha_d),
    ];

    let result_conflict = integrate_branches(
        &repo_path,
        &base_sha,
        "integration_conflict",
        &binary_branches,
    )
    .await
    .unwrap();
    assert!(
        result_conflict.merge_sha.is_empty(),
        "Should not produce a merge SHA on conflict"
    );
    assert_eq!(
        result_conflict.conflicts.len(),
        1,
        "Should report one conflict"
    );
    assert_eq!(
        result_conflict.conflicts[0].path, "binary.bin",
        "Conflicted path should be binary.bin"
    );
}

/// Minimal temp directory helper (replaces `tempfile::TempDir`).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gestalt_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
