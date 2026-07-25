use gestalt_router::agent::{AgentRunner, SubprocessRunner};
use gestalt_router::run::AgentSpec;
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct MockEventLog;
impl gestalt_router::agent::EventLog for MockEventLog {}

fn init_temp_git_repo() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();
    assert!(status.success());

    // Configure local git user for commits
    std::process::Command::new("git")
        .arg("config")
        .arg("user.name")
        .arg("Gestalt Test")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();
    std::process::Command::new("git")
        .arg("config")
        .arg("user.email")
        .arg("test@gestalt.local")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    // Create an initial commit so git status works normally
    let initial_file = temp_dir.path().join("README.md");
    std::fs::write(&initial_file, "# Test Repo").unwrap();

    std::process::Command::new("git")
        .arg("add")
        .arg("README.md")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();
    std::process::Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg("Initial commit")
        .current_dir(temp_dir.path())
        .status()
        .unwrap();

    temp_dir
}

#[tokio::test]
async fn test_agent_success() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let spec = AgentSpec {
        id: "success-agent".to_string(),
        command: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            "echo -n 'hello world'; echo 'changes' >> README.md".to_string(),
        ],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let outcome = runner
        .run(&spec, temp_git.path(), "test success", &MockEventLog)
        .await
        .unwrap();

    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.stdout_path.exists());
    let stdout_content = std::fs::read_to_string(&outcome.stdout_path).unwrap();
    assert_eq!(stdout_content, "hello world");

    assert!(!outcome.files_changed.is_empty());
    assert!(outcome.files_changed[0].ends_with("README.md"));

    let _ = std::fs::remove_file(outcome.stdout_path);
    let _ = std::fs::remove_file(outcome.stderr_path);
}

#[tokio::test]
async fn test_agent_failure() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(10));

    let spec = AgentSpec {
        id: "failure-agent".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "echo 'some error' >&2; exit 1".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let outcome = runner
        .run(&spec, temp_git.path(), "test failure", &MockEventLog)
        .await
        .unwrap();

    assert_eq!(outcome.exit_code, Some(1));
    assert!(outcome.stderr_path.exists());
    let stderr_content = std::fs::read_to_string(&outcome.stderr_path).unwrap();
    assert!(stderr_content.contains("some error"));

    let _ = std::fs::remove_file(outcome.stdout_path);
    let _ = std::fs::remove_file(outcome.stderr_path);
}

#[tokio::test]
async fn test_agent_timeout() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(1));

    let spec = AgentSpec {
        id: "timeout-agent".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "sleep 999".to_string()],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let start = Instant::now();
    let outcome = runner
        .run(&spec, temp_git.path(), "test timeout", &MockEventLog)
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(outcome.exit_code, Some(-1));
    assert!(elapsed >= Duration::from_secs(1));
    assert!(elapsed < Duration::from_secs(6));

    let _ = std::fs::remove_file(outcome.stdout_path);
    let _ = std::fs::remove_file(outcome.stderr_path);
}

#[tokio::test]
async fn test_process_group_kill() {
    let temp_git = init_temp_git_repo();
    let runner = SubprocessRunner::new(Duration::from_secs(1));

    let pid_file = temp_git.path().join("child.pid");
    let script = format!(
        "sleep 999 & echo $! > {}; sleep 999",
        pid_file.to_str().unwrap()
    );

    let spec = AgentSpec {
        id: "pg-agent".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), script],
        allowed_paths: vec![],
        env: HashMap::new(),
    };

    let outcome = runner
        .run(&spec, temp_git.path(), "test pg kill", &MockEventLog)
        .await
        .unwrap();
    assert_eq!(outcome.exit_code, Some(-1));

    assert!(pid_file.exists());
    let pid_str = std::fs::read_to_string(&pid_file).unwrap();
    let grandchild_pid: i32 = pid_str.trim().parse().unwrap();

    #[cfg(unix)]
    {
        // Wait briefly for the grandchild kill to propagate
        tokio::time::sleep(Duration::from_millis(500)).await;
        let is_alive = unsafe { libc::kill(grandchild_pid, 0) == 0 };
        assert!(!is_alive, "Grandchild process sleep 999 should have been killed!");
    }

    let _ = std::fs::remove_file(outcome.stdout_path);
    let _ = std::fs::remove_file(outcome.stderr_path);
}
