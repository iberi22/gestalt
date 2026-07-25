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
