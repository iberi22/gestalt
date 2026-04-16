use anyhow::{bail, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant};
use tracing::info;

use gestalt_core::ports::outbound::vfs::{
    FileEventType, FileWatchEvent, FileWatcher, FlushError, FlushReport, LockStatus, PendingChange,
    VirtualFileSystem as VirtualFs,
};

/// Core FileManager Actor implementation.
/// Handles in-memory file states and serialized patch application.
pub struct FileManagerActor {
    files: HashMap<PathBuf, FileState>,
    binary_files: HashMap<PathBuf, Vec<u8>>,
    dirs: HashSet<PathBuf>,
    locks: HashMap<PathBuf, String>,
    receiver: mpsc::Receiver<FileCommand>,
    last_modification: Option<Instant>,
    global_version: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileState {
    pub content: Arc<String>,
    pub version: u64,
}

pub enum FileCommand {
    ReadFile {
        path: PathBuf,
        reply: oneshot::Sender<Result<String>>,
    },
    ReadFileState {
        path: PathBuf,
        reply: oneshot::Sender<Result<FileState>>,
    },
    ReadBytes {
        path: PathBuf,
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    WriteString {
        path: PathBuf,
        content: String,
        owner: String,
        reply: oneshot::Sender<Result<()>>,
    },
    WriteBytes {
        path: PathBuf,
        data: Vec<u8>,
        owner: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ApplyPatch {
        path: PathBuf,
        patch: String, // Unified Diff Patch
        base_version: u64,
        owner: String,
        reply: oneshot::Sender<Result<FileState>>,
    },
    CreateDirAll {
        path: PathBuf,
        reply: oneshot::Sender<Result<()>>,
    },
    Flush {
        reply: oneshot::Sender<Result<FlushReport>>,
    },
    GetPendingChanges {
        reply: oneshot::Sender<Vec<PendingChange>>,
    },
    AcquireLock {
        path: PathBuf,
        owner: String,
        reply: oneshot::Sender<Result<LockStatus>>,
    },
    ReleaseLocks {
        owner: String,
    },
    Discard,
    GetVersion {
        reply: oneshot::Sender<u64>,
    },
}

#[derive(Clone)]
pub struct FileManager {
    sender: mpsc::Sender<FileCommand>,
}

impl FileManager {
    pub fn new() -> (Self, FileManagerActor) {
        let (sender, receiver) = mpsc::channel(100);
        let actor = FileManagerActor {
            files: HashMap::new(),
            binary_files: HashMap::new(),
            dirs: HashSet::new(),
            locks: HashMap::new(),
            receiver,
            last_modification: None,
            global_version: 0,
        };
        (Self { sender }, actor)
    }

    pub async fn read_file_state(&self, path: PathBuf) -> Result<FileState> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::ReadFileState { path, reply })
            .await?;
        rx.await?
    }

    pub async fn apply_patch(
        &self,
        path: PathBuf,
        patch: String,
        base_version: u64,
        owner: String,
    ) -> Result<FileState> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::ApplyPatch {
                path,
                patch,
                base_version,
                owner,
                reply,
            })
            .await?;
        rx.await?
    }
}

#[async_trait]
impl VirtualFs for FileManager {
    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::ReadBytes {
                path: path.to_path_buf(),
                reply,
            })
            .await?;
        rx.await?
    }

    async fn write(&self, path: &Path, data: Vec<u8>, owner: &str) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::WriteBytes {
                path: path.to_path_buf(),
                data,
                owner: owner.to_string(),
                reply,
            })
            .await?;
        rx.await?
    }

    async fn list(&self, path: &Path) -> Result<Vec<PathBuf>> {
        // FileManager doesn't have a direct List command yet, we'll simulate or add one.
        // For now, let's list from disk and merge with pending dirs/files.
        let mut entries = HashSet::new();

        // This is a bit inefficient as it requires roundtrips to the actor for pending state
        let pending = self.pending_changes().await;
        for change in pending {
            let p = match change {
                PendingChange::CreateDir { path } => path,
                PendingChange::WriteFile { path, .. } => path,
            };
            if let Some(parent) = p.parent() {
                if parent == path {
                    entries.insert(p);
                }
            }
        }

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
        let pending = self.pending_changes().await;
        for change in pending {
            let p = match change {
                PendingChange::CreateDir { path } => path,
                PendingChange::WriteFile { path, .. } => path,
            };
            if p == path {
                return Ok(true);
            }
        }
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::CreateDirAll {
                path: path.to_path_buf(),
                reply,
            })
            .await?;
        rx.await?
    }

    async fn flush(&self) -> Result<FlushReport> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::Flush { reply }).await?;
        rx.await?
    }

    async fn pending_changes(&self) -> Vec<PendingChange> {
        let (reply, rx) = oneshot::channel();
        if self
            .sender
            .send(FileCommand::GetPendingChanges { reply })
            .await
            .is_err()
        {
            return vec![];
        }
        rx.await.unwrap_or_default()
    }

    async fn acquire_lock(&self, path: &Path, owner: &str) -> Result<LockStatus> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::AcquireLock {
                path: path.to_path_buf(),
                owner: owner.to_string(),
                reply,
            })
            .await?;
        rx.await?
    }

    async fn release_locks(&self, owner: &str) {
        let _ = self
            .sender
            .send(FileCommand::ReleaseLocks {
                owner: owner.to_string(),
            })
            .await;
    }

    async fn discard(&self) {
        let _ = self.sender.send(FileCommand::Discard).await;
    }

    async fn version(&self) -> u64 {
        let (reply, rx) = oneshot::channel();
        if self
            .sender
            .send(FileCommand::GetVersion { reply })
            .await
            .is_err()
        {
            return 0;
        }
        rx.await.unwrap_or(0)
    }
}

impl FileWatcher for FileManager {
    fn watch(&self, path: PathBuf, interval: Duration) -> mpsc::Receiver<FileWatchEvent> {
        let (tx, rx) = mpsc::channel(128);
        tokio::spawn(async move {
            let mut known: HashMap<PathBuf, (u64, std::time::SystemTime)> = HashMap::new();

            loop {
                let mut current: HashMap<PathBuf, (u64, std::time::SystemTime)> = HashMap::new();

                if path.is_file() {
                    if let Ok(meta) = tokio::fs::metadata(&path).await {
                        current.insert(
                            path.clone(),
                            (
                                meta.len(),
                                meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                            ),
                        );
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
                                current.insert(
                                    p,
                                    (
                                        meta.len(),
                                        meta.modified()
                                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                                    ),
                                );
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

impl FileManagerActor {
    pub async fn run(mut self) {
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            tokio::select! {
                cmd = self.receiver.recv() => {
                    if let Some(command) = cmd {
                        self.handle_command(command).await;
                    } else {
                        break;
                    }
                }
                _ = interval.tick() => {
                    if let Some(last_mod) = self.last_modification {
                        if last_mod.elapsed() >= Duration::from_secs(5) {
                            info!("Debounced flush triggered after 5s of inactivity.");
                            let _ = self.perform_flush().await;
                            self.last_modification = None;
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, command: FileCommand) {
        match command {
            FileCommand::ReadFile { path, reply } => {
                let res = self
                    .do_read_file_state(path)
                    .await
                    .map(|s| (*s.content).clone());
                let _ = reply.send(res);
            }
            FileCommand::ReadFileState { path, reply } => {
                let res = self.do_read_file_state(path).await;
                let _ = reply.send(res);
            }
            FileCommand::ReadBytes { path, reply } => {
                let res = self.do_read_bytes(path).await;
                let _ = reply.send(res);
            }
            FileCommand::WriteString {
                path,
                content,
                owner,
                reply,
            } => {
                let res = self.do_write_string(path, content, owner).await;
                if res.is_ok() {
                    self.last_modification = Some(Instant::now());
                }
                let _ = reply.send(res);
            }
            FileCommand::WriteBytes {
                path,
                data,
                owner,
                reply,
            } => {
                let res = self.do_write_bytes(path, data, owner).await;
                if res.is_ok() {
                    self.last_modification = Some(Instant::now());
                }
                let _ = reply.send(res);
            }
            FileCommand::ApplyPatch {
                path,
                patch,
                base_version,
                owner,
                reply,
            } => {
                let res = self.do_apply_patch(path, patch, base_version, owner).await;
                if res.is_ok() {
                    self.last_modification = Some(Instant::now());
                }
                let _ = reply.send(res);
            }
            FileCommand::CreateDirAll { path, reply } => {
                self.dirs.insert(path);
                let _ = reply.send(Ok(()));
            }
            FileCommand::Flush { reply } => {
                let res = self.perform_flush().await;
                // Explicit flush resets debounce state to avoid redundant delayed flushes.
                self.last_modification = None;
                let _ = reply.send(res);
            }
            FileCommand::GetPendingChanges { reply } => {
                let mut pending = Vec::new();
                for path in &self.dirs {
                    pending.push(PendingChange::CreateDir { path: path.clone() });
                }
                for (path, state) in &self.files {
                    pending.push(PendingChange::WriteFile {
                        path: path.clone(),
                        bytes: state.content.len(),
                    });
                }
                for (path, data) in &self.binary_files {
                    pending.push(PendingChange::WriteFile {
                        path: path.clone(),
                        bytes: data.len(),
                    });
                }
                let _ = reply.send(pending);
            }
            FileCommand::AcquireLock { path, owner, reply } => {
                let status = match self.locks.get(&path) {
                    None => {
                        self.locks.insert(path, owner);
                        LockStatus::Acquired
                    }
                    Some(current_owner) if current_owner == &owner => {
                        LockStatus::AlreadyHeldByOwner
                    }
                    Some(current_owner) => LockStatus::HeldByOther {
                        owner: current_owner.clone(),
                    },
                };
                let _ = reply.send(Ok(status));
            }
            FileCommand::ReleaseLocks { owner } => {
                self.locks.retain(|_, v| v != &owner);
            }
            FileCommand::Discard => {
                self.files.clear();
                self.binary_files.clear();
                self.dirs.clear();
                self.locks.clear();
            }
            FileCommand::GetVersion { reply } => {
                let _ = reply.send(self.global_version);
            }
        }
    }

    async fn do_read_file_state(&mut self, path: PathBuf) -> Result<FileState> {
        if let Some(state) = self.files.get(&path) {
            return Ok(state.clone());
        }
        if let Some(bytes) = self.binary_files.get(&path) {
            let content = String::from_utf8(bytes.clone())?;
            let state = FileState {
                content: Arc::new(content),
                version: 0,
            };
            self.files.insert(path, state.clone());
            return Ok(state);
        }

        // Load from disk if not in memory
        let content = tokio::fs::read_to_string(&path).await?;
        let state = FileState {
            content: Arc::new(content),
            version: 0,
        };
        self.files.insert(path, state.clone());
        Ok(state)
    }

    async fn do_write_string(
        &mut self,
        path: PathBuf,
        content: String,
        owner: String,
    ) -> Result<()> {
        match self.locks.get(&path) {
            Some(current_owner) if current_owner != &owner => {
                bail!(
                    "lock conflict for '{}': held by '{}'",
                    path.display(),
                    current_owner
                );
            }
            _ => {
                self.locks.insert(path.clone(), owner);
            }
        }

        let current_version = self.files.get(&path).map(|s| s.version).unwrap_or(0);
        self.files.insert(
            path.clone(),
            FileState {
                content: Arc::new(content),
                version: current_version + 1,
            },
        );
        self.binary_files.remove(&path);
        Ok(())
    }

    async fn do_read_bytes(&mut self, path: PathBuf) -> Result<Vec<u8>> {
        if let Some(data) = self.binary_files.get(&path) {
            return Ok(data.clone());
        }
        if let Some(state) = self.files.get(&path) {
            return Ok(state.content.as_bytes().to_vec());
        }
        Ok(tokio::fs::read(path).await?)
    }

    async fn do_write_bytes(&mut self, path: PathBuf, data: Vec<u8>, owner: String) -> Result<()> {
        match self.locks.get(&path) {
            Some(current_owner) if current_owner != &owner => {
                bail!(
                    "lock conflict for '{}': held by '{}'",
                    path.display(),
                    current_owner
                );
            }
            _ => {
                self.locks.insert(path.clone(), owner);
            }
        }

        self.files.remove(&path);
        self.binary_files.insert(path, data);
        Ok(())
    }

    async fn do_apply_patch(
        &mut self,
        path: PathBuf,
        patch_str: String,
        base_version: u64,
        owner: String,
    ) -> Result<FileState> {
        let current_state = self.do_read_file_state(path.clone()).await?;

        if current_state.version != base_version {
            bail!(
                "Invalid base version: {} (current is {})",
                base_version,
                current_state.version
            );
        }

        match self.locks.get(&path) {
            Some(current_owner) if current_owner != &owner => {
                bail!(
                    "lock conflict for '{}': held by '{}'",
                    path.display(),
                    current_owner
                );
            }
            _ => {
                self.locks.insert(path.clone(), owner.clone());
            }
        }

        let patch = UnifiedDiff::parse(&patch_str)?;

        // Strict single-base patching to avoid silent merges and non-determinism.
        let new_content = patch.apply(&current_state.content)?;

        let new_state = FileState {
            content: Arc::new(new_content),
            version: current_state.version + 1,
        };

        self.files.insert(path.clone(), new_state.clone());
        self.binary_files.remove(&path);
        Ok(new_state)
    }

    async fn perform_flush(&mut self) -> Result<FlushReport> {
        info!("Flushing VFS state to physical SSD.");
        let mut report = FlushReport::default();
        let mut set = JoinSet::new();

        enum FlushOp {
            Dir(PathBuf),
            File(PathBuf),
            Error(FlushError),
        }

        for dir in self.dirs.iter().cloned() {
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

        for (path, state) in self.files.iter() {
            let path = path.clone();
            let content = state.content.clone();
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
                if let Err(err) = tokio::fs::write(&path, content.as_str()).await {
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

        for (path, data) in self.binary_files.iter() {
            let path = path.clone();
            let data = data.clone();
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
                if let Err(err) = tokio::fs::write(&path, data).await {
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

        if !report.written_files.is_empty() || !report.created_dirs.is_empty() {
            self.global_version += 1;
        }

        let created_dirs = report.created_dirs.clone();
        for created in created_dirs {
            self.dirs.remove(&created);
        }

        let written_files = report.written_files.clone();
        for written in written_files {
            self.files.remove(&written);
            self.binary_files.remove(&written);
            self.locks.remove(&written);
        }

        Ok(report)
    }
}

#[derive(Debug)]
struct UnifiedDiff {
    hunks: Vec<Hunk>,
}

#[derive(Debug, Clone)]
struct Hunk {
    old_range: (usize, usize),
    _new_range: (usize, usize),
    lines: Vec<String>,
}

impl UnifiedDiff {
    fn parse(patch: &str) -> Result<Self> {
        let mut hunks = Vec::new();
        let lines = patch.lines();

        for line in lines {
            if line.starts_with("@@") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 3 {
                    continue;
                }

                let old_part = parts[1].trim_start_matches('-');
                let new_part = parts[2].trim_start_matches('+');

                let parse_range = |s: &str| -> Result<(usize, usize)> {
                    let r: Vec<usize> = s
                        .split(',')
                        .map(|v| {
                            v.parse::<usize>()
                                .map_err(|e| anyhow::anyhow!("Invalid hunk range '{}': {}", s, e))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    if r.len() == 1 {
                        Ok((r[0], 1))
                    } else {
                        Ok((r[0], r[1]))
                    }
                };

                let old_range = parse_range(old_part)?;
                let new_range = parse_range(new_part)?;

                hunks.push(Hunk {
                    old_range,
                    _new_range: new_range,
                    lines: Vec::new(),
                });
            } else if !hunks.is_empty() {
                hunks.last_mut().unwrap().lines.push(line.to_string());
            }
        }

        Ok(UnifiedDiff { hunks })
    }

    fn apply(&self, base: &str) -> Result<String> {
        let mut base_lines: Vec<String> = base.lines().map(|s| s.to_string()).collect();

        let mut sorted_hunks = self.hunks.clone();
        sorted_hunks.sort_by_key(|h| std::cmp::Reverse(h.old_range.0));

        for hunk in sorted_hunks {
            let start = hunk.old_range.0.saturating_sub(1);
            let len = hunk.old_range.1;

            let mut new_hunk_lines = Vec::new();
            for line in &hunk.lines {
                if let Some(stripped) = line.strip_prefix('+').or_else(|| line.strip_prefix(' ')) {
                    new_hunk_lines.push(stripped.to_string());
                }
            }

            if start + len <= base_lines.len() {
                base_lines.splice(start..start + len, new_hunk_lines);
            } else if start <= base_lines.len() {
                base_lines.splice(start.., new_hunk_lines);
            } else {
                base_lines.extend(new_hunk_lines);
            }
        }

        Ok(base_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{FileEventType, FileManager, FileWatcher, LockStatus, VirtualFs};
    use anyhow::Result;
    use tempfile::tempdir;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_read_file_from_disk() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("test.txt");
        tokio::fs::write(&file_path, "hello world").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let state = manager.read_file_state(file_path).await?;
        assert_eq!(state.content.as_str(), "hello world");
        assert_eq!(state.version, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_apply_patch_success() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let patch = "@@ -1,3 +1,4 @@\n line1\n+new line\n line2\n line3";
        let state = manager
            .apply_patch(file_path, patch.to_string(), 0, "agent1".to_string())
            .await?;

        assert_eq!(state.content.as_str(), "line1\nnew line\nline2\nline3");
        assert_eq!(state.version, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_debounced_flush() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("flush_test.txt");
        tokio::fs::write(&file_path, "initial").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let patch = "@@ -1,1 +1,1 @@\n-initial\n+modified";
        manager
            .apply_patch(
                file_path.clone(),
                patch.to_string(),
                0,
                "agent1".to_string(),
            )
            .await?;

        let content_immediate = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content_immediate, "initial");

        tokio::time::sleep(Duration::from_secs(6)).await;

        let content_flushed = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content_flushed, "modified");

        Ok(())
    }

    #[tokio::test]
    async fn test_lock_conflict() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("locked.txt");
        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let status = manager.acquire_lock(&file_path, "agent-a").await?;
        assert!(matches!(status, LockStatus::Acquired));

        let err = manager
            .write_string(&file_path, "value".to_string(), "agent-b")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("lock conflict"));

        Ok(())
    }

    #[tokio::test]
    async fn test_outdated_base_version_rejection() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("conflict.txt");
        tokio::fs::write(&file_path, "line1\nline2").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        // Update to v1
        manager
            .apply_patch(
                file_path.clone(),
                "@@ -1,2 +1,3 @@\n line1\n+agent1\n line2".to_string(),
                0,
                "a1".to_string(),
            )
            .await?;

        // Attempt update based on v0 should fail if logic strictly enforced (it is in our bail!)
        let err = manager
            .apply_patch(
                file_path,
                "@@ -1,2 +1,3 @@\n line1\n line2\n+agent2".to_string(),
                0,
                "a2".to_string(),
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("Invalid base version"));
        Ok(())
    }

    #[tokio::test]
    async fn test_sequential_version_increments() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("seq.txt");
        tokio::fs::write(&file_path, "start").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let s1 = manager
            .apply_patch(
                file_path.clone(),
                "@@ -1,1 +1,1 @@\n-start\n+v1".to_string(),
                0,
                "a1".to_string(),
            )
            .await?;
        assert_eq!(s1.version, 1);

        let s2 = manager
            .apply_patch(
                file_path.clone(),
                "@@ -1,1 +1,1 @@\n-v1\n+v2".to_string(),
                1,
                "a1".to_string(),
            )
            .await?;
        assert_eq!(s2.version, 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_write_and_read_bytes_roundtrip() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("blob.bin");

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let data = vec![1_u8, 2, 3, 4, 255];
        manager
            .write_bytes(&file_path, data.clone(), "agent-a")
            .await?;
        let read = manager.read_bytes(&file_path).await?;
        assert_eq!(read, data);

        let report = manager.flush().await?;
        assert!(report.errors.is_empty());
        assert_eq!(tokio::fs::read(&file_path).await?, data);
        Ok(())
    }

    #[tokio::test]
    async fn test_watcher_detects_created_file() -> Result<()> {
        let tmp = tempdir()?;
        let dir = tmp.path().to_path_buf();
        let file = dir.join("new.txt");

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());
        let mut rx = manager.watch(dir.clone(), Duration::from_millis(50));

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
}
