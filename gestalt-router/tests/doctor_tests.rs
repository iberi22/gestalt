use gestalt_router::doctor::{Doctor, ManifestJson};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tempfile::tempdir;

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

fn get_test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
}

fn init_test_repo(dir: &Path) {
    // Initialize a clean, empty git repository for testing
    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());

    // Configure dummy git user
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir)
        .status()
        .unwrap();

    // Create a dummy file and commit it
    let file_path = dir.join("dummy.txt");
    fs::write(&file_path, "initial content").unwrap();

    let status = std::process::Command::new("git")
        .args(["add", "dummy.txt"])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());

    let status = std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn test_detect_orphaned_worktree_without_manifest() {
    let _lock = get_test_lock();

    let temp_home = tempdir().unwrap();
    let temp_repo = tempdir().unwrap();

    // Set HOME variable to redirect tilde expansion (~/.gestalt/runs)
    std::env::set_var("HOME", temp_home.path());

    // Initialize test repo
    init_test_repo(temp_repo.path());

    // Create a physical worktree under ~/.gestalt/runs/run-wt-orphan/wts/agent-a
    let runs_dir = temp_home.path().join(".gestalt").join("runs");
    let wt_path = runs_dir.join("run-wt-orphan").join("wts").join("agent-a");
    fs::create_dir_all(&wt_path.parent().unwrap()).unwrap();

    let status = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "gestalt/run-wt-orphan/agent-a",
            &wt_path.to_string_lossy(),
        ])
        .current_dir(temp_repo.path())
        .status()
        .unwrap();
    assert!(status.success());

    // Instantiate doctor
    let doctor = Doctor::new(false, false);
    let log_fn = |msg: &str| println!("LOG: {}", msg);

    // List runs
    let runs = doctor.list_orphaned(&log_fn, temp_repo.path());

    // We should find run-wt-orphan as Orphaned
    let run = runs.iter().find(|r| r.run_id == "run-wt-orphan").expect("Should find run-wt-orphan");
    assert_eq!(run.manifest_exists, false);
    assert_eq!(run.status, "Orphaned");
    assert!(!run.worktrees.is_empty());
    assert!(run.branches.contains(&"gestalt/run-wt-orphan/agent-a".to_string()));

    // Prune it!
    let prune_res = doctor.prune_run("run-wt-orphan", &log_fn, temp_repo.path());
    assert!(prune_res.is_ok());

    // Verify worktree is removed
    assert!(!wt_path.exists());

    // Verify branch is deleted
    let branch_check = std::process::Command::new("git")
        .args(["branch", "--list", "gestalt/run-wt-orphan/agent-a"])
        .current_dir(temp_repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&branch_check.stdout);
    assert!(!stdout.contains("gestalt/run-wt-orphan/agent-a"));
}

#[test]
fn test_detect_manifest_without_physical_worktree() {
    let _lock = get_test_lock();

    let temp_home = tempdir().unwrap();
    let temp_repo = tempdir().unwrap();

    // Set HOME variable
    std::env::set_var("HOME", temp_home.path());

    // Initialize test repo
    init_test_repo(temp_repo.path());

    // Write a manifest with no physical worktree
    let runs_dir = temp_home.path().join(".gestalt").join("runs");
    let run_dir = runs_dir.join("run-manifest-orphan");
    fs::create_dir_all(&run_dir).unwrap();

    let manifest = ManifestJson {
        run_id: "run-manifest-orphan".to_string(),
        sha_base: Some("abcdef123".to_string()),
        worktrees: Some(vec![run_dir.join("wts").join("agent-b").to_string_lossy().to_string()]),
        branches: Some(vec!["gestalt/run-manifest-orphan/agent-b".to_string()]),
        created_at: Some("2025-01-01T00:00:00Z".to_string()),
    };

    let manifest_str = serde_json::to_string(&manifest).unwrap();
    fs::write(run_dir.join("manifest.json"), manifest_str).unwrap();

    // Instantiate doctor
    let doctor = Doctor::new(false, false);
    let log_fn = |msg: &str| println!("LOG: {}", msg);

    // List runs
    let runs = doctor.list_orphaned(&log_fn, temp_repo.path());

    let run = runs.iter().find(|r| r.run_id == "run-manifest-orphan").expect("Should find run-manifest-orphan");
    assert_eq!(run.manifest_exists, true);
    assert_eq!(run.status, "Orphaned");

    // Prune all!
    let pruned = doctor.prune_all(&log_fn, temp_repo.path()).unwrap();
    assert!(pruned.contains(&"run-manifest-orphan".to_string()));

    // Verify manifest directory is gone
    assert!(!run_dir.exists());
}

#[test]
fn test_active_run_requires_force() {
    let _lock = get_test_lock();

    let temp_home = tempdir().unwrap();
    let temp_repo = tempdir().unwrap();

    std::env::set_var("HOME", temp_home.path());
    init_test_repo(temp_repo.path());

    let runs_dir = temp_home.path().join(".gestalt").join("runs");
    let wt_path = runs_dir.join("run-active").join("wts").join("agent-c");
    fs::create_dir_all(&wt_path.parent().unwrap()).unwrap();

    // Add physical worktree
    let status = std::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            "gestalt/run-active/agent-c",
            &wt_path.to_string_lossy(),
        ])
        .current_dir(temp_repo.path())
        .status()
        .unwrap();
    assert!(status.success());

    // Write manifest
    let run_dir = runs_dir.join("run-active");
    let manifest = ManifestJson {
        run_id: "run-active".to_string(),
        sha_base: Some("abcdef123".to_string()),
        worktrees: Some(vec![wt_path.to_string_lossy().to_string()]),
        branches: Some(vec!["gestalt/run-active/agent-c".to_string()]),
        created_at: Some("2025-01-01T00:00:00Z".to_string()),
    };
    let manifest_str = serde_json::to_string(&manifest).unwrap();
    fs::write(run_dir.join("manifest.json"), manifest_str).unwrap();

    let log_fn = |msg: &str| println!("LOG: {}", msg);

    // Doctor with force: false
    let doctor_safe = Doctor::new(false, false);
    let runs = doctor_safe.list_orphaned(&log_fn, temp_repo.path());
    let run = runs.iter().find(|r| r.run_id == "run-active").unwrap();
    assert_eq!(run.status, "Active");

    // Prune should fail
    let prune_res = doctor_safe.prune_run("run-active", &log_fn, temp_repo.path());
    assert!(prune_res.is_err());
    assert!(wt_path.exists()); // remains untouched

    // Doctor with force: true
    let doctor_force = Doctor::new(true, false);
    let prune_res_force = doctor_force.prune_run("run-active", &log_fn, temp_repo.path());
    assert!(prune_res_force.is_ok());
    assert!(!wt_path.exists()); // pruned
}

#[test]
fn test_archive_events_on_prune() {
    let _lock = get_test_lock();

    let temp_home = tempdir().unwrap();
    let temp_repo = tempdir().unwrap();

    std::env::set_var("HOME", temp_home.path());
    init_test_repo(temp_repo.path());

    let runs_dir = temp_home.path().join(".gestalt").join("runs");
    let run_dir = runs_dir.join("run-archive-test");
    fs::create_dir_all(&run_dir).unwrap();

    // Write manifest
    let manifest = ManifestJson {
        run_id: "run-archive-test".to_string(),
        sha_base: Some("abc".to_string()),
        worktrees: Some(vec![]),
        branches: Some(vec![]),
        created_at: Some("2025-01-01T00:00:00Z".to_string()),
    };
    let manifest_str = serde_json::to_string(&manifest).unwrap();
    fs::write(run_dir.join("manifest.json"), manifest_str).unwrap();

    // Create an events file
    let events_file = run_dir.join("events.jsonl");
    fs::write(&events_file, "{\"event\": \"test_event\"}\n").unwrap();

    let log_fn = |msg: &str| println!("LOG: {}", msg);
    let doctor = Doctor::new(false, false);

    let prune_res = doctor.prune_run("run-archive-test", &log_fn, temp_repo.path());
    assert!(prune_res.is_ok());

    // Check archive destination
    let archive_file = temp_home.path()
        .join(".gestalt")
        .join("archive")
        .join("runs")
        .join("run-archive-test")
        .join("events.jsonl");

    assert!(archive_file.exists());
    let archived_content = fs::read_to_string(&archive_file).unwrap();
    assert_eq!(archived_content, "{\"event\": \"test_event\"}\n");

    // Check run folder is gone
    assert!(!run_dir.exists());
}
