use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingChange {
    CreateDir { path: PathBuf },
    WriteFile { path: PathBuf, bytes: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    Acquired,
    AlreadyHeldByOwner,
    HeldByOther { owner: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushError {
    pub path: PathBuf,
    pub operation: &'static str,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlushReport {
    pub created_dirs: Vec<PathBuf>,
    pub written_files: Vec<PathBuf>,
    pub errors: Vec<FlushError>,
}

impl FlushReport {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[async_trait]
pub trait VirtualFs: Send + Sync {
    async fn read_to_string(&self, path: &Path) -> Result<String>;
    async fn write_string(&self, path: &Path, content: String, owner: &str) -> Result<()>;
    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn flush(&self) -> Result<FlushReport>;
    async fn pending_changes(&self) -> Vec<PendingChange>;
    async fn acquire_lock(&self, path: &Path, owner: &str) -> Result<LockStatus>;
    async fn release_locks(&self, owner: &str);
    async fn discard(&self);
    async fn version(&self) -> u64;
}

#[derive(Debug, Default)]
struct OverlayState {
    files: HashMap<PathBuf, String>,
    dirs: HashSet<PathBuf>,
    locks: HashMap<PathBuf, String>,
    version: u64,
}

#[derive(Debug, Clone, Default)]
pub struct OverlayFs {
    state: Arc<Mutex<OverlayState>>,
}

impl OverlayFs {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VirtualFs for OverlayFs {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let state = self.state.lock().await;
        if let Some(content) = state.files.get(path) {
            return Ok(content.clone());
        }
        drop(state);
        Ok(tokio::fs::read_to_string(path).await?)
    }

    async fn write_string(&self, path: &Path, content: String, owner: &str) -> Result<()> {
        let mut state = self.state.lock().await;
        match state.locks.get(path) {
            Some(current_owner) if current_owner != owner => {
                anyhow::bail!(
                    "lock conflict for '{}': held by '{}'",
                    path.display(),
                    current_owner
                );
            }
            _ => {
                state.locks.insert(path.to_path_buf(), owner.to_string());
            }
        }
        state.files.insert(path.to_path_buf(), content);
        Ok(())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let mut state = self.state.lock().await;
        state.dirs.insert(path.to_path_buf());
        Ok(())
    }

    async fn flush(&self) -> Result<FlushReport> {
        let (pending_dirs, pending_files) = {
            let state = self.state.lock().await;
            (
                state.dirs.iter().cloned().collect::<Vec<_>>(),
                state
                    .files
                    .iter()
                    .map(|(path, content)| (path.clone(), content.clone()))
                    .collect::<Vec<_>>(),
            )
        };

        let mut report = FlushReport::default();

        for dir in &pending_dirs {
            if let Err(err) = tokio::fs::create_dir_all(dir).await {
                report.errors.push(FlushError {
                    path: dir.clone(),
                    operation: "create_dir_all",
                    error: err.to_string(),
                });
            } else {
                report.created_dirs.push(dir.clone());
            }
        }

        for (path, content) in &pending_files {
            if let Some(parent) = path.parent() {
                if let Err(err) = tokio::fs::create_dir_all(parent).await {
                    report.errors.push(FlushError {
                        path: path.clone(),
                        operation: "create_dir_all_parent",
                        error: err.to_string(),
                    });
                    continue;
                }
            }
            if let Err(err) = tokio::fs::write(path, content).await {
                report.errors.push(FlushError {
                    path: path.clone(),
                    operation: "write",
                    error: err.to_string(),
                });
            } else {
                report.written_files.push(path.clone());
            }
        }

        let mut state = self.state.lock().await;
        for created in &report.created_dirs {
            state.dirs.remove(created);
        }
        for written in &report.written_files {
            state.files.remove(written);
            state.locks.remove(written);
        }

        if !report.written_files.is_empty() || !report.created_dirs.is_empty() {
            state.version += 1;
        }

        Ok(report)
    }

    async fn pending_changes(&self) -> Vec<PendingChange> {
        let state = self.state.lock().await;
        let mut pending = Vec::with_capacity(state.dirs.len() + state.files.len());

        let mut dirs = state.dirs.iter().cloned().collect::<Vec<_>>();
        dirs.sort();
        for path in dirs {
            pending.push(PendingChange::CreateDir { path });
        }

        let mut files = state
            .files
            .iter()
            .map(|(path, content)| (path.clone(), content.len()))
            .collect::<Vec<_>>();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, bytes) in files {
            pending.push(PendingChange::WriteFile { path, bytes });
        }

        pending
    }

    async fn acquire_lock(&self, path: &Path, owner: &str) -> Result<LockStatus> {
        let mut state = self.state.lock().await;
        let status = match state.locks.get(path) {
            None => {
                state.locks.insert(path.to_path_buf(), owner.to_string());
                LockStatus::Acquired
            }
            Some(current_owner) if current_owner == owner => LockStatus::AlreadyHeldByOwner,
            Some(current_owner) => LockStatus::HeldByOther {
                owner: current_owner.clone(),
            },
        };
        Ok(status)
    }

    async fn release_locks(&self, owner: &str) {
        let mut state = self.state.lock().await;
        state
            .locks
            .retain(|_, current_owner| current_owner != owner);
    }

    async fn discard(&self) {
        let mut state = self.state.lock().await;
        state.files.clear();
        state.dirs.clear();
        state.locks.clear();
    }

    async fn version(&self) -> u64 {
        let state = self.state.lock().await;
        state.version
    }
}

#[cfg(test)]
mod tests {
    use super::{LockStatus, OverlayFs, PendingChange, VirtualFs};
    use anyhow::Result;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_then_read_returns_overlay_value_without_disk_mutation() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("notes.txt");
        tokio::fs::write(&file, "disk-value").await?;

        let vfs = OverlayFs::new();
        vfs.write_string(&file, "overlay-value".to_string(), "agent-a")
            .await?;

        let overlay_read = vfs.read_to_string(&file).await?;
        let disk_read = tokio::fs::read_to_string(&file).await?;

        assert_eq!(overlay_read, "overlay-value");
        assert_eq!(disk_read, "disk-value");
        Ok(())
    }

    #[tokio::test]
    async fn read_falls_back_to_disk_when_not_in_overlay() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("readme.md");
        tokio::fs::write(&file, "from-disk").await?;

        let vfs = OverlayFs::new();
        let content = vfs.read_to_string(&file).await?;

        assert_eq!(content, "from-disk");
        Ok(())
    }

    #[tokio::test]
    async fn flush_persists_overlay_changes_to_disk() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("nested").join("state.json");

        let vfs = OverlayFs::new();
        vfs.write_string(&file, "{\"ok\":true}".to_string(), "agent-a")
            .await?;
        let report = vfs.flush().await?;

        assert!(report.errors.is_empty());
        assert_eq!(tokio::fs::read_to_string(&file).await?, "{\"ok\":true}");
        assert!(report.written_files.contains(&file));
        assert!(vfs.pending_changes().await.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn discard_clears_pending_overlay_changes() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("scratch.txt");
        let dir = tmp.path().join("overlay-dir");

        let vfs = OverlayFs::new();
        vfs.write_string(&file, "temp".to_string(), "agent-a")
            .await?;
        vfs.create_dir_all(&dir).await?;
        assert_eq!(vfs.pending_changes().await.len(), 2);

        vfs.discard().await;
        assert!(vfs.pending_changes().await.is_empty());
        assert!(!dir.exists());
        assert!(!file.exists());
        Ok(())
    }

    #[tokio::test]
    async fn pending_changes_reports_exact_paths() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("a.txt");
        let dir = tmp.path().join("workspace");

        let vfs = OverlayFs::new();
        vfs.create_dir_all(&dir).await?;
        vfs.write_string(&file, "1234".to_string(), "agent-a")
            .await?;

        let pending = vfs.pending_changes().await;
        assert!(pending.contains(&PendingChange::CreateDir { path: dir }));
        assert!(pending.contains(&PendingChange::WriteFile {
            path: file,
            bytes: 4
        }));
        Ok(())
    }

    #[tokio::test]
    async fn lock_prevents_conflicting_writes() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("locked.txt");
        let vfs = OverlayFs::new();

        let status = vfs.acquire_lock(&file, "agent-a").await?;
        assert!(matches!(status, LockStatus::Acquired));

        let err = vfs
            .write_string(&file, "value".to_string(), "agent-b")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("lock conflict"));

        vfs.release_locks("agent-a").await;
        vfs.write_string(&file, "value".to_string(), "agent-b")
            .await?;
        Ok(())
    }
}
