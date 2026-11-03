//! SQLite-backed persistence for the Gestalt core.
//!
//! This module provides a concrete, local-first implementation of the
//! [`RepoManager`] and [`VectorDb`] boundary ports on top of [`rusqlite`]
//! (with the `bundled` feature, so no system SQLite is required) and
//! `sqlite-vec` for cosine-similarity vector search.
//!
//! [`rusqlite`] is synchronous, so every database operation is dispatched to a
//! blocking thread via [`tokio::task::spawn_blocking`] to keep the public API
//! `async` without blocking the async runtime.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use rusqlite::{params, Connection};
use serde_json::Value;
use uuid::Uuid;

use crate::ports::outbound::repo_manager::{RepoManager, Repository, ScoredResult, VectorDb};

/// Number of pooled connections opened for a file-backed database.
///
/// SQLite access is serialized through a mutex, so this is deliberately small;
/// it mainly avoids the cost of re-opening connections between calls.
const DEFAULT_POOL_SIZE: usize = 4;

/// Idempotent schema applied on every connection creation. Statements use
/// `IF NOT EXISTS` so migrations can safely run each time a connection is
/// opened.
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS repositories (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    url         TEXT NOT NULL,
    local_path  TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE IF NOT EXISTS embeddings (
    id          TEXT NOT NULL,
    collection  TEXT NOT NULL,
    vector      BLOB NOT NULL,
    metadata    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (id, collection)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_collection
    ON embeddings (collection);
"#;

/// Registers the `sqlite-vec` extension as a SQLite auto-extension so its
/// `vec_distance_cosine` / `vec_f32` scalar functions are available on every
/// subsequently-opened connection.
///
/// This must happen exactly once, before any connection is opened. The result
/// is memoized so repeated calls are cheap and idempotent.
fn register_sqlite_vec() -> anyhow::Result<()> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();

    match RESULT.get_or_init(|| {
        (|| -> Result<(), String> {
            // sqlite-vec exposes `sqlite3_vec_init` with the standard SQLite
            // extension entry-point signature. The crate's FFI declaration
            // elides the arguments, so we take its address and hand it to
            // SQLite via rusqlite's typed auto-extension registration (the
            // same pattern sqlite-vec uses in its own integration test).
            unsafe {
                let init: rusqlite::auto_extension::RawAutoExtension =
                    std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ());
                rusqlite::auto_extension::register_auto_extension(init)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        })()
    }) {
        Ok(()) => Ok(()),
        Err(msg) => Err(anyhow::anyhow!(
            "failed to register sqlite-vec extension: {msg}"
        )),
    }
}

/// Serialize a float vector into the native-endian `f32` byte layout that
/// `sqlite-vec` expects for BLOB storage (it `memcpy`s BLOBs into `f32*`,
/// so the bytes must match the host's endianness).
fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(std::mem::size_of_val(vector));
    for value in vector {
        blob.extend_from_slice(&value.to_ne_bytes());
    }
    blob
}

/// Returns `true` when `path` points at an in-memory database, in which case a
/// single shared connection must be used (multiple `:memory:` connections are
/// independent databases).
fn is_in_memory(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s == ":memory:" || s.starts_with("file::memory:") || s.contains("mode=memory")
}

/// The default base directory used by [`SqliteDb::clone_repo`] to store
/// cloned repositories when no explicit directory is configured.
fn default_repos_dir() -> PathBuf {
    std::env::temp_dir().join("gestalt-repos")
}

/// Derives a repository name from a git URL (`.../owner/repo.git` -> `repo`).
fn repo_name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(|s| s.trim_end_matches(".git"))
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .to_string()
}

/// A small, mutex-serialized connection pool backed by SQLite.
#[derive(Clone)]
pub struct SqliteDb {
    pool: Arc<Mutex<Vec<Connection>>>,
    db_path: Arc<PathBuf>,
    max_size: usize,
    repos_dir: Arc<PathBuf>,
}

impl SqliteDb {
    /// Opens (or creates) a SQLite database at `path`, runs migrations, and
    /// warms a small connection pool.
    ///
    /// `sqlite-vec` is registered as an auto-extension before any connection
    /// is opened.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        register_sqlite_vec()?;

        let db_path = path.as_ref().to_path_buf();
        let max_size = if is_in_memory(&db_path) {
            1
        } else {
            DEFAULT_POOL_SIZE
        };

        let mut connections = Vec::with_capacity(max_size);
        for _ in 0..max_size {
            let conn = Connection::open(&db_path)?;
            conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
            conn.execute_batch(SCHEMA)?;
            connections.push(conn);
        }

        Ok(Self {
            pool: Arc::new(Mutex::new(connections)),
            db_path: Arc::new(db_path),
            max_size,
            repos_dir: Arc::new(default_repos_dir()),
        })
    }

    /// Opens an in-memory SQLite database (convenient for tests and ephemeral
    /// workloads). The in-memory database is shared by all operations because
    /// only a single connection is created.
    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::open(":memory:")
    }

    /// Overrides the base directory used by [`SqliteDb::clone_repo`].
    pub fn with_repos_dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.repos_dir = Arc::new(dir.as_ref().to_path_buf());
        self
    }

    /// Returns the base directory where cloned repositories are stored.
    ///
    /// Adapters built on top of [`SqliteDb`] use this as the single source of
    /// truth for their clone destination.
    pub fn repos_dir(&self) -> &Path {
        self.repos_dir.as_ref()
    }

    /// Checks a connection out of the pool. If the pool is unexpectedly empty
    /// (which should not happen given mutex serialization), a fresh connection
    /// is opened and migrated so callers always make progress.
    fn checkout(&self) -> anyhow::Result<Connection> {
        let mut pool = self
            .pool
            .lock()
            .map_err(|e| anyhow::anyhow!("connection pool poisoned: {e}"))?;

        if let Some(conn) = pool.pop() {
            return Ok(conn);
        }

        let conn = Connection::open(self.db_path.as_ref())?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(conn)
    }

    /// Returns a connection to the pool. If the pool is already at capacity
    /// (or the mutex is poisoned), the connection is simply dropped.
    fn checkin(&self, conn: Connection) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                pool.push(conn);
            }
        }
    }

    /// Runs `f` with a pooled connection, guaranteeing the connection is
    /// returned to the pool afterwards regardless of the outcome.
    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> anyhow::Result<T>) -> anyhow::Result<T> {
        let conn = self.checkout()?;
        let result = f(&conn);
        self.checkin(conn);
        result
    }

    // ------------------------------------------------------------------
    // Synchronous primitives (private; wrapped in spawn_blocking by the
    // async API below).
    // ------------------------------------------------------------------

    fn insert_repo_sync(&self, repo: &Repository) -> anyhow::Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO repositories (id, name, url, local_path)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     url = excluded.url,
                     local_path = excluded.local_path",
                params![repo.id, repo.name, repo.url, repo.local_path],
            )?;
            Ok(())
        })
    }

    fn list_repos_sync(&self) -> anyhow::Result<Vec<Repository>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, url, local_path
                 FROM repositories
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Repository {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    url: row.get(2)?,
                    local_path: row.get(3)?,
                })
            })?;

            let mut repos = Vec::new();
            for row in rows {
                repos.push(row?);
            }
            Ok(repos)
        })
    }

    fn insert_embedding_sync(
        &self,
        collection: &str,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> anyhow::Result<()> {
        let blob = vector_to_blob(vector);
        let metadata_json = serde_json::to_string(metadata)?;

        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO embeddings (id, collection, vector, metadata)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id, collection) DO UPDATE SET
                     vector = excluded.vector,
                     metadata = excluded.metadata",
                params![id, collection, blob, metadata_json],
            )?;
            Ok(())
        })
    }

    fn search_similar_sync(
        &self,
        collection: &str,
        vector: &[f32],
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        if vector.is_empty() {
            return Ok(Vec::new());
        }

        let query_blob = vector_to_blob(vector);
        // `sqlite-vec` aborts the whole statement when two vectors have
        // different widths, so rows from other embedding models are filtered
        // out by blob size (`length()` is bytes for BLOBs) instead of taking
        // the entire search down with them.
        let query_width = query_blob.len() as i64;
        let limit = limit as i64;

        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, metadata, 1.0 - vec_distance_cosine(vector, ?1) AS score
                 FROM embeddings
                 WHERE collection = ?2 AND length(vector) = ?3
                 ORDER BY score DESC
                 LIMIT ?4",
            )?;

            let rows =
                stmt.query_map(params![query_blob, collection, query_width, limit], |row| {
                    let id: String = row.get(0)?;
                    let metadata_json: String = row.get(1)?;
                    let score: f64 = row.get(2)?;
                    let metadata: Value =
                        serde_json::from_str(&metadata_json).unwrap_or(Value::Null);
                    Ok(ScoredResult {
                        id,
                        score: score as f32,
                        metadata,
                    })
                })?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            Ok(results)
        })
    }

    fn clone_repo_sync(&self, url: &str) -> anyhow::Result<Repository> {
        let name = repo_name_from_url(url);
        std::fs::create_dir_all(self.repos_dir.as_ref())?;
        let local_path = self.repos_dir.join(&name);

        git2::Repository::clone(url, &local_path)?;

        let repo = Repository {
            id: Uuid::new_v4().to_string(),
            name,
            url: url.to_string(),
            local_path: Some(local_path.to_string_lossy().to_string()),
        };
        self.insert_repo_sync(&repo)?;
        Ok(repo)
    }

    // ------------------------------------------------------------------
    // Async CRUD API (the public surface; each call runs on a blocking
    // thread so it never stalls the async runtime).
    // ------------------------------------------------------------------

    /// Inserts or updates a repository record.
    pub async fn insert_repo(&self, repo: &Repository) -> anyhow::Result<()> {
        let db = self.clone();
        let repo = repo.clone();
        blocking(move || db.insert_repo_sync(&repo)).await
    }

    /// Lists all stored repositories.
    pub async fn list_repos(&self) -> anyhow::Result<Vec<Repository>> {
        let db = self.clone();
        blocking(move || db.list_repos_sync()).await
    }

    /// Inserts or updates an embedding vector for `id` within `collection`.
    pub async fn insert_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: Value,
    ) -> anyhow::Result<()> {
        let db = self.clone();
        let collection = collection.to_string();
        let id = id.to_string();
        blocking(move || db.insert_embedding_sync(&collection, &id, &vector, &metadata)).await
    }

    /// Searches `collection` for the vectors most similar to `vector`,
    /// returning results ordered by descending similarity.
    pub async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        let db = self.clone();
        let collection = collection.to_string();
        blocking(move || db.search_similar_sync(&collection, &vector, limit)).await
    }
}

/// Dispatches a synchronous database closure onto the blocking thread pool and
/// awaits its result, mapping a panicked task into a clean error.
async fn blocking<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await?
}

#[async_trait]
impl RepoManager for SqliteDb {
    async fn clone_repo(&self, url: &str) -> anyhow::Result<Repository> {
        let db = self.clone();
        let url = url.to_string();
        blocking(move || db.clone_repo_sync(&url)).await
    }

    async fn list_repos(&self) -> anyhow::Result<Vec<Repository>> {
        self.list_repos().await
    }
}

#[async_trait]
impl VectorDb for SqliteDb {
    async fn store_embedding(
        &self,
        collection: &str,
        id: &str,
        vector: Vec<f32>,
        metadata: Value,
    ) -> anyhow::Result<()> {
        self.insert_embedding(collection, id, vector, metadata)
            .await
    }

    async fn search_similar(
        &self,
        collection: &str,
        vector: Vec<f32>,
        limit: usize,
    ) -> anyhow::Result<Vec<ScoredResult>> {
        self.search_similar(collection, vector, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_create_tables() {
        let db = SqliteDb::open_in_memory().unwrap();

        db.with_conn(|conn| {
            let repositories: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'repositories'",
                [],
                |row| row.get(0),
            )?;
            let embeddings: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'embeddings'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(repositories, 1);
            assert_eq!(embeddings, 1);
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn insert_and_list_repos_roundtrip() {
        let db = SqliteDb::open_in_memory().unwrap();

        db.insert_repo(&Repository {
            id: "repo-1".to_string(),
            name: "gestalt".to_string(),
            url: "https://github.com/iberi22/gestalt.git".to_string(),
            local_path: Some("/tmp/gestalt".to_string()),
        })
        .await
        .unwrap();

        db.insert_repo(&Repository {
            id: "repo-2".to_string(),
            name: "xavier".to_string(),
            url: "https://github.com/iberi22/xavier.git".to_string(),
            local_path: None,
        })
        .await
        .unwrap();

        let repos = db.list_repos().await.unwrap();
        assert_eq!(repos.len(), 2);

        let by_id = |id: &str| repos.iter().find(|r| r.id == id).unwrap();
        assert_eq!(by_id("repo-1").name, "gestalt");
        assert_eq!(by_id("repo-2").local_path, None);
    }

    #[tokio::test]
    async fn insert_and_search_embeddings() {
        let db = SqliteDb::open_in_memory().unwrap();

        // Two orthogonal 3D vectors.
        db.insert_embedding(
            "chunks",
            "a",
            vec![1.0, 0.0, 0.0],
            serde_json::json!({ "path": "a.rs" }),
        )
        .await
        .unwrap();
        db.insert_embedding(
            "chunks",
            "b",
            vec![0.0, 1.0, 0.0],
            serde_json::json!({ "path": "b.rs" }),
        )
        .await
        .unwrap();

        let results = db
            .search_similar("chunks", vec![1.0, 0.0, 0.0], 2)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        // "a" should be the most similar (cosine similarity ~1.0).
        assert_eq!(results[0].id, "a");
        assert!(results[0].score > 0.99);
        // "b" is orthogonal (cosine similarity ~0.0).
        assert_eq!(results[1].id, "b");
        assert!(results[1].score.abs() < 0.01);

        // Metadata is deserialized back out.
        assert_eq!(results[0].metadata["path"], "a.rs");
    }

    #[tokio::test]
    async fn search_respects_limit_and_collection() {
        let db = SqliteDb::open_in_memory().unwrap();

        db.insert_embedding("c1", "x", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();
        db.insert_embedding("c2", "y", vec![1.0, 0.0], serde_json::json!({}))
            .await
            .unwrap();

        let results = db.search_similar("c1", vec![1.0, 0.0], 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "x");

        // Limit truncates the result set.
        db.insert_embedding("c1", "z", vec![0.9, 0.1], serde_json::json!({}))
            .await
            .unwrap();
        let limited = db.search_similar("c1", vec![1.0, 0.0], 1).await.unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[tokio::test]
    async fn vector_db_trait_store_and_search() {
        let db = SqliteDb::open_in_memory().unwrap();
        let db: Arc<dyn VectorDb> = Arc::new(db);

        db.store_embedding("docs", "one", vec![1.0, 2.0], serde_json::json!({ "k": 1 }))
            .await
            .unwrap();

        let hits = db.search_similar("docs", vec![1.0, 2.0], 1).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "one");
    }

    #[tokio::test]
    async fn repo_manager_trait_list_repos() {
        let db = SqliteDb::open_in_memory().unwrap();
        db.insert_repo(&Repository {
            id: "r".to_string(),
            name: "n".to_string(),
            url: "u".to_string(),
            local_path: None,
        })
        .await
        .unwrap();

        let mgr: Arc<dyn RepoManager> = Arc::new(db);
        let repos = mgr.list_repos().await.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, "r");
    }

    #[tokio::test]
    async fn clone_repo_local() {
        // Create a local git repo to clone without network access.
        let src = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(src.path()).unwrap();
        std::fs::write(src.path().join("README.md"), "# test\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();

        let dest = tempfile::tempdir().unwrap();
        let db = SqliteDb::open_in_memory()
            .unwrap()
            .with_repos_dir(dest.path());

        let url = src.path().to_str().unwrap();
        let cloned = db.clone_repo(url).await.unwrap();

        assert_eq!(cloned.name, repo_name_from_url(url));
        assert!(cloned.local_path.is_some());

        let repos = db.list_repos().await.unwrap();
        assert_eq!(repos.len(), 1);
    }
}
