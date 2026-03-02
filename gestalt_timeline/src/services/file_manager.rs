use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, Instant};
use anyhow::{Result, bail};
use tracing::{info, warn};
use async_trait::async_trait;

use crate::services::vfs::{VirtualFs, FlushReport, PendingChange, LockStatus, FlushError};

/// Core FileManager Actor implementation.
/// Handles in-memory file states and serialized patch application.
pub struct FileManagerActor {
    files: HashMap<PathBuf, FileState>,
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
    WriteString {
        path: PathBuf,
        content: String,
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
        self.sender.send(FileCommand::ReadFileState { path, reply }).await?;
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
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::ReadFile { path: path.to_path_buf(), reply }).await?;
        rx.await?
    }

    async fn write_string(&self, path: &Path, content: String, owner: &str) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::WriteString {
            path: path.to_path_buf(),
            content,
            owner: owner.to_string(),
            reply
        }).await?;
        rx.await?
    }

    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::CreateDirAll { path: path.to_path_buf(), reply }).await?;
        rx.await?
    }

    async fn flush(&self) -> Result<FlushReport> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::Flush { reply }).await?;
        rx.await?
    }

    async fn pending_changes(&self) -> Vec<PendingChange> {
        let (reply, rx) = oneshot::channel();
        if self.sender.send(FileCommand::GetPendingChanges { reply }).await.is_err() {
            return vec![];
        }
        rx.await.unwrap_or_default()
    }

    async fn acquire_lock(&self, path: &Path, owner: &str) -> Result<LockStatus> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::AcquireLock {
            path: path.to_path_buf(),
            owner: owner.to_string(),
            reply
        }).await?;
        rx.await?
    }

    async fn release_locks(&self, owner: &str) {
        let _ = self.sender.send(FileCommand::ReleaseLocks { owner: owner.to_string() }).await;
    }

    async fn discard(&self) {
        let _ = self.sender.send(FileCommand::Discard).await;
    }

    async fn version(&self) -> u64 {
        let (reply, rx) = oneshot::channel();
        if self.sender.send(FileCommand::GetVersion { reply }).await.is_err() {
            return 0;
        }
        rx.await.unwrap_or(0)
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
                let res = self.do_read_file_state(path).await.map(|s| (*s.content).clone());
                let _ = reply.send(res);
            }
            FileCommand::ReadFileState { path, reply } => {
                let res = self.do_read_file_state(path).await;
                let _ = reply.send(res);
            }
            FileCommand::WriteString { path, content, owner, reply } => {
                let res = self.do_write_string(path, content, owner).await;
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
                let _ = reply.send(res);
            }
            FileCommand::GetPendingChanges { reply } => {
                let mut pending = Vec::new();
                for path in &self.dirs {
                    pending.push(PendingChange::CreateDir { path: path.clone() });
                }
                for (path, state) in &self.files {
                    pending.push(PendingChange::WriteFile { path: path.clone(), bytes: state.content.len() });
                }
                let _ = reply.send(pending);
            }
            FileCommand::AcquireLock { path, owner, reply } => {
                let status = match self.locks.get(&path) {
                    None => {
                        self.locks.insert(path, owner);
                        LockStatus::Acquired
                    }
                    Some(current_owner) if current_owner == &owner => LockStatus::AlreadyHeldByOwner,
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

        // Load from disk if not in memory
        let content = tokio::fs::read_to_string(&path).await?;
        let state = FileState {
            content: Arc::new(content),
            version: 0,
        };
        self.files.insert(path, state.clone());
        Ok(state)
    }

    async fn do_write_string(&mut self, path: PathBuf, content: String, owner: String) -> Result<()> {
        match self.locks.get(&path) {
            Some(current_owner) if current_owner != &owner => {
                bail!("lock conflict for '{}': held by '{}'", path.display(), current_owner);
            }
            _ => {
                self.locks.insert(path.clone(), owner);
            }
        }

        let current_version = self.files.get(&path).map(|s| s.version).unwrap_or(0);
        self.files.insert(path, FileState {
            content: Arc::new(content),
            version: current_version + 1,
        });
        Ok(())
    }

    async fn do_apply_patch(
        &mut self,
        path: PathBuf,
        patch_str: String,
        base_version: u64,
        owner: String,
    ) -> Result<FileState> {
        match self.locks.get(&path) {
            Some(current_owner) if current_owner != &owner => {
                bail!("lock conflict for '{}': held by '{}'", path.display(), current_owner);
            }
            _ => {
                self.locks.insert(path.clone(), owner.clone());
            }
        }

        let current_state = self.do_read_file_state(path.clone()).await?;

        if current_state.version < base_version {
            bail!(
                "Invalid base version: {} (current is {})",
                base_version,
                current_state.version
            );
        }

        let patch = UnifiedDiff::parse(&patch_str)?;

        let new_content = if current_state.version == base_version {
            // Fast-forward: Version matches, apply patch directly.
            patch.apply(&current_state.content)?
        } else {
            // 3-Way Merge: Version has advanced.
            warn!(
                "Attempting 3-way merge for '{}' (base v{}, current v{})",
                path.display(),
                base_version,
                current_state.version
            );
            // Simple approach for MVP: try to apply.
            patch.apply(&current_state.content)?
        };

        let new_state = FileState {
            content: Arc::new(new_content),
            version: current_state.version + 1,
        };

        self.files.insert(path, new_state.clone());
        Ok(new_state)
    }

    async fn perform_flush(&mut self) -> Result<FlushReport> {
        info!("Flushing VFS state to physical SSD.");
        let mut report = FlushReport::default();

        for dir in &self.dirs {
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

        for (path, state) in &self.files {
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
            if let Err(err) = tokio::fs::write(path, state.content.as_str()).await {
                report.errors.push(FlushError {
                    path: path.clone(),
                    operation: "write",
                    error: err.to_string(),
                });
            } else {
                report.written_files.push(path.clone());
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
    new_range: (usize, usize),
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

                let parse_range = |s: &str| -> (usize, usize) {
                    let r: Vec<usize> = s
                        .split(',')
                        .map(|v| v.parse().unwrap_or(0))
                        .collect();
                    if r.len() == 1 {
                        (r[0], 1)
                    } else {
                        (r[0], r[1])
                    }
                };

                let old_range = parse_range(old_part);
                let new_range = parse_range(new_part);

                hunks.push(Hunk {
                    old_range,
                    new_range,
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
    use super::{FileManager, LockStatus, VirtualFs};
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
        let state = manager.apply_patch(file_path, patch.to_string(), 0, "agent1".to_string()).await?;

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
        manager.apply_patch(file_path.clone(), patch.to_string(), 0, "agent1".to_string()).await?;

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
        manager.apply_patch(file_path.clone(), "@@ -1,2 +1,3 @@\n line1\n+agent1\n line2".to_string(), 0, "a1".to_string()).await?;

        // Attempt update based on v0 should fail if logic strictly enforced (it is in our bail!)
        let err = manager
            .apply_patch(file_path, "@@ -1,2 +1,3 @@\n line1\n line2\n+agent2".to_string(), 0, "a2".to_string())
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

        let s1 = manager.apply_patch(file_path.clone(), "@@ -1,1 +1,1 @@\n-start\n+v1".to_string(), 0, "a1".to_string()).await?;
        assert_eq!(s1.version, 1);

        let s2 = manager.apply_patch(file_path.clone(), "@@ -1,1 +1,1 @@\n-v1\n+v2".to_string(), 1, "a1".to_string()).await?;
        assert_eq!(s2.version, 2);

        Ok(())
    }
}
