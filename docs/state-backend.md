# Gestalt State Backend

## Overview

The state backend is the heart of Gestalt's agent orchestration. It provides a 3-tier
architecture for managing concurrent agent execution, file system locks, and timeline events.

## SQL Schema (StateDB)

### `runs` table

Tracks orchestrator runs (each `gestalt router` invocation).

```sql
CREATE TABLE IF NOT EXISTS runs (
    id          TEXT PRIMARY KEY,
    task        TEXT NOT NULL,
    base_ref    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);
```

### `agents` table

Tracks individual agents within a run.

```sql
CREATE TABLE IF NOT EXISTS agents (
    id          TEXT PRIMARY KEY,
    run_id      TEXT NOT NULL REFERENCES runs(id),
    name        TEXT NOT NULL,
    state       TEXT NOT NULL DEFAULT 'pending',
    goal        TEXT,
    worktree    TEXT,
    branch      TEXT,
    output      TEXT,
    error       TEXT,
    duration_ms INTEGER,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);
```

### `locks` table

File-level locks for concurrency control (replaces Semaphore).

```sql
CREATE TABLE IF NOT EXISTS locks (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    path        TEXT NOT NULL,
    lock_type   TEXT NOT NULL DEFAULT 'exclusive',
    acquired_at TEXT NOT NULL DEFAULT (datetime('now')),
    released_at TEXT,
    UNIQUE(path, lock_type)
);
```

### `timeline` table

Event log for agent runs (replaces JsonlEventLog).

```sql
CREATE TABLE IF NOT EXISTS timeline (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT NOT NULL REFERENCES runs(id),
    agent_id    TEXT,
    event_type  TEXT NOT NULL,
    payload     TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_timeline_run_id ON timeline(run_id);
CREATE INDEX IF NOT EXISTS idx_timeline_created_at ON timeline(created_at);
```

## StateDb API

Defined in `gestalt-state/src/state_db.rs`:

```rust
impl StateDb {
    pub async fn new(path: &Path) -> Result<Self>;
    pub async fn create_run(&self, task: &str, base_ref: &str) -> Result<String>;
    pub async fn get_run(&self, run_id: &str) -> Result<Option<Run>>;
    pub async fn update_run_status(&self, run_id: &str, status: &str) -> Result<()>;
    pub async fn create_agent(&self, run_id: &str, name: &str, goal: Option<&str>) -> Result<String>;
    pub async fn update_agent_state(&self, agent_id: &str, state: &str) -> Result<()>;
    pub async fn acquire_lock(&self, agent_id: &str, path: &str) -> Result<bool>;
    pub async fn release_lock(&self, agent_id: &str, path: &str) -> Result<()>;
    pub async fn append_timeline(&self, run_id: &str, agent_id: Option<&str>, event_type: &str, payload: Option<&str>) -> Result<i64>;
    pub async fn get_timeline(&self, run_id: &str, limit: i64) -> Result<Vec<TimelineEntry>>;
}
```

## MemState API

Defined in `gestalt-state/src/mem_state.rs`:

```rust
impl MemState {
    pub fn new() -> Self;
    pub fn set_agent_state(&self, id: &str, state: AgentState);
    pub fn get_agent_state(&self, id: &str) -> Option<AgentState>;
    pub fn acquire_lock(&self, path: &str, agent_id: &str) -> bool;
    pub fn release_lock(&self, path: &str, agent_id: &str);
    pub fn get_active_locks(&self) -> Vec<(String, String)>;
    pub fn timeline_subscribe(&self) -> broadcast::Receiver<TimelineEvent>;
    pub fn timeline_publish(&self, event: TimelineEvent);
}
```

## Migration from Old System

### What Changed

| Old | New | Reason |
|-----|-----|--------|
| `tokio::sync::Semaphore` | `locks` table + MemState | File-level granularity |
| JSON state files | `StateDb` (SQLite) | Atomicity, queryability |
| `JsonlEventLog` | `StateDbEventLog` | ACID compliance, indexing |
| SurrealDB timeline | SQLite timeline | Simpler deployment (no external DB) |

### Migration Path

1. `StateDb` auto-creates tables on initialization (`PRAGMA journal_mode=WAL;`)
2. Old JSON state files are ignored (not migrated — fresh state)
3. Old `events.jsonl` files are ignored (not migrated — fresh timeline)
4. Xavier memory store is untouched (it was always separate)
