//! Agent CLI diff wrapper
//!
//! Captures diffs from agent CLI tools (agy, cursor-agent, kimi) and
//! converts them to block-level edits (BlockEdit) for a [`VirtualFS`].
//!
//! # Usage
//!
//! ```rust,ignore
//! let vfs: Arc<dyn VirtualFS> = Arc::new(InMemoryVfs::new());
//! let mem_state = MemState::new();
//! let wrapper = AgentWrapper::new(
//!     vfs,
//!     "agent-1".into(),
//!     "run-abc".into(),
//!     "agy --file src/main.rs fix typo".into(),
//! )
//! .with_mem_state(mem_state);
//! let edits = wrapper.execute().await?;
//! println!("Edits: {:?}", edits);
//! ```
//!
//! # Warnings
//!
//! Dead-code items in this module are `pub` API surfaces that are
//! consumed from the integration site (`main.rs` calling `execute()`)
//! rather than from within the module itself. Allow `dead_code` to
//! avoid spurious warnings during incremental development.

#![allow(dead_code)]

use gestalt_core::ports::outbound::vfs::{
    BlockEdit as VfsBlockEdit, FileVersion, VfsError, VirtualFS,
};
use gestalt_router::run_state::MemState;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{error, info, warn};

/// A structured file edit produced by parsing agent CLI output.
///
/// Each variant represents a single atomic operation at a specific line
/// number in a file. These are created by parsing unified diff output
/// emitted by agent CLI tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BlockEdit {
    /// Insert `content` before the given `line`.
    Insert {
        path: String,
        line: usize,
        content: String,
    },
    /// Delete the line at the given position.
    Delete { path: String, line: usize },
    /// Replace `old` with `new` at the given line.
    Replace {
        path: String,
        line: usize,
        old: String,
        new: String,
    },
}

impl BlockEdit {
    /// Return the file path this edit targets.
    pub fn path(&self) -> &str {
        match self {
            BlockEdit::Insert { path, .. }
            | BlockEdit::Delete { path, .. }
            | BlockEdit::Replace { path, .. } => path.as_str(),
        }
    }
}

/// Wraps an agent CLI command, capturing its output and parsing diffs
/// to block-level edits on a [`VirtualFS`].
///
/// The execution flow:
/// 1. Execute the agent CLI command, capturing stdout + stderr.
/// 2. Parse unified-diff output from the captured output.
/// 3. Convert parsed diffs to [`BlockEdit`] enum variants.
/// 4. Store each edit in MemState timeline events (if configured).
/// 5. Apply each edit to the VFS (converting to the VFS BlockEdit format).
/// 6. Return the list of edits.
pub struct AgentWrapper {
    /// Full command string (program + arguments).
    pub command: String,
    /// VirtualFS instance that receives block-level edit operations.
    pub vfs: Arc<dyn VirtualFS>,
    /// Identifier for the agent instance.
    pub agent_id: String,
    /// Identifier for this execution run.
    pub run_id: String,
    /// Optional MemState for timeline event broadcasting.
    pub mem_state: Option<MemState>,
}

impl AgentWrapper {
    /// Create a new [`AgentWrapper`].
    ///
    /// Call [`with_mem_state`](Self::with_mem_state) to attach a
    /// [`MemState`] for live timeline events.
    pub fn new(vfs: Arc<dyn VirtualFS>, agent_id: String, run_id: String, command: String) -> Self {
        Self {
            command,
            vfs,
            agent_id,
            run_id,
            mem_state: None,
        }
    }

    /// Attach a [`MemState`] instance for broadcasting timeline events.
    pub fn with_mem_state(mut self, mem_state: MemState) -> Self {
        self.mem_state = Some(mem_state);
        self
    }

    /// Push a timeline event to MemState if configured.
    fn push_event(&self, event_type: &str, payload: &str) {
        if let Some(ref mem) = self.mem_state {
            mem.push_event(&self.run_id, Some(&self.agent_id), event_type, payload);
        }
    }

    /// Run the agent and capture diffs, returning structured [`BlockEdit`] events.
    ///
    /// 1. Captures stdout + stderr from the agent subprocess.
    /// 2. Parses unified-diff hunks (standard `diff -u` format) from output.
    /// 3. Creates [`BlockEdit`] enum variants for each hunk.
    /// 4. Broadcasts each edit as a `block_edit` timeline event on MemState.
    /// 5. Applies each edit to the underlying [`VirtualFS`].
    /// 6. Returns the list of edits (may be empty if nothing changed).
    pub async fn execute(&self) -> Result<Vec<BlockEdit>, String> {
        let start = std::time::Instant::now();
        info!(
            agent_id = %self.agent_id,
            run_id = %self.run_id,
            command = %self.command,
            "AgentWrapper executing",
        );

        // 1. Run agent command, capturing stdout + stderr
        let (program, args) = split_command(&self.command);
        if program.is_empty() {
            return Err("Empty command".to_string());
        }

        let output = Command::new(&program)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run agent command '{}': {}", self.command, e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            let exit_desc = output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            warn!(
                agent_id = %self.agent_id,
                exit_code = %exit_desc,
                "Agent command exited with non-zero status",
            );
        }

        // 2. Parse unified diffs from combined output
        let edits = parse_unified_diffs(&stdout, &stderr);

        // 3-4. Store each BlockEdit in MemState timeline + apply to VFS
        for edit in &edits {
            let payload = serde_json::to_string(edit).unwrap_or_else(|_| format!("{:?}", edit));
            self.push_event("block_edit", &payload);

            // 5. Apply edit to VFS
            if let Err(e) = self.apply_edit_to_vfs(edit).await {
                error!(
                    path = %edit.path(),
                    error = %e,
                    agent_id = %self.agent_id,
                    "Failed to apply BlockEdit to VFS",
                );
                return Err(format!(
                    "Failed to apply BlockEdit for '{}': {}",
                    edit.path(),
                    e
                ));
            }
        }

        let elapsed = start.elapsed();
        info!(
            edits = %edits.len(),
            elapsed_ms = %elapsed.as_millis(),
            "AgentWrapper completed",
        );

        Ok(edits)
    }

    /// Apply a single [`BlockEdit`] to the [`VirtualFS`].
    async fn apply_edit_to_vfs(&self, edit: &BlockEdit) -> Result<String, String> {
        match edit {
            BlockEdit::Insert {
                path,
                line,
                content,
            } => {
                let current = match self.vfs.read_file(path).await {
                    Ok((c, _)) => c,
                    Err(VfsError::NotFound(_)) => String::new(),
                    Err(e) => return Err(e.to_string()),
                };

                let mut lines: Vec<&str> = current.lines().collect();
                let line_idx = line.saturating_sub(1); // Convert to 0-based
                let insert_idx = line_idx.min(lines.len());

                let new_lines: Vec<&str> = content.lines().collect();
                for (i, l) in new_lines.iter().enumerate() {
                    lines.insert(insert_idx + i, l);
                }
                let new_content = lines.join("\n");

                let block = VfsBlockEdit {
                    agent_id: self.agent_id.clone(),
                    run_id: self.run_id.clone(),
                    old_string: current,
                    new_string: new_content,
                    context: String::new(),
                };
                self.vfs
                    .write_block(path, block)
                    .await
                    .map_err(|e| e.to_string())
            },
            BlockEdit::Delete { path, line } => {
                let current = match self.vfs.read_file(path).await {
                    Ok((c, _)) => c,
                    Err(VfsError::NotFound(_)) => return Ok(String::new()),
                    Err(e) => return Err(e.to_string()),
                };

                let mut lines: Vec<&str> = current.lines().collect();
                let line_idx = line.saturating_sub(1);
                if line_idx < lines.len() {
                    lines.remove(line_idx);
                }
                let new_content = lines.join("\n");

                let block = VfsBlockEdit {
                    agent_id: self.agent_id.clone(),
                    run_id: self.run_id.clone(),
                    old_string: current,
                    new_string: new_content,
                    context: String::new(),
                };
                self.vfs
                    .write_block(path, block)
                    .await
                    .map_err(|e| e.to_string())
            },
            BlockEdit::Replace {
                path,
                line,
                old,
                new,
            } => {
                let current = match self.vfs.read_file(path).await {
                    Ok((c, _)) => c,
                    Err(VfsError::NotFound(_)) => return Err(format!("File not found: {}", path)),
                    Err(e) => return Err(e.to_string()),
                };

                let new_content = if current.contains(old.as_str()) {
                    current.replacen(old, new, 1)
                } else {
                    // Fallback: replace the entire line at the given position
                    let mut lines: Vec<&str> = current.lines().collect();
                    let line_idx = line.saturating_sub(1);
                    if line_idx < lines.len() {
                        lines[line_idx] = new;
                        lines.join("\n")
                    } else {
                        return Err(format!(
                            "Cannot replace at line {}: file '{}' has {} lines",
                            line,
                            path,
                            lines.len()
                        ));
                    }
                };

                let block = VfsBlockEdit {
                    agent_id: self.agent_id.clone(),
                    run_id: self.run_id.clone(),
                    old_string: current,
                    new_string: new_content,
                    context: String::new(),
                };
                self.vfs
                    .write_block(path, block)
                    .await
                    .map_err(|e| e.to_string())
            },
        }
    }
}

// ── Unified-Diff Parser ────────────────────────────────────────────────

/// Parse unified-diff output from an agent's stdout and stderr into
/// structured [`BlockEdit`] variants.
///
/// Handles the standard unified diff format produced by `diff -u` and
/// most version-control tools (`git diff`, `diff --unified`), including
/// the optional `---` / `+++` path headers and `@@` hunk headers.
fn parse_unified_diffs(stdout: &str, stderr: &str) -> Vec<BlockEdit> {
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    let mut edits: Vec<BlockEdit> = Vec::new();
    let mut current_file: Option<String> = None;
    let mut pending_removals: Vec<(usize, String)> = Vec::new();
    let mut in_hunk = false;
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;

    for line in combined.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            // e.g. "+++ b/src/main.rs"
            let path = path.trim().strip_prefix("b/").unwrap_or(path.trim());
            current_file = Some(path.to_string());
            in_hunk = false;
            // Flush any leftovers from a previous file
            flush_pending_removals(&mut pending_removals, &mut edits, &current_file);
        } else if line.starts_with("--- ") {
            // We use +++ for the path, skip ---
            in_hunk = false;
        } else if line.starts_with("@@") {
            // Flush pending removals from the previous hunk before starting a new one
            flush_pending_removals(&mut pending_removals, &mut edits, &current_file);

            // Parse @@ -old_start,old_count +new_start,new_count @@
            if let Some((old_start, _old_cnt, new_start, _new_cnt)) = parse_hunk_header(line) {
                old_line_num = old_start;
                new_line_num = new_start;
                in_hunk = true;
            } else {
                in_hunk = false;
            }
        } else if in_hunk {
            let file = match current_file.as_ref() {
                Some(f) => f.clone(),
                None => continue,
            };

            if line.starts_with('-') {
                let content = line.strip_prefix('-').unwrap_or("").to_string();
                pending_removals.push((old_line_num, content));
                old_line_num += 1;
            } else if line.starts_with('+') {
                let content = line.strip_prefix('+').unwrap_or("").to_string();
                if !pending_removals.is_empty() {
                    // Pair last removal with this addition as a Replace
                    let (removed_line, removed_content) = pending_removals.remove(0);
                    let old_lines = std::iter::once(removed_content)
                        .chain(pending_removals.drain(..).map(|(_, c)| c));
                    edits.push(BlockEdit::Replace {
                        path: file.clone(),
                        line: removed_line,
                        old: old_lines.collect::<Vec<_>>().join("\n"),
                        new: content,
                    });
                } else {
                    edits.push(BlockEdit::Insert {
                        path: file.clone(),
                        line: new_line_num,
                        content,
                    });
                }
                new_line_num += 1;
            } else {
                // Context line — flush pending removals as Deletes
                flush_pending_removals(&mut pending_removals, &mut edits, &current_file);
                old_line_num += 1;
                new_line_num += 1;
            }
        }
    }

    // Flush any remaining pending removals at end of output
    flush_pending_removals(&mut pending_removals, &mut edits, &current_file);

    edits
}

/// Flush pending removals as [`BlockEdit::Delete`] variants.
fn flush_pending_removals(
    pending: &mut Vec<(usize, String)>,
    edits: &mut Vec<BlockEdit>,
    file: &Option<String>,
) {
    if pending.is_empty() {
        return;
    }
    let file = match file {
        Some(f) => f.clone(),
        None => {
            pending.clear();
            return;
        },
    };
    for (line_num, _content) in pending.drain(..) {
        edits.push(BlockEdit::Delete {
            path: file.clone(),
            line: line_num,
        });
    }
}

/// Parse a unified-diff hunk header of the form `@@ -old,count +new,count @@`.
///
/// Returns `(old_start, old_count, new_start, new_count)` or `None` if
/// the header cannot be parsed.
fn parse_hunk_header(header: &str) -> Option<(usize, usize, usize, usize)> {
    // Find the ranges between @@ markers
    let stripped = header
        .strip_prefix("@@")?
        .strip_suffix("@@")
        .or_else(|| {
            // Some diffs may not end with @@ cleanly; try finding last @@
            let end = header.rfind("@@")?;
            if end > 2 {
                Some(&header[2..end])
            } else {
                None
            }
        })?
        .trim();

    // Split on space to get "-old,count +new,count"
    let parts: Vec<&str> = stripped.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let old_part = parts[0].strip_prefix('-')?;
    let new_part = parts[1].strip_prefix('+')?;

    let parse_range = |s: &str| -> Option<(usize, usize)> {
        if let Some((start, count)) = s.split_once(',') {
            Some((start.parse().ok()?, count.parse().ok()?))
        } else {
            // If no count, it means 1 (hunk of length 1)
            Some((s.parse().ok()?, 1))
        }
    };

    let (old_start, old_count) = parse_range(old_part)?;
    let (new_start, new_count) = parse_range(new_part)?;

    Some((old_start, old_count, new_start, new_count))
}

/// Split a command string into (program, args) by whitespace.
fn split_command(cmd: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), vec![]);
    }
    let program = parts[0].to_string();
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();
    (program, args)
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

    async fn write_block(&self, path: &str, block: VfsBlockEdit) -> Result<String, VfsError> {
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

    // ── Diff parser tests ──────────────────────────────────────────────

    #[test]
    fn test_parse_empty_output() {
        let edits = parse_unified_diffs("", "");
        assert!(edits.is_empty(), "expected no edits for empty output");
    }

    #[test]
    fn test_parse_simple_replace() {
        let stdout = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,7 @@
  context line 1
  context line 2
-old line
+new line
  context line 3
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 1, "expected one Replace edit");
        match &edits[0] {
            BlockEdit::Replace {
                path,
                line,
                old,
                new,
            } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(*line, 12); // line 12 in old file (- region started at old_line_offset + offset)
                assert_eq!(old, "old line");
                assert_eq!(new, "new line");
            },
            other => panic!("expected Replace, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_insert_only() {
        let stdout = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -5,6 +5,7 @@
  existing
+new line here
  still there
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 1, "expected one Insert edit");
        match &edits[0] {
            BlockEdit::Insert {
                path,
                line,
                content,
            } => {
                assert_eq!(path, "src/lib.rs");
                assert_eq!(*line, 6);
                assert_eq!(content, "new line here");
            },
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_delete_only() {
        let stdout = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -8,6 +8,5 @@
  before
-deleted line
  after
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 1, "expected one Delete edit");
        match &edits[0] {
            BlockEdit::Delete { path, line } => {
                assert_eq!(path, "src/main.rs");
                assert_eq!(*line, 9);
            },
            other => panic!("expected Delete, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_multi_hunk() {
        let stdout = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,6 +10,7 @@
  keep A
-old A
+new A
  keep B
@@ -20,6 +21,7 @@
  keep C
-old C
+new C
  keep D
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 2, "expected two edits from two hunks");
        assert!(matches!(&edits[0], BlockEdit::Replace { .. }));
        assert!(matches!(&edits[1], BlockEdit::Replace { .. }));
    }

    #[test]
    fn test_parse_new_file() {
        // A new file diff has only + lines and the context is the whole file
        let stdout = "\
--- /dev/null
+++ b/new_file.rs
@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 3, "expected three Insert edits for new file");
        for edit in &edits {
            assert!(matches!(edit, BlockEdit::Insert { .. }));
            assert_eq!(edit.path(), "new_file.rs");
        }
    }

    #[test]
    fn test_parse_multiple_files() {
        let stdout = "\
--- a/a.rs
+++ b/a.rs
@@ -1,3 +1,3 @@
-old a
+new a
--- a/b.rs
+++ b/b.rs
@@ -5,5 +5,5 @@
-old b
+new b
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 2, "expected two Replace edits across files");
        assert_eq!(edits[0].path(), "a.rs");
        assert_eq!(edits[1].path(), "b.rs");
    }

    #[test]
    fn test_parse_stderr_fallback() {
        // When diff output is on stderr, it should still be parsed
        let stderr = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -3,6 +3,6 @@
  keep
-remove
+added
";
        let edits = parse_unified_diffs("", stderr);
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], BlockEdit::Replace { .. }));
    }

    #[test]
    fn test_parse_no_diff_output() {
        // Agent output with no diff markers — should produce no edits
        let stdout = "Hello from agent\nAll done!\n";
        let edits = parse_unified_diffs(stdout, "");
        assert!(edits.is_empty());
    }

    // ── split_command tests ────────────────────────────────────────────

    #[test]
    fn test_split_command_simple() {
        let (prog, args) = split_command("echo hello world");
        assert_eq!(prog, "echo");
        assert_eq!(args, vec!["hello", "world"]);
    }

    #[test]
    fn test_split_command_empty() {
        let (prog, args) = split_command("");
        assert_eq!(prog, "");
        assert!(args.is_empty());
    }

    #[test]
    fn test_split_command_no_args() {
        let (prog, args) = split_command("ls");
        assert_eq!(prog, "ls");
        assert!(args.is_empty());
    }

    // ── parse_hunk_header tests ────────────────────────────────────────

    #[test]
    fn test_parse_hunk_header_standard() {
        let result = parse_hunk_header("@@ -10,6 +11,7 @@");
        assert_eq!(result, Some((10, 6, 11, 7)));
    }

    #[test]
    fn test_parse_hunk_header_no_count() {
        // Some diffs omit the count when it's 1
        let result = parse_hunk_header("@@ -1 +2 @@");
        assert_eq!(result, Some((1, 1, 2, 1)));
    }

    #[test]
    fn test_parse_hunk_header_invalid() {
        assert!(parse_hunk_header("not a hunk").is_none());
        assert!(parse_hunk_header("").is_none());
    }

    // ── InMemoryVfs tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_in_memory_vfs_read_write() {
        let vfs = InMemoryVfs::new();

        // Reading non-existent file should fail
        let result = vfs.read_file("/nonexistent").await;
        assert!(matches!(result, Err(VfsError::NotFound(_))));

        // Write a block
        let block = VfsBlockEdit {
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
            VfsBlockEdit {
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
        let block = VfsBlockEdit {
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

    // ── AgentWrapper execute integration test ──────────────────────────

    #[tokio::test]
    async fn test_agent_wrapper_execute_no_diff_output() {
        let vfs = Arc::new(InMemoryVfs::new());
        let wrapper = AgentWrapper::new(
            vfs.clone(),
            "agent-1".into(),
            "run-test".into(),
            "echo hello from agent".into(),
        );

        // echo outputs plain text, not a diff — no edits expected
        let edits = wrapper.execute().await.unwrap();
        assert!(edits.is_empty(), "expected no edits from plain echo output");
    }

    #[tokio::test]
    async fn test_agent_wrapper_execute_with_diff() {
        let vfs = Arc::new(InMemoryVfs::new());

        // Test: parse unified diff output and verify the edit structure
        let stdout = "\
--- a/f.txt
+++ b/f.txt
@@ -1,2 +1,2 @@
-old
+new
";
        let edits = parse_unified_diffs(stdout, "");
        assert_eq!(edits.len(), 1, "expected one Replace edit from diff output");
        assert_eq!(edits[0].path(), "f.txt");
        assert!(matches!(&edits[0], BlockEdit::Replace { .. }));

        // Test VFS application via AgentWrapper's apply_edit_to_vfs
        let wrapper = AgentWrapper {
            command: "echo".into(),
            vfs: vfs.clone(),
            agent_id: "agent-1".into(),
            run_id: "run-test".into(),
            mem_state: None,
        };
        let insert_edit = BlockEdit::Insert {
            path: "test.txt".into(),
            line: 1,
            content: "hello world".into(),
        };
        let hash = wrapper.apply_edit_to_vfs(&insert_edit).await.unwrap();
        assert!(!hash.is_empty());
        let (content, _) = vfs.read_file("test.txt").await.unwrap();
        assert_eq!(content, "hello world");
    }

    // ── Task-specified tests ───────────────────────────────────────────

    #[test]
    fn test_parse_unified_diffs_insert() {
        let diff = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n line1\n+inserted\n line2\n line3\n";
        let edits = parse_unified_diffs(diff, "");
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], BlockEdit::Insert { line: 2, .. }));
    }

    #[test]
    fn test_parse_unified_diffs_replace() {
        let diff = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n";
        let edits = parse_unified_diffs(diff, "");
        assert_eq!(edits.len(), 1);
        assert!(matches!(&edits[0], BlockEdit::Replace { .. }));
    }

    #[test]
    fn test_split_command() {
        let (prog, args) = split_command("agy --file main.rs fix");
        assert_eq!(prog, "agy");
        assert_eq!(args, vec!["--file", "main.rs", "fix"]);
    }
}
