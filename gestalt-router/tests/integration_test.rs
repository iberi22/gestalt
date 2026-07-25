use gestalt_router::run::{AgentSpec, Router, RunSpec};
use gestalt_router::run_state::AgentState;
use std::collections::HashMap;
use std::path::PathBuf;

fn init_test_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().to_path_buf();

    // Init git repo
    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .status()
        .unwrap();
    assert!(status.success());

    // Configure local git user
    std::process::Command::new("git")
        .args(&["config", "user.name", "Gestalt Tester"])
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(&["config", "user.email", "tester@gestalt.local"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // Create 2 initial files
    let file1 = repo_path.join("file1.txt");
    std::fs::write(&file1, "Line 1 of file 1\nLine 2 of file 1\n").unwrap();

    let file2 = repo_path.join("file2.txt");
    std::fs::write(&file2, "Line 1 of file 2\nLine 2 of file 2\n").unwrap();

    // Commit initial state
    std::process::Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(&repo_path)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(&["commit", "-m", "Initial commit"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    // Checkout main branch
    std::process::Command::new("git")
        .args(&["checkout", "-B", "main"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    (dir, repo_path)
}

#[tokio::test]
async fn test_merge_clean_different_files() {
    let (_dir, repo_path) = init_test_repo();
    let router = Router::new(repo_path.clone());

    // Agent 1 modifies file1.txt
    let agent1 = AgentSpec {
        id: "agent-1".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "echo 'Agent 1 change' >> file1.txt".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    // Agent 2 modifies file2.txt
    let agent2 = AgentSpec {
        id: "agent-2".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "echo 'Agent 2 change' >> file2.txt".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "Refactor distinct files".to_string(),
        agents: vec![agent1, agent2],
        integration_branch: "gestalt/integration-test-clean".to_string(),
        timeout: 15,
        max_parallel: 2,
    };

    let report = router.execute(spec).await.unwrap();

    // Verify Agent 1 and Agent 2 results
    assert_eq!(report.agents.len(), 2);
    assert_eq!(report.agents[0].agent_id, "agent-1");
    assert_eq!(report.agents[0].state, AgentState::Success);
    assert_eq!(report.agents[1].agent_id, "agent-2");
    assert_eq!(report.agents[1].state, AgentState::Success);

    // Verify merge and branch integration
    assert_eq!(report.conflicts.len(), 0);
    assert_eq!(report.merged_branches.len(), 2);

    // Checkout integration branch in main repo to verify actual content merged
    std::process::Command::new("git")
        .args(&["checkout", "gestalt/integration-test-clean"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    let content1 = std::fs::read_to_string(repo_path.join("file1.txt")).unwrap();
    let content2 = std::fs::read_to_string(repo_path.join("file2.txt")).unwrap();

    assert!(content1.contains("Agent 1 change"));
    assert!(content2.contains("Agent 2 change"));
}

#[tokio::test]
async fn test_merge_conflict_same_region() {
    let (_dir, repo_path) = init_test_repo();
    let router = Router::new(repo_path.clone());

    // Agent 1 overwrites file1.txt to A
    let agent1 = AgentSpec {
        id: "agent-1".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "echo 'Conflict A' > file1.txt".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    // Agent 2 overwrites file1.txt to B
    let agent2 = AgentSpec {
        id: "agent-2".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "echo 'Conflict B' > file1.txt".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "Conflicting edits on same file".to_string(),
        agents: vec![agent1, agent2],
        integration_branch: "gestalt/integration-test-conflict".to_string(),
        timeout: 15,
        max_parallel: 2,
    };

    let report = router.execute(spec).await.unwrap();

    // Verify results
    assert_eq!(report.agents.len(), 2);
    assert_eq!(report.agents[0].state, AgentState::Success);
    assert_eq!(report.agents[1].state, AgentState::Success);

    // One merge should succeed, and the other should result in conflict
    assert_eq!(report.merged_branches.len(), 1);
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0], "file1.txt");
}

#[tokio::test]
async fn test_agent_timeout_sigkill() {
    let (_dir, repo_path) = init_test_repo();
    let router = Router::new(repo_path);

    // Agent runs a slow command that exceeds the timeout
    let slow_agent = AgentSpec {
        id: "slow-agent".to_string(),
        command: "sleep".to_string(),
        args: vec!["10".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "Timeout test".to_string(),
        agents: vec![slow_agent],
        integration_branch: "gestalt/integration-test-timeout".to_string(),
        timeout: 1, // Only 1 second timeout
        max_parallel: 1,
    };

    let report = router.execute(spec).await.unwrap();

    assert_eq!(report.agents.len(), 1);
    assert_eq!(report.agents[0].agent_id, "slow-agent");
    assert_eq!(report.agents[0].state, AgentState::Timeout);
    assert!(report.agents[0].error.as_ref().unwrap().contains("timed out"));
    assert_eq!(report.merged_branches.len(), 0);
}

#[tokio::test]
async fn test_symlink_escape_detection() {
    let (_dir, repo_path) = init_test_repo();
    let router = Router::new(repo_path.clone());

    // Agent attempts to create an escaping symlink and a legit change
    let malicious_agent = AgentSpec {
        id: "escape-agent".to_string(),
        command: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            "ln -s /etc/passwd escape.txt && echo 'legit' > legit.txt".to_string(),
        ],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let spec = RunSpec {
        base_ref: "main".to_string(),
        task: "Symlink escape test".to_string(),
        agents: vec![malicious_agent],
        integration_branch: "gestalt/integration-test-symlink".to_string(),
        timeout: 15,
        max_parallel: 1,
    };

    let report = router.execute(spec).await.unwrap();

    assert_eq!(report.agents.len(), 1);
    assert_eq!(report.agents[0].agent_id, "escape-agent");
    assert_eq!(report.agents[0].state, AgentState::Quarantined);
    assert_eq!(report.merged_branches.len(), 1);

    // Checkout integration branch and verify that legit.txt exists but escape.txt does not!
    std::process::Command::new("git")
        .args(&["checkout", "gestalt/integration-test-symlink"])
        .current_dir(&repo_path)
        .status()
        .unwrap();

    assert!(repo_path.join("legit.txt").exists());
    assert!(!repo_path.join("escape.txt").exists());
}
