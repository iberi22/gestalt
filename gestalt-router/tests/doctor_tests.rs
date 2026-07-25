use gestalt_router::doctor::{Doctor, DoctorError, OrphanedRun, ManifestJson};
use std::path::PathBuf;

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
    let err = DoctorError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
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
        worktrees: vec![
            PathBuf::from("/tmp/wt-a"),
            PathBuf::from("/tmp/wt-b"),
        ],
        branches: vec!["feat/a".to_string(), "feat/b".to_string()],
        manifest_exists: true,
        status: "Active".to_string(),
    };
    assert_eq!(orphan.worktrees.len(), 2);
    assert_eq!(orphan.branches.len(), 2);
}
