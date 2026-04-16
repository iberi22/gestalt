use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::Duration;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEventType {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatchEvent {
    pub path: PathBuf,
    pub event_type: FileEventType,
}

#[async_trait]
pub trait VirtualFileSystem: Send + Sync {
    async fn read(&self, path: &Path) -> Result<Vec<u8>>;
    async fn write(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()>;
    async fn list(&self, path: &Path) -> Result<Vec<PathBuf>>;
    async fn exists(&self, path: &Path) -> Result<bool>;

    // Extended/Compatibility methods
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let bytes = self.read(path).await?;
        Ok(String::from_utf8(bytes)?)
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        self.read(path).await
    }

    async fn write_string(&self, path: &Path, content: String, owner: &str) -> Result<()> {
        self.write(path, content.into_bytes(), owner).await
    }

    async fn write_bytes(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()> {
        self.write(path, data, owner).await
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()>;
    async fn flush(&self) -> Result<FlushReport>;
    async fn pending_changes(&self) -> Vec<PendingChange>;
    async fn acquire_lock(&self, path: &Path, owner: &str) -> Result<LockStatus>;
    async fn release_locks(&self, owner: &str);
    async fn discard(&self);
    async fn version(&self) -> u64;
}

pub trait FileWatcher: Send + Sync {
    fn watch(&self, path: PathBuf, interval: Duration) -> mpsc::Receiver<FileWatchEvent>;
}

#[derive(Debug, Default)]
struct OverlayState {
    text_files: HashMap<PathBuf, String>,
    binary_files: HashMap<PathBuf, Vec<u8>>,
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
impl VirtualFileSystem for OverlayFs {
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let state = self.state.lock().await;
        if let Some(content) = state.binary_files.get(path) {
            return Ok(content.clone());
        }
        if let Some(content) = state.text_files.get(path) {
            return Ok(content.as_bytes().to_vec());
        }
        drop(state);
        Ok(tokio::fs::read(path).await?)
    }

    async fn write(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()> {
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
        state.text_files.remove(path);
        state.binary_files.insert(path.to_path_buf(), data);
        Ok(())
    }

    async fn list(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let state = self.state.lock().await;
        let mut entries = HashSet::new();

        // Overlay files
        for p in state.text_files.keys().chain(state.binary_files.keys()) {
            if let Some(parent) = p.parent() {
                if parent == path {
                    entries.insert(p.clone());
                }
            }
        }

        // Overlay dirs
        for p in &state.dirs {
            if let Some(parent) = p.parent() {
                if parent == path {
                    entries.insert(p.clone());
                }
            }
        }

        drop(state);

        // Real filesystem
        if tokio::fs::metadata(path).await.is_ok() {
            let mut dir = tokio::fs::read_dir(path).await?;
            while let Some(entry) = dir.next_entry().await? {
                entries.insert(entry.path());
            }
        }

        let mut result: Vec<_> = entries.into_iter().collect();
        result.sort();
        Ok(result)
    }

    async fn exists(&self, path: &Path) -> Result<bool> {
        let state = self.state.lock().await;
        if state.text_files.contains_key(path)
            || state.binary_files.contains_key(path)
            || state.dirs.contains(path)
        {
            return Ok(true);
        }
        drop(state);
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let mut state = self.state.lock().await;
        state.dirs.insert(path.to_path_buf());
        Ok(())
    }

    async fn flush(&self) -> Result<FlushReport> {
        let (pending_dirs, pending_files, pending_bytes) = {
            let state = self.state.lock().await;
            (
                state.dirs.iter().cloned().collect::<Vec<_>>(),
                state
                    .text_files
                    .iter()
                    .map(|(path, content)| (path.clone(), content.clone()))
                    .collect::<Vec<_>>(),
                state
                    .binary_files
                    .iter()
                    .map(|(path, content)| (path.clone(), content.clone()))
                    .collect::<Vec<_>>(),
            )
        };

        let mut report = FlushReport::default();
        let mut set = JoinSet::new();

        enum FlushOp {
            Dir(PathBuf),
            File(PathBuf),
            Error(FlushError),
        }

        for dir in pending_dirs {
            set.spawn(async move {
                if let Err(err) = tokio::fs::create_dir_all(&dir).await {
                    FlushOp::Error(FlushError {
                        path: dir,
                        operation: "create_dir_all",
                        error: err.to_string(),
                    })
                } else {
                    FlushOp::Dir(dir)
                }
            });
        }

        for (path, content) in pending_files {
            set.spawn(async move {
                if let Some(parent) = path.parent() {
                    if let Err(err) = tokio::fs::create_dir_all(parent).await {
                        return FlushOp::Error(FlushError {
                            path,
                            operation: "create_dir_all_parent",
                            error: err.to_string(),
                        });
                    }
                }
                if let Err(err) = tokio::fs::write(&path, content).await {
                    FlushOp::Error(FlushError {
                        path,
                        operation: "write",
                        error: err.to_string(),
                    })
                } else {
                    FlushOp::File(path)
                }
            });
        }

        for (path, content) in pending_bytes {
            set.spawn(async move {
                if let Some(parent) = path.parent() {
                    if let Err(err) = tokio::fs::create_dir_all(parent).await {
                        return FlushOp::Error(FlushError {
                            path,
                            operation: "create_dir_all_parent",
                            error: err.to_string(),
                        });
                    }
                }
                if let Err(err) = tokio::fs::write(&path, content).await {
                    FlushOp::Error(FlushError {
                        path,
                        operation: "write",
                        error: err.to_string(),
                    })
                } else {
                    FlushOp::File(path)
                }
            });
        }

        while let Some(res) = set.join_next().await {
            match res? {
                FlushOp::Dir(p) => report.created_dirs.push(p),
                FlushOp::File(p) => report.written_files.push(p),
                FlushOp::Error(e) => report.errors.push(e),
            }
        }

        let mut state = self.state.lock().await;
        for created in &report.created_dirs {
            state.dirs.remove(created);
        }
        for written in &report.written_files {
            state.text_files.remove(written);
            state.binary_files.remove(written);
            state.locks.remove(written);
        }

        if !report.written_files.is_empty() || !report.created_dirs.is_empty() {
            state.version += 1;
        }

        Ok(report)
    }

    async fn pending_changes(&self) -> Vec<PendingChange> {
        let state = self.state.lock().await;
        let total_files = state.text_files.len() + state.binary_files.len();
        let mut pending = Vec::with_capacity(state.dirs.len() + total_files);

        let mut dirs = state.dirs.iter().cloned().collect::<Vec<_>>();
        dirs.sort();
        for path in dirs {
            pending.push(PendingChange::CreateDir { path });
        }

        let mut files = state
            .text_files
            .iter()
            .map(|(path, content)| (path.clone(), content.len()))
            .collect::<Vec<_>>();
        files.extend(
            state
                .binary_files
                .iter()
                .map(|(path, content)| (path.clone(), content.len())),
        );
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
        state.text_files.clear();
        state.binary_files.clear();
        state.dirs.clear();
        state.locks.clear();
    }

    async fn version(&self) -> u64 {
        let state = self.state.lock().await;
        state.version
    }
}

pub struct RealFileSystem;

#[async_trait]
impl VirtualFileSystem for RealFileSystem {
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    async fn write(&self, path: &Path, data: Vec<u8>, _owner: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        Ok(tokio::fs::write(path, data).await?)
    }

    async fn list(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await?;
        while let Some(entry) = dir.next_entry().await? {
            entries.push(entry.path());
        }
        entries.sort();
        Ok(entries)
    }

    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        Ok(tokio::fs::create_dir_all(path).await?)
    }

    async fn flush(&self) -> Result<FlushReport> {
        Ok(FlushReport::default())
    }

    async fn pending_changes(&self) -> Vec<PendingChange> {
        Vec::new()
    }

    async fn acquire_lock(&self, _path: &Path, _owner: &str) -> Result<LockStatus> {
        Ok(LockStatus::Acquired)
    }

    async fn release_locks(&self, _owner: &str) {}

    async fn discard(&self) {}

    async fn version(&self) -> u64 {
        0
    }
}

impl FileWatcher for OverlayFs {
    fn watch(&self, path: PathBuf, interval: Duration) -> mpsc::Receiver<FileWatchEvent> {
        let (tx, rx) = mpsc::channel(128);
        tokio::spawn(async move {
            let mut known: HashMap<PathBuf, (u64, std::time::SystemTime)> = HashMap::new();

            loop {
                let mut current: HashMap<PathBuf, (u64, std::time::SystemTime)> = HashMap::new();

                if path.is_file() {
                    if let Ok(meta) = tokio::fs::metadata(&path).await {
                        let size = meta.len();
                        let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        current.insert(path.clone(), (size, modified));
                    }
                } else if path.is_dir() {
                    let mut dir = match tokio::fs::read_dir(&path).await {
                        Ok(d) => d,
                        Err(_) => {
                            tokio::time::sleep(interval).await;
                            continue;
                        }
                    };
                    while let Ok(Some(entry)) = dir.next_entry().await {
                        let p = entry.path();
                        if let Ok(meta) = entry.metadata().await {
                            if meta.is_file() {
                                let size = meta.len();
                                let modified =
                                    meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                current.insert(p, (size, modified));
                            }
                        }
                    }
                }

                for (p, state) in &current {
                    match known.get(p) {
                        None => {
                            if tx
                                .send(FileWatchEvent {
                                    path: p.clone(),
                                    event_type: FileEventType::Created,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        Some(old) if old != state => {
                            if tx
                                .send(FileWatchEvent {
                                    path: p.clone(),
                                    event_type: FileEventType::Modified,
                                })
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        _ => {}
                    }
                }

                for p in known.keys() {
                    if !current.contains_key(p)
                        && tx
                            .send(FileWatchEvent {
                                path: p.clone(),
                                event_type: FileEventType::Deleted,
                            })
                            .await
                            .is_err()
                    {
                        return;
                    }
                }

                known = current;
                tokio::time::sleep(interval).await;
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FileEventType, FileWatcher, LockStatus, OverlayFs, PendingChange, RealFileSystem,
        VirtualFileSystem,
    };
    use anyhow::Result;
    use tempfile::tempdir;
    use tokio::time::Duration;

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

    #[tokio::test]
    async fn write_and_read_bytes_roundtrip() -> Result<()> {
        let tmp = tempdir()?;
        let file = tmp.path().join("blob.bin");
        let vfs = OverlayFs::new();

        let data = vec![0_u8, 1, 2, 3, 255];
        vfs.write_bytes(&file, data.clone(), "agent-a").await?;
        let read = vfs.read_bytes(&file).await?;
        assert_eq!(read, data);

        let report = vfs.flush().await?;
        assert!(report.errors.is_empty());
        assert_eq!(tokio::fs::read(&file).await?, data);
        Ok(())
    }

    #[tokio::test]
    async fn watcher_detects_created_file() -> Result<()> {
        let tmp = tempdir()?;
        let dir = tmp.path().to_path_buf();
        let file = dir.join("watch.txt");
        let vfs = OverlayFs::new();
        let mut rx = vfs.watch(dir.clone(), Duration::from_millis(50));

        tokio::time::sleep(Duration::from_millis(80)).await;
        tokio::fs::write(&file, "hello").await?;

        let mut seen = false;
        for _ in 0..20 {
            if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await
            {
                if evt.path == file && evt.event_type == FileEventType::Created {
                    seen = true;
                    break;
                }
            }
        }

        assert!(seen);
        Ok(())
    }

    #[tokio::test]
    async fn test_minimal_vfs_methods() -> Result<()> {
        let tmp = tempdir()?;
        let dir = tmp.path().to_path_buf();
        let file = dir.join("minimal.txt");

        // Test with RealFileSystem
        let real_vfs = RealFileSystem;
        real_vfs
            .write(&file, b"hello minimal".to_vec(), "owner")
            .await?;
        assert!(real_vfs.exists(&file).await?);
        let data = real_vfs.read(&file).await?;
        assert_eq!(data, b"hello minimal");
        let list = real_vfs.list(&dir).await?;
        assert!(list.contains(&file));

        // Test with OverlayFs
        let overlay_vfs = OverlayFs::new();
        let overlay_file = dir.join("overlay_minimal.txt");
        overlay_vfs
            .write(&overlay_file, b"hello overlay minimal".to_vec(), "owner")
            .await?;
        assert!(overlay_vfs.exists(&overlay_file).await?);
        let data = overlay_vfs.read(&overlay_file).await?;
        assert_eq!(data, b"hello overlay minimal");
        let list = overlay_vfs.list(&dir).await?;
        assert!(list.contains(&overlay_file));

        Ok(())
    }
}
