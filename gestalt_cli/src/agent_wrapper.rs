//! Agent CLI diff wrapper
//!
//! Captures diffs from agent CLI tools (agy, cursor-agent, kimi) and
//! converts them to block-level edits (BlockEdit) for a [`VirtualFS`].
//!
//! # Usage
//!
//! ```rust,ignore
//! let vfs: Arc<dyn VirtualFS> = Arc::new(InMemoryVfs::new());
//! let wrapper = AgentWrapper::new(
//!     "agy".into(),
//!     vec!["--file", "src/main.rs", "fix typo"],
//!     vfs,
//!     "agent-1".into(),
//!     "run-abc".into(),
//!     vec!["src/main.rs".into()],
//! );
//! let changed = wrapper.execute().await?;
//! println!("Changed files: {:?}", changed);
//! ```

use gestalt_core::ports::outbound::vfs::{BlockEdit, FileVersion, VfsError, VirtualFS};
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{error, info, warn};

/// Wraps an agent CLI command, capturing pre/post state of tracked files
/// and converting diffs to block-level edits on a [`VirtualFS`].
pub struct AgentWrapper {
    /// Agent binary to execute (e.g. "agy", "cursor-agent", "kimi").
    pub command: String,
    /// Arguments passed to the agent command.
    pub args: Vec<String>,
    /// VirtualFS instance that receives [`BlockEdit`] operations.
    pub vfs: Arc<dyn VirtualFS>,
    /// Identifier for the agent instance.
    pub agent_id: String,
    /// Identifier for this execution run.
    pub run_id: String,
    /// File paths to monitor for changes.
    pub tracked_paths: Vec<String>,
}

impl AgentWrapper {
    /// Create a new [`AgentWrapper`].
    pub fn new(
        command: String,
        args: Vec<String>,
        vfs: Arc<dyn VirtualFS>,
        agent_id: String,
        run_id: String,
        tracked_paths: Vec<String>,
    ) -> Self {
        Self {
            command,
            args,
            vfs,
            agent_id,
            run_id,
            tracked_paths,
        }
    }

    /// Run the agent and capture diffs, sending block edits to [`VirtualFS`].
    ///
    /// The execution flow:
    /// 1. Snapshot pre-state of all tracked paths via [`VirtualFS::read_file`].
    /// 2. Execute the agent CLI command.
    /// 3. Snapshot post-state of all tracked paths.
    /// 4. Compute a line-level diff for each changed file.
    /// 5. Send each hunk as a [`BlockEdit`] via [`VirtualFS::write_block`].
    /// 6. Return the list of changed file paths.
    pub async fn execute(&self) -> Result<Vec<String>, String> {
        let start = std::time::Instant::now();
        info!(
            agent_id = %self.agent_id,
            run_id = %self.run_id,
            command = %self.command,
            args = ?self.args,
            tracked = %self.tracked_paths.len(),
            "AgentWrapper executing",
        );

        // 1. Snapshot pre-state of tracked_paths
        let pre_snapshots = self.snapshot_all().await;

        // 2. Run agent command
        let status = Command::new(&self.command)
            .args(&self.args)
            .status()
            .map_err(|e| format!("Failed to run agent command '{}': {}", self.command, e))?;

        if !status.success() {
            let exit_desc = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            warn!(
                agent_id = %self.agent_id,
                exit_code = %exit_desc,
                "Agent command exited with non-zero status",
            );
        }

        // 3. Snapshot post-state
        let post_snapshots = self.snapshot_all().await;

        // 4-5. Compute diffs for each changed file and send BlockEdits
        let mut changed_files = Vec::new();

        for path in &self.tracked_paths {
            let old = pre_snapshots
                .get(path)
                .and_then(|v| v.as_deref())
                .unwrap_or("");
            let new = post_snapshots
                .get(path)
                .and_then(|v| v.as_deref())
                .unwrap_or("");

            if old == new {
                continue;
            }

            // Compute block-level edits
            let edits = self.compute_diff(old, new, path);
            if edits.is_empty() {
                continue;
            }

            // Send each BlockEdit to VirtualFS
            for edit in &edits {
                match self.vfs.write_block(path, edit.clone()).await {
                    Ok(hash) => {
                        info!(
                            path = %path,
                            hash = %hash,
                            agent_id = %self.agent_id,
                            "Block edit applied",
                        );
                    }
                    Err(e) => {
                        error!(
                            path = %path,
                            error = %e,
                            agent_id = %self.agent_id,
                            "Failed to apply block edit",
                        );
                        return Err(format!("Failed to write block for '{}': {}", path, e));
                    }
                }
            }

            changed_files.push(path.clone());
        }

        let elapsed = start.elapsed();
        info!(
            changed = %changed_files.len(),
            elapsed_ms = %elapsed.as_millis(),
            "AgentWrapper completed",
        );

        Ok(changed_files)
    }

    /// Capture current state of all tracked paths from [`VirtualFS`].
    async fn snapshot_all(&self) -> HashMap<String, Option<String>> {
        let mut snapshots = HashMap::new();
        for path in &self.tracked_paths {
            snapshots.insert(path.clone(), self.snapshot(path).await);
        }
        snapshots
    }

    /// Capture current state of a single file from [`VirtualFS`].
    ///
    /// Returns `None` if the file does not exist (new file case).
    async fn snapshot(&self, path: &str) -> Option<String> {
        match self.vfs.read_file(path).await {
            Ok((content, _hash)) => Some(content),
            Err(VfsError::NotFound(_)) => None,
            Err(e) => {
                warn!(
                    path = %path,
                    error = %e,
                    "Snapshot failed for path",
                );
                None
            }
        }
    }

    /// Compute block-level edits between old and new content.
    ///
    /// Uses a longest-common-prefix/suffix approach to find the changed
    /// region in the middle. Each changed hunk produces a single
    /// [`BlockEdit`] with enough surrounding context for the
    /// [`VirtualFS::write_block`] implementation to locate the replacement.
    ///
    /// For new files (empty `old`) a single block with empty
    /// `old_string` creates the full content. For deletions (empty `new`)
    /// a single block removes the entire old content.
    fn compute_diff(&self, old: &str, new: &str, _path: &str) -> Vec<BlockEdit> {
        if old == new {
            return vec![];
        }

        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        // New file: create with full new content
        if old.is_empty() {
            return vec![BlockEdit {
                agent_id: self.agent_id.clone(),
                run_id: self.run_id.clone(),
                old_string: String::new(),
                new_string: new.to_string(),
                context: String::new(),
            }];
        }

        // File deleted
        if new.is_empty() {
            return vec![BlockEdit {
                agent_id: self.agent_id.clone(),
                run_id: self.run_id.clone(),
                old_string: old.to_string(),
                new_string: String::new(),
                context: String::new(),
            }];
        }

        // Find longest common prefix of lines
        let max_prefix = old_lines
            .iter()
            .zip(new_lines.iter())
            .take_while(|(a, b)| *a == *b)
            .count();

        // Find longest common suffix of lines
        let max_suffix = old_lines
            .iter()
            .rev()
            .zip(new_lines.iter().rev())
            .take_while(|(a, b)| *a == *b)
            .count();

        // Clamp so prefix + suffix don't overlap
        let old_suffix_start = old_lines.len().saturating_sub(max_suffix);
        let new_suffix_start = new_lines.len().saturating_sub(max_suffix);
        let max_prefix = max_prefix.min(old_suffix_start).min(new_suffix_start);
        let old_suffix_start = old_lines.len().saturating_sub(max_suffix);
        let new_suffix_start = new_lines.len().saturating_sub(max_suffix);

        // Extract the changed middle section
        let old_mid_start = max_prefix;
        let old_mid_end = old_suffix_start.max(max_prefix);
        let new_mid_start = max_prefix;
        let new_mid_end = new_suffix_start.max(max_prefix);

        let old_mid = old_lines[old_mid_start..old_mid_end].join("\n");
        let new_mid = new_lines[new_mid_start..new_mid_end].join("\n");

        // Build context: up to 3 lines before the changed block
        let context_start = if max_prefix >= 3 { max_prefix - 3 } else { 0 };
        let context = old_lines[context_start..max_prefix].join("\n");

        vec![BlockEdit {
            agent_id: self.agent_id.clone(),
            run_id: self.run_id.clone(),
            old_string: old_mid,
            new_string: new_mid,
            context,
        }]
    }
}

// ── In-memory VirtualFS for testing ────────────────────────────────────────

/// A simple in-memory [`VirtualFS`] backed by a [`HashMap`].
///
/// Useful for testing [`AgentWrapper`] and development scenarios where
/// no persistent state database is available.
pub struct InMemoryVfs {
    files: Mutex<HashMap<String, (String, String)>>,
    version_counter: Mutex<u64>,
}

impl InMemoryVfs {
    /// Create a new empty in-memory VFS.
    pub fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
            version_counter: Mutex::new(0),
        }
    }

    fn next_hash(&self) -> String {
        let mut counter = self.version_counter.lock().unwrap();
        *counter += 1;
        format!("v{:020}", *counter)
    }
}

impl Default for InMemoryVfs {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VirtualFS for InMemoryVfs {
    async fn read_file(&self, path: &str) -> Result<(String, String), VfsError> {
        let files = self
            .files
            .lock()
            .map_err(|e| VfsError::Internal(format!("lock poisoned: {e}")))?;
        files
            .get(path)
            .cloned()
            .ok_or_else(|| VfsError::NotFound(path.to_string()))
    }

    async fn write_block(&self, path: &str, block: BlockEdit) -> Result<String, VfsError> {
        let hash = {
            let mut files = self
                .files
                .lock()
                .map_err(|e| VfsError::Internal(format!("lock poisoned: {e}")))?;

            let current_content = files.get(path).map(|(c, _)| c.clone()).unwrap_or_default();

            let new_content = if block.old_string.is_empty() && current_content.is_empty() {
                // New file
                block.new_string.clone()
            } else if current_content.contains(&block.old_string) {
                current_content.replace(&block.old_string, &block.new_string)
            } else if !block.context.is_empty() && current_content.contains(&block.context) {
                current_content.replace(&block.old_string, &block.new_string)
            } else if current_content.is_empty() {
                block.new_string.clone()
            } else {
                // old_string not found — append as a safeguard
                format!("{}{}", current_content, block.new_string)
            };

            let hash = self.next_hash();
            files.insert(path.to_string(), (new_content, hash.clone()));
            hash
        };

        Ok(hash)
    }

    async fn list_versions(&self, path: &str) -> Result<Vec<FileVersion>, VfsError> {
        let files = self
            .files
            .lock()
            .map_err(|e| VfsError::Internal(format!("lock poisoned: {e}")))?;
        if let Some((content, hash)) = files.get(path) {
            Ok(vec![FileVersion {
                hash: hash.clone(),
                content: content.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                agent_id: "in-memory".to_string(),
            }])
        } else {
            Ok(vec![])
        }
    }

    async fn get_diff(&self, _path: &str, _from: &str, _to: &str) -> Result<String, VfsError> {
        // Simple implementation: the in-memory VFS only keeps the latest version,
        // so cross-version diffing is not supported.
        Ok(String::new())
    }

    async fn lock(&self, _path: &str, _agent: &str) -> Result<bool, VfsError> {
        Ok(true)
    }

    async fn unlock(&self, _path: &str, _agent: &str) -> Result<bool, VfsError> {
        Ok(true)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_wrapper() -> AgentWrapper {
        AgentWrapper {
            command: "echo".into(),
            args: vec![],
            vfs: Arc::new(InMemoryVfs::new()),
            agent_id: "test-agent".into(),
            run_id: "test-run".into(),
            tracked_paths: vec!["/test.txt".into()],
        }
    }

    #[test]
    fn test_compute_diff_no_change() {
        let wrapper = test_wrapper();
        let edits = wrapper.compute_diff("same content", "same content", "/f");
        assert!(edits.is_empty(), "expected no edits for identical content");
    }

    #[test]
    fn test_compute_diff_new_file() {
        let wrapper = test_wrapper();
        let edits = wrapper.compute_diff("", "hello\nworld", "/f");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "");
        assert_eq!(edits[0].new_string, "hello\nworld");
        assert_eq!(edits[0].agent_id, "test-agent");
        assert_eq!(edits[0].run_id, "test-run");
    }

    #[test]
    fn test_compute_diff_deleted_file() {
        let wrapper = test_wrapper();
        let edits = wrapper.compute_diff("delete me", "", "/f");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "delete me");
        assert_eq!(edits[0].new_string, "");
    }

    #[test]
    fn test_compute_diff_full_replacement() {
        let wrapper = test_wrapper();
        let old = "line1\nline2\nline3";
        let new = "new1\nnew2\nnew3";
        let edits = wrapper.compute_diff(old, new, "/f");
        assert_eq!(edits.len(), 1, "full replacement should produce one edit");
        assert_eq!(edits[0].old_string, old);
        assert_eq!(edits[0].new_string, new);
    }

    #[test]
    fn test_compute_diff_partial_change() {
        let wrapper = test_wrapper();
        let old = "keep1\nkeep2\nchange_this\nkeep3\nkeep4";
        let new = "keep1\nkeep2\nchanged\nkeep3\nkeep4";
        let edits = wrapper.compute_diff(old, new, "/f");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "change_this");
        assert_eq!(edits[0].new_string, "changed");
        // Context should include lines before the change
        assert!(edits[0].context.contains("keep1"));
    }

    #[test]
    fn test_compute_diff_partial_change_front() {
        let wrapper = test_wrapper();
        let old = "old_start\nmiddle\nend";
        let new = "new_start\nmiddle\nend";
        let edits = wrapper.compute_diff(old, new, "/f");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "old_start");
        assert_eq!(edits[0].new_string, "new_start");
    }

    #[test]
    fn test_compute_diff_partial_change_end() {
        let wrapper = test_wrapper();
        let old = "start\nmiddle\nold_end";
        let new = "start\nmiddle\nnew_end";
        let edits = wrapper.compute_diff(old, new, "/f");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "old_end");
        assert_eq!(edits[0].new_string, "new_end");
    }

    #[test]
    fn test_compute_diff_multi_hunk() {
        // When prefix and suffix don't cover everything, the middle
        // section captures all changes as one block
        let wrapper = test_wrapper();
        let old = "aaa\nbbb\nccc\nddd\neee";
        let new = "aaa\nxxx\nccc\nyyy\neee";
        let edits = wrapper.compute_diff(old, new, "/f");
        // bbb -> xxx and ddd -> yyy, but prefix="aaa" and suffix="eee"
        // so the middle is "bbb\nccc\nddd" -> "xxx\nccc\nyyy"
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].old_string, "bbb\nccc\nddd");
        assert_eq!(edits[0].new_string, "xxx\nccc\nyyy");
    }

    // ── InMemoryVfs tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_in_memory_vfs_read_write() {
        let vfs = InMemoryVfs::new();

        // Reading non-existent file should fail
        let result = vfs.read_file("/nonexistent").await;
        assert!(matches!(result, Err(VfsError::NotFound(_))));

        // Write a block
        let block = BlockEdit {
            agent_id: "test".into(),
            run_id: "r1".into(),
            old_string: String::new(),
            new_string: "hello vfs".into(),
            context: String::new(),
        };
        let hash = vfs.write_block("/f", block).await.unwrap();
        assert!(!hash.is_empty());

        // Read it back
        let (content, _hash) = vfs.read_file("/f").await.unwrap();
        assert_eq!(content, "hello vfs");
    }

    #[tokio::test]
    async fn test_in_memory_vfs_write_block_replace() {
        let vfs = InMemoryVfs::new();

        // Create initial content
        vfs.write_block(
            "/f",
            BlockEdit {
                agent_id: "test".into(),
                run_id: "r1".into(),
                old_string: String::new(),
                new_string: "old content".into(),
                context: String::new(),
            },
        )
        .await
        .unwrap();

        // Replace
        let block = BlockEdit {
            agent_id: "test".into(),
            run_id: "r1".into(),
            old_string: "old".into(),
            new_string: "new".into(),
            context: String::new(),
        };
        vfs.write_block("/f", block).await.unwrap();

        let (content, _) = vfs.read_file("/f").await.unwrap();
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_agent_wrapper_execute_new_file() {
        // Create a wrapper that runs a simple echo command
        let vfs = Arc::new(InMemoryVfs::new());
        let wrapper = AgentWrapper {
            command: "echo".into(),
            args: vec!["written-by-agent".into()],
            vfs: vfs.clone(),
            agent_id: "agent-1".into(),
            run_id: "run-test".into(),
            tracked_paths: vec!["/output.txt".into()],
        };

        // This wont actually write to VFS because `echo` writes to stdout,
        // not to /output.txt in the VFS. But it exercises the snapshot path.
        let changed = wrapper.execute().await.unwrap();
        // No file should be changed since echo doesn't touch our VFS-managed file
        assert!(changed.is_empty());
    }
}
