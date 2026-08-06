use gestalt_router::overlap::{
    detect_overlap, get_modified_files, test_mergeability, MergeTestResult, OverlapInfo,
    OverlapResult,
};
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_git(repo_path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .expect("failed to execute git command");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn create_temp_git_repo() -> PathBuf {
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push(format!("gestalt-test-repo-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    run_git(&temp_dir, &["init"]);
    run_git(&temp_dir, &["config", "user.name", "Test User"]);
    run_git(&temp_dir, &["config", "user.email", "test@example.com"]);

    let file1 = temp_dir.join("file1.txt");
    std::fs::write(&file1, "base content\nline 2\n").unwrap();
    run_git(&temp_dir, &["add", "file1.txt"]);
    run_git(&temp_dir, &["commit", "-m", "initial commit"]);
    run_git(&temp_dir, &["branch", "-m", "main"]);

    temp_dir
}

fn create_branch(repo_path: &Path, branch: &str, content_changes: &[(&str, &str)]) {
    run_git(repo_path, &["checkout", "-b", branch]);
    for (file, content) in content_changes {
        let path = repo_path.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        run_git(repo_path, &["add", file]);
    }
    run_git(
        repo_path,
        &["commit", "-m", &format!("changes on {}", branch)],
    );
}

#[test]
fn test_detect_overlap_disjoint() {
    let files_a = vec![PathBuf::from("src/main.rs"), PathBuf::from("src/lib.rs")];
    let files_b = vec![PathBuf::from("tests/test.rs"), PathBuf::from("README.md")];
    let result = detect_overlap(&files_a, &files_b);
    assert!(result.disjoint);
    assert!(result.shared_paths.is_empty());
}

#[tokio::test]
async fn test_concurrent_writes_conflict() {
    use gestalt_router::overlap::LiveConflictDetector;
    use gestalt_state::memstate::MemState;

    // 1. Create MemState and subscribe to it
    let mem_state = MemState::new();

    // 2. Spawn LiveConflictDetector in a background task
    let detector = LiveConflictDetector::new(mem_state.clone(), None);
    let handle = tokio::spawn(detector.run());

    // 3. Set Agent A to Running and acquire a lock
    mem_state.set_agent_state("run-1", "agent-a", "running");
    let lock_a = mem_state.try_lock("file.txt", "agent-a", "run-1", 30);
    assert!(lock_a, "Agent A should successfully acquire the lock");

    // 4. Set Agent B to Running and attempt to acquire the same lock
    mem_state.set_agent_state("run-1", "agent-b", "running");
    let lock_b = mem_state.try_lock("file.txt", "agent-b", "run-1", 30);
    assert!(!lock_b, "Agent B lock acquisition should fail (conflict)");

    // 5. Verify that Agent B's state transitions deterministically to "crashed" on conflict
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let state_b = mem_state.get_agent_state("run-1", "agent-b");
    assert_eq!(
        state_b,
        Some("crashed".to_string()),
        "Agent B's state should transition to crashed on conflict"
    );

    // Agent A should still be running cleanly
    let state_a = mem_state.get_agent_state("run-1", "agent-a");
    assert_eq!(
        state_a,
        Some("running".to_string()),
        "Agent A's state should remain running"
    );

    // 6. Dropping mem_state and waiting for the detector task to finish cleanly
    drop(mem_state);

    // Wait for the spawned detector task to exit cleanly (should not block/timeout!)
    let detector_finished = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
    assert!(
        detector_finished.is_ok(),
        "LiveConflictDetector task should terminate cleanly after MemState is dropped"
    );
}

#[test]
fn test_find_overlaps_empty_branches() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    create_branch(&repo_path, "branch-empty-a", &[]);
    create_branch(&repo_path, "branch-empty-b", &[]);

    let active_branches = vec![
        ("agent-empty-a".to_string(), "branch-empty-a".to_string()),
        ("agent-empty-b".to_string(), "branch-empty-b".to_string()),
    ];

    let overlaps =
        gestalt_router::overlap::find_overlaps(&repo_path, &base_sha, &active_branches).unwrap();
    assert!(
        overlaps.is_empty(),
        "expected no overlaps for empty branches"
    );
}

#[test]
fn test_find_overlaps_identical_branches() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    // Both branch-a and branch-b modify identical files
    create_branch(&repo_path, "branch-ident-a", &[("shared.txt", "content a")]);
    // Go back to main
    run_git(&repo_path, &["checkout", "main"]);
    create_branch(&repo_path, "branch-ident-b", &[("shared.txt", "content b")]);

    let active_branches = vec![
        ("agent-ident-a".to_string(), "branch-ident-a".to_string()),
        ("agent-ident-b".to_string(), "branch-ident-b".to_string()),
    ];

    let overlaps =
        gestalt_router::overlap::find_overlaps(&repo_path, &base_sha, &active_branches).unwrap();
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0].files, vec![PathBuf::from("shared.txt")]);
}

#[test]
fn test_find_overlaps_50_plus_branches() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    // Create 55 branches, each modifying a disjoint file
    let mut active_branches = Vec::new();
    for i in 1..=55 {
        let branch_name = format!("branch-{}", i);
        let file_name = format!("file-{}.txt", i);
        create_branch(&repo_path, &branch_name, &[(&file_name, "content")]);
        active_branches.push((format!("agent-{}", i), branch_name));
        run_git(&repo_path, &["checkout", "main"]);
    }

    let overlaps =
        gestalt_router::overlap::find_overlaps(&repo_path, &base_sha, &active_branches).unwrap();
    assert!(
        overlaps.is_empty(),
        "expected no overlaps as all files are disjoint"
    );
}

#[test]
fn test_detect_overlap_shared() {
    let files_a = vec![PathBuf::from("src/main.rs"), PathBuf::from("Cargo.toml")];
    let files_b = vec![PathBuf::from("Cargo.toml"), PathBuf::from("README.md")];
    let result = detect_overlap(&files_a, &files_b);
    assert!(!result.disjoint);
    assert_eq!(result.shared_paths, vec![PathBuf::from("Cargo.toml")]);
}

#[test]
fn test_detect_overlap_multiple_shared() {
    let files_a = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
    ];
    let files_b = vec![
        PathBuf::from("Cargo.toml"),
        PathBuf::from("README.md"),
        PathBuf::from("LICENSE"),
    ];
    let result = detect_overlap(&files_a, &files_b);
    assert!(!result.disjoint);
    assert!(result.shared_paths.contains(&PathBuf::from("Cargo.toml")));
    assert!(result.shared_paths.contains(&PathBuf::from("README.md")));
    assert_eq!(result.shared_paths.len(), 2);
}

#[test]
fn test_detect_overlap_empty() {
    let files_a: Vec<PathBuf> = vec![];
    let files_b = vec![PathBuf::from("Cargo.toml")];
    let result = detect_overlap(&files_a, &files_b);
    assert!(result.disjoint);
    assert!(result.shared_paths.is_empty());
}

#[test]
fn test_get_modified_files_empty_repo() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);
    run_git(&repo_path, &["checkout", "-b", "test-branch"]);

    let files = get_modified_files(&repo_path, &base_sha, "test-branch").unwrap();
    assert!(files.is_empty());
}

#[test]
fn test_get_modified_files_with_changes() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);
    let branch = "feature-branch";
    create_branch(&repo_path, branch, &[("new_file.txt", "new content")]);

    let files = get_modified_files(&repo_path, &base_sha, branch).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].to_string_lossy().contains("new_file.txt"));
}

#[test]
fn test_get_modified_files_multiple_changes() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);
    let branch = "multi-feature";
    create_branch(
        &repo_path,
        branch,
        &[
            ("src/lib.rs", "pub fn new() {}"),
            ("src/main.rs", "fn main() {}"),
        ],
    );

    let files = get_modified_files(&repo_path, &base_sha, branch).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn test_test_mergeability_no_conflicts() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    let branch_a = "branch-a";
    let branch_b = "branch-b";
    create_branch(&repo_path, branch_a, &[("file_a.txt", "content a")]);
    run_git(&repo_path, &["checkout", "main"]);
    create_branch(&repo_path, branch_b, &[("file_b.txt", "content b")]);

    run_git(&repo_path, &["checkout", "main"]);
    let result = test_mergeability(&repo_path, &base_sha, branch_a, branch_b).unwrap();
    assert_eq!(result, MergeTestResult::Clean);
}

#[test]
fn test_overlap_info_construction() {
    let info = OverlapInfo {
        agent_a: "agent-1".to_string(),
        agent_b: "agent-2".to_string(),
        files: vec![PathBuf::from("Cargo.toml")],
    };
    assert_eq!(info.agent_a, "agent-1");
    assert_eq!(info.agent_b, "agent-2");
    assert_eq!(info.files.len(), 1);
}

#[test]
fn test_overlap_result_disjoint() {
    let result = OverlapResult {
        shared_paths: vec![],
        disjoint: true,
    };
    assert!(result.disjoint);
    assert!(result.shared_paths.is_empty());
}
