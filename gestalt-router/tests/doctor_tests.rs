use gestalt_router::doctor::{Doctor, DoctorError, OrphanedRun, ManifestJson};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn init_test_repo(dir: &Path) {
    std::process::Command::new("git")
        .arg("init")
        .current_dir(dir)
        .status().unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir).status().unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir).status().unwrap();
    std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(dir).status().unwrap();
}

#[test]
fn test_orphaned_run_detection() {
    let dir = tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    fs::create_dir_all(&repo_path).unwrap();
    init_test_repo(&repo_path);

    let worktrees_dir = dir.path().join("worktrees");
    fs::create_dir_all(&worktrees_dir).unwrap();

    let doctor = Doctor::new(&repo_path, &worktrees_dir);
    let orphans = doctor.find_orphaned_runs().unwrap();
    assert!(orphans.is_empty(), "Clean repo should have no orphans");
}

#[test]
fn test_orphaned_run_from_manifest() {
    let orphan = OrphanedRun {
        run_id: "test-run-123".to_string(),
        agent_id: "agent-1".to_string(),
        worktree_path: "/tmp/gestalt/test-run-123-agent-1".to_string(),
        state: "Running".to_string(),
        age_secs: 3600,
    };
    assert_eq!(orphan.run_id, "test-run-123");
    assert_eq!(orphan.state, "Running");
    assert!(orphan.age_secs >= 0);
}

#[test]
fn test_manifest_json_serialization() {
    let manifest = ManifestJson {
        run_id: "run-1".to_string(),
        created_at: "2026-07-25T12:00:00Z".to_string(),
        agent_id: "agent-1".to_string(),
        state: "Running".to_string(),
        pid: Some(12345),
    };
    let json = serde_json::to_string(&manifest).unwrap();
    assert!(json.contains("run-1"));
    assert!(json.contains("Running"));

    let deserialized: ManifestJson = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.run_id, "run-1");
    assert_eq!(deserialized.pid, Some(12345));
}

#[test]
fn test_doctor_error_display() {
    let err = DoctorError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

#[test]
fn test_doctor_find_orphans_nonexistent_dir() {
    let dir = tempdir().unwrap();
    let repo_path = dir.path().join("no_repo");
    let worktrees_dir = dir.path().join("no_worktrees");

    let doctor = Doctor::new(&repo_path, &worktrees_dir);
    let result = doctor.find_orphaned_runs();
    assert!(result.is_err(), "Should error on nonexistent repo");
}

#[test]
fn test_orphaned_run_collection_empty() {
    let orphans: Vec<OrphanedRun> = vec![];
    assert!(orphans.is_empty());
}

#[test]
fn test_manifest_json_without_pid() {
    let manifest = ManifestJson {
        run_id: "run-2".to_string(),
        created_at: "2026-07-25T13:00:00Z".to_string(),
        agent_id: "agent-2".to_string(),
        state: "Success".to_string(),
        pid: None,
    };
    assert!(manifest.pid.is_none());
    assert_eq!(manifest.state, "Success");
}
