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
        .args([
            "run",
            "-p",
            "gestalt_cli",
            "--",
            "status",
            "--url",
            "http://127.0.0.1:65535",
        ])
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

#[test]
fn test_version_flag() {
    let bin_path = env!("CARGO_BIN_EXE_gestalt_cli");
    let output = Command::new(bin_path)
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("gestalt {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn test_doctor_command() {
    let bin_path = env!("CARGO_BIN_EXE_gestalt_cli");
    let output = Command::new(bin_path)
        .arg("doctor")
        .output()
        .expect("Failed to execute command");

    // The doctor command might succeed or fail depending on if services are offline/online.
    // That's fine, but let's check that the output contains the diagnostic checks we implemented!
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(combined.contains("Running Gestalt Doctor Environment Check"));
    assert!(combined.contains("Xavier reachability"));
    assert!(combined.contains("StateDb open"));
    assert!(combined.contains("Agent registry parse"));
    assert!(combined.contains("Bus serve reachability"));
    assert!(combined.contains("Verdict:"));
}
