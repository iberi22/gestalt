//! Comprehensive integration tests for gestalt-router.
//!
//! Covers: type validation, checkpoint, overlap, integrate, timeline, workflow,
//! and edge-case scenarios across all public modules.
//!
//! Git operations use std::process::Command (NOT pub(crate) run_git_cmd).

use gestalt_router::checkpoint::{checkpoint, clean_path, is_symlink_escape, CheckpointResult};
use gestalt_router::integrate::{
    integrate_branches, AgentIntegrationSpec, IntegrateResult, MergeResult,
};
use gestalt_router::overlap::{
    detect_overlap, find_overlaps, get_modified_files, ConflictInfo as OverlapConflictInfo,
    ConflictKind as OverlapConflictKind, MergeTestResult, OverlapInfo, OverlapResult,
};
use gestalt_router::run::{
    AgentResult, AgentSpec, AgentStatus, ConflictInfo, ConflictKind, RouterError, RouterErrorKind,
    RunReport, RunSpec,
};
use gestalt_router::run_state::{AgentState, RunManifest};
use gestalt_router::timeline::{Event, EventLog, JsonlEventLog, VersionedEvent};
use gestalt_router::worktree::{WorktreeInfo, WorktreeManager};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{}_{}", prefix, Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to execute git command");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Set up a minimal git repository with one commit on `main` containing 3 files.
/// Returns the base SHA.
fn setup_git_repo(dir: &Path) -> String {
    run_git(dir, &["init", "-b", "main"]);
    run_git(dir, &["config", "user.name", "Test User"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);

    std::fs::write(dir.join("file1.txt"), "Line 1\nLine 2\n").unwrap();
    std::fs::write(dir.join("file2.txt"), "Apple\nBanana\n").unwrap();
    std::fs::write(dir.join("file3.txt"), "Cat\nDog\n").unwrap();

    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", "initial commit"]);
    run_git(dir, &["rev-parse", "HEAD"])
}

// ===========================================================================
// A) TYPE VALIDATION (5+ tests)
// ===========================================================================

#[test]
fn test_agent_spec_serialization_roundtrip() {
    let spec = AgentSpec {
        id: "agent-42".into(),
        command: "/bin/echo".into(),
        args: vec!["hello".into(), "world".into()],
        allowed_paths: Some(vec!["/tmp/work".into(), "/var/data".into()]),
        env: Some(HashMap::from([
            ("PATH".into(), "/usr/bin".into()),
            ("HOME".into(), "/home/user".into()),
        ])),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deser: AgentSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.id, "agent-42");
    assert_eq!(deser.command, "/bin/echo");
    assert_eq!(deser.args, vec!["hello", "world"]);
    assert_eq!(
        deser.allowed_paths.as_ref().unwrap(),
        &vec!["/tmp/work".to_string(), "/var/data".to_string()]
    );
    assert_eq!(deser.env.as_ref().unwrap().get("PATH").unwrap(), "/usr/bin");
    assert_eq!(
        deser.env.as_ref().unwrap().get("HOME").unwrap(),
        "/home/user"
    );
}

#[test]
fn test_agent_spec_optional_fields_roundtrip() {
    // allowed_paths = None, env = None
    let spec = AgentSpec {
        id: "minimal".into(),
        command: "true".into(),
        args: vec![],
        allowed_paths: None,
        env: None,
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deser: AgentSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.id, "minimal");
    assert!(deser.allowed_paths.is_none());
    assert!(deser.env.is_none());
    assert!(deser.args.is_empty());
}

#[test]
fn test_run_spec_with_all_fields() {
    let agents = vec![
        AgentSpec {
            id: "a1".into(),
            command: "echo".into(),
            args: vec!["hi".into()],
            allowed_paths: None,
            env: None,
        },
        AgentSpec {
            id: "a2".into(),
            command: "cat".into(),
            args: vec!["/dev/null".into()],
            allowed_paths: None,
            env: None,
        },
    ];

    let spec = RunSpec {
        base_ref: "develop".into(),
        task: "implement feature X".into(),
        agents,
        max_parallel: 2,
        timeout: 300,
        push: true,
        integration_branch: Some("release/v2".into()),
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deser: RunSpec = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.base_ref, "develop");
    assert_eq!(deser.task, "implement feature X");
    assert_eq!(deser.agents.len(), 2);
    assert_eq!(deser.max_parallel, 2);
    assert_eq!(deser.timeout, 300);
    assert!(deser.push);
    assert_eq!(deser.integration_branch.as_deref(), Some("release/v2"));
}

#[test]
fn test_run_spec_without_integration_branch() {
    let spec = RunSpec {
        base_ref: "main".into(),
        task: "quick fix".into(),
        agents: vec![],
        max_parallel: 1,
        timeout: 60,
        push: false,
        integration_branch: None,
    };

    let json = serde_json::to_string(&spec).unwrap();
    let deser: RunSpec = serde_json::from_str(&json).unwrap();
    assert!(deser.integration_branch.is_none());
    assert!(!deser.push);
    assert!(deser.agents.is_empty());
}

#[test]
fn test_agent_result_construction_and_serialization() {
    let result = AgentResult {
        agent_id: "agent-x".into(),
        state: AgentState::Success,
        output: Some("build complete".into()),
        error: None,
        branch: Some("feat/agent-x".into()),
        changed_files: vec!["src/lib.rs".into(), "Cargo.toml".into()],
        duration_ms: 1234,
        run_id: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
        worktree_path: Some("/tmp/worktrees/wt-1".into()),
    };

    let json = serde_json::to_string(&result).unwrap();
    let deser: AgentResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.agent_id, "agent-x");
    assert_eq!(deser.state, AgentState::Success);
    assert_eq!(deser.output.as_deref(), Some("build complete"));
    assert!(deser.error.is_none());
    assert_eq!(deser.branch.as_deref(), Some("feat/agent-x"));
    assert_eq!(deser.changed_files.len(), 2);
    assert_eq!(deser.duration_ms, 1234);
    assert!(deser.run_id.is_some());
    assert!(deser.worktree_path.is_some());
}

#[test]
fn test_agent_result_failure_case() {
    let result = AgentResult {
        agent_id: "agent-fail".into(),
        state: AgentState::Crashed,
        output: None,
        error: Some("exit code 1".into()),
        branch: None,
        changed_files: vec![],
        duration_ms: 500,
        run_id: None,
        worktree_path: None,
    };

    assert_eq!(result.state, AgentState::Crashed);
    assert_eq!(result.error.as_deref(), Some("exit code 1"));
    assert!(result.run_id.is_none());
}

#[test]
fn test_conflict_info_and_conflict_kind() {
    let info = ConflictInfo {
        agent_id: "agent-a".into(),
        path: "src/main.rs".into(),
    };
    assert_eq!(info.agent_id, "agent-a");
    assert_eq!(info.path, "src/main.rs");

    // All variants
    assert_eq!(format!("{:?}", ConflictKind::Overlap), "Overlap");
    assert_eq!(
        format!("{:?}", ConflictKind::MergeConflict),
        "MergeConflict"
    );
    assert_eq!(
        format!("{:?}", ConflictKind::BinaryConflict),
        "BinaryConflict"
    );

    // Serialization roundtrip
    for kind in &[
        ConflictKind::Overlap,
        ConflictKind::MergeConflict,
        ConflictKind::BinaryConflict,
    ] {
        let json = serde_json::to_string(kind).unwrap();
        let deser: ConflictKind = serde_json::from_str(&json).unwrap();
        assert_eq!(*kind, deser);
    }
}

#[test]
fn test_router_error_and_error_kind() {
    // Helper constructors
    let err = RouterError::GitError("merge failed");
    assert_eq!(err.kind, RouterErrorKind::GitError);
    assert_eq!(err.message, "merge failed");
    assert!(err.source.is_none());

    let err = RouterError::AgentError("agent crashed");
    assert_eq!(err.kind, RouterErrorKind::AgentError);
    assert_eq!(err.message, "agent crashed");

    let err = RouterError::Timeout("operation timed out");
    assert_eq!(err.kind, RouterErrorKind::Timeout);
    assert_eq!(err.message, "operation timed out");

    let err = RouterError::InvalidSpec("missing command");
    assert_eq!(err.kind, RouterErrorKind::InvalidSpec);
    assert_eq!(err.message, "missing command");

    let err = RouterError::TimelineError("corrupt log");
    assert_eq!(err.kind, RouterErrorKind::TimelineError);
    assert_eq!(err.message, "corrupt log");

    // new() with source
    let inner = std::io::Error::other("io failure");
    let err = RouterError::new(
        RouterErrorKind::GitError,
        "git command failed",
        Some(Box::new(inner)),
    );
    assert_eq!(err.kind, RouterErrorKind::GitError);
    assert!(err.source.is_some());

    // Display (via thiserror)
    let display = format!("{}", RouterError::GitError("something broke"));
    assert_eq!(display, "something broke");
}

#[test]
fn test_agent_status_variants() {
    // AgentStatus (the enum in run.rs) shares discriminant shape with AgentState
    let variants = vec![
        (AgentStatus::Pending, "Pending"),
        (AgentStatus::Running, "Running"),
        (AgentStatus::Success, "Success"),
        (AgentStatus::Timeout, "Timeout"),
        (AgentStatus::Crashed, "Crashed"),
        (AgentStatus::NoChanges, "NoChanges"),
        (AgentStatus::Quarantined, "Quarantined"),
    ];

    for (variant, name) in &variants {
        let json = serde_json::to_string(variant).unwrap();
        assert_eq!(json, format!("\"{}\"", name));
        let deser: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(*variant, deser);
    }
}

#[test]
fn test_run_report_serialization() {
    let result = AgentResult {
        agent_id: "a1".into(),
        state: AgentState::Success,
        output: Some("done".into()),
        error: None,
        branch: Some("branch-a1".into()),
        changed_files: vec!["f1.rs".into()],
        duration_ms: 100,
        run_id: None,
        worktree_path: None,
    };

    let conflict = ConflictInfo {
        agent_id: "a2".into(),
        path: "f2.rs".into(),
    };

    let report = RunReport {
        run_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        task: "test task".into(),
        agents: vec![result],
        duration_ms: 0,
        merged_branches: vec!["branch-a1".into()],
        conflicts: vec![conflict],
        events_path: "/tmp/run-events.jsonl".into(),
        success: true,
    };

    let json = serde_json::to_string(&report).unwrap();
    let deser: RunReport = serde_json::from_str(&json).unwrap();

    assert_eq!(
        deser.run_id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(deser.agents.len(), 1);
    assert_eq!(deser.agents[0].agent_id, "a1");
    assert_eq!(deser.merged_branches, vec!["branch-a1"]);
    assert_eq!(deser.conflicts.len(), 1);
    assert_eq!(deser.conflicts[0].path, "f2.rs");
    assert_eq!(deser.events_path, "/tmp/run-events.jsonl");
    assert!(deser.success);
}

// ===========================================================================
// B) CHECKPOINT (3+ tests)
// ===========================================================================

#[test]
fn test_clean_path_basic() {
    assert_eq!(clean_path(Path::new("a/b/../c")), PathBuf::from("a/c"));
    assert_eq!(clean_path(Path::new("a/b/../../c")), PathBuf::from("c"));
    assert_eq!(clean_path(Path::new("a/./b/./c")), PathBuf::from("a/b/c"));
    assert_eq!(clean_path(Path::new(".")), PathBuf::from(""));
    assert_eq!(clean_path(Path::new("..")), PathBuf::from(""));
    assert_eq!(clean_path(Path::new("/")), PathBuf::from("/"));
    assert_eq!(clean_path(Path::new("/a/b/../c")), PathBuf::from("/a/c"));
}

#[test]
fn test_is_symlink_escape_logic() {
    let root = Path::new("/app/worktree");

    // Relative target within worktree
    assert!(!is_symlink_escape(root, Path::new("sub/link"), "file.txt"));
    assert!(!is_symlink_escape(
        root,
        Path::new("sub/link"),
        "../other.txt"
    ));

    // Relative target escaping the worktree
    assert!(is_symlink_escape(
        root,
        Path::new("sub/link"),
        "../../etc/passwd"
    ));

    // Absolute targets are always escapes
    assert!(is_symlink_escape(
        root,
        Path::new("sub/link"),
        "/etc/passwd"
    ));
    assert!(is_symlink_escape(root, Path::new("link"), "/bin/sh"));
}

#[test]
fn test_checkpoint_integration_real_git_repo() {
    let tmp = TempDir::new("gestalt-checkpoint-test");
    let dir = &tmp.path;

    // Init repo with initial commit
    let _ = setup_git_repo(dir);

    // Create new changes to commit
    std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.join("lib.rs"), "pub fn helper() {}").unwrap();

    // Run checkpoint
    let res = checkpoint(dir, "feat: add core functions").unwrap();
    assert!(res.success);
    assert!(
        res.commit_sha.is_some(),
        "expected a commit SHA when files changed"
    );
    assert_eq!(res.files_committed.len(), 2);
    assert!(res.files_committed.contains(&"main.rs".to_string()));
    assert!(res.files_committed.contains(&"lib.rs".to_string()));
    assert!(res.symlink_escapes.is_empty());
    assert!(res.excluded_files.is_empty());

    // Verify commit message
    let log_msg = run_git(dir, &["log", "-1", "--pretty=%B"]);
    assert_eq!(log_msg, "feat: add core functions");
}

#[test]
fn test_checkpoint_with_gitignored_files() {
    let tmp = TempDir::new("gestalt-checkpoint-ignore");
    let dir = &tmp.path;

    let _ = setup_git_repo(dir);

    // Create .gitignore and an ignored file
    std::fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
    std::fs::write(dir.join("build.log"), "some build output").unwrap();

    // Also create a real file to commit
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn f() {}").unwrap();

    let res = checkpoint(dir, "feat: add lib").unwrap();
    assert!(res.success);
    assert!(res.commit_sha.is_some());

    // build.log should be excluded
    let excluded_names: Vec<&str> = res.excluded_files.iter().map(|e| e.path.as_str()).collect();
    assert!(
        excluded_names.contains(&"build.log"),
        "build.log should be listed as excluded, got {:?}",
        excluded_names
    );

    // .gitignore and src/lib.rs should be committed
    assert!(res.files_committed.contains(&".gitignore".to_string()));
    assert!(res.files_committed.contains(&"src/lib.rs".to_string()));
    assert!(!res.files_committed.contains(&"build.log".to_string()));
}

#[test]
#[cfg(unix)]
fn test_checkpoint_symlink_escape_detection() {
    use std::os::unix::fs::symlink;

    let tmp = TempDir::new("gestalt-checkpoint-sym");
    let dir = &tmp.path;

    let _ = setup_git_repo(dir);

    // Normal file
    std::fs::write(dir.join("safe.txt"), "hello").unwrap();

    // Safe symlink (relative within repo)
    symlink("safe.txt", dir.join("safe_link")).unwrap();

    // Escaped symlink pointing outside
    symlink("/etc/passwd", dir.join("leak_link")).unwrap();

    let res = checkpoint(dir, "feat: add files").unwrap();
    assert!(res.success);
    assert!(res.commit_sha.is_some());

    // safe_link should be committed, leak_link should NOT
    assert!(res.files_committed.contains(&"safe.txt".to_string()));
    assert!(res.files_committed.contains(&"safe_link".to_string()));
    assert!(!res.files_committed.contains(&"leak_link".to_string()));

    // Symlink escape should be reported
    let escape_paths: Vec<&str> = res
        .symlink_escapes
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert!(
        escape_paths.contains(&"leak_link"),
        "leak_link should be in symlink_escapes, got {:?}",
        escape_paths
    );
}

// ===========================================================================
// C) OVERLAP (3+ tests)
// ===========================================================================

#[test]
fn test_find_overlaps_disjoint_branches() {
    let tmp = TempDir::new("gestalt-overlap-disjoint");
    let dir = &tmp.path;
    let base_sha = setup_git_repo(dir);

    // Branch A: modifies file1.txt
    run_git(dir, &["checkout", "-b", "branch-a"]);
    std::fs::write(dir.join("file1.txt"), "Modified by A\n").unwrap();
    run_git(dir, &["commit", "-am", "branch-a changes"]);
    run_git(dir, &["rev-parse", "branch-a"]);

    // Branch B: modifies file2.txt
    run_git(dir, &["checkout", "main"]);
    run_git(dir, &["checkout", "-b", "branch-b"]);
    std::fs::write(dir.join("file2.txt"), "Modified by B\n").unwrap();
    run_git(dir, &["commit", "-am", "branch-b changes"]);
    run_git(dir, &["rev-parse", "branch-b"]);

    run_git(dir, &["checkout", "main"]);

    let active_branches = vec![
        ("agent-a".to_string(), "branch-a".to_string()),
        ("agent-b".to_string(), "branch-b".to_string()),
    ];

    let overlaps = find_overlaps(dir, &base_sha, &active_branches).unwrap();
    assert!(
        overlaps.is_empty(),
        "expected no overlaps for disjoint branches, got {:?}",
        overlaps
    );
}

#[test]
fn test_find_overlaps_with_shared_paths() {
    let tmp = TempDir::new("gestalt-overlap-shared");
    let dir = &tmp.path;
    let base_sha = setup_git_repo(dir);

    // Both branches modify file1.txt
    run_git(dir, &["checkout", "-b", "branch-a"]);
    std::fs::write(dir.join("file1.txt"), "Agent A content\n").unwrap();
    run_git(dir, &["commit", "-am", "branch-a modifies file1"]);

    run_git(dir, &["checkout", "main"]);
    run_git(dir, &["checkout", "-b", "branch-b"]);
    std::fs::write(dir.join("file1.txt"), "Agent B content\n").unwrap();
    std::fs::write(dir.join("file3.txt"), "Agent B also touches file3\n").unwrap();
    run_git(dir, &["commit", "-am", "branch-b modifies file1 and file3"]);

    run_git(dir, &["checkout", "main"]);

    let active_branches = vec![
        ("agent-a".to_string(), "branch-a".to_string()),
        ("agent-b".to_string(), "branch-b".to_string()),
    ];

    let overlaps = find_overlaps(dir, &base_sha, &active_branches).unwrap();
    assert_eq!(overlaps.len(), 1, "expected one overlap pair");
    assert_eq!(overlaps[0].agent_a, "agent-a");
    assert_eq!(overlaps[0].agent_b, "agent-b");

    // file1.txt should be the shared path
    let file_names: Vec<String> = overlaps[0]
        .files
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(
        file_names.contains(&"file1.txt".to_string()),
        "expected file1.txt in shared paths, got {:?}",
        file_names
    );
}

#[test]
fn test_detect_overlap_finds_shared_paths() {
    // Disjoint
    let files_a = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
    let files_b = vec![PathBuf::from("c.rs"), PathBuf::from("d.rs")];
    let result = detect_overlap(&files_a, &files_b);
    assert!(result.disjoint);
    assert!(result.shared_paths.is_empty());

    // One shared path
    let files_a = vec![PathBuf::from("shared.rs"), PathBuf::from("a.rs")];
    let files_b = vec![PathBuf::from("shared.rs"), PathBuf::from("b.rs")];
    let result = detect_overlap(&files_a, &files_b);
    assert!(!result.disjoint);
    assert_eq!(result.shared_paths, vec![PathBuf::from("shared.rs")]);

    // Multiple shared paths
    let files_a = vec![
        PathBuf::from("x.rs"),
        PathBuf::from("y.rs"),
        PathBuf::from("z.rs"),
    ];
    let files_b = vec![
        PathBuf::from("y.rs"),
        PathBuf::from("z.rs"),
        PathBuf::from("w.rs"),
    ];
    let result = detect_overlap(&files_a, &files_b);
    assert!(!result.disjoint);
    assert_eq!(result.shared_paths.len(), 2);
    assert!(result.shared_paths.contains(&PathBuf::from("y.rs")));
    assert!(result.shared_paths.contains(&PathBuf::from("z.rs")));

    // Both empty
    let result = detect_overlap(&[], &[]);
    assert!(result.disjoint);

    // One empty
    let result = detect_overlap(&[PathBuf::from("f.rs")], &[]);
    assert!(result.disjoint);
}

#[test]
fn test_get_modified_files_returns_correct_files() {
    let tmp = TempDir::new("gestalt-modified-files");
    let dir = &tmp.path;
    let base_sha = setup_git_repo(dir);

    // Create branch with new files
    run_git(dir, &["checkout", "-b", "feature"]);
    std::fs::write(dir.join("new_file.txt"), "new stuff").unwrap();
    std::fs::write(dir.join("another.rs"), "fn main() {}").unwrap();
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", "add new files"]);
    run_git(dir, &["checkout", "main"]);

    let files = get_modified_files(dir, &base_sha, "feature").unwrap();
    assert_eq!(files.len(), 2);
    let names: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"new_file.txt".to_string()));
    assert!(names.contains(&"another.rs".to_string()));
}

#[test]
fn test_get_modified_files_no_changes() {
    let tmp = TempDir::new("gestalt-no-changes");
    let dir = &tmp.path;
    let base_sha = setup_git_repo(dir);

    run_git(dir, &["checkout", "-b", "empty-branch"]);
    run_git(dir, &["commit", "--allow-empty", "-m", "no changes"]);

    let files = get_modified_files(dir, &base_sha, "empty-branch").unwrap();
    assert!(
        files.is_empty(),
        "expected no modified files, got {:?}",
        files
    );
}

// ===========================================================================
// D) INTEGRATE (3+ tests)
// ===========================================================================

#[test]
fn test_integrate_result_structure() {
    let result = IntegrateResult {
        merge_sha: "abc123def456".into(),
        merged_branches: vec!["branch-a".into(), "branch-b".into()],
        conflicts: vec![ConflictInfo {
            agent_id: "agent-x".into(),
            path: "src/main.rs".into(),
        }],
    };

    assert_eq!(result.merge_sha, "abc123def456");
    assert_eq!(result.merged_branches.len(), 2);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].path, "src/main.rs");
}

#[test]
fn test_integrate_result_fields_serialization() {
    let result = IntegrateResult {
        merge_sha: String::new(),
        merged_branches: vec![],
        conflicts: vec![],
    };

    let json = serde_json::to_string(&result).unwrap();
    let deser: IntegrateResult = serde_json::from_str(&json).unwrap();
    assert!(deser.merge_sha.is_empty());
    assert!(deser.merged_branches.is_empty());
    assert!(deser.conflicts.is_empty());
}

#[test]
fn test_agent_integration_spec() {
    let spec = AgentIntegrationSpec {
        id: "agent-007".into(),
        branch: "secret/mission".into(),
    };
    assert_eq!(spec.id, "agent-007");
    assert_eq!(spec.branch, "secret/mission");

    let json = serde_json::to_string(&spec).unwrap();
    let deser: AgentIntegrationSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.id, "agent-007");
    assert_eq!(deser.branch, "secret/mission");
}

#[test]
fn test_mergeresult_variants() {
    // Success variant
    let success = MergeResult::Success {
        merged_commit_sha: "deadbeef".into(),
    };
    match &success {
        MergeResult::Success { merged_commit_sha } => {
            assert_eq!(merged_commit_sha, "deadbeef");
        }
        _ => panic!("expected Success variant"),
    }

    // HardConflict variant
    let conflict = MergeResult::HardConflict {
        conflicted_files: vec!["f1.txt".into(), "f2.txt".into()],
        branches_preserved: vec!["branch-a".into()],
    };
    match &conflict {
        MergeResult::HardConflict {
            conflicted_files,
            branches_preserved,
        } => {
            assert_eq!(conflicted_files.len(), 2);
            assert_eq!(branches_preserved.len(), 1);
        }
        _ => panic!("expected HardConflict variant"),
    }

    // Serialization roundtrip
    for variant in &[success, conflict] {
        let json = serde_json::to_string(variant).unwrap();
        let deser: MergeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(*variant, deser);
    }
}

#[test]
fn test_integrate_branches_no_conflicts() {
    let tmp = TempDir::new("gestalt-integrate-clean");
    let dir = &tmp.path;
    let base_sha = setup_git_repo(dir);

    // Agent A branch: modifies file1.txt
    run_git(dir, &["checkout", "-b", "branch_agent_a"]);
    std::fs::write(dir.join("file1.txt"), "Modified by Agent A\n").unwrap();
    run_git(dir, &["commit", "-am", "agent a changes"]);

    // Agent B branch: modifies file2.txt (different file, no conflict)
    run_git(dir, &["checkout", "main"]);
    run_git(dir, &["checkout", "-b", "branch_agent_b"]);
    std::fs::write(dir.join("file2.txt"), "Modified by Agent B\n").unwrap();
    run_git(dir, &["commit", "-am", "agent b changes"]);

    run_git(dir, &["checkout", "main"]);

    let branches = vec![
        ("agent_a".to_string(), "branch_agent_a".to_string()),
        ("agent_b".to_string(), "branch_agent_b".to_string()),
    ];

    let result = integrate_branches(dir, &base_sha, "integration/main", &branches).unwrap();
    assert!(!result.merge_sha.is_empty(), "expected a merge SHA");
    assert_eq!(result.merged_branches.len(), 2);
    assert!(
        result.conflicts.is_empty(),
        "expected no conflicts, got {:?}",
        result.conflicts
    );
}

// ===========================================================================
// E) TIMELINE (3+ tests)
// ===========================================================================

#[test]
fn test_event_serialization_all_variants() {
    let run_id = Uuid::new_v4();

    let events: Vec<Event> = vec![
        Event::RunStarted {
            run_id,
            task: "build".into(),
            agents: vec!["a1".into(), "a2".into()],
            sha_base: "abc123".into(),
        },
        Event::AgentStateChanged {
            run_id,
            agent_id: "a1".into(),
            from: AgentState::Pending,
            to: AgentState::Running,
        },
        Event::CheckpointCommitted {
            commit_hash: "def456".into(),
        },
        Event::OverlapDetected {
            run_id,
            agent_a: "a1".into(),
            agent_b: "a2".into(),
            files: vec!["src/main.rs".into()],
        },
        Event::MergeConflict {
            run_id,
            agent: "a1".into(),
            path: "Cargo.toml".into(),
        },
        Event::MergeComputed {
            target_branch: "main".into(),
            success: true,
        },
        Event::BranchPublished {
            branch: "release/v2".into(),
        },
        Event::SymlinkEscape {
            path: "leak_link".into(),
        },
        Event::ExcludedFile {
            path: "build.log".into(),
        },
        Event::RunFinished {
            run_id,
            summary: "completed with 2 agents".into(),
        },
    ];

    // All variants should serialize and deserialize roundtrip correctly
    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let deser: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(*event, deser, "roundtrip failed for event: {:?}", event);
    }
}

#[test]
fn test_versioned_event_wrapper() {
    let event = Event::CheckpointCommitted {
        commit_hash: "abc123".into(),
    };

    let versioned = VersionedEvent {
        v: 1,
        event: event.clone(),
    };

    let json = serde_json::to_string(&versioned).unwrap();
    let deser: VersionedEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.v, 1);
    assert_eq!(deser.event, event);

    // Ensure JSON contains both `v` and the flattened event fields
    assert!(json.contains("\"v\":1"));
    assert!(json.contains("\"type\":\"CheckpointCommitted\""));
    assert!(json.contains("\"commit_hash\":\"abc123\""));
}

#[test]
fn test_jsonl_event_log_creation_and_logging() {
    let tmp = TempDir::new("gestalt-timeline");
    let run_id = Uuid::new_v4();

    let log = JsonlEventLog::new_with_dir(run_id, tmp.path.clone()).unwrap();

    // Log several events
    let event1 = Event::RunStarted {
        run_id,
        task: "test task".into(),
        agents: vec!["agent-1".into()],
        sha_base: "abc".into(),
    };
    log.append(event1.clone()).unwrap();

    let event2 = Event::AgentStateChanged {
        run_id,
        agent_id: "agent-1".into(),
        from: AgentState::Pending,
        to: AgentState::Running,
    };
    log.append(event2.clone()).unwrap();

    let event3 = Event::RunFinished {
        run_id,
        summary: "done".into(),
    };
    log.append(event3.clone()).unwrap();

    // Read back
    let events = log.read_events(run_id).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], event1);
    assert_eq!(events[1], event2);
    assert_eq!(events[2], event3);
}

#[test]
fn test_event_log_list_runs() {
    let tmp = TempDir::new("gestalt-timeline-list");
    let run_id1 = Uuid::new_v4();
    let run_id2 = Uuid::new_v4();

    let log1 = JsonlEventLog::new_with_dir(run_id1, tmp.path.clone()).unwrap();
    let log2 = JsonlEventLog::new_with_dir(run_id2, tmp.path.clone()).unwrap();

    log1.append(Event::RunStarted {
        run_id: run_id1,
        task: "first".into(),
        agents: vec![],
        sha_base: "a".into(),
    })
    .unwrap();

    log2.append(Event::RunStarted {
        run_id: run_id2,
        task: "second".into(),
        agents: vec![],
        sha_base: "b".into(),
    })
    .unwrap();

    let listed = log1.list_runs().unwrap();
    assert!(listed.contains(&run_id1));
    assert!(listed.contains(&run_id2));
}

#[test]
fn test_event_log_read_empty_run() {
    let tmp = TempDir::new("gestalt-timeline-empty");
    let run_id = Uuid::new_v4();

    // Create a log for this run but don't write anything
    let _log = JsonlEventLog::new_with_dir(run_id, tmp.path.clone()).unwrap();

    // Reading a non-existent run should return empty vec
    let other_id = Uuid::new_v4();
    let log2 = JsonlEventLog::new_with_dir(other_id, tmp.path.clone()).unwrap();
    let events = log2.read_events(other_id).unwrap();
    assert!(events.is_empty(), "expected empty events for untouched run");
}

// ===========================================================================
// F) WORKFLOW (3+ tests)
// ===========================================================================

#[test]
fn test_run_manifest_with_agent_state() {
    let agents = vec![
        AgentSpec {
            id: "agent-1".into(),
            command: "echo".into(),
            args: vec!["hi".into()],
            allowed_paths: None,
            env: None,
        },
        AgentSpec {
            id: "agent-2".into(),
            command: "cat".into(),
            args: vec!["/dev/null".into()],
            allowed_paths: None,
            env: None,
        },
    ];

    let spec = RunSpec {
        base_ref: "main".into(),
        task: "multi-agent workflow".into(),
        agents,
        max_parallel: 2,
        timeout: 100,
        push: false,
        integration_branch: None,
    };

    let run_id = Uuid::new_v4();
    let mut agent_states = HashMap::new();
    agent_states.insert("agent-1".into(), AgentState::Pending);
    agent_states.insert("agent-2".into(), AgentState::Pending);

    let manifest = RunManifest {
        run_id,
        spec: spec.clone(),
        agent_states,
    };

    assert_eq!(manifest.run_id, run_id);
    assert_eq!(manifest.spec.task, "multi-agent workflow");
    assert_eq!(manifest.agent_states.len(), 2);
    assert_eq!(
        manifest.agent_states.get("agent-1").unwrap(),
        &AgentState::Pending
    );

    // Serialization roundtrip
    let json = serde_json::to_string(&manifest).unwrap();
    let deser: RunManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.run_id, run_id);
    assert_eq!(deser.agent_states.len(), 2);
}

#[test]
fn test_state_transition_lifecycle() {
    // Simulate the lifecycle: Pending -> Running -> Success
    let mut states: HashMap<String, AgentState> = HashMap::new();
    states.insert("agent-1".into(), AgentState::Pending);

    // Pending -> Running
    states.insert("agent-1".into(), AgentState::Running);
    assert_eq!(states.get("agent-1").unwrap(), &AgentState::Running);

    // Running -> Success
    states.insert("agent-1".into(), AgentState::Success);
    assert_eq!(states.get("agent-1").unwrap(), &AgentState::Success);

    // Verify all states are reachable via direct construction
    let all_states = vec![
        AgentState::Pending,
        AgentState::Running,
        AgentState::Success,
        AgentState::Timeout,
        AgentState::Crashed,
        AgentState::NoChanges,
        AgentState::Quarantined,
    ];
    for state in &all_states {
        let json = serde_json::to_string(state).unwrap();
        let deser: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(*state, deser);
    }
}

#[test]
fn test_multi_agent_state_tracking() {
    let mut states: HashMap<String, AgentState> = HashMap::new();

    // Three agents start as Pending
    for id in &["alice", "bob", "charlie"] {
        states.insert((*id).into(), AgentState::Pending);
    }
    assert_eq!(states.len(), 3);
    assert!(states.values().all(|s| *s == AgentState::Pending));

    // alice runs, bob runs, charlie pending
    states.insert("alice".into(), AgentState::Running);
    states.insert("bob".into(), AgentState::Running);
    assert_eq!(states.get("alice").unwrap(), &AgentState::Running);
    assert_eq!(states.get("bob").unwrap(), &AgentState::Running);
    assert_eq!(states.get("charlie").unwrap(), &AgentState::Pending);

    // alice succeeds, bob crashes, charlie runs
    states.insert("alice".into(), AgentState::Success);
    states.insert("bob".into(), AgentState::Crashed);
    states.insert("charlie".into(), AgentState::Running);
    assert_eq!(states.get("alice").unwrap(), &AgentState::Success);
    assert_eq!(states.get("bob").unwrap(), &AgentState::Crashed);
    assert_eq!(states.get("charlie").unwrap(), &AgentState::Running);

    // charlie times out
    states.insert("charlie".into(), AgentState::Timeout);
    assert_eq!(states.get("charlie").unwrap(), &AgentState::Timeout);
}

// ===========================================================================
// G) WORKTREE (tests for WorktreeManager constructs)
// ===========================================================================

#[test]
fn test_worktree_manager_new() {
    let manager = WorktreeManager::new(PathBuf::from("/tmp/gestalt-test"));
    assert_eq!(manager.base_dir, PathBuf::from("/tmp/gestalt-test"));
}

#[test]
fn test_worktree_manager_default() {
    let manager = WorktreeManager::default();
    assert_eq!(manager.base_dir, PathBuf::from("/tmp/gestalt"));
}

#[test]
fn test_worktree_info_construction() {
    let info = WorktreeInfo {
        path: PathBuf::from("/tmp/wt"),
        branch: Some("feature-x".into()),
        sha: Some("abc123".into()),
        is_active: true,
    };
    assert_eq!(info.path, PathBuf::from("/tmp/wt"));
    assert_eq!(info.branch.as_deref(), Some("feature-x"));
    assert_eq!(info.sha.as_deref(), Some("abc123"));
    assert!(info.is_active);

    let inactive = WorktreeInfo {
        path: PathBuf::from("/tmp/gone"),
        branch: None,
        sha: None,
        is_active: false,
    };
    assert!(!inactive.is_active);
    assert!(inactive.branch.is_none());
}

// ===========================================================================
// H) OVERLAP TYPE CONSTRUCTS
// ===========================================================================

#[test]
fn test_overlap_info_construction() {
    let info = OverlapInfo {
        agent_a: "agent-1".into(),
        agent_b: "agent-2".into(),
        files: vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/lib.rs")],
    };
    assert_eq!(info.agent_a, "agent-1");
    assert_eq!(info.agent_b, "agent-2");
    assert_eq!(info.files.len(), 2);
}

#[test]
fn test_overlap_result_construction() {
    let disjoint = OverlapResult {
        shared_paths: vec![],
        disjoint: true,
    };
    assert!(disjoint.disjoint);
    assert!(disjoint.shared_paths.is_empty());

    let overlapping = OverlapResult {
        shared_paths: vec![PathBuf::from("shared.txt")],
        disjoint: false,
    };
    assert!(!overlapping.disjoint);
    assert_eq!(overlapping.shared_paths.len(), 1);
}

#[test]
fn test_overlap_conflict_info_and_kind() {
    let info = OverlapConflictInfo {
        path: PathBuf::from("src/main.rs"),
        kind: OverlapConflictKind::BothModified,
    };
    assert!(info.path.to_string_lossy().ends_with("main.rs"));
    assert_eq!(info.kind, OverlapConflictKind::BothModified);

    // All ConflictKind variants in overlap module
    let kinds = vec![
        OverlapConflictKind::Content,
        OverlapConflictKind::BothModified,
        OverlapConflictKind::AddedByUs,
        OverlapConflictKind::AddedByThem,
    ];
    for kind in &kinds {
        let json = serde_json::to_string(kind).unwrap();
        let deser: OverlapConflictKind = serde_json::from_str(&json).unwrap();
        assert_eq!(*kind, deser);
    }
}

#[test]
fn test_merge_test_result_clean() {
    let result = MergeTestResult::Clean;
    assert_eq!(format!("{:?}", result), "Clean");

    let json = serde_json::to_string(&result).unwrap();
    let deser: MergeTestResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, MergeTestResult::Clean);
}

#[test]
fn test_merge_test_result_conflicts() {
    let conflicts = vec![
        OverlapConflictInfo {
            path: PathBuf::from("f1.txt"),
            kind: OverlapConflictKind::BothModified,
        },
        OverlapConflictInfo {
            path: PathBuf::from("f2.txt"),
            kind: OverlapConflictKind::Content,
        },
    ];
    let result = MergeTestResult::Conflicts(conflicts);
    match &result {
        MergeTestResult::Conflicts(list) => {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0].path, PathBuf::from("f1.txt"));
        }
        _ => panic!("expected Conflicts variant"),
    }

    let json = serde_json::to_string(&result).unwrap();
    let deser: MergeTestResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, result);
}

// ===========================================================================
// I) EDGE CASES AND FAILURE SCENARIOS
// ===========================================================================

#[test]
fn test_router_error_nested_source_display() {
    let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err = RouterError::new(
        RouterErrorKind::AgentError,
        "agent binary missing",
        Some(Box::new(inner)),
    );
    let display = err.to_string();
    assert_eq!(display, "agent binary missing");
}

#[test]
fn test_checkpoint_no_changes_returns_none_sha() {
    let tmp = TempDir::new("gestalt-cp-no-changes");
    let dir = &tmp.path;
    let _ = setup_git_repo(dir);

    // Run checkpoint immediately with no changes
    let res = checkpoint(dir, "no-op commit").unwrap();
    assert!(res.success);
    assert!(
        res.commit_sha.is_none(),
        "expected no SHA when nothing changed"
    );
    assert!(res.files_committed.is_empty());
}

#[test]
fn test_checkpoint_hook_bypass() {
    let tmp = TempDir::new("gestalt-cp-hook");
    let dir = &tmp.path;
    let _ = setup_git_repo(dir);

    // Create a blocking pre-commit hook
    let hooks_dir = dir.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let hook_path = hooks_dir.join("pre-commit");
    std::fs::write(&hook_path, "#!/bin/sh\nexit 1\n").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms).unwrap();
    }

    // Make a change
    std::fs::write(dir.join("bypass_test.txt"), "bypassed hook").unwrap();

    // checkpoint should bypass the hook and commit successfully
    let res = checkpoint(dir, "bypass pre-commit hook").unwrap();
    assert!(res.success);
    assert!(
        res.commit_sha.is_some(),
        "expected commit despite blocking pre-commit hook"
    );
    assert!(res.files_committed.contains(&"bypass_test.txt".to_string()));
}

#[test]
fn test_overlap_info_serialization() {
    let info = OverlapInfo {
        agent_a: "alpha".into(),
        agent_b: "beta".into(),
        files: vec![PathBuf::from("conflict.rs")],
    };
    let json = serde_json::to_string(&info).unwrap();
    let deser: OverlapInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.agent_a, "alpha");
    assert_eq!(deser.agent_b, "beta");
    assert_eq!(deser.files[0], PathBuf::from("conflict.rs"));
}

#[test]
fn test_checkpoint_result_construction() {
    let result = CheckpointResult {
        success: true,
        commit_sha: Some("abc123".into()),
        symlink_escapes: vec![],
        excluded_files: vec![],
        files_committed: vec!["a.rs".into(), "b.rs".into()],
    };
    assert!(result.success);
    assert_eq!(result.commit_sha.as_deref(), Some("abc123"));
    assert_eq!(result.files_committed.len(), 2);

    let json = serde_json::to_string(&result).unwrap();
    let deser: CheckpointResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.success, result.success);
    assert_eq!(deser.commit_sha, result.commit_sha);
}
