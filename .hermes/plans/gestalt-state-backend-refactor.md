# Gestalt State Backend Refactor — Plan de Implementación

> **Para Hermes:** Usar subagent-driven-development con 4 subagentes cursor-agent Grok 4.5 LOW para implementación. Documentación con Grok 4.5 HIGH.

**Goal:** Reemplazar el sistema de estado actual de Gestalt (JSON files, JSONL timeline, git worktrees, Semaphore) con una arquitectura de 3 capas: MemState (DashMap) + StateDB (SQLite) + Xavier sync.

**Architecture:** Gestalt obtiene su propia base de datos de estado operativo (SQLite local con WAL mode). La memoria activa vive en DashMap (0-latency). El timeline se vuelve consultable por los agentes. Los worktrees se mantienen temporalmente pero sin `Mutex<()>` serializador. Xavier se usa solo para contexto PRE-run y archive POST-run.

**Tech Stack:** Rust, SQLite (rusqlite WAL mode), DashMap, tokio broadcast channel.

**Estructura final de crates:**
```
gestalt/
├── gestalt-state/          ← NUEVO: StateDB + MemState
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs          ← Re-export público
│   │   ├── statedb.rs      ← SQLite schema + operaciones
│   │   ├── memstate.rs     ← DashMap wrapper
│   │   └── schema.rs       ← Tipos compartidos
├── gestalt-router/         ← MODIFICADO: usa gestalt-state
│   ├── src/
│   │   ├── router.rs       ← Sin Semaphore, usa MemState
│   │   ├── agent.rs        ← SubprocessRunner sin cambios
│   │   └── run.rs          ← RunSpec, AgentResult
├── gestalt_core/           ← MODIFICADO: eliminar xavier2/
│   └── src/application/agent/
│       ├── xavier/         ← SE MANTIENE
│       └── xavier2/        ← SE ELIMINA (dead code)
└── gestalt_cli/            ← MODIFICADO: compila con cambios
```

---

## Task 1: Create gestalt-state crate

**Objective:** Crear el crate `gestalt-state` con SQLite schema (4 tablas) y operaciones CRUD.

**Files:**
- Create: `gestalt-state/Cargo.toml`
- Create: `gestalt-state/src/lib.rs`
- Create: `gestalt-state/src/schema.rs`
- Create: `gestalt-state/src/statedb.rs`
- Create: `gestalt-state/src/memstate.rs`
- Modify: `gestalt/Cargo.toml` (workspace members)
- Modify: `gestalt-router/Cargo.toml` (add dependency)

**Step 1: Create Cargo.toml**

```toml
[package]
name = "gestalt-state"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.49", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.15", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
thiserror = "1.0"
rusqlite = { version = "0.33", features = ["bundled"] }
dashmap = "6.1"
anyhow = "1.0"
async-trait = "0.1"
```

**Step 2: Schema (gestalt-state/src/schema.rs)**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;

/// Estados posibles de un agente
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    Pending,
    Running,
    Success,
    Timeout,
    Crashed,
    NoChanges,
    Quarantined,
}

/// Estado de un run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: Uuid,
    pub spec_json: String,
    pub status: String,       // "running" | "completed" | "failed"
    pub created_at: String,   // ISO 8601
    pub completed_at: Option<String>,
}

/// Estado persistente de un agente en un run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub run_id: Uuid,
    pub agent_id: String,
    pub state: AgentState,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub changed_files: Vec<String>,
    pub started_at: Option<String>,
}

/// Lock de archivo (para evitar escrituras concurrentes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLock {
    pub path: String,
    pub agent_id: String,
    pub run_id: Uuid,
    pub acquired_at: String,
    pub ttl_secs: u64,
}

/// Evento de timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub seq: i64,
    pub run_id: Uuid,
    pub agent_id: Option<String>,
    pub event_type: String,   // "state_changed" | "lock_acquired" | "lock_released" | "file_modified"
    pub payload: serde_json::Value,
    pub created_at: String,
}
```

**Step 3: StateDB (gestalt-state/src/statedb.rs)**

```rust
use crate::schema::*;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

/// SQLite state backend con WAL mode.
/// Operaciones: runs CRUD, agents CRUD, locks CRUD, timeline append+query.
pub struct StateDb {
    conn: Mutex<Connection>,
}

impl StateDb {
    /// Open or create database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    /// Create tables if not exist.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
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
                ttl_secs INTEGER NOT NULL DEFAULT 30,
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );
            CREATE TABLE IF NOT EXISTS timeline (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                agent_id TEXT,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );
            CREATE INDEX IF NOT EXISTS idx_timeline_run ON timeline(run_id);
            CREATE INDEX IF NOT EXISTS idx_agents_run ON agents(run_id);
            CREATE INDEX IF NOT EXISTS idx_locks_agent ON locks(agent_id);
        ")?;
        Ok(())
    }

    // ── Runs ──
    pub fn create_run(&self, run_id: Uuid, spec_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO runs (run_id, spec_json, status, created_at) VALUES (?1, ?2, 'running', datetime('now'))",
            params![run_id.to_string(), spec_json],
        )?;
        Ok(())
    }

    pub fn complete_run(&self, run_id: Uuid, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE runs SET status = ?1, completed_at = datetime('now') WHERE run_id = ?2",
            params![status, run_id.to_string()],
        )?;
        Ok(())
    }

    pub fn get_run(&self, run_id: Uuid) -> Result<Option<RunRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT run_id, spec_json, status, created_at, completed_at FROM runs WHERE run_id = ?1"
        )?;
        let mut rows = stmt.query_map(params![run_id.to_string()], |row| {
            Ok(RunRecord {
                run_id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                spec_json: row.get(1)?,
                status: row.get(2)?,
                created_at: row.get(3)?,
                completed_at: row.get(4)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    // ── Agents ──
    pub fn upsert_agent(&self, run_id: Uuid, agent_id: &str, state: AgentState, output: Option<&str>, error: Option<&str>, duration_ms: u64, changed_files: &[String]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let state_str = format!("{:?}", state);
        let files_json = serde_json::to_string(changed_files)?;
        conn.execute(
            "INSERT INTO agents (run_id, agent_id, state, output, error, duration_ms, changed_files)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, agent_id) DO UPDATE SET
                state = excluded.state,
                output = excluded.output,
                error = excluded.error,
                duration_ms = excluded.duration_ms,
                changed_files = excluded.changed_files",
            params![run_id.to_string(), agent_id, state_str, output, error, duration_ms, files_json],
        )?;
        Ok(())
    }

    // ── Locks (atomic acquire) ──
    /// Intenta adquirir lock. Retorna true si se adquirió, false si ya existe.
    pub fn acquire_lock(&self, path: &str, agent_id: &str, run_id: Uuid, ttl_secs: u64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        // Clean expired locks first
        conn.execute("DELETE FROM locks WHERE acquired_at < datetime('now', '-' || ttl_secs || ' seconds')", [])?;
        // Try insert
        let rows = conn.execute(
            "INSERT OR IGNORE INTO locks (path, agent_id, run_id, acquired_at, ttl_secs) VALUES (?1, ?2, ?3, datetime('now'), ?4)",
            params![path, agent_id, run_id.to_string(), ttl_secs],
        )?;
        Ok(rows > 0)
    }

    pub fn release_lock(&self, path: &str, agent_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM locks WHERE path = ?1 AND agent_id = ?2",
            params![path, agent_id],
        )?;
        Ok(rows > 0)
    }

    pub fn get_locks(&self) -> Result<Vec<FileLock>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, agent_id, run_id, acquired_at, ttl_secs FROM locks"
        )?;
        let locks = stmt.query_map([], |row| {
            Ok(FileLock {
                path: row.get(0)?,
                agent_id: row.get(1)?,
                run_id: Uuid::parse_str(&row.get::<_, String>(2)?).unwrap_or_default(),
                acquired_at: row.get(3)?,
                ttl_secs: row.get::<_, i64>(4)? as u64,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(locks)
    }

    // ── Timeline ──
    pub fn push_event(&self, run_id: Uuid, agent_id: Option<&str>, event_type: &str, payload: &serde_json::Value) -> Result<TimelineEvent> {
        let conn = self.conn.lock().unwrap();
        let payload_str = serde_json::to_string(payload)?;
        conn.execute(
            "INSERT INTO timeline (run_id, agent_id, event_type, payload, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![run_id.to_string(), agent_id, event_type, payload_str],
        )?;
        let seq = conn.last_insert_rowid();
        Ok(TimelineEvent {
            seq,
            run_id,
            agent_id: agent_id.map(|s| s.to_string()),
            event_type: event_type.to_string(),
            payload: payload.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn get_timeline(&self, run_id: Uuid, limit: i64) -> Result<Vec<TimelineEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT seq, run_id, agent_id, event_type, payload, created_at FROM timeline WHERE run_id = ?1 ORDER BY seq DESC LIMIT ?2"
        )?;
        let events = stmt.query_map(params![run_id.to_string(), limit], |row| {
            let payload_str: String = row.get(4)?;
            Ok(TimelineEvent {
                seq: row.get(0)?,
                run_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                agent_id: row.get(2)?,
                event_type: row.get(3)?,
                payload: serde_json::from_str(&payload_str).unwrap_or_default(),
                created_at: row.get(5)?,
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_query_run() {
        let db = StateDb::open(Path::new(":memory:")).unwrap();
        let run_id = Uuid::new_v4();
        db.create_run(run_id, "{\"task\":\"test\"}").unwrap();
        let run = db.get_run(run_id).unwrap().unwrap();
        assert_eq!(run.status, "running");
    }

    #[test]
    fn test_acquire_lock_atomic() {
        let db = StateDb::open(Path::new(":memory:")).unwrap();
        let run_id = Uuid::new_v4();
        let acquired = db.acquire_lock("src/main.rs", "agent-1", run_id, 30).unwrap();
        assert!(acquired);
        // Second acquire should fail
        let acquired2 = db.acquire_lock("src/main.rs", "agent-2", run_id, 30).unwrap();
        assert!(!acquired2);
    }

    #[test]
    fn test_timeline_events() {
        let db = StateDb::open(Path::new(":memory:")).unwrap();
        let run_id = Uuid::new_v4();
        db.create_run(run_id, "{}").unwrap();
        db.push_event(run_id, Some("agent-1"), "state_changed", &serde_json::json!({"from":"Pending","to":"Running"})).unwrap();
        db.push_event(run_id, Some("agent-1"), "state_changed", &serde_json::json!({"from":"Running","to":"Success"})).unwrap();
        let events = db.get_timeline(run_id, 10).unwrap();
        assert_eq!(events.len(), 2);
    }
}
```

**Step 4: MemState (gestalt-state/src/memstate.rs)**

```rust
use crate::schema::*;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Estado en memoria de todos los runs activos.
/// Operaciones: 0-latency reads, broadcast de eventos a agentes conectados.
#[derive(Clone)]
pub struct MemState {
    /// agent_states[(run_id, agent_id)] -> AgentState
    pub agent_states: Arc<DashMap<(Uuid, String), AgentState>>,
    /// active_locks[path] -> (agent_id, run_id)
    pub active_locks: Arc<DashMap<String, (String, Uuid)>>,
    /// Timeline event broadcaster (para WebSocket)
    pub timeline_tx: broadcast::Sender<TimelineEvent>,
}

impl MemState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            agent_states: Arc::new(DashMap::new()),
            active_locks: Arc::new(DashMap::new()),
            timeline_tx: tx,
        }
    }

    /// Get agent state (0-latency)
    pub fn get_agent_state(&self, run_id: Uuid, agent_id: &str) -> Option<AgentState> {
        self.agent_states.get(&(run_id, agent_id.to_string())).map(|v| *v)
    }

    /// Set agent state and broadcast event
    pub fn set_agent_state(&self, run_id: Uuid, agent_id: &str, state: AgentState) {
        let key = (run_id, agent_id.to_string());
        let old = self.agent_states.insert(key, state);
        let _ = self.timeline_tx.send(TimelineEvent {
            seq: 0,
            run_id,
            agent_id: Some(agent_id.to_string()),
            event_type: "state_changed".into(),
            payload: serde_json::json!({
                "from": format!("{:?}", old.unwrap_or(AgentState::Pending)),
                "to": format!("{:?}", state),
            }),
            created_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Try acquire lock in memory (fast path). Syncs to StateDB on conflict.
    pub fn try_lock(&self, path: &str, agent_id: &str, run_id: Uuid) -> bool {
        use dashmap::mapref::entry::Entry;
        match self.active_locks.entry(path.to_string()) {
            Entry::Vacant(e) => {
                e.insert((agent_id.to_string(), run_id));
                let _ = self.timeline_tx.send(TimelineEvent {
                    seq: 0,
                    run_id,
                    agent_id: Some(agent_id.to_string()),
                    event_type: "lock_acquired".into(),
                    payload: serde_json::json!({"path": path}),
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
                true
            }
            Entry::Occupied(e) => {
                // If owned by same agent, renew
                if e.get().0 == agent_id {
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Release lock
    pub fn release_lock(&self, path: &str, agent_id: &str) -> bool {
        if let Some((existing_agent, _)) = self.active_locks.get(path) {
            if existing_agent.as_str() == agent_id {
                self.active_locks.remove(path);
                return true;
            }
        }
        false
    }

    /// Subscribe to timeline events
    pub fn subscribe(&self) -> broadcast::Receiver<TimelineEvent> {
        self.timeline_tx.subscribe()
    }
}
```

**Step 5: lib.rs (gestalt-state/src/lib.rs)**

```rust
pub mod schema;
pub mod statedb;
pub mod memstate;

pub use schema::*;
pub use statedb::StateDb;
pub use memstate::MemState;
```

**Step 6: Update workspace Cargo.toml**

Add to `members` in `/home/belal/proyectosSWAL/gestalt/Cargo.toml`:
```toml
members = [
    "gestalt_core",
    "gestalt_cli",
    "synapse-agentic",
    "gestalt-router",
    "gestalt-merge",
    "gestalt-state",   // ← NEW
]
```

**Step 7: Add to gestalt-router Cargo.toml**

```toml
gestalt-state = { path = "../gestalt-state" }
dashmap = "6.1"  # Wait, already in gestalt-state. Router doesn't need direct dep.
```

**Verification:**
```bash
cd /home/belal/proyectosSWAL/gestalt
unset OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR
PKG_CONFIG_PATH="$(nix eval nixpkgs#openssl.dev --raw)/lib/pkgconfig"
cargo check -p gestalt-state 2>&1 | tail -5
# Expected: "error: expected X" or "Finished"
cargo test -p gestalt-state 2>&1 | tail -10
# Expected: "test result: ok. 3 passed; 0 failed"
```

---

## Task 2: MemState - Refactor Router

**Objective:** Refactor `gestalt-router/src/router.rs` para usar `gestalt-state` en lugar de `Semaphore` + `RunManifest` JSON.

**Files:**
- Modify: `gestalt-router/Cargo.toml` (add gestalt-state dep)
- Modify: `gestalt-router/src/router.rs` (eliminar Semaphore, usar MemState)
- Modify: `gestalt-router/src/run.rs` (eliminar struct Router duplicado)
- Modify: `gestalt-router/src/run_state.rs` (RunManifest → usar gestalt-state)
- Keep: `gestalt-router/src/timeline.rs` (reemplazar EventLog trait implementation con StateDB)
- Keep: `gestalt-router/src/worktree.rs` (transición, eliminar Mutex<()>)

**Key changes in router.rs:**

1. **Eliminar `Semaphore`** — reemplazar con `MemState::try_lock()` por archivo
2. **Eliminar `RunManifest` JSON** — usar `StateDb::create_run()` / `StateDb::upsert_agent()`
3. **Reemplazar `JsonlEventLog`** — implementar `EventLog` trait usando `StateDb::push_event()`
4. **Mantener Xavier PRE/POST** — pero usando StateDb para estado activo

**Nuevo execute() flow:**
```rust
pub async fn execute(&self, spec: RunSpec) -> Result<RunReport, RouterError> {
    let run_id = Uuid::new_v4();
    
    // 1. StateDB: create run
    self.state_db.create_run(run_id, &serde_json::to_string(&spec)?)?;
    
    // 2. PRE: Xavier context (optional)
    self.pre_fetch_context(&spec.task).await;
    
    // 3. Spawn agents without Semaphore
    let mut join_set = JoinSet::new();
    for agent in &spec.agents {
        let mem = self.mem_state.clone();
        let state_db = self.state_db.clone(); // Arc
        let run_id = run_id;
        let agent = agent.clone();
        let timeout = Duration::from_secs(spec.timeout);
        
        join_set.spawn(async move {
            mem.set_agent_state(run_id, &agent.id, AgentState::Running);
            state_db.upsert_agent(run_id, &agent.id, AgentState::Running, None, None, 0, &[])?;
            
            // ... execute agent in worktree ...
            // On completion:
            mem.set_agent_state(run_id, &agent.id, final_state);
            state_db.upsert_agent(run_id, &agent.id, final_state, output, error, duration, &changed_files)?;
            
            Ok(AgentResult { ... })
        });
    }
    
    // 4. Wait for all
    while let Some(res) = join_set.join_next().await { ... }
    
    // 5. POST: Xavier store (optional)
    self.post_store_results(run_id, &agent_results).await;
    
    // 6. Complete run
    self.state_db.complete_run(run_id, "completed")?;
    
    Ok(RunReport { ... })
}
```

**Eliminar código muerto:**
- `gestalt_core/src/application/agent/xavier2/` — directorio completo (no registrado en mod.rs)
- `gestalt_router::run::Router` struct (línea 174-176 en run.rs) — duplicado del Router real

**Verification:**
```bash
cd /home/belal/proyectosSWAL/gestalt
cargo check -p gestalt-router 2>&1 | grep "^error" | wc -l
# Debe ser 0 (puede haber warnings de código no usado)
cargo test -p gestalt-router 2>&1 | tail -5
```

---

## Task 3: EventLog trait → StateDB Implementation

**Objective:** Implementar el trait `EventLog` usando `StateDb` en lugar de `JsonlEventLog`.

**Files:**
- Modify: `gestalt-router/src/timeline.rs`

**Changes:**
1. Eliminar `JsonlEventLog` struct y su implementación
2. Crear `StateDbEventLog` que implementa `EventLog` vía `StateDb`
3. `EventLog::log()` → `StateDb::push_event()`
4. `EventLog::list_runs()` → `StateDb` query
5. `EventLog::read_events()` → `StateDb::get_timeline()`

```rust
pub struct StateDbEventLog {
    db: Arc<StateDb>,
    run_id: Uuid,
}

impl EventLog for StateDbEventLog {
    fn log(&self, event: Event) -> Result<(), RouterError> {
        let payload = serde_json::to_value(&event).map_err(|e| RouterError::timeline_error(e.to_string()))?;
        let agent_id = extract_agent_id(&event);
        self.db.push_event(self.run_id, agent_id.as_deref(), &event_type(&event), &payload)
            .map_err(|e| RouterError::timeline_error(e.to_string()))?;
        Ok(())
    }
    // ...
}
```

**Verification:**
```bash
cargo check -p gestalt-router 2>&1 | grep "^error"
```

---

## Task 4: Documentation + Cleanup (Grok 4.5 HIGH)

**Objective:** Documentar la nueva arquitectura y limpiar dead code.

**Files:**
- Create: `gestalt/ARCHITECTURE.md` (nueva arquitectura de estado)
- Create: `gestalt/docs/state-backend.md` (StateDB + MemState design)
- Delete: `gestalt_core/src/application/agent/xavier2/` (dead code)
- Modify: `gestalt-router/AGENTS.md` (actualizar non-negotiables)

**Key docs to produce:**
1. `ARCHITECTURE.md` — Visión general de las 3 capas (MemState, StateDB, Xavier)
2. `docs/state-backend.md` — Esquema SQL, operaciones, diagrama de flujo
3. `AGENTS.md` — Actualizar reglas de estado

**Verification:**
```bash
# Check no xavier2 remains
ls ~/proyectosSWAL/gestalt/gestalt_core/src/application/agent/xavier2/ 2>&1
# Expected: "No such file or directory"

# Full workspace check
cd /home/belal/proyectosSWAL/gestalt
cargo check --workspace 2>&1 | grep "^error" | wc -l
# Expected: 0
```

---

## Execution Log (to update as tasks complete)

| Date | Phase | Action | Status |
|---|---|---|---|
| — | Task 1 | gestalt-state crate (SQLite + schema tests) | ⏳ |
| — | Task 1 Judge | Review StateDb atomicity, lock semantics | ⏳ |
| — | Task 2 | MemState + Router refactor | ⏳ |
| — | Task 2 Judge | Review Semaphore removal, error paths | ⏳ |
| — | Task 3 | EventLog → StateDB | ⏳ |
| — | Task 3 Judge | Review timeline migration | ⏳ |
| — | Task 4 | Docs + Cleanup (Grok 4.5 HIGH) | ⏳ |
| — | Final Verify | `cargo check --workspace` 0 errors + tests pass | ⏳ |
