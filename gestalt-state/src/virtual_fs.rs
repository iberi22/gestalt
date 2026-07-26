//! SQLite-backed VirtualFS implementation.
//!
//! Stores versioned file content in a `file_versions` table within the
//! same SQLite database that [`StateDb`](crate::StateDb) manages, and
//! delegates file locking to `StateDb`'s `acquire_lock` / `release_lock`.

use async_trait::async_trait;
use chrono::Utc;
use gestalt_core::ports::outbound::vfs::{
    BlockEdit, FileVersion, VfsError, VirtualFS,
};
use sha2::{Digest, Sha256};

use crate::StateDb;

/// Maximum number of versions returned by [`list_versions`](Self::list_versions)
/// to guard against unbounded queries.
const MAX_VERSIONS: i64 = 1000;

/// A persistent, versioned file system backed by the same SQLite database
/// as [`StateDb`](crate::StateDb).
///
/// Every write creates a new version identified by a SHA-256 hash of the
/// content. Locking is delegated to the shared `StateDb` instance so that
/// file locks are consistent across the whole agent orchestration layer.
#[derive(Clone)]
pub struct StateDbVfs {
    state_db: StateDb,
}

impl StateDbVfs {
    /// Create a new `StateDbVfs` backed by `state_db`.
    ///
    /// Runs the `file_versions` table migration on construction.
    pub fn new(state_db: StateDb) -> Self {
        let vfs = Self { state_db };
        vfs.migrate();
        vfs
    }

    /// Create the `file_versions` table if it doesn't exist.
    fn migrate(&self) {
        if let Ok(conn) = self.state_db.conn() {
            let _ = conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS file_versions (
                    path TEXT NOT NULL,
                    version_hash TEXT NOT NULL,
                    content TEXT NOT NULL,
                    agent_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (path, version_hash)
                );

                CREATE INDEX IF NOT EXISTS idx_file_versions_path
                    ON file_versions(path);

                CREATE INDEX IF NOT EXISTS idx_file_versions_created
                    ON file_versions(path, created_at DESC);
                ",
            );
        }
    }
}

// ── File-hash helper ──────────────────────────────────────────────────────

fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ── Simple line-diff ──────────────────────────────────────────────────────

/// Produce a minimal unified-diff-like string between two text blocks.
fn simple_diff(from: &str, to: &str) -> String {
    let from_lines: Vec<&str> = from.lines().collect();
    let to_lines: Vec<&str> = to.lines().collect();

    // Very simple LCS-based diff — not a full Myers algorithm but
    // adequate for moderate-sized file versions in an agent context.
    let mut output = String::new();

    // Use a simple sliding-window approach: iterate through from_lines
    // and to_lines, emitting removals and additions.
    let mut i = 0;
    let mut j = 0;
    let mut removed = Vec::new();
    let mut added = Vec::new();

    while i < from_lines.len() || j < to_lines.len() {
        if i < from_lines.len() && j < to_lines.len() && from_lines[i] == to_lines[j] {
            // Flush any pending changes
            flush_diff_chunk(&mut output, &mut removed, &mut added);
            output.push(' ');
            output.push_str(from_lines[i]);
            output.push('\n');
            i += 1;
            j += 1;
        } else if j < to_lines.len()
            && (i >= from_lines.len()
                || (j + 1 < to_lines.len() && from_lines.get(i) == Some(&to_lines[j + 1])))
        {
            added.push(to_lines[j]);
            j += 1;
        } else if i < from_lines.len() {
            removed.push(from_lines[i]);
            i += 1;
        } else if j < to_lines.len() {
            added.push(to_lines[j]);
            j += 1;
        }
    }

    flush_diff_chunk(&mut output, &mut removed, &mut added);

    if output.is_empty() {
        output.push_str("(no differences)\n");
    }

    output
}

fn flush_diff_chunk(
    output: &mut String,
    removed: &mut Vec<&str>,
    added: &mut Vec<&str>,
) {
    if removed.is_empty() && added.is_empty() {
        return;
    }
    for line in removed.iter() {
        output.push('-');
        output.push_str(line);
        output.push('\n');
    }
    for line in added.iter() {
        output.push('+');
        output.push_str(line);
        output.push('\n');
    }
    removed.clear();
    added.clear();
}

// ── VirtualFS implementation ──────────────────────────────────────────────

#[async_trait]
impl VirtualFS for StateDbVfs {
    async fn read_file(&self, path: &str) -> Result<(String, String), VfsError> {
        // Delegate to synchronous SQLite via tokio::task::spawn_blocking
        let path = path.to_string();
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = state_db.conn().map_err(|e| {
                VfsError::Internal(format!("failed to acquire DB connection: {e}"))
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT content, version_hash FROM file_versions
                     WHERE path = ?1
                     ORDER BY created_at DESC
                     LIMIT 1",
                )
                .map_err(|e| VfsError::Internal(format!("query prepare failed: {e}")))?;

            let result = stmt
                .query_row(rusqlite::params![path], |row| {
                    let content: String = row.get(0)?;
                    let hash: String = row.get(1)?;
                    Ok((content, hash))
                })
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        VfsError::NotFound(format!("file not found: {path}"))
                    }
                    other => VfsError::Internal(format!("query failed: {other}")),
                })?;

            Ok(result)
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn write_block(&self, path: &str, block: BlockEdit) -> Result<String, VfsError> {
        let path = path.to_string();
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = state_db.conn().map_err(|e| {
                VfsError::Internal(format!("failed to acquire DB connection: {e}"))
            })?;

            // 1. Read latest content (or empty string if file doesn't exist yet)
            let current_content: String = conn
                .query_row(
                    "SELECT content FROM file_versions
                     WHERE path = ?1
                     ORDER BY created_at DESC
                     LIMIT 1",
                    rusqlite::params![path],
                    |row| row.get(0),
                )
                .unwrap_or_default();

            // 2. Apply block edit: replace old_string with new_string
            let new_content = if block.old_string.is_empty() && block.new_string.is_empty() {
                // No-op
                current_content.clone()
            } else if current_content.contains(&block.old_string) {
                current_content.replace(&block.old_string, &block.new_string)
            } else {
                // old_string not found — try with context for uniqueness
                // If context is provided, try to find old_string within context
                if !block.context.is_empty() && current_content.contains(&block.context) {
                    // Replace old_string within the context area
                    current_content.replace(&block.old_string, &block.new_string)
                } else {
                    // old_string not found and no context match — append as new content
                    if current_content.is_empty() {
                        block.new_string.clone()
                    } else {
                        format!("{}{}", current_content, block.new_string)
                    }
                }
            };

            // 3. Compute hash
            let hash = sha256_hex(&new_content);
            let now = Utc::now().to_rfc3339();

            // 4. Insert new version
            conn.execute(
                "INSERT INTO file_versions (path, version_hash, content, agent_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![path, hash, new_content, block.agent_id, now],
            )
            .map_err(|e| VfsError::Internal(format!("failed to insert version: {e}")))?;

            Ok(hash)
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn list_versions(&self, path: &str) -> Result<Vec<FileVersion>, VfsError> {
        let path = path.to_string();
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = state_db.conn().map_err(|e| {
                VfsError::Internal(format!("failed to acquire DB connection: {e}"))
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT version_hash, content, created_at, agent_id
                     FROM file_versions
                     WHERE path = ?1
                     ORDER BY created_at DESC
                     LIMIT ?2",
                )
                .map_err(|e| VfsError::Internal(format!("query prepare failed: {e}")))?;

            let versions = stmt
                .query_map(rusqlite::params![path, MAX_VERSIONS], |row| {
                    Ok(FileVersion {
                        hash: row.get(0)?,
                        content: row.get(1)?,
                        timestamp: row.get(2)?,
                        agent_id: row.get(3)?,
                    })
                })
                .map_err(|e| VfsError::Internal(format!("query failed: {e}")))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| VfsError::Internal(format!("row read failed: {e}")))?;

            Ok(versions)
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn get_diff(&self, path: &str, from: &str, to: &str) -> Result<String, VfsError> {
        let path = path.to_string();
        let from = from.to_string();
        let to = to.to_string();
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            let conn = state_db.conn().map_err(|e| {
                VfsError::Internal(format!("failed to acquire DB connection: {e}"))
            })?;

            // Fetch both versions' content
            let from_content: String = conn
                .query_row(
                    "SELECT content FROM file_versions
                     WHERE path = ?1 AND version_hash = ?2",
                    rusqlite::params![path, from],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        VfsError::NotFound(format!("from-version not found: {from}"))
                    }
                    other => VfsError::Internal(format!("query failed: {other}")),
                })?;

            let to_content: String = conn
                .query_row(
                    "SELECT content FROM file_versions
                     WHERE path = ?1 AND version_hash = ?2",
                    rusqlite::params![path, to],
                    |row| row.get(0),
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        VfsError::NotFound(format!("to-version not found: {to}"))
                    }
                    other => VfsError::Internal(format!("query failed: {other}")),
                })?;

            Ok(simple_diff(&from_content, &to_content))
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn lock(&self, path: &str, agent: &str) -> Result<bool, VfsError> {
        let path = path.to_string();
        let agent = agent.to_string();
        let run_id = format!("vfs-{}", agent);
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            state_db
                .acquire_lock(&path, &agent, &run_id, 300) // 5 min TTL
                .map_err(|e| VfsError::Internal(format!("lock acquisition failed: {e}")))
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }

    async fn unlock(&self, path: &str, agent: &str) -> Result<bool, VfsError> {
        let path = path.to_string();
        let agent = agent.to_string();
        let state_db = self.state_db.clone();

        tokio::task::spawn_blocking(move || {
            state_db
                .release_lock(&path, &agent)
                .map_err(|e| VfsError::Internal(format!("lock release failed: {e}")))
        })
        .await
        .map_err(|e| VfsError::Internal(format!("task join failed: {e}")))?
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_vfs() -> StateDbVfs {
        let db = StateDb::open(":memory:").expect("Failed to open in-memory DB");
        StateDbVfs::new(db)
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let vfs = setup_vfs();
        let err = vfs.read_file("/nonexistent").await.unwrap_err();
        assert!(
            matches!(&err, VfsError::NotFound(_)),
            "expected NotFound, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_write_block_and_read_file() {
        let vfs = setup_vfs();

        // Write first version
        let block = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "".to_string(),
            new_string: "Hello, World!".to_string(),
            context: "".to_string(),
        };
        let hash = vfs
            .write_block("/hello.txt", block)
            .await
            .expect("write_block failed");

        // Read back
        let (content, read_hash) = vfs
            .read_file("/hello.txt")
            .await
            .expect("read_file failed");
        assert_eq!(content, "Hello, World!");
        assert_eq!(read_hash, hash);
    }

    #[tokio::test]
    async fn test_write_block_replaces_old_string() {
        let vfs = setup_vfs();

        // First write
        let block1 = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "".to_string(),
            new_string: "Hello, World!".to_string(),
            context: "".to_string(),
        };
        vfs.write_block("/file.txt", block1)
            .await
            .expect("first write failed");

        // Replace "World" with "Rust"
        let block2 = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "World".to_string(),
            new_string: "Rust".to_string(),
            context: "".to_string(),
        };
        vfs.write_block("/file.txt", block2)
            .await
            .expect("second write failed");

        let (content, _) = vfs.read_file("/file.txt").await.expect("read_file failed");
        assert_eq!(content, "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_list_versions() {
        let vfs = setup_vfs();

        // Write two versions
        let block1 = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "".to_string(),
            new_string: "v1".to_string(),
            context: "".to_string(),
        };
        let hash1 = vfs
            .write_block("/ver.txt", block1)
            .await
            .expect("first write");

        let block2 = BlockEdit {
            agent_id: "agent-2".to_string(),
            run_id: "run-001".to_string(),
            old_string: "v1".to_string(),
            new_string: "v2".to_string(),
            context: "".to_string(),
        };
        let hash2 = vfs
            .write_block("/ver.txt", block2)
            .await
            .expect("second write");

        let versions = vfs
            .list_versions("/ver.txt")
            .await
            .expect("list_versions failed");

        assert_eq!(versions.len(), 2, "should have 2 versions");
        // Most recent first
        assert_eq!(versions[0].hash, hash2, "first should be newest");
        assert_eq!(versions[1].hash, hash1, "second should be oldest");
        assert_eq!(versions[0].agent_id, "agent-2");
        assert_eq!(versions[1].agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_lock_unlock() {
        let vfs = setup_vfs();

        // Lock should succeed
        let locked = vfs.lock("/test.lock", "agent-1").await.expect("lock failed");
        assert!(locked, "lock should be acquired");

        // Second lock on same path should fail
        let locked = vfs
            .lock("/test.lock", "agent-2")
            .await
            .expect("lock failed");
        assert!(!locked, "second lock should be rejected");

        // Unlock by original owner
        let unlocked = vfs
            .unlock("/test.lock", "agent-1")
            .await
            .expect("unlock failed");
        assert!(unlocked, "unlock should succeed");

        // After unlock, another agent can lock
        let locked = vfs
            .lock("/test.lock", "agent-2")
            .await
            .expect("lock failed");
        assert!(locked, "lock should succeed after release");
    }

    #[tokio::test]
    async fn test_get_diff() {
        let vfs = setup_vfs();

        // Create three versions
        let block1 = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "".to_string(),
            new_string: "line1\nline2\nline3".to_string(),
            context: "".to_string(),
        };
        let hash1 = vfs
            .write_block("/diff.txt", block1)
            .await
            .expect("first write");

        let block2 = BlockEdit {
            agent_id: "agent-1".to_string(),
            run_id: "run-001".to_string(),
            old_string: "line2".to_string(),
            new_string: "line2-modified".to_string(),
            context: "".to_string(),
        };
        let hash2 = vfs
            .write_block("/diff.txt", block2)
            .await
            .expect("second write");

        let diff = vfs
            .get_diff("/diff.txt", &hash1, &hash2)
            .await
            .expect("get_diff failed");

        // Diff should contain the removed line2 and added line2-modified
        assert!(
            diff.contains("-line2"),
            "diff should show removed 'line2'\ndiff:\n{diff}"
        );
        assert!(
            diff.contains("+line2-modified"),
            "diff should show added 'line2-modified'\ndiff:\n{diff}"
        );

        // The unchanged line1 and line3 should appear
        assert!(
            diff.contains(" line1"),
            "diff should show unchanged 'line1'\ndiff:\n{diff}"
        );
        assert!(
            diff.contains(" line3"),
            "diff should show unchanged 'line3'\ndiff:\n{diff}"
        );
    }
}
