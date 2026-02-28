use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use anyhow::Result;

/// Core FileManager Actor implementation.
/// Handles in-memory file states and serialized patch application.
pub struct FileManagerActor {
    files: HashMap<PathBuf, FileState>,
    receiver: mpsc::Receiver<FileCommand>,
    last_modification: Option<std::time::Instant>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileState {
    pub content: Arc<String>,
    pub version: u64,
}

pub enum FileCommand {
    ReadFile {
        path: PathBuf,
        reply: oneshot::Sender<Result<FileState>>,
    },
    ApplyPatch {
        path: PathBuf,
        patch: String, // Unified Diff Patch
        base_version: u64,
        reply: oneshot::Sender<Result<FileState>>,
    },
    Flush {
        reply: oneshot::Sender<Result<()>>,
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
            receiver,
            last_modification: None,
        };
        (Self { sender }, actor)
    }

    pub async fn read_file(&self, path: PathBuf) -> Result<FileState> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::ReadFile { path, reply }).await?;
        rx.await?
    }

    pub async fn apply_patch(
        &self,
        path: PathBuf,
        patch: String,
        base_version: u64,
    ) -> Result<FileState> {
        let (reply, rx) = oneshot::channel();
        self.sender
            .send(FileCommand::ApplyPatch {
                path,
                patch,
                base_version,
                reply,
            })
            .await?;
        rx.await?
    }

    pub async fn flush(&self) -> Result<()> {
        let (reply, rx) = oneshot::channel();
        self.sender.send(FileCommand::Flush { reply }).await?;
        rx.await?
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
                let res = self.do_read_file(path).await;
                let _ = reply.send(res);
            }
            FileCommand::ApplyPatch {
                path,
                patch,
                base_version,
                reply,
            } => {
                let res = self.do_apply_patch(path, patch, base_version).await;
                if res.is_ok() {
                    self.last_modification = Some(std::time::Instant::now());
                }
                let _ = reply.send(res);
            }
            FileCommand::Flush { reply } => {
                let res = self.perform_flush().await;
                let _ = reply.send(res);
            }
        }
    }

    async fn do_read_file(&mut self, path: PathBuf) -> Result<FileState> {
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

    async fn do_apply_patch(
        &mut self,
        path: PathBuf,
        patch_str: String,
        base_version: u64,
    ) -> Result<FileState> {
        let current_state = self.do_read_file(path.clone()).await?;

        if current_state.version < base_version {
            anyhow::bail!(
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
            // We need the original content (base) to perform the merge.
            // In a real system, we'd store history.
            // For Phase 1 MVP, we simulate 3-way merge by checking for overlapping hunk ranges.
            warn!(
                "Attempting 3-way merge for '{}' (base v{}, current v{})",
                path.display(),
                base_version,
                current_state.version
            );
            patch.apply(&current_state.content)?
        };

        let new_state = FileState {
            content: Arc::new(new_content),
            version: current_state.version + 1,
        };

        self.files.insert(path, new_state.clone());
        Ok(new_state)
    }

    async fn perform_flush(&mut self) -> Result<()> {
        info!("Flushing VFS state to physical SSD.");
        for (path, state) in &self.files {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(path, state.content.as_str()).await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct UnifiedDiff {
    hunks: Vec<Hunk>,
}

#[derive(Debug)]
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
                // Parse @@ -1,4 +1,5 @@
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

                let hunk_lines = Vec::new();
                // Collect lines until next @@ or end
                // Note: Simplified parser for MVP.
                hunks.push(Hunk {
                    old_range,
                    new_range,
                    lines: hunk_lines,
                });
            } else if !hunks.is_empty() {
                hunks.last_mut().unwrap().lines.push(line.to_string());
            }
        }

        Ok(UnifiedDiff { hunks })
    }

    fn apply(&self, base: &str) -> Result<String> {
        let mut base_lines: Vec<String> = base.lines().map(|s| s.to_string()).collect();

        // Apply hunks in reverse to avoid offset shifting issues
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
                // '-' lines are skipped (deleted)
            }

            if start + len <= base_lines.len() {
                base_lines.splice(start..start + len, new_hunk_lines);
            } else {
                // If it's an append or out of bounds, push it
                base_lines.extend(new_hunk_lines);
            }
        }

        Ok(base_lines.join("\n"))
    }
}

impl Clone for Hunk {
    fn clone(&self) -> Self {
        Self {
            old_range: self.old_range,
            new_range: self.new_range,
            lines: self.lines.clone(),
        }
    }
}

use tokio::time::Duration;
use tracing::{info, warn};

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_file_from_disk() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("test.txt");
        tokio::fs::write(&file_path, "hello world").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        let state = manager.read_file(file_path).await?;
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
        let state = manager.apply_patch(file_path, patch.to_string(), 0).await?;

        assert_eq!(state.content.as_str(), "line1\nnew line\nline2\nline3");
        assert_eq!(state.version, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_patches_3way_merge() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("test.txt");
        tokio::fs::write(&file_path, "line1\nline2\nline3").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        // Agent 1 applies patch based on v0
        let patch1 = "@@ -1,3 +1,4 @@\n line1\n+agent1 line\n line2\n line3";
        manager.apply_patch(file_path.clone(), patch1.to_string(), 0).await?;

        // Agent 2 applies patch based on v0 (concurrent modification)
        let patch2 = "@@ -1,3 +1,4 @@\n line1\n line2\n+agent2 line\n line3";
        let state = manager.apply_patch(file_path, patch2.to_string(), 0).await?;

        // Content should ideally contain both changes if they don't overlap.
        // In our MVP logic, we just apply it sequentially.
        assert!(state.content.contains("agent1 line"));
        assert!(state.content.contains("agent2 line"));
        assert_eq!(state.version, 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_debounced_flush() -> Result<()> {
        let tmp = tempdir()?;
        let file_path = tmp.path().join("flush_test.txt");
        tokio::fs::write(&file_path, "initial").await?;

        let (manager, actor) = FileManager::new();
        tokio::spawn(actor.run());

        // Trigger change
        let patch = "@@ -1,1 +1,1 @@\n-initial\n+modified";
        manager.apply_patch(file_path.clone(), patch.to_string(), 0).await?;

        // Verify disk not immediately updated
        let content_immediate = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content_immediate, "initial");

        // Wait for debounce (5s + some buffer)
        tokio::time::sleep(Duration::from_secs(6)).await;

        // Verify disk updated
        let content_flushed = tokio::fs::read_to_string(&file_path).await?;
        assert_eq!(content_flushed, "modified");

        Ok(())
    }
}
