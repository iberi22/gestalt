use gestalt_router::doctor::{Doctor, DoctorError, ManifestJson, OrphanedRun};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[test]
fn test_orphaned_run_construction() {
    let orphan = OrphanedRun {
        run_id: "test-run-123".to_string(),
        worktrees: vec![PathBuf::from("/tmp/gestalt/wt-1")],
        branches: vec!["feat/test".to_string()],
        manifest_exists: true,
        status: "Active".to_string(),
    };
    assert_eq!(orphan.run_id, "test-run-123");
    assert_eq!(orphan.status, "Active");
    assert_eq!(orphan.worktrees.len(), 1);
}

#[test]
fn test_orphaned_run_orphaned_status() {
    let orphan = OrphanedRun {
        run_id: "orphan-run".to_string(),
        worktrees: vec![],
        branches: vec![],
        manifest_exists: false,
        status: "Orphaned".to_string(),
    };
    assert_eq!(orphan.status, "Orphaned");
    assert!(!orphan.manifest_exists);
}

#[test]
fn test_manifest_json_serialization() {
    let manifest = ManifestJson {
        run_id: "run-1".to_string(),
        sha_base: Some("abc123".to_string()),
        worktrees: Some(vec!["/tmp/wt-1".to_string()]),
        branches: Some(vec!["feat/a".to_string()]),
        created_at: Some("2026-07-25T12:00:00Z".to_string()),
    };
    let json = serde_json::to_string(&manifest).unwrap();
    assert!(json.contains("run-1"));

    let deserialized: ManifestJson = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.run_id, "run-1");
    assert_eq!(deserialized.sha_base.unwrap(), "abc123");
}

#[test]
fn test_manifest_json_minimal() {
    let manifest = ManifestJson {
        run_id: "minimal-run".to_string(),
        sha_base: None,
        worktrees: None,
        branches: None,
        created_at: None,
    };
    assert!(manifest.sha_base.is_none());
    assert!(manifest.worktrees.is_none());
}

#[test]
fn test_doctor_construction() {
    let doctor = Doctor::new(false, false);
    assert!(!doctor.force);
    assert!(!doctor.push);
}

#[test]
fn test_doctor_with_flags() {
    let doctor = Doctor::new(true, true);
    assert!(doctor.force);
    assert!(doctor.push);
}

#[test]
fn test_doctor_error_display() {
    let err = DoctorError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "file not found",
    ));
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

#[test]
fn test_doctor_error_git() {
    let err = DoctorError::Git("merge failed".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("merge failed"));
}

#[test]
fn test_doctor_error_other() {
    let err = DoctorError::Other("unknown error".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("unknown error"));
}

#[test]
fn test_orphaned_run_collection_empty() {
    let orphans: Vec<OrphanedRun> = vec![];
    assert!(orphans.is_empty());
}

#[test]
fn test_orphaned_run_with_multiple_worktrees() {
    let orphan = OrphanedRun {
        run_id: "multi-wt".to_string(),
        worktrees: vec![PathBuf::from("/tmp/wt-a"), PathBuf::from("/tmp/wt-b")],
        branches: vec!["feat/a".to_string(), "feat/b".to_string()],
        manifest_exists: true,
        status: "Active".to_string(),
    };
    assert_eq!(orphan.worktrees.len(), 2);
    assert_eq!(orphan.branches.len(), 2);
}

#[test]
fn test_doctor_pruning_and_orphans() {
    let temp_dir = TempDir::new();
    let gestalt_home = temp_dir.path().to_path_buf();

    // Set GESTALT_HOME to our temp directory so we don't modify ~/.gestalt
    std::env::set_var("GESTALT_HOME", &gestalt_home);

    let runs_dir = gestalt_home.join("runs");
    std::fs::create_dir_all(&runs_dir).unwrap();

    // 1. Create an active run (has manifest AND at least one physical worktree)
    let active_run_id = "00000000-0000-0000-0000-000000000001";
    let active_run_dir = runs_dir.join(active_run_id);
    std::fs::create_dir_all(&active_run_dir).unwrap();

    let active_manifest_content = serde_json::json!({
        "run_id": active_run_id,
        "sha_base": "base-sha",
        "worktrees": [active_run_dir.to_str().unwrap()],
        "branches": ["gestalt/active-run"]
    });
    std::fs::write(
        active_run_dir.join("manifest.json"),
        active_manifest_content.to_string(),
    )
    .unwrap();

    // 2. Create an orphaned run (has manifest but NO physical worktrees/directories existing elsewhere)
    let orphaned_run_id = "00000000-0000-0000-0000-000000000002";
    let orphaned_run_dir = runs_dir.join(orphaned_run_id);
    std::fs::create_dir_all(&orphaned_run_dir).unwrap();

    // We point worktrees to non-existent path
    let non_existent_wt = gestalt_home.join("non_existent_wt");
    let orphaned_manifest_content = serde_json::json!({
        "run_id": orphaned_run_id,
        "sha_base": "base-sha",
        "worktrees": [non_existent_wt.to_str().unwrap()],
        "branches": ["gestalt/orphaned-run"]
    });
    std::fs::write(
        orphaned_run_dir.join("manifest.json"),
        orphaned_manifest_content.to_string(),
    )
    .unwrap();

    let doctor = Doctor::new(false, false);
    let log_func = |msg: &str| {
        println!("LOG: {}", msg);
    };

    // Initialize a dummy repo to run git commands (needed for git list/worktree logic, but can be empty or we can pass a temp git repo)
    let repo_temp = TempDir::new();
    let repo_path = repo_temp.path();
    std::process::Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .unwrap();

    // Get list of runs
    let all_runs = doctor.list_orphaned(&log_func, repo_path);
    assert!(all_runs.iter().any(|r| r.run_id == active_run_id));
    assert!(all_runs.iter().any(|r| r.run_id == orphaned_run_id));

    let active_run = all_runs.iter().find(|r| r.run_id == active_run_id).unwrap();
    assert_eq!(active_run.status, "Active");

    let orphaned_run = all_runs
        .iter()
        .find(|r| r.run_id == orphaned_run_id)
        .unwrap();
    assert_eq!(orphaned_run.status, "Orphaned");

    // Test find_orphaned_runs
    let orphaned_only = doctor.find_orphaned_runs(&log_func, repo_path);
    assert_eq!(orphaned_only.len(), 1);
    assert_eq!(orphaned_only[0].run_id, orphaned_run_id);

    // Prune orphaned run without force -> should succeed for orphaned status
    let res_prune = doctor.prune_run(orphaned_run_id, &log_func, repo_path);
    assert!(res_prune.is_ok(), "Should be able to prune orphaned run");
    assert!(
        !orphaned_run_dir.exists(),
        "Orphaned run directory should be cleaned up"
    );

    // Try to prune active run without force -> should fail
    let res_prune_active = doctor.prune_run(active_run_id, &log_func, repo_path);
    assert!(
        res_prune_active.is_err(),
        "Should NOT be able to prune active run without force"
    );
    assert!(
        active_run_dir.exists(),
        "Active run directory should still exist"
    );

    // Prune active run with force -> should succeed
    let doctor_force = Doctor::new(true, false);
    let res_prune_active_force = doctor_force.prune_run(active_run_id, &log_func, repo_path);
    assert!(
        res_prune_active_force.is_ok(),
        "Should be able to prune active run with force"
    );
    assert!(
        !active_run_dir.exists(),
        "Active run directory should be cleaned up"
    );

    std::env::remove_var("GESTALT_HOME");
}

/// Minimal temp directory helper (replaces `tempfile::TempDir`).
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gestalt_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
