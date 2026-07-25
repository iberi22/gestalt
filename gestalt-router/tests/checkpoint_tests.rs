use gestalt_router::checkpoint::{checkpoint, clean_path, is_symlink_escape};
use gestalt_router::checkpoint::{CheckpointResult, SymlinkEscape, ExcludedFile};
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gestalt-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();

        Command::new("git")
            .arg("init").current_dir(&path).output().unwrap();
        Command::new("git")
            .args(["config", "user.name", "Gestalt Test"])
            .current_dir(&path).output().unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@gestalt.local"])
            .current_dir(&path).output().unwrap();

        std::fs::write(path.join("initial.txt"), "initial content").unwrap();
        Command::new("git")
            .args(["add", "initial.txt"]).current_dir(&path).output().unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"]).current_dir(&path).output().unwrap();
        Command::new("git")
            .args(["branch", "-m", "main"]).current_dir(&path).output().unwrap();

        TempRepo { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[test]
fn test_clean_path_normal() {
    let cleaned = clean_path(Path::new("/home/user/project/src/lib.rs"));
    assert_eq!(cleaned, PathBuf::from("/home/user/project/src/lib.rs"));
}

#[test]
fn test_clean_path_with_dotdot() {
    let cleaned = clean_path(Path::new("/home/user/project/../src/lib.rs"));
    assert_eq!(cleaned, PathBuf::from("/home/user/src/lib.rs"));
}

#[test]
fn test_clean_path_with_dot() {
    let cleaned = clean_path(Path::new("./src/./lib.rs"));
    assert_eq!(cleaned, PathBuf::from("src/lib.rs"));
}

#[test]
fn test_clean_path_absolute_root() {
    let cleaned = clean_path(Path::new("/"));
    assert_eq!(cleaned, PathBuf::from("/"));
}

#[test]
fn test_checkpoint_creates_commit() {
    let repo = TempRepo::new();
    let test_file = repo.path().join("new_file.txt");
    std::fs::write(&test_file, "new content").unwrap();

    let result = checkpoint(repo.path(), "test checkpoint").unwrap();
    assert!(result.success, "Checkpoint should succeed");
    assert!(result.commit_sha.is_some(), "Should have a commit SHA");
}

#[test]
fn test_checkpoint_clean_repo_no_commit() {
    let repo = TempRepo::new();
    // touch a file outside the worktree doesn't count
    // Actually, the repo is clean, so no commit
    let result = checkpoint(repo.path(), "no changes").unwrap();
    assert!(result.commit_sha.is_none(), "No commit on clean repo");
}

#[test]
fn test_symlink_escape_detection() {
    let repo = TempRepo::new();
    let worktree = repo.path();
    let symlink_path = Path::new("escape.link");
    let target = "/etc/passwd";

    assert!(
        is_symlink_escape(worktree, symlink_path, target),
        "Symlink to /etc should be detected as escape"
    );
}

#[test]
fn test_symlink_escape_internal() {
    let repo = TempRepo::new();
    let worktree = repo.path();
    let symlink_path = Path::new("internal.link");
    let internal_target = worktree.join("initial.txt");
    let target_str = internal_target.to_str().unwrap();

    assert!(
        !is_symlink_escape(worktree, symlink_path, target_str),
        "Symlink inside worktree should NOT be detected as escape"
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
    assert_eq!(result.commit_sha.as_deref().unwrap(), "abc123");
}

#[test]
fn test_checkpoint_result_no_errors() {
    let result = CheckpointResult {
        success: true,
        commit_sha: Some("def456".to_string()),
        symlink_escapes: vec![],
        excluded_files: vec![],
        files_committed: vec!["README.md".to_string()],
    };
    assert!(result.symlink_escapes.is_empty());
    assert!(result.excluded_files.is_empty());
}

#[test]
fn test_symlink_escape_struct() {
    let escape = SymlinkEscape {
        path: "symlink.txt".to_string(),
        target: "/outside/repo".to_string(),
    };
    assert_eq!(escape.path, "symlink.txt");
    assert_eq!(escape.target, "/outside/repo");
}

#[test]
fn test_excluded_file_struct() {
    let excluded = ExcludedFile {
        path: "target/debug/build.log".to_string(),
        reason: "matches .gitignore pattern".to_string(),
    };
    assert!(excluded.path.contains("target"));
    assert!(excluded.reason.contains("gitignore"));
}

#[test]
fn test_checkpoint_multiple_files() {
    let repo = TempRepo::new();
    std::fs::write(repo.path().join("file_a.rs"), "fn a() {}").unwrap();
    std::fs::write(repo.path().join("file_b.rs"), "fn b() {}").unwrap();

    let result = checkpoint(repo.path(), "two files").unwrap();
    assert!(result.success);
    assert!(result.commit_sha.is_some());
    assert_eq!(result.files_committed.len(), 2, "Both files should be committed");
}
