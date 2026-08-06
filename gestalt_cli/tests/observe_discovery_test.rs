use std::fs;
use std::process::Command;

fn get_binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_gestalt_cli")
}

#[test]
fn test_observe_help() {
    let output = Command::new(get_binary_path())
        .args(["observe", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("observe"));
    assert!(stdout.contains("--once"));
}

#[test]
fn test_observe_once_empty() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home_path = temp_dir.path();

    // Prepare a mock PATH dir
    let bin_dir = home_path.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path_dirs = vec![bin_dir];
    for p in std::env::split_paths(&old_path) {
        path_dirs.push(p);
    }
    let new_path = std::env::join_paths(path_dirs).unwrap();

    let output = Command::new(get_binary_path())
        .args(["observe", "--once"])
        .env("HOME", home_path)
        .env("PATH", new_path)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("detected"));
    assert!(stdout.to_lowercase().contains("agent"));
}

#[test]
fn test_observe_once_populated() {
    let temp_dir = tempfile::tempdir().unwrap();
    let home_path = temp_dir.path();

    // Create mock config directories
    let opencode_dir = home_path.join(".config/opencode");
    fs::create_dir_all(&opencode_dir).unwrap();

    let codex_dir = home_path.join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    // Create mock PATH binary dir
    let bin_dir = home_path.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let hermes_bin = bin_dir.join("hermes");
    fs::write(&hermes_bin, b"").unwrap();

    // Create mock Orca hook dir
    let hooks_dir = home_path.join(".orca/agent-hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let hook = hooks_dir.join("hook_test");
    fs::write(&hook, b"").unwrap();

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path_dirs = vec![bin_dir];
    for p in std::env::split_paths(&old_path) {
        path_dirs.push(p);
    }
    let new_path = std::env::join_paths(path_dirs).unwrap();

    let output = Command::new(get_binary_path())
        .args(["observe", "--once"])
        .env("HOME", home_path)
        .env("PATH", new_path)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("detected"));
    assert!(stdout.contains("hermes"));
    assert!(stdout.contains(".config/opencode"));
    assert!(stdout.contains("hook_test"));
}
