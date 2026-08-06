//! Integration tests for the agent pipeline chaining functionality
//!
//! describe: This test module verifies TOML spec parsing, topological sorting,
//! sequential execution, event emission, and environment context propagation.
//!
//! it("should topologically sort dependencies and detect cycles"):
//! it("should execute mock steps in correct order"):
//! it("should propagate exit failures and continue on error when specified"):

#[path = "../src/agent_wrapper.rs"]
pub mod agent_wrapper;

#[path = "../src/chain.rs"]
pub mod chain;

use chain::{topological_sort, run_chain, ChainStep};
use std::fs;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

/// Thread safety guard that serializes test execution and isolates env variables.
struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    old_home: Option<std::ffi::OsString>,
    old_xavier: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn new(temp_home: &std::path::Path) -> Self {
        let lock = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old_home = std::env::var_os("HOME");
        let old_xavier = std::env::var_os("XAVIER_CONTEXT");

        std::env::set_var("HOME", temp_home);
        std::env::remove_var("XAVIER_CONTEXT");

        Self {
            _lock: lock,
            old_home,
            old_xavier,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(ref h) = self.old_home {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(ref x) = self.old_xavier {
            std::env::set_var("XAVIER_CONTEXT", x);
        } else {
            std::env::remove_var("XAVIER_CONTEXT");
        }
    }
}

#[test]
fn test_describe_topological_sort_success() {
    // it("should sort independent steps and simple dependent steps correctly")
    let steps = vec![
        ChainStep {
            name: "implement".to_string(),
            agent: "codex".to_string(),
            task: "implement research".to_string(),
            on_event: "run_finished".to_string(),
            requires: vec!["scout".to_string()],
        },
        ChainStep {
            name: "scout".to_string(),
            agent: "opencode".to_string(),
            task: "research problem".to_string(),
            on_event: "run_finished".to_string(),
            requires: vec![],
        },
    ];

    let sorted = topological_sort(&steps).expect("Should sort successfully");
    assert_eq!(sorted.len(), 2);
    assert_eq!(sorted[0].name, "scout");
    assert_eq!(sorted[1].name, "implement");
}

#[test]
fn test_describe_topological_sort_missing_dependency() {
    // it("should fail if a required dependency is missing")
    let steps = vec![ChainStep {
        name: "implement".to_string(),
        agent: "codex".to_string(),
        task: "implement".to_string(),
        on_event: "run_finished".to_string(),
        requires: vec!["missing_step".to_string()],
    }];

    let err = topological_sort(&steps).unwrap_err();
    assert!(err.contains("requires undefined step"));
}

#[test]
fn test_describe_topological_sort_cycle_detected() {
    // it("should fail if there is a circular dependency loop")
    let steps = vec![
        ChainStep {
            name: "step_a".to_string(),
            agent: "agent".to_string(),
            task: "task".to_string(),
            on_event: "run_finished".to_string(),
            requires: vec!["step_b".to_string()],
        },
        ChainStep {
            name: "step_b".to_string(),
            agent: "agent".to_string(),
            task: "task".to_string(),
            on_event: "run_finished".to_string(),
            requires: vec!["step_a".to_string()],
        },
    ];

    let err = topological_sort(&steps).unwrap_err();
    assert!(err.contains("Circular dependency or cycle detected"));
}

#[tokio::test]
async fn test_it_executes_mock_chain_success() {
    // it("should execute mock steps sequentially and propagate summaries forward")
    let dir = tempdir().expect("Failed to create temp dir");
    let _guard = EnvGuard::new(dir.path());
    let spec_path = dir.path().join("pipeline.toml");

    let spec_content = r#"
[[steps]]
name = "step1"
agent = "echo"
task = "hello_from_step1"

[[steps]]
name = "step2"
agent = "echo"
task = "hello_from_step2"
requires = ["step1"]
"#;

    fs::write(&spec_path, spec_content).expect("Failed to write temp spec");

    let result = run_chain(spec_path.to_str().unwrap(), Some("test-project".to_string()), false).await;
    assert!(result.is_ok(), "Chain execution failed: {:?}", result);

    // Verify context propagation
    let xavier_ctx = std::env::var("XAVIER_CONTEXT").expect("XAVIER_CONTEXT should be set");
    assert!(xavier_ctx.contains("hello_from_step1"), "Should contain output of step1");
    assert!(xavier_ctx.contains("hello_from_step2"), "Should contain output of step2");
}

#[tokio::test]
async fn test_it_halts_chain_on_error() {
    // it("should stop the chain when a step exits with a non-zero status")
    let dir = tempdir().expect("Failed to create temp dir");
    let _guard = EnvGuard::new(dir.path());
    let spec_path = dir.path().join("pipeline.toml");

    // "false" is a standard Unix command that exits with code 1
    let spec_content = r#"
[[steps]]
name = "fail_step"
agent = "false"
task = ""

[[steps]]
name = "subsequent_step"
agent = "echo"
task = "should_not_run"
requires = ["fail_step"]
"#;

    fs::write(&spec_path, spec_content).expect("Failed to write temp spec");

    let result = run_chain(spec_path.to_str().unwrap(), Some("test-project".to_string()), false).await;
    assert!(result.is_err(), "Chain should have failed");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Chain halted") || err_msg.contains("failed"));
}

#[tokio::test]
async fn test_it_continues_on_error_when_flag_enabled() {
    // it("should execute subsequent steps even if an intermediate step fails when continue_on_error is true")
    let dir = tempdir().expect("Failed to create temp dir");
    let _guard = EnvGuard::new(dir.path());
    let spec_path = dir.path().join("pipeline.toml");

    let spec_content = r#"
[[steps]]
name = "fail_step"
agent = "false"
task = ""

[[steps]]
name = "ok_step"
agent = "echo"
task = "hello_after_failure"
"#;

    fs::write(&spec_path, spec_content).expect("Failed to write temp spec");

    let result = run_chain(spec_path.to_str().unwrap(), Some("test-project".to_string()), true).await;
    assert!(result.is_err(), "Overall chain should still return Err at the end due to a failed step");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("failed to execute successfully"), "Expected failure summary, got: {}", err_msg);

    // Verify context propagation of the successful step (indicating it ran even after the failure)
    let xavier_ctx = std::env::var("XAVIER_CONTEXT").expect("XAVIER_CONTEXT should be set");
    assert!(xavier_ctx.contains("hello_after_failure"), "The successful step should have executed");
}
