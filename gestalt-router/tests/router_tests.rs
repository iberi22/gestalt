use gestalt_router::checkpoint::{checkpoint, clean_path, is_symlink_escape, run_git_cmd};
use gestalt_router::integrate::{integrate, MergeResult};
use gestalt_router::run::{AgentResult, AgentSpec, RouterError, RunReport, RunSpec};
use gestalt_router::run_state::{AgentState, RunManifest};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
        allowed_paths: vec!["/tmp".to_string()],
        env: HashMap::from([("KEY".to_string(), "VALUE".to_string())]),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "test task".to_string(),
        agents: vec![agent],
        integration_branch: "integration-1".to_string(),
        timeout: 3600,
        max_parallel: 2,
    };

    let serialized = serde_json::to_string(&spec).unwrap();
    let deserialized: RunSpec = serde_json::from_str(&serialized).unwrap();

    assert_eq!(deserialized.base_ref, "main");
    assert_eq!(deserialized.task, "test task");
    assert_eq!(deserialized.agents.len(), 1);
    assert_eq!(deserialized.agents[0].id, "agent-1");
    assert_eq!(deserialized.agents[0].command, "echo");
    assert_eq!(deserialized.agents[0].args[0], "hello");
    assert_eq!(deserialized.agents[0].allowed_paths[0], "/tmp");
    assert_eq!(deserialized.agents[0].env.get("KEY").unwrap(), "VALUE");
    assert_eq!(deserialized.integration_branch, "integration-1");
    assert_eq!(deserialized.timeout, 3600);
    assert_eq!(deserialized.max_parallel, 2);
}

#[test]
fn test_run_manifest() {
    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "task".to_string(),
        agents: vec![],
        integration_branch: "int".to_string(),
        timeout: 100,
        max_parallel: 1,
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
    let err = RouterError::GitError("failed to merge".to_string());
    assert_eq!(err.to_string(), "Git error: failed to merge");

    let err2 = RouterError::AgentError("agent crashed".to_string());
    assert_eq!(err2.to_string(), "Agent error: agent crashed");

    let err3 = RouterError::Timeout;
    assert_eq!(err3.to_string(), "Timeout error");

    let err4 = RouterError::InvalidSpec("missing command".to_string());
    assert_eq!(err4.to_string(), "Invalid specification: missing command");
}

#[test]
fn test_agent_result_and_report() {
    let result = AgentResult {
        agent_id: "agent-1".to_string(),
        state: AgentState::Success,
        output: Some("done".to_string()),
        error: None,
    };

    let report = RunReport {
        run_id: Uuid::new_v4(),
        agents: vec![result],
        merged_branches: vec!["feature-1".to_string()],
        conflicts: vec![],
        events_path: "/tmp/events".to_string(),
    };

    assert_eq!(report.agents[0].agent_id, "agent-1");
    assert_eq!(report.agents[0].state, AgentState::Success);
    assert_eq!(report.merged_branches[0], "feature-1");
    assert_eq!(report.events_path, "/tmp/events");
}

#[test]
fn test_is_symlink_escape() {
    let root = Path::new("/app/worktree");

    // Normal inside-worktree relative target
    assert!(!is_symlink_escape(root, Path::new("sub/link"), "file.txt"));
    assert!(!is_symlink_escape(
        root,
        Path::new("sub/link"),
        "../file.txt"
    ));

    // Escaping relative target
    assert!(is_symlink_escape(
        root,
        Path::new("sub/link"),
        "../../etc/passwd"
    ));

    // Absolute targets
    assert!(is_symlink_escape(
        root,
        Path::new("sub/link"),
        "/etc/passwd"
    ));
}

#[test]
fn test_clean_path() {
    assert_eq!(clean_path(Path::new("a/b/../c")), PathBuf::from("a/c"));
    assert_eq!(clean_path(Path::new("a/b/../../c")), PathBuf::from("c"));
}

#[test]
fn test_checkpoint_integration() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Init fresh git repo
    let (s, _, _) = run_git_cmd(dir, &["init", "-b", "main"]).unwrap();
    assert_eq!(s, 0);

    // Set mock identity
    let _ = run_git_cmd(dir, &["config", "user.name", "Test User"]);
    let _ = run_git_cmd(dir, &["config", "user.email", "test@example.com"]);

    // Create a regular file and commit it first
    let base_file = dir.join("base.txt");
    std::fs::write(&base_file, "base content").unwrap();
    let (s2, _, _) = run_git_cmd(dir, &["add", "base.txt"]).unwrap();
    assert_eq!(s2, 0);
    let (s3, _, _) = run_git_cmd(dir, &["commit", "-m", "initial"]).unwrap();
    assert_eq!(s3, 0);

    // 1. Hook bypass verification: write pre-commit hook that returns exit 1
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

    // 2. Add gitignored file
    let gitignore_path = dir.join(".gitignore");
    std::fs::write(&gitignore_path, "*.log\n").unwrap();
    let ignored_file = dir.join("agent_run.log");
    std::fs::write(&ignored_file, "some log data").unwrap();

    // 3. Create regular change to commit
    let src_file = dir.join("main.rs");
    std::fs::write(&src_file, "fn main() {}").unwrap();

    // 4. Create an escaped symlink on disk
    #[cfg(unix)]
    {
        let symlink_path = dir.join("leak_link");
        let _ = std::os::unix::fs::symlink("/etc/passwd", &symlink_path);
    }

    // Run checkpoint
    let res = checkpoint(dir, "feat: implement main").unwrap();
    assert!(res.success);

    // Verify gitignored file was logged as ExcludedFile and not staged
    assert!(res.excluded_files.iter().any(|f| f.path == "agent_run.log"));

    // Verify hook was bypassed (we successfully committed even with pre-commit hook exit 1)
    assert!(res.commit_sha.is_some());

    // Verify symlink escape was caught and not committed
    #[cfg(unix)]
    {
        assert!(res.symlink_escapes.iter().any(|se| se.path == "leak_link"));
        // Check that leak_link is not part of the commit
        let (_, stdout, _) = run_git_cmd(dir, &["ls-tree", "HEAD", "--name-only"]).unwrap();
        assert!(!stdout.contains("leak_link"));
    }
}

#[test]
fn test_integrate_binary_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();

    // Init fresh git repo
    let _ = run_git_cmd(dir, &["init", "-b", "main"]);
    let _ = run_git_cmd(dir, &["config", "user.name", "Test User"]);
    let _ = run_git_cmd(dir, &["config", "user.email", "test@example.com"]);

    // Create a base commit with a binary file
    let base_file = dir.join("base.bin");
    std::fs::write(&base_file, b"\x00\x01\x02").unwrap();
    let _ = run_git_cmd(dir, &["add", "base.bin"]);
    let _ = run_git_cmd(dir, &["commit", "-m", "init"]);

    let (_, base_sha, _) = run_git_cmd(dir, &["rev-parse", "main"]).unwrap();
    let base_sha = base_sha.trim();

    // Agent A modifies base.bin on branch-a
    let _ = run_git_cmd(dir, &["checkout", "-b", "branch-a"]);
    std::fs::write(&base_file, b"\x00\x01\x02\x03_agent_a").unwrap();
    let _ = run_git_cmd(
        dir,
        &["commit", "-a", "-m", "agent a changes", "--no-verify"],
    );

    // Agent B modifies base.bin on branch-b
    let _ = run_git_cmd(dir, &["checkout", "main"]);
    let _ = run_git_cmd(dir, &["checkout", "-b", "branch-b"]);
    std::fs::write(&base_file, b"\x00\x01\x02\x04_agent_b").unwrap();
    let _ = run_git_cmd(
        dir,
        &["commit", "-a", "-m", "agent b changes", "--no-verify"],
    );

    // Integrate branch-a and branch-b
    let branches = vec![
        ("agent-a".to_string(), "branch-a".to_string()),
        ("agent-b".to_string(), "branch-b".to_string()),
    ];

    let res = integrate(dir, base_sha, &branches).unwrap();

    match res {
        MergeResult::HardConflict {
            conflicted_files,
            branches_preserved,
        } => {
            assert!(conflicted_files.contains(&"base.bin".to_string()));
            assert!(branches_preserved.contains(&"branch-a".to_string()));
            assert!(branches_preserved.contains(&"branch-b".to_string()));
        }
        _ => panic!("Expected HardConflict for binary file modified by multiple agents"),
    }
}
