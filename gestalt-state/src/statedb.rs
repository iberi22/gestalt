use crate::schema::{AgentRecord, FileLock, RunRecord, TimelineEvent};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics tracked during database transactions.
#[derive(Debug, Default)]
pub struct DbMetrics {
    /// Accumulated duration of writes (in milliseconds).
    pub write_latency_ms: AtomicU64,
    /// Number of conflict/busy occurrences (lock conflicts).
    pub conflict_count: AtomicU64,
    /// Total number of transaction retries attempted.
    pub retry_count: AtomicU64,
    /// Total number of successful transaction writes.
    pub write_count: AtomicU64,
}

/// Persistent SQLite-backed state store for agent orchestration.
///
/// Uses WAL mode for concurrent read performance and stores
/// runs, agents, file locks, and timeline events.
#[derive(Clone)]
pub struct StateDb {
    conn: Arc<Mutex<Connection>>,
    metrics: Arc<DbMetrics>,
}

/// Transaction-hardened variant of the SQLite StateDB.
pub type TransactionalStateDb = StateDb;

/// Transaction-hardened variant of the SQLite StateDB (Pascal/upper-case alias).
pub type TransactionalStateDB = StateDb;

impl StateDb {
    /// Open (or create) a SQLite database at `path` and run migrations.
    ///
    /// Enables WAL journal mode and foreign keys automatically.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent dir for {}", path.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite DB at {}", path.display()))?;

        // Enable WAL mode for concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMAs")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            metrics: Arc::new(DbMetrics::default()),
        };

        db.migrate()?;

        Ok(db)
    }

    /// Access the metrics of the StateDb.
    pub fn metrics(&self) -> Arc<DbMetrics> {
        self.metrics.clone()
    }

    /// Run a closure inside a SQLite exclusive transaction with automated retries and metric tracking.
    ///
    /// If the transaction closure returns an Err, the transaction is automatically rolled back.
    /// Lock conflicts are automatically retried up to 5 times with a brief delay.
    pub fn execute_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: Fn(&rusqlite::Transaction<'_>) -> Result<T>,
    {
        enum TxAttemptResult<T> {
            Success(T),
            UserError(anyhow::Error),
            DbError { is_busy: bool, err: rusqlite::Error },
        }

        let start_time = std::time::Instant::now();
        let max_retries = 5;
        let mut attempts = 0;

        loop {
            // Helper scope to ensure MutexGuard and Transaction are dropped before sleep/retries
            let run_attempt = || {
                let mut conn = match self.conn.lock() {
                    Ok(guard) => guard,
                    Err(e) => return TxAttemptResult::UserError(anyhow::anyhow!("DB lock poisoned: {e}")),
                };

                let tx = match conn.transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive) {
                    Ok(t) => t,
                    Err(e) => {
                        let is_busy = matches!(e, rusqlite::Error::SqliteFailure(ref err, _) if err.code == rusqlite::ErrorCode::DatabaseBusy || err.code == rusqlite::ErrorCode::DatabaseLocked);
                        return TxAttemptResult::DbError { is_busy, err: e };
                    }
                };

                match f(&tx) {
                    Ok(res) => {
                        if let Err(e) = tx.commit() {
                            let is_busy = matches!(e, rusqlite::Error::SqliteFailure(ref err, _) if err.code == rusqlite::ErrorCode::DatabaseBusy || err.code == rusqlite::ErrorCode::DatabaseLocked);
                            return TxAttemptResult::DbError { is_busy, err: e };
                        }
                        TxAttemptResult::Success(res)
                    }
                    Err(err) => {
                        let _ = tx.rollback();
                        TxAttemptResult::UserError(err)
                    }
                }
            };

            match run_attempt() {
                TxAttemptResult::Success(val) => {
                    // Success! Record metrics and return result
                    let elapsed = start_time.elapsed().as_millis() as u64;
                    self.metrics.write_latency_ms.fetch_add(elapsed, Ordering::SeqCst);
                    self.metrics.write_count.fetch_add(1, Ordering::SeqCst);
                    return Ok(val);
                }
                TxAttemptResult::UserError(user_err) => {
                    // Non-retryable closure/user error. Rollback was handled or is automatic.
                    return Err(user_err);
                }
                TxAttemptResult::DbError { is_busy, err } => {
                    if is_busy && attempts < max_retries {
                        attempts += 1;
                        self.metrics.conflict_count.fetch_add(1, Ordering::SeqCst);
                        self.metrics.retry_count.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(50 * attempts));
                        continue;
                    }
                    return Err(anyhow::anyhow!("Database error during transaction execution: {err}"));
                }
            }
        }
    }

    /// Create all tables and indexes if they don't exist.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                spec_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                created_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS agents (
                run_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'Pending',
                output TEXT,
                error TEXT,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                changed_files TEXT NOT NULL DEFAULT '[]',
                started_at TEXT,
                PRIMARY KEY (run_id, agent_id),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );

            CREATE TABLE IF NOT EXISTS locks (
                path TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                acquired_at TEXT NOT NULL,
                ttl_secs INTEGER NOT NULL DEFAULT 30
            );

            CREATE TABLE IF NOT EXISTS timeline (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                agent_id TEXT,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_timeline_run ON timeline(run_id);
            CREATE INDEX IF NOT EXISTS idx_agents_run ON agents(run_id);
            ",
        )
        .context("Failed to run migrations")?;
        Ok(())
    }

    /// Access the internal SQLite connection.
    ///
    /// This is crate-internal so that [`super::virtual_fs::StateDbVfs`]
    /// can share the same database for the `file_versions` table.
    pub(crate) fn conn(&self) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>, anyhow::Error> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DB lock poisoned: {e}"))
    }

    // ── Runs ──────────────────────────────────────────────────────────

    /// Create a new execution run.
    pub fn create_run(&self, run_id: &str, spec_json: &str) -> Result<()> {
        self.execute_transaction(|tx| {
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO runs (run_id, spec_json, status, created_at) VALUES (?1, ?2, 'running', ?3)",
                rusqlite::params![run_id, spec_json, now],
            )
            .context("Failed to create run")?;
            Ok(())
        })
    }

    /// Mark a run as completed with a given status.
    pub fn complete_run(&self, run_id: &str, status: &str) -> Result<()> {
        self.execute_transaction(|tx| {
            let now = Utc::now().to_rfc3339();
            let rows = tx
                .execute(
                    "UPDATE runs SET status = ?1, completed_at = ?2 WHERE run_id = ?3",
                    rusqlite::params![status, now, run_id],
                )
                .context("Failed to complete run")?;
            if rows == 0 {
                anyhow::bail!("Run {run_id} not found");
            }
            Ok(())
        })
    }

    /// Retrieve a run by its ID.
    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT run_id, spec_json, status, created_at, completed_at FROM runs WHERE run_id = ?1")
            .context("Failed to prepare get_run query")?;

        let mut rows = stmt.query_map(rusqlite::params![run_id], |row| {
            let created_at_str: String = row.get(3)?;
            let completed_at_str: Option<String> = row.get(4)?;

            Ok(RunRecord {
                run_id: row.get(0)?,
                spec_json: row.get(1)?,
                status: row.get(2)?,
                created_at: created_at_str
                    .parse::<DateTime<Utc>>()
                    .unwrap_or_else(|_| Utc::now()),
                completed_at: completed_at_str
                    .map(|s| s.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now())),
            })
        })?;

        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(anyhow::anyhow!("Failed to read run: {e}")),
            None => Ok(None),
        }
    }

    // ── Agents ────────────────────────────────────────────────────────

    /// Insert or update an agent's state within a run.
    ///
    /// Uses `ON CONFLICT DO UPDATE` so the same agent can be updated
    /// multiple times during a run (e.g. Pending → Running → Success).
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_agent(
        &self,
        run_id: &str,
        agent_id: &str,
        state: &str,
        output: Option<&str>,
        error: Option<&str>,
        duration_ms: i64,
        changed_files: &str,
    ) -> Result<()> {
        self.execute_transaction(|tx| {
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO agents (run_id, agent_id, state, output, error, duration_ms, changed_files, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(run_id, agent_id) DO UPDATE SET
                    state = excluded.state,
                    output = excluded.output,
                    error = excluded.error,
                    duration_ms = excluded.duration_ms,
                    changed_files = excluded.changed_files,
                    started_at = COALESCE(agents.started_at, excluded.started_at)",
                rusqlite::params![run_id, agent_id, state, output, error, duration_ms, changed_files, now],
            )
            .context("Failed to upsert agent")?;
            Ok(())
        })
    }

    /// Retrieve an agent record by run_id and agent_id.
    pub fn get_agent(&self, run_id: &str, agent_id: &str) -> Result<Option<AgentRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT run_id, agent_id, state, output, error, duration_ms, changed_files, started_at
                 FROM agents WHERE run_id = ?1 AND agent_id = ?2",
            )
            .context("Failed to prepare get_agent query")?;

        let mut rows = stmt.query_map(rusqlite::params![run_id, agent_id], |row| {
            let started_at_str: Option<String> = row.get(7)?;
            Ok(AgentRecord {
                run_id: row.get(0)?,
                agent_id: row.get(1)?,
                state: row.get(2)?,
                output: row.get(3)?,
                error: row.get(4)?,
                duration_ms: row.get(5)?,
                changed_files: row.get(6)?,
                started_at: started_at_str
                    .map(|s| s.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now())),
            })
        })?;

        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(anyhow::anyhow!("Failed to read agent: {e}")),
            None => Ok(None),
        }
    }

    // ── File Locks ────────────────────────────────────────────────────

    /// Try to acquire an exclusive lock on `path` for `agent_id`.
    ///
    /// Returns `true` if the lock was acquired. Automatically cleans up
    /// expired locks (based on `ttl_secs`) before attempting acquisition.
    pub fn acquire_lock(
        &self,
        path: &str,
        agent_id: &str,
        run_id: &str,
        ttl_secs: i64,
    ) -> Result<bool> {
        self.execute_transaction(|tx| {
            // Clean up expired locks
            let deadline_iso = (Utc::now() - chrono::Duration::seconds(ttl_secs)).to_rfc3339();

            // SQLite datetime comparison works because RFC3339 timestamps sort lexicographically
            tx.execute(
                "DELETE FROM locks WHERE acquired_at < ?1",
                rusqlite::params![deadline_iso],
            )
            .context("Failed to clean expired locks")?;

            // Attempt to acquire the lock
            let now_iso = Utc::now().to_rfc3339();
            let result = tx.execute(
                "INSERT OR IGNORE INTO locks (path, agent_id, run_id, acquired_at, ttl_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![path, agent_id, run_id, now_iso, ttl_secs],
            );

            match result {
                Ok(affected) => Ok(affected > 0),
                Err(e) => Err(anyhow::anyhow!("Failed to acquire lock: {e}")),
            }
        })
    }

    /// Release a lock previously acquired by `agent_id` on `path`.
    ///
    /// Returns `true` if a lock was actually released.
    pub fn release_lock(&self, path: &str, agent_id: &str) -> Result<bool> {
        self.execute_transaction(|tx| {
            let rows = tx
                .execute(
                    "DELETE FROM locks WHERE path = ?1 AND agent_id = ?2",
                    rusqlite::params![path, agent_id],
                )
                .context("Failed to release lock")?;
            Ok(rows > 0)
        })
    }

    /// List all currently held file locks.
    pub fn get_locks(&self) -> Result<Vec<FileLock>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT path, agent_id, run_id, acquired_at, ttl_secs FROM locks")
            .context("Failed to prepare get_locks query")?;

        let locks = stmt
            .query_map([], |row| {
                let acquired_at_str: String = row.get(3)?;
                Ok(FileLock {
                    path: row.get(0)?,
                    agent_id: row.get(1)?,
                    run_id: row.get(2)?,
                    acquired_at: acquired_at_str
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                    ttl_secs: row.get(4)?,
                })
            })
            .context("Failed to query locks")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read locks")?;

        Ok(locks)
    }

    // ── Timeline Events ───────────────────────────────────────────────

    /// Push a new timeline event and return it with the assigned seq number.
    pub fn push_event(
        &self,
        run_id: &str,
        agent_id: Option<&str>,
        event_type: &str,
        payload: &str,
    ) -> Result<TimelineEvent> {
        self.execute_transaction(|tx| {
            let now = Utc::now().to_rfc3339();

            tx.execute(
                "INSERT INTO timeline (run_id, agent_id, event_type, payload, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![run_id, agent_id, event_type, payload, now],
            )
            .context("Failed to insert timeline event")?;

            let seq = tx.last_insert_rowid();

            Ok(TimelineEvent {
                seq: Some(seq),
                run_id: run_id.to_string(),
                agent_id: agent_id.map(|s| s.to_string()),
                event_type: event_type.to_string(),
                payload: payload.to_string(),
                created_at: now.parse::<DateTime<Utc>>().unwrap_or_else(|_| Utc::now()),
            })
        })
    }

    /// Fetch timeline events for a run, ordered by sequence number.
    pub fn get_timeline(&self, run_id: &str, limit: i64) -> Result<Vec<TimelineEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT seq, run_id, agent_id, event_type, payload, created_at
                 FROM timeline WHERE run_id = ?1
                 ORDER BY seq ASC LIMIT ?2",
            )
            .context("Failed to prepare get_timeline query")?;

        let events = stmt
            .query_map(rusqlite::params![run_id, limit], |row| {
                let seq: i64 = row.get(0)?;
                let created_at_str: String = row.get(5)?;
                Ok(TimelineEvent {
                    seq: Some(seq),
                    run_id: row.get(1)?,
                    agent_id: row.get(2)?,
                    event_type: row.get(3)?,
                    payload: row.get(4)?,
                    created_at: created_at_str
                        .parse::<DateTime<Utc>>()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .context("Failed to query timeline")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read timeline events")?;

        Ok(events)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> StateDb {
        StateDb::open(":memory:").expect("Failed to open in-memory DB")
    }

    fn rand_num_helper() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    }

    #[test]
    fn test_create_and_query_run() {
        let db = setup_db();

        db.create_run("run-001", r#"{"task": "test"}"#)
            .expect("create_run failed");

        let run = db.get_run("run-001").expect("get_run failed");
        assert!(run.is_some(), "Run should exist");
        let run = run.unwrap();
        assert_eq!(run.run_id, "run-001");
        assert_eq!(run.status, "running");
        assert!(
            run.completed_at.is_none(),
            "Run should not be completed yet"
        );

        db.complete_run("run-001", "success")
            .expect("complete_run failed");

        let run = db.get_run("run-001").expect("get_run failed");
        let run = run.unwrap();
        assert_eq!(run.status, "success");
        assert!(run.completed_at.is_some(), "Run should be completed");
    }

    #[test]
    fn test_acquire_lock_atomic() {
        let db = setup_db();

        // Acquire lock should succeed
        let acquired = db
            .acquire_lock("/tmp/test.lock", "agent-1", "run-001", 30)
            .expect("acquire_lock failed");
        assert!(acquired, "First lock should be acquired");

        // Second acquire on same path should fail
        let acquired = db
            .acquire_lock("/tmp/test.lock", "agent-2", "run-001", 30)
            .expect("acquire_lock failed");
        assert!(!acquired, "Second lock should be rejected");

        // Release by original owner
        let released = db
            .release_lock("/tmp/test.lock", "agent-1")
            .expect("release_lock failed");
        assert!(released, "Lock should be released");

        // After release, another agent can acquire
        let acquired = db
            .acquire_lock("/tmp/test.lock", "agent-2", "run-002", 30)
            .expect("acquire_lock failed");
        assert!(acquired, "Lock should be acquired after release");

        // Verify locks listing
        let locks = db.get_locks().expect("get_locks failed");
        assert_eq!(locks.len(), 1, "Should have exactly 1 lock");
        assert_eq!(locks[0].agent_id, "agent-2");

        // Release with wrong agent should fail
        let released = db
            .release_lock("/tmp/test.lock", "agent-1")
            .expect("release_lock failed");
        assert!(!released, "Wrong agent should not release lock");
    }

    #[test]
    fn test_timeline_events() {
        let db = setup_db();

        db.create_run("run-001", r#"{"task": "test"}"#)
            .expect("create_run failed");

        let evt = db
            .push_event("run-001", Some("agent-1"), "started", r#"{"reason": "ok"}"#)
            .expect("push_event failed");
        assert!(evt.seq.is_some(), "Event should have a seq number");
        assert_eq!(evt.run_id, "run-001");

        let evt2 = db
            .push_event(
                "run-001",
                Some("agent-1"),
                "completed",
                r#"{"result": "success"}"#,
            )
            .expect("push_event failed");
        assert!(
            evt2.seq.unwrap() > evt.seq.unwrap(),
            "Second event should have higher seq"
        );

        let timeline = db.get_timeline("run-001", 10).expect("get_timeline failed");
        assert_eq!(timeline.len(), 2, "Should have 2 timeline events");

        // First event should be the oldest (ASC order)
        assert_eq!(timeline[0].event_type, "started");
        assert_eq!(timeline[1].event_type, "completed");
    }

    #[test]
    fn test_agent_state_update() {
        let db = setup_db();

        db.create_run("run-001", r#"{"task": "test"}"#)
            .expect("create_run failed");

        // Initial upsert — agent starts in Pending state
        db.upsert_agent("run-001", "agent-1", "pending", None, None, 0, "[]")
            .expect("upsert_agent failed");

        let agent = db
            .get_agent("run-001", "agent-1")
            .expect("get_agent failed");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().state, "pending");

        // Update to Running
        db.upsert_agent(
            "run-001",
            "agent-1",
            "running",
            Some("working..."),
            None,
            1500,
            "[]",
        )
        .expect("upsert_agent failed");

        let agent = db
            .get_agent("run-001", "agent-1")
            .expect("get_agent failed");
        let agent = agent.unwrap();
        assert_eq!(agent.state, "running");
        assert_eq!(agent.output, Some("working...".to_string()));
        assert_eq!(agent.duration_ms, 1500);

        // Update to Success with changed files
        db.upsert_agent(
            "run-001",
            "agent-1",
            "success",
            Some("done"),
            None,
            3200,
            r#"["src/main.rs", "Cargo.toml"]"#,
        )
        .expect("upsert_agent failed");

        let agent = db
            .get_agent("run-001", "agent-1")
            .expect("get_agent failed");
        let agent = agent.unwrap();
        assert_eq!(agent.state, "success");
        assert_eq!(agent.changed_files, r#"["src/main.rs", "Cargo.toml"]"#);
        assert!(
            agent.started_at.is_some(),
            "started_at should be set from initial upsert"
        );
    }

    #[test]
    fn test_transaction_rollback() {
        let db = setup_db();

        // 1. Run a transaction that inserts a run and then fails
        let res: Result<()> = db.execute_transaction(|tx| {
            tx.execute(
                "INSERT INTO runs (run_id, spec_json, status, created_at) VALUES ('rollback-run', '{}', 'running', 'now')",
                [],
            )?;
            anyhow::bail!("Simulated write failure for rollback verification");
        });

        // 2. Verify transaction returned error
        assert!(res.is_err(), "Transaction should return an error");

        // 3. Verify that the run was rolled back and is NOT present in the database
        let run = db.get_run("rollback-run").expect("get_run failed");
        assert!(run.is_none(), "Run should not exist due to rollback");
    }

    #[test]
    fn test_transaction_metrics_tracking() {
        let db = setup_db();

        let metrics = db.metrics();
        assert_eq!(metrics.write_count.load(Ordering::SeqCst), 0);

        // Perform 3 writes
        db.create_run("run-1", "{}").unwrap();
        db.create_run("run-2", "{}").unwrap();
        db.create_run("run-3", "{}").unwrap();

        assert_eq!(metrics.write_count.load(Ordering::SeqCst), 3);
        let _ = metrics.write_latency_ms.load(Ordering::SeqCst);
        assert_eq!(metrics.conflict_count.load(Ordering::SeqCst), 0);
        assert_eq!(metrics.retry_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_parallel_concurrency_and_conflicts() {
        // Create a temporary file for concurrent SQLite access
        let rand_num = rand_num_helper();
        let db_path = std::env::temp_dir().join(format!("concurrency_test_{}.db", rand_num));
        if db_path.exists() {
            let _ = std::fs::remove_file(&db_path);
        }

        // Open db and initialize tables
        let db = StateDb::open(&db_path).expect("Failed to open DB file");
        db.create_run("run-shared", "{}").unwrap();

        let num_threads = 5;
        let writes_per_thread = 5;
        let mut handles = vec![];

        for t in 0..num_threads {
            let db_clone = db.clone();
            let handle = std::thread::spawn(move || {
                for w in 0..writes_per_thread {
                    let agent_id = format!("agent-{t}-{w}");
                    db_clone
                        .upsert_agent("run-shared", &agent_id, "success", Some("ok"), None, 100, "[]")
                        .expect("Concurrent upsert failed");
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all agents were successfully inserted
        for t in 0..num_threads {
            for w in 0..writes_per_thread {
                let agent_id = format!("agent-{t}-{w}");
                let agent = db.get_agent("run-shared", &agent_id).unwrap();
                assert!(agent.is_some(), "Agent {} should be persisted", agent_id);
            }
        }

        // Verify metrics
        let metrics = db.metrics();
        let total_expected_writes = 1 + (num_threads * writes_per_thread); // 1 for create_run + thread writes
        assert_eq!(metrics.write_count.load(Ordering::SeqCst), total_expected_writes as u64);

        // Under high concurrency, conflicts/retries may or may not occur depending on execution speed,
        // but we verify that the metrics exist and can be safely read.
        let conflicts = metrics.conflict_count.load(Ordering::SeqCst);
        let retries = metrics.retry_count.load(Ordering::SeqCst);
        println!("Concurrency metrics: conflicts={}, retries={}, write_latency_ms={}", conflicts, retries, metrics.write_latency_ms.load(Ordering::SeqCst));
    }
}
