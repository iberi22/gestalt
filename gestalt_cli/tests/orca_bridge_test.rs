#[path = "../src/observe/orca_bridge.rs"]
pub mod orca_bridge;

use std::fs;

#[test]
fn test_parse_real_endpoint_env_format() {
    let temp_dir = tempfile::tempdir().unwrap();
    let env_path = temp_dir.path().join("endpoint.env");

    let content = r#"
# Orca Agent Hooks Configuration
ORCA_AGENT_HOOK_PORT=42423
ORCA_AGENT_HOOK_TOKEN="some-secure-token-value-here"
"#;
    fs::write(&env_path, content).unwrap();

    let config = orca_bridge::read_orca_endpoint(&env_path).unwrap();
    assert_eq!(config.port, 42423);
    assert_eq!(config.token, "some-secure-token-value-here");
}

#[test]
fn test_parse_real_endpoint_env_format_alternative() {
    let temp_dir = tempfile::tempdir().unwrap();
    let env_path = temp_dir.path().join("endpoint.env");

    let content = r#"
ORCA_AGENT_HOOK_PORT=50000
TOKEN='another-token'
"#;
    fs::write(&env_path, content).unwrap();

    let config = orca_bridge::read_orca_endpoint(&env_path).unwrap();
    assert_eq!(config.port, 50000);
    assert_eq!(config.token, "another-token");
}

#[test]
fn test_last_status_json_fallback() {
    let temp_dir = tempfile::tempdir().unwrap();
    let status_path = temp_dir.path().join("last-status.json");

    let content = r#"[
        {
            "agent": "claude",
            "status": "running",
            "message": "Executing tasks",
            "run_id": "run-abc",
            "project": "gestalt",
            "timestamp": "2026-08-06T12:00:00Z"
        },
        {
            "agent": "codex",
            "state": "success",
            "summary": "Finished workspace sweep",
            "run_id": "run-xyz",
            "project": "gestalt"
        }
    ]"#;
    fs::write(&status_path, content).unwrap();

    let events = orca_bridge::read_last_status(&status_path).expect("Failed to read last-status.json");
    assert_eq!(events.len(), 2);

    let ev1 = &events[0];
    assert_eq!(ev1.agent, "claude");
    assert_eq!(ev1.event_type, "run_started");
    assert_eq!(ev1.state.as_deref(), Some("Running"));
    assert_eq!(ev1.summary, "Executing tasks");
    assert_eq!(ev1.run_id.as_deref(), Some("run-abc"));
    assert_eq!(ev1.project.as_deref(), Some("gestalt"));
    assert_eq!(ev1.ts, "2026-08-06T12:00:00Z");

    let ev2 = &events[1];
    assert_eq!(ev2.agent, "codex");
    assert_eq!(ev2.event_type, "run_finished");
    assert_eq!(ev2.state.as_deref(), Some("Success"));
    assert_eq!(ev2.summary, "Finished workspace sweep");
    assert_eq!(ev2.run_id.as_deref(), Some("run-xyz"));
    assert_eq!(ev2.project.as_deref(), Some("gestalt"));
}

#[tokio::test]
async fn test_endpoint_unreachable_graceful_degradation() {
    let _guard = observe_env_guard();

    // Set HOME to a temporary directory without any endpoint.env or last-status.json
    let temp_dir = tempfile::tempdir().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    // Should degrade gracefully, returning empty and Ok, without panic or hang
    let result = orca_bridge::poll_orca().await;
    assert!(result.is_ok());
    let events = result.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn test_poll_unreachable_server_with_fallback() {
    let _guard = observe_env_guard();

    let temp_dir = tempfile::tempdir().unwrap();
    let agent_hooks_dir = temp_dir.path().join(".config/orca/agent-hooks");
    fs::create_dir_all(&agent_hooks_dir).unwrap();

    // Write endpoint.env with a port that is unlikely to be active
    let env_content = "ORCA_AGENT_HOOK_PORT=42423\nORCA_AGENT_HOOK_TOKEN=test-token";
    fs::write(agent_hooks_dir.join("endpoint.env"), env_content).unwrap();

    // Write last-status.json fallback file
    let last_status_content = r#"[
        {
            "agent": "kimi",
            "status": "failed",
            "message": "Out of memory"
        }
    ]"#;
    fs::write(agent_hooks_dir.join("last-status.json"), last_status_content).unwrap();

    std::env::set_var("HOME", temp_dir.path());

    // Should try the unreachable server, timeout/fail, and fallback to last-status.json
    let result = orca_bridge::poll_orca().await;
    assert!(result.is_ok());
    let events = result.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].agent, "kimi");
    assert_eq!(events[0].event_type, "run_finished");
    assert_eq!(events[0].state.as_deref(), Some("Crashed"));
    assert_eq!(events[0].summary, "Out of memory");
}

// Environment guard helper to run tests thread-safely when they modify HOME.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    orig_home: Option<std::ffi::OsString>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(ref h) = self.orig_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
    }
}

fn observe_env_guard() -> EnvGuard {
    let lock = ENV_MUTEX.lock().unwrap();
    let orig_home = std::env::var_os("HOME");
    EnvGuard {
        _lock: lock,
        orig_home,
    }
}
