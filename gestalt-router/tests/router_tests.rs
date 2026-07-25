use gestalt_router::checkpoint::{checkpoint, clean_path, is_symlink_escape, CheckpointResult};
use gestalt_router::run::{AgentResult, AgentStatus, ConflictInfo, RunReport};
use gestalt_router::run_state::{AgentState, RunManifest};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

fn init_test_repo(dir: &Path) {
    Command::new("git")
        .arg("init")
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(dir)
        .status()
        .unwrap();
    Command::new("git")
        .args(["branch", "-m", "main"])
        .current_dir(dir)
        .status()
        .unwrap();
}

#[test]
fn test_agent_state_serialization() {
    let state = AgentState::Pending;
    let serialized = serde_json::to_string(&state).unwrap();
    assert_eq!(serialized, "\"Pending\"");
    let deserialized: AgentState = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, AgentState::Pending);
}

#[test]
fn test_agent_state_transitions() {
    use AgentState::*;
    let states = vec![
        Pending,
        Running,
        Success,
        NoChanges,
        Timeout,
        Crashed,
        Quarantined,
    ];
    assert_eq!(states.len(), 7);
    assert_ne!(Pending, Running);
    assert_ne!(Running, Success);
}

#[test]
fn test_agent_status_from_str() {
    let pending = AgentStatus::Pending;
    assert_eq!(format!("{:?}", pending), "Pending");
}

#[test]
fn test_run_manifest_creation() {
    let manifest = RunManifest {
        run_id: Uuid::new_v4(),
        spec: gestalt_router::run::RunSpec {
            base_ref: "main".to_string(),
            task: "test task".to_string(),
            agents: vec![],
            max_parallel: 2,
            timeout: 60,
            push: false,
            integration_branch: None,
        },
        agent_states: std::collections::HashMap::new(),
    };
    assert_eq!(manifest.spec.task, "test task");
    assert_eq!(manifest.spec.max_parallel, 2);
}

#[test]
fn test_agent_result_construction() {
    let result = AgentResult {
        agent_id: "agent-1".to_string(),
        state: AgentState::Success,
        output: Some("done".to_string()),
        error: None,
        branch: Some("feat/test".to_string()),
        changed_files: vec!["src/lib.rs".to_string()],
        duration_ms: 1500,
        run_id: Some(Uuid::new_v4()),
        worktree_path: Some("/tmp/worktree".to_string()),
    };
    assert_eq!(result.agent_id, "agent-1");
    assert_eq!(result.changed_files.len(), 1);
    assert!(result.duration_ms >= 0);
}

#[test]
fn test_conflict_info_construction() {
    let ci = ConflictInfo {
        agent_id: "agent-a".to_string(),
        path: "Cargo.toml".to_string(),
    };
    assert_eq!(ci.agent_id, "agent-a");
    assert_eq!(ci.path, "Cargo.toml");
}

#[test]
fn test_run_report_construction() {
    let report = RunReport {
        run_id: Uuid::new_v4(),
        agents: vec![],
        merged_branches: vec!["feat/a".to_string()],
        conflicts: vec![],
        events_path: "/tmp/events.jsonl".to_string(),
        success: true,
    };
    assert_eq!(report.success, true);
    assert_eq!(report.merged_branches.len(), 1);
}

#[test]
fn test_clean_path_normal() {
    let path = PathBuf::from("/home/user/project/src/lib.rs");
    let cleaned = clean_path(&path);
    assert_eq!(cleaned, path);
}

#[test]
fn test_clean_path_with_dotdot() {
    let path = PathBuf::from("/home/user/project/../src/lib.rs");
    let cleaned = clean_path(&path);
    assert_eq!(cleaned, PathBuf::from("/home/user/src/lib.rs"));
}

#[test]
fn test_clean_path_with_dot() {
    let path = PathBuf::from("./src/./lib.rs");
    let cleaned = clean_path(&path);
    assert_eq!(cleaned, PathBuf::from("src/lib.rs"));
}

#[test]
fn test_is_symlink_escape_detects_escape() {
    let worktree = PathBuf::from("/tmp/worktree");
    let symlink = PathBuf::from("escape_link");
    let res = is_symlink_escape(&worktree, &symlink, "/etc/passwd");
    assert!(res, "Symlink pointing to /etc should be an escape");
}

#[test]
fn test_is_symlink_escape_allows_internal() {
    let worktree = PathBuf::from("/tmp/worktree");
    let symlink = PathBuf::from("internal_link");
    let res = is_symlink_escape(&worktree, &symlink, "/tmp/worktree/target.txt");
    assert!(!res, "Symlink inside worktree should NOT be an escape");
}

#[test]
fn test_checkpoint_creates_commit_on_dirty_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    init_test_repo(repo_path);

    let test_file = repo_path.join("test.txt");
    std::fs::write(&test_file, "hello").unwrap();

    let result = checkpoint(repo_path, "test checkpoint").unwrap();
    assert!(result.success, "Checkpoint should succeed on dirty repo");
    assert!(result.commit_sha.is_some(), "Commit SHA should be present");
    assert!(
        !result.files_committed.is_empty(),
        "Files should be committed"
    );
}

#[test]
fn test_checkpoint_returns_no_changes_on_clean_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    init_test_repo(repo_path);

    let result = checkpoint(repo_path, "no-op checkpoint").unwrap();
    assert!(
        result.success,
        "Checkpoint on clean repo should still return success"
    );
    assert!(result.commit_sha.is_none(), "No commit should be created");
}

#[test]
fn test_run_checkpoint_returns_true_on_changes() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    init_test_repo(repo_path);

    let test_file = repo_path.join("modified.rs");
    std::fs::write(&test_file, "fn test() {}").unwrap();

    let result = gestalt_router::checkpoint::run_checkpoint(repo_path, "test-agent").unwrap();
    assert!(
        result,
        "run_checkpoint should return true when changes committed"
    );
}

#[test]
fn test_run_checkpoint_returns_false_on_clean() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    init_test_repo(repo_path);

    let result = gestalt_router::checkpoint::run_checkpoint(repo_path, "test-agent").unwrap();
    assert!(
        !result,
        "run_checkpoint should return false when no changes"
    );
}

#[test]
fn test_checkpoint_result_construction() {
    let result = CheckpointResult {
        success: true,
        commit_sha: Some("abc123".to_string()),
        symlink_escapes: vec![],
        excluded_files: vec![],
        files_committed: vec!["src/main.rs".to_string()],
    };
    assert!(result.success);
    assert_eq!(result.commit_sha.unwrap(), "abc123");
}

#[test]
fn test_checkpoint_excludes_ignored_files() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    init_test_repo(repo_path);

    std::fs::write(repo_path.join(".gitignore"), "target/\n").unwrap();
    Command::new("git")
        .args(["add", ".gitignore"])
        .current_dir(repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add gitignore"])
        .current_dir(repo_path)
        .status()
        .unwrap();

    std::fs::create_dir_all(repo_path.join("target")).unwrap();
    std::fs::write(repo_path.join("target/build.log"), "build output").unwrap();
    std::fs::write(repo_path.join("src/main.rs"), "fn main() {}").unwrap();

    let result = checkpoint(repo_path, "test").unwrap();
    assert!(result.success);
    assert!(
        !result.files_committed.iter().any(|f| f.contains("target/")),
        "Ignored files should not be committed"
    );
}
