#[path = "../src/observe/proc_monitor.rs"]
mod proc_monitor;

use gestalt_router::run::AgentSpec;
use proc_monitor::{match_agent, ProcMonitor};
use std::fs::{create_dir_all, remove_dir_all, write};
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_match_agent_exact_and_generic() {
    let specs: Vec<AgentSpec> = vec![];

    // hermes-agent python cmdline matches hermes
    let hermes_cmdline = vec![
        "/home/belal/.local/share/uv/tools/hermes-agent/bin/python",
        "agent.py",
    ];
    assert_eq!(match_agent(&hermes_cmdline, &specs), Some("hermes"));

    // node with opencode in argv matches opencode
    let opencode_cmdline = vec!["node", "opencode-agent.js"];
    assert_eq!(match_agent(&opencode_cmdline, &specs), Some("opencode"));

    // generic node does NOT match any agent
    let generic_node = vec!["node", "server.js"];
    assert_eq!(match_agent(&generic_node, &specs), None);

    // generic python does NOT match any agent
    let generic_python = vec!["python", "script.py"];
    assert_eq!(match_agent(&generic_python, &specs), None);

    // exact binary matching with other agent names
    let jules_cmdline = vec!["jules-agent", "run"];
    assert_eq!(match_agent(&jules_cmdline, &specs), Some("jules"));
}

#[test]
fn test_match_agent_with_specs() {
    let specs = vec![
        AgentSpec {
            id: "my-hermes-spec".to_string(),
            command: "hermes-agent".to_string(),
            args: vec![],
            allowed_paths: None,
            env: None,
        },
        AgentSpec {
            id: "my-opencode-spec".to_string(),
            command: "opencode-agent.js".to_string(),
            args: vec![],
            allowed_paths: None,
            env: None,
        },
    ];

    let hermes_cmdline = vec![
        "/home/belal/.local/share/uv/tools/hermes-agent/bin/python",
        "agent.py",
    ];
    // Should match the specific AgentSpec from specs list
    assert_eq!(match_agent(&hermes_cmdline, &specs), Some("my-hermes-spec"));

    let opencode_cmdline = vec!["node", "opencode-agent.js"];
    // Should match the specific AgentSpec from specs list
    assert_eq!(
        match_agent(&opencode_cmdline, &specs),
        Some("my-opencode-spec")
    );

    // Claude is not in specs, so it should fall back to static keyword
    let claude_cmdline = vec!["claude-executable", "do-stuff"];
    assert_eq!(match_agent(&claude_cmdline, &specs), Some("claude"));
}

#[test]
fn test_proc_monitor_lifecycle_events() {
    // Create a unique temporary directory for simulated /proc filesystem
    let temp_dir =
        std::env::temp_dir().join(format!("simulated_proc_test_{}", uuid::Uuid::new_v4()));
    create_dir_all(&temp_dir).unwrap();

    let specs: Vec<AgentSpec> = vec![];
    let mut monitor = ProcMonitor::new(specs).with_proc_path(temp_dir.clone());

    // 1. Initial poll with empty proc should return no events
    let events = monitor.poll();
    assert!(events.is_empty());

    // 2. Simulate start of hermes process (PID 1001)
    let pid_dir = temp_dir.join("1001");
    create_dir_all(&pid_dir).unwrap();

    // Command line: hermes-agent python path
    let cmdline_content = b"/home/belal/.local/share/uv/tools/hermes-agent/bin/python\0agent.py\0";
    write(pid_dir.join("cmdline"), cmdline_content).unwrap();

    // Poll should detect start
    let events = monitor.poll();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].agent, "hermes");
    assert_eq!(events[0].event_type, "run_started");
    assert_eq!(events[0].state, Some("Running".to_string()));

    // 3. Poll again, should not emit any new starting events
    let events = monitor.poll();
    assert!(events.is_empty());

    // Wait a brief moment to ensure some measurable duration elapsed
    sleep(Duration::from_millis(50));

    // 4. Simulate process exit (PID 1001 folder deleted)
    remove_dir_all(&pid_dir).unwrap();

    // Poll should detect finish and emit run_finished
    let events = monitor.poll();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].agent, "hermes");
    assert_eq!(events[0].event_type, "run_finished");
    assert_eq!(events[0].state, Some("Success".to_string()));

    // Verify metadata contains duration and exit code
    let metadata = &events[0].metadata;
    assert!(metadata.is_object());
    let duration_ms = metadata.get("duration_ms").unwrap().as_u64().unwrap();
    assert!(
        duration_ms >= 40,
        "Expected duration to be recorded, got {}ms",
        duration_ms
    );
    let exit_code = metadata.get("exit_code").unwrap().as_u64().unwrap();
    assert_eq!(exit_code, 0);

    // Cleanup the temp dir
    let _ = remove_dir_all(&temp_dir);
}
