use gestalt_router::checkpoint::Checkpointer;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gestalt-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();

        // git init
        let output = Command::new("git")
            .arg("init")
            .current_dir(&path)
            .output()
            .expect("failed to init git");
        assert!(output.status.success(), "git init failed");

        // git config user.name
        Command::new("git")
            .args(&["config", "user.name", "Gestalt Test"])
            .current_dir(&path)
            .output()
            .unwrap();

        // git config user.email
        Command::new("git")
            .args(&["config", "user.email", "test@gestalt.io"])
            .current_dir(&path)
            .output()
            .unwrap();

        Self { path }
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_checkpoint_normal_and_no_changes() {
    let repo = TempRepo::new();
    let agent_id = "agent-1";
    let run_id = Uuid::new_v4();

    // 1. Create a normal file
    let file1_path = repo.path.join("file1.txt");
    std::fs::write(&file1_path, "hello world").unwrap();

    // Run first checkpoint
    let res = Checkpointer::checkpoint(&repo.path, agent_id, run_id).unwrap();
    assert!(!res.sha.is_empty(), "SHA should not be empty");
    assert_eq!(res.files_changed, vec!["file1.txt"]);
    assert!(res.warnings.is_empty(), "Warnings should be empty: {:?}", res.warnings);

    // Verify commit message
    let commit_msg_output = Command::new("git")
        .args(&["log", "-1", "--pretty=%B"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    let commit_msg = String::from_utf8_lossy(&commit_msg_output.stdout).trim().to_string();
    assert_eq!(commit_msg, format!("gestalt: checkpoint {} {}", agent_id, run_id));

    // 2. Run checkpoint again with no changes
    let res2 = Checkpointer::checkpoint(&repo.path, agent_id, run_id).unwrap();
    assert!(res2.sha.is_empty(), "SHA should be empty when no changes");
    assert!(res2.files_changed.is_empty(), "Files changed should be empty");
    assert!(res2.warnings.contains(&"NoChanges".to_string()), "Should contain NoChanges warning");
}

#[test]
fn test_checkpoint_ignored_files() {
    let repo = TempRepo::new();
    let agent_id = "agent-ignored";
    let run_id = Uuid::new_v4();

    // Create a .gitignore
    let gitignore_path = repo.path.join(".gitignore");
    std::fs::write(&gitignore_path, "ignored.txt\n").unwrap();

    // Create ignored file
    let ignored_path = repo.path.join("ignored.txt");
    std::fs::write(&ignored_path, "secret content").unwrap();

    // Create a normal file so there is at least one change to commit
    let file2_path = repo.path.join("file2.txt");
    std::fs::write(&file2_path, "regular content").unwrap();

    // Run checkpoint
    let res = Checkpointer::checkpoint(&repo.path, agent_id, run_id).unwrap();
    assert!(!res.sha.is_empty());

    // Check files changed: should include .gitignore and file2.txt, but NOT ignored.txt
    assert!(res.files_changed.contains(&".gitignore".to_string()));
    assert!(res.files_changed.contains(&"file2.txt".to_string()));
    assert!(!res.files_changed.contains(&"ignored.txt".to_string()));

    // Check warning: should warn about ignored.txt
    let ignored_warn = res.warnings.iter().any(|w| w.contains("ExcludedFile: ignored.txt") || w.contains("ignored.txt"));
    assert!(ignored_warn, "Should contain warning for ignored.txt: {:?}", res.warnings);
}

#[test]
#[cfg(unix)]
fn test_checkpoint_symlink_escape() {
    let repo = TempRepo::new();
    let agent_id = "agent-symlink";
    let run_id = Uuid::new_v4();

    // Create a normal file
    let file1_path = repo.path.join("file1.txt");
    std::fs::write(&file1_path, "base file").unwrap();

    // Create a safe symlink pointing to file1.txt
    let safe_link_path = repo.path.join("safe_link");
    std::os::unix::fs::symlink("file1.txt", &safe_link_path).unwrap();

    // Create an escaped symlink pointing outside worktree (e.g., to /etc/passwd)
    let escaped_link_path = repo.path.join("escaped_link");
    std::os::unix::fs::symlink("/etc/passwd", &escaped_link_path).unwrap();

    // Run checkpoint
    let res = Checkpointer::checkpoint(&repo.path, agent_id, run_id).unwrap();
    assert!(!res.sha.is_empty());

    // Safe link and normal file should be included
    assert!(res.files_changed.contains(&"file1.txt".to_string()));
    assert!(res.files_changed.contains(&"safe_link".to_string()));

    // Escaped link should NOT be included
    assert!(!res.files_changed.contains(&"escaped_link".to_string()));

    // Warning list should contain SymlinkEscape for escaped_link
    let escape_warn = res.warnings.iter().any(|w| w.contains("SymlinkEscape") && w.contains("escaped_link"));
    assert!(escape_warn, "Should warn about symlink escape: {:?}", res.warnings);
}

#[test]
fn test_checkpoint_bypass_pre_commit_hooks() {
    let repo = TempRepo::new();
    let agent_id = "agent-bypass";
    let run_id = Uuid::new_v4();

    // Create a failing pre-commit hook
    let hooks_dir = repo.path.join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let hook_file = hooks_dir.join("pre-commit");
    std::fs::write(&hook_file, "#!/bin/sh\necho 'pre-commit hook blocked!'\nexit 1\n").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Create a file to commit
    let file3_path = repo.path.join("file3.txt");
    std::fs::write(&file3_path, "bypassed").unwrap();

    // Run checkpoint: if hooks are not bypassed, this will return GitError because pre-commit exited with 1
    let res = Checkpointer::checkpoint(&repo.path, agent_id, run_id).unwrap();
    assert!(!res.sha.is_empty(), "Commit should succeed and return SHA despite blocking hook");
    assert_eq!(res.files_changed, vec!["file3.txt"]);
}
