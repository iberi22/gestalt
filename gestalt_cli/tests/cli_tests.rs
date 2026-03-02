use std::process::Command;

#[test]
fn test_help_command() {
    let output = Command::new("cargo")
        .args(["run", "-p", "gestalt_cli", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OpenClaw ↔ Gestalt Bridge CLI"));
    assert!(stdout.contains("repl"));
}

#[test]
fn test_status_offline() {
    let output = Command::new("cargo")
        .args(["run", "-p", "gestalt_cli", "--", "status", "--url", "http://127.0.0.1:65535"])
        .output()
        .expect("Failed to execute command");

    // Should fail because nothing is listening on that port
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Check for some indicators of failure.
    // Since we're using tracing, it might look different than a simple println.
    // But we know it should return an error.
    assert!(!combined.is_empty());
}
