use gestalt_router::overlap::{
    detect_overlap, get_modified_files, test_mergeability, ConflictKind, MergeTestResult,
};
use std::path::{Path, PathBuf};

fn run_git(repo_path: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
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

    // Initialize git repo
    run_git(&temp_dir, &["init"]);
    run_git(&temp_dir, &["config", "user.name", "Test User"]);
    run_git(&temp_dir, &["config", "user.email", "test@example.com"]);

    // Create initial base commit
    let file1 = temp_dir.join("file1.txt");
    std::fs::write(&file1, "base content\nline 2\n").unwrap();
    run_git(&temp_dir, &["add", "file1.txt"]);
    run_git(&temp_dir, &["commit", "-m", "initial commit"]);
    run_git(&temp_dir, &["branch", "-m", "main"]);

    temp_dir
}

fn cleanup_repo(repo_path: PathBuf) {
    let _ = std::fs::remove_dir_all(repo_path);
}

#[test]
fn test_detect_overlap_disjoint() {
    let files_a = vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/run.rs")];
    let files_b = vec![PathBuf::from("tests/tests.rs"), PathBuf::from("Cargo.toml")];

    let result = detect_overlap(&files_a, &files_b);
    assert!(result.disjoint);
    assert!(result.shared_paths.is_empty());
}

#[test]
fn test_detect_overlap_with_shared() {
    let files_a = vec![
        PathBuf::from("src/lib.rs"),
        PathBuf::from("src/run.rs"),
        PathBuf::from("Cargo.toml"),
    ];
    let files_b = vec![
        PathBuf::from("tests/tests.rs"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from("src/lib.rs"),
    ];

    let result = detect_overlap(&files_a, &files_b);
    assert!(!result.disjoint);
    assert_eq!(result.shared_paths.len(), 2);
    assert_eq!(result.shared_paths[0], PathBuf::from("Cargo.toml"));
    assert_eq!(result.shared_paths[1], PathBuf::from("src/lib.rs"));
}

#[test]
fn test_get_modified_files_logic() {
    let repo_path = create_temp_git_repo();

    // Get the base SHA (initial commit)
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    // Create branch-a and switch to it
    run_git(&repo_path, &["checkout", "-b", "branch-a"]);

    // Modify file1.txt
    let file1 = repo_path.join("file1.txt");
    std::fs::write(&file1, "modified content\nline 2\n").unwrap();

    // Create a new file
    let file2 = repo_path.join("file2.txt");
    std::fs::write(&file2, "new file content\n").unwrap();

    run_git(&repo_path, &["add", "file1.txt", "file2.txt"]);
    run_git(&repo_path, &["commit", "-m", "modify and add"]);

    // Get modified files on branch-a relative to base_sha
    let modified = get_modified_files(&repo_path, &base_sha, "branch-a").unwrap();

    assert_eq!(modified.len(), 2);
    assert!(modified.contains(&PathBuf::from("file1.txt")));
    assert!(modified.contains(&PathBuf::from("file2.txt")));

    cleanup_repo(repo_path);
}

#[test]
fn test_test_mergeability_clean() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    // Create branch-a and modify file1.txt
    run_git(&repo_path, &["checkout", "-b", "branch-a"]);
    let file1 = repo_path.join("file1.txt");
    std::fs::write(&file1, "branch-a content\nline 2\n").unwrap();
    run_git(&repo_path, &["add", "file1.txt"]);
    run_git(&repo_path, &["commit", "-m", "change in A"]);

    // Go back to main, create branch-b and add a new file (disjoint)
    run_git(&repo_path, &["checkout", "main"]);
    run_git(&repo_path, &["checkout", "-b", "branch-b"]);
    let file3 = repo_path.join("file3.txt");
    std::fs::write(&file3, "branch-b content\n").unwrap();
    run_git(&repo_path, &["add", "file3.txt"]);
    run_git(&repo_path, &["commit", "-m", "change in B"]);

    // Test mergeability
    let result = test_mergeability(&repo_path, &base_sha, "branch-a", "branch-b").unwrap();
    assert!(matches!(result, MergeTestResult::Clean));

    cleanup_repo(repo_path);
}

#[test]
fn test_test_mergeability_conflict() {
    let repo_path = create_temp_git_repo();
    let base_sha = run_git(&repo_path, &["rev-parse", "HEAD"]);

    // Create branch-a and modify file1.txt
    run_git(&repo_path, &["checkout", "-b", "branch-a"]);
    let file1 = repo_path.join("file1.txt");
    std::fs::write(&file1, "branch-a change\nline 2\n").unwrap();
    run_git(&repo_path, &["add", "file1.txt"]);
    run_git(&repo_path, &["commit", "-m", "change in A"]);

    // Go back to main, create branch-b and modify same file differently
    run_git(&repo_path, &["checkout", "main"]);
    run_git(&repo_path, &["checkout", "-b", "branch-b"]);
    std::fs::write(&file1, "branch-b change\nline 2\n").unwrap();
    run_git(&repo_path, &["add", "file1.txt"]);
    run_git(&repo_path, &["commit", "-m", "change in B"]);

    // Test mergeability
    let result = test_mergeability(&repo_path, &base_sha, "branch-a", "branch-b").unwrap();
    match result {
        MergeTestResult::Conflicts(conflicts) => {
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].path, PathBuf::from("file1.txt"));
            // Depending on git version and exact output, it could parse as Content or BothModified.
            // Both are acceptable conflict kinds.
            assert!(
                conflicts[0].kind == ConflictKind::Content
                    || conflicts[0].kind == ConflictKind::BothModified
            );
        }
        _ => panic!("Expected conflicts, got Clean"),
    }

    cleanup_repo(repo_path);
}
