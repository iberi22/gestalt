# Análisis Arquitectónico: ¿Xavier como Backend de Estado en Tiempo Real para Gestalt?

**Fecha:** 2026-07-26
**Autor:** Análisis Hermes Agent
**Contexto:** Decisión arquitectónica para Gestalt (orquestador multi-agente local) sobre si usar Xavier como backend de estado en tiempo real.

---

## Resumen Ejecutivo

**Xavier NO debe ser el backend de estado en tiempo real para Gestalt.** Esa aproximación es sobreingeniería en el sentido de usar la herramienta incorrecta para el trabajo. Xavier está diseñado para memoria persistente y búsqueda semántica — no para estado operativo en tiempo real. Gestalt necesita su propia base de datos operacional (SQLite local) con una capa de caché en memoria (DashMap), y usar Xavier exclusivamente para su propósito natural: contexto pre-run y almacenamiento post-run.

**La respuesta correcta no es "Xavier o base de datos propia" — es una arquitectura de 3 capas donde cada componente hace lo que mejor sabe hacer.**

---

## 1. El Problema: ¿Qué necesita Gestalt en Tiempo Real?

La visión del usuario para Gestalt (100+ tareas simultáneas, virtual file versioning, block-level editing, timeline en tiempo real, overlap resolution automática) se traduce en estos requisitos de estado:

| Requisito | Frecuencia | Latencia Requerida | Atomicidad |
|-----------|-----------|-------------------|------------|
| Transición de estado de agente (Pending→Running→Success) | ~10-100/seg | <1ms | Sí |
| Adquirir/liberar lock de archivo | ~100-1000/seg | <0.5ms | Sí (UNIQUE+IF NOT EXISTS) |
| Consultar estado de agente activo | ~1000/seg | <0.1ms (lectura) | No |
| Timeline: insertar evento | ~10-100/seg | <1ms | No |
| Timeline: consultar eventos | ~1/seg | <50ms | No |
| Bloque de código: marcar como editado | ~50-500/seg | <1ms | Sí |
| Virtual file version: read/write | ~100-1000/seg | <0.5ms | Sí |
| Overlap detection en tiempo real | ~10/seg | <5ms | No |
| Búsqueda semántica de contexto (pre-run) | ~1/run | 200ms (aceptable) | No |
| Almacenamiento persistente de resultados (post-run) | ~1/run | 200ms (aceptable) | No |

---

## 2. El Diagnóstico: Por qué Xavier es INCORRECTO para Tiempo Real

### 2.1 Análisis del XavierClient Actual (gestalt_core)

El `XavierClient` en `gestalt_core/src/application/agent/xavier/client.rs` es extremadamente simple:

```rust
pub struct XavierClient {
    pub endpoint: String,
    pub token: String,
    client: Client,  // reqwest::Client
}
```

Solo ofrece 3 operaciones:
- `search(query, limit, mode)` → `POST /v1/memories/search` (50-200ms)
- `add(content, path, kind, metadata)` → `POST /v1/memories` (50-200ms)
- `stats()` / `health()` → lecturas simples

### 2.2 MemoryRecord: Sobrecarga Masiva para Estado Operativo

`MemoryRecord` (en `xavier/src/memory/store.rs`) tiene **16 campos**:

```rust
pub struct MemoryRecord {
    pub id: String,
    pub workspace_id: String,
    pub path: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub embedding: Vec<f32>,          // ← 1536 floats
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub revision: u64,
    pub primary: bool,
    pub parent_id: Option<String>,
    pub cluster_id: Option<String>,
    pub level: MemoryLevel,
    pub relation: Option<RelationKind>,
    pub clearance: ClearanceLevel,
    pub revisions: Vec<MemoryRevision>,
    pub encrypted_dek: Option<Vec<u8>>,
    pub content_iv: Option<Vec<u8>>,
    pub metadata_iv: Option<Vec<u8>>,
    pub score: f32,
    pub deleted_at: Option<DateTime<Utc>>,
}
```

**El problema:** Para actualizar el estado de un agente de `Running` → `Success`, tendrías que:
1. Serializar un `MemoryRecord` completo (incluyendo embedding de 1536 floats = ~6KB)
2. Hacer un HTTP POST con todo ese payload
3. Re-generar embedding en Xavier
4. Parsear respuesta

Esto es **~500x más caro** que un `UPDATE agent_states SET status='success' WHERE id=?` en SQLite (~0.1ms vs 50-200ms).

### 2.3 Carencias Críticas de Xavier

| Capacidad | ¿Xavier la tiene? | ¿Gestalt la necesita? |
|-----------|-------------------|----------------------|
| **Atomic CAS** ("UPDATE status='running' WHERE status='pending'") | ❌ No | ✅ Crítico para evitar race conditions |
| **Partial UPDATE** (cambiar 1 campo de 20) | ❌ No (full upsert) | ✅ Esencial para eficiencia |
| **TTL/Expiry automático** (locks que expiran) | ❌ No | ✅ Los locks huérfanos rompen el sistema |
| **Sub-millisecond lookups** | ❌ No (50-200ms) | ✅ Estado de agente consultado ~1000/seg |
| **WebSocket/pubsub** (timeline streaming) | ❌ No | ✅ Timeline en tiempo real |
| **Transacciones** (rollback de grupo de operaciones) | ❌ No | ✅ Para consistencia de run |
| **UNIQUE constraint** (por path de archivo) | ❌ No (embedding store) | ✅ Para prevenir locks duplicados |
| **Consultas ad-hoc** ("agentes con estado crashed") | ❌ Búsqueda semántica, no SQL | ✅ Necesario para diagnóstico |

### 2.4 Arquitectura Actual: Acoplamientos Débiles y Cuellos de Botella

El Router actual (`gestalt-router/src/router.rs`) revela problemas de diseño:

```
PRE:  Xavier.search(task)          → 50-200ms (no bloqueante, bien)
      ↓
Worktree creation (serial, git)    → 100-500ms por agente
      ↓
Semaphore (max_parallel=N)         → BLOQUEANTE para N+1 agentes
      ↓
RunManifest (json file + Mutex)    → Lectura/escritura por archivo
      ↓
JsonlEventLog (append-only file)    → síncrono, fsync en cada evento
      ↓
RunFinished → POST: Xavier.add()   → 50-200ms (no bloqueante, bien)
      ↓
Overlap detection (git diff POST-hoc) → Bloqueante, después de todo
```

**Problemas específicos:**
1. **RunManifest en archivo JSON** con `Mutex` → contención en cada transición de estado (~4 writes por agente por run)
2. **JsonlEventLog** con fsync en cada append → ~10-30ms por evento (para 100 eventos/run = 1-3s solo en I/O de timeline)
3. **WorktreeManager** con git worktrees → no escala a 100+ tareas (git worktrees son O(n) en disco)
4. **Overlap detection post-hoc** con git diff → si se detecta tarde, trabajo desperdiciado
5. **No hay consulta en tiempo real** — nadie puede preguntar "¿en qué está trabajando el agente X?"

---

## 3. La Solución: Arquitectura de 3 Capas

```
┌──────────────────────────────────────────────────────────────────┐
│                        GESTALT ROUTER                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  TIER 1: CACHE EN MEMORIA (DashMap)                             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ • agent_states: Arc<DashMap<String, AgentState>>        │    │
│  │ • file_locks: Arc<DashMap<String, LockEntry>>           │    │
│  │ • active_runs: Arc<DashMap<Uuid, RunContext>>           │    │
│  │ • timeline_buffer: Arc<DashMap<Uuid, VecDeque<Event>>>  │    │
│  │ • virtual_files: Arc<DashMap<String, VirtualFile>>      │    │
│  │                                                         │    │
│  │  Latencia: ~0ns (puntero en memoria)                    │    │
│  │  Sincronización: tokio::sync::RwLock por shard          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │ sync periódico + write-through    │
│                              ▼                                   │
│  TIER 2: SQLITE LOCAL (gestalt's own operational DB)             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ • agent_states:  UPDATE status=? WHERE id=?             │    │
│  │ • file_locks:    INSERT INTO locks UNIQUE(path)         │    │
│  │ • timeline:      INSERT INTO events (run_id, ...)       │    │
│  │ • virtual_files: UPDATE/INSERT file_versions            │    │
│  │ • checkpoints:   INSERT INTO snapshots                  │    │
│  │ • runs:          SELECT status FROM runs WHERE...       │    │
│  │                                                         │    │
│  │  Latencia: ~0.1-1ms (local, sin red)                   │    │
│  │  Consistencia: WAL mode, IMMEDIATE transactions         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │ (cuando corresponde)              │
│                              ▼                                   │
│  TIER 3: XAVIER (memoria semántica externa)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ • PRE-run:  search(context) → búsqueda semántica        │    │
│  │ • POST-run: add(run_result) → almacenar resultados      │    │
│  │ • Post-hoc: search(analytics) → consultas cross-sesión  │    │
│  │ • Meta-análisis: qué patrones de error son comunes      │    │
│  │                                                         │    │
│  │  Latencia: 50-200ms (aceptable para operaciones async)  │    │
│  │  Propósito: contexto persistente searchable             │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 4. Diseño Detallado de Cada Capa

### 4.1 Tier 1: Cache en Memoria (DashMap)

**Propósito:** Estado caliente con latencia cero para operaciones de alta frecuencia.

```rust
use dashmap::DashMap;

pub struct GestaltState {
    // Agentes activos: agent_id → estado actual
    pub agent_states: Arc<DashMap<String, AgentState>>,

    // Locks de archivo: path → LockEntry { agent_id, acquired_at, ttl }
    pub file_locks: Arc<DashMap<String, LockEntry>>,

    // Contexto de runs activos: run_id → metadatos
    pub active_runs: Arc<DashMap<Uuid, RunContext>>,

    // Buffer de timeline para runs activos (máx ~1000 eventos en memoria)
    pub timeline_buffer: Arc<DashMap<Uuid, VecDeque<Event>>>,

    // Archivos virtuales: path → VirtualFile { content_hash, edit_count, locks }
    pub virtual_files: Arc<DashMap<String, VirtualFile>>,
}

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub agent_id: String,
    pub acquired_at: Instant,
    pub ttl: Duration,
}

impl LockEntry {
    pub fn is_expired(&self) -> bool {
        self.acquired_at.elapsed() > self.ttl
    }
}
```

**Operaciones atómicas sin base de datos:**
```rust
// Adquirir lock — atómico en DashMap, cero I/O
fn acquire_lock(state: &GestaltState, path: &str, agent_id: &str) -> bool {
    state.file_locks
        .entry(path.to_string())
        .or_insert_with(|| LockEntry::new(agent_id, Duration::from_secs(30)))
        .agent_id == agent_id  // mismo agente → reentrante
        || entry.is_expired()   // TTL expirado → reclamable
}

// Transición atómica de estado
fn transition_agent(
    state: &GestaltState,
    agent_id: &str,
    from: AgentState,
    to: AgentState,
) -> bool {
    state.agent_states
        .alter_if(agent_id, |_, current| current == from, |_, _| to)
        .is_some()
}
```

**Flush asíncrono a SQLite:**
```rust
// Background task: cada ~100ms sincroniza cambios a SQLite
tokio::spawn(async {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        sync_dirty_entries(&state, &sqlite_db).await;
    }
});
```

### 4.2 Tier 2: SQLite Local (Base de Datos Operacional)

**Propósito:** Persistencia local rápida con atomicidad SQL.

```sql
-- Esquema SQLite para Gestalt operacional
-- Archivo: ~/.gestalt/gestalt_state.db (WAL mode, synchronous=NORMAL)

-- Estado persistente de agentes
CREATE TABLE IF NOT EXISTS agent_states (
    run_id      TEXT NOT NULL,
    agent_id    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending|running|success|timeout|crashed|no_changes|quarantined
    output      TEXT,
    error       TEXT,
    branch      TEXT,
    duration_ms INTEGER DEFAULT 0,
    changed_files TEXT,  -- JSON array
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (run_id, agent_id)
);
CREATE INDEX idx_agent_states_status ON agent_states(status);

-- Locks de archivo con TTL nativo
CREATE TABLE IF NOT EXISTS file_locks (
    path        TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    run_id      TEXT,
    acquired_at INTEGER NOT NULL DEFAULT (unixepoch()),
    ttl_seconds INTEGER NOT NULL DEFAULT 30
);
CREATE INDEX idx_locks_expired ON file_locks(acquired_at + ttl_seconds);

-- Timeline: append-only, consultable en tiempo real
CREATE TABLE IF NOT EXISTS timeline (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    payload     TEXT NOT NULL,  -- JSON
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_timeline_run ON timeline(run_id, id);

-- Archivos virtuales (para virtual file versioning)
CREATE TABLE IF NOT EXISTS virtual_files (
    path        TEXT PRIMARY KEY,
    run_id      TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    content     BLOB,
    edit_count  INTEGER DEFAULT 0,
    locked_by   TEXT,
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Runs completados
CREATE TABLE IF NOT EXISTS runs (
    run_id      TEXT PRIMARY KEY,
    task        TEXT NOT NULL,
    base_ref    TEXT,
    max_parallel INTEGER,
    status      TEXT NOT NULL DEFAULT 'running',
    created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    finished_at INTEGER,
    summary     TEXT
);
```

**Ventajas sobre Xavier para estado operacional:**

| Operación | SQLite | Xavier (HTTP) |
|-----------|--------|---------------|
| Transición de estado | `UPDATE agent_states SET status=? WHERE run_id=? AND agent_id=? AND status=?` (0.1ms, atómico) | No soportado — necesitas full POST con re-escritura completa |
| Adquirir lock | `INSERT OR IGNORE INTO file_locks(path,agent_id) VALUES(?,?)` (0.2ms, UNIQUE garantiza exclusividad) | No hay concepto de lock en Xavier |
| Timeline: insertar | `INSERT INTO timeline(run_id,event_type,payload) VALUES(?,?,?)` (0.1ms) | No hay endpoint de timeline |
| Timeline: consultar últimos N | `SELECT * FROM timeline WHERE run_id=? ORDER BY id DESC LIMIT 50` (0.3ms) | No hay endpoint de timeline |
| Runs en estado "running" | `SELECT * FROM runs WHERE status='running'` (0.2ms) | Búsqueda semántica sobre contenido serializado |
| Locks expirados | `SELECT * FROM file_locks WHERE (acquired_at + ttl_seconds) < unixepoch()` (0.3ms) | No hay concepto de TTL ni expiry |
| Rollback de run | `BEGIN; DELETE FROM agent_states WHERE run_id=?; DELETE FROM timeline WHERE run_id=?; COMMIT;` (1-2ms, transaccional) | No hay transacciones |

### 4.3 Tier 3: Xavier (Uso Correcto)

**Xavier se usa exclusivamente para su propósito natural:**

```rust
impl GestaltPipeline {
    /// PRE-run: recuperar contexto semántico de Xavier
    async fn fetch_context(&self, task: &str) -> Vec<MemoryRecord> {
        // Solo se llama UNA VEZ por run, no por agente
        // Latencia 50-200ms es aceptable aquí
        match self.xavier.search(task, 5, "hybrid").await {
            Ok(results) => results,
            Err(_) => vec![], // non-fatal
        }
    }

    /// POST-run: archivar resultados en Xavier
    async fn archive_run(&self, report: &RunReport) {
        // Solo se llama UNA VEZ al finalizar el run
        // Latencia 50-200ms es aceptable aquí
        let metadata = serde_json::json!({
            "duration_ms": report.total_duration_ms,
            "agents": report.agents.len(),
            "success": report.success,
        });
        let _ = self.xavier.add(
            &report.summary,
            &format!("gestalt/run/{}", report.run_id),
            "run_result",
            metadata,
        ).await;
    }

    /// Cross-session: buscar runs anteriores por similitud semántica
    async fn find_similar_runs(&self, query: &str) -> Vec<MemoryResult> {
        // Búsqueda semántica híbrida — esto es lo que Xavier hace mejor
        self.xavier.search(query, 10, "hybrid").await
            .map(|r| r.results)
            .unwrap_or_default()
    }
}
```

---

## 5. Plan de Migración: De la Arquitectura Actual a la de 3 Capas

### Fase 0: SQLite Setup (día 1-2)
```bash
cargo add rusqlite --features bundled
cargo add dashmap
cargo add tokio --features "fs,io-util,time,sync,macros,rt"
```

Crear `gestalt_core/src/state/`:
- `mod.rs` — re-exportar `GestaltState`, `StateStore` trait
- `memory.rs` — implementación DashMap (`InMemoryState`)
- `sqlite.rs` — implementación SQLite (`SqliteStateStore`)
- `sync.rs` — background sync task (memoria → SQLite)

### Fase 1: Reemplazar RunManifest (JSON file + Mutex) por DashMap + SQLite
- Eliminar `write_manifest_atomically()` y `manifest_mutex`
- El estado de agentes vive en `GestaltState.agent_states` (DashMap)
- Sincronización periódica a SQLite

### Fase 2: Reemplazar JsonlEventLog por SQLite timeline
- `EventLog` trait se mantiene
- `JsonlEventLog` se reemplaza por `SqliteEventLog`
- Implementación: `INSERT INTO timeline(...)`
- Consulta: `SELECT * FROM timeline WHERE run_id=? ORDER BY id`

### Fase 3: File Lock Manager (reemplaza worktree isolation)
- `FileLockManager` usa DashMap + SQLite
- Lock TTL automático: `ttl_seconds=30` para evitar locks huérfanos
- Lock heartbeat: agentes renuevan locks periódicamente

### Fase 4: Virtual File System (posterior, bloque de código)
- `VirtualFileStore` en SQLite + DashMap
- Edición por bloque: `UPDATE virtual_files SET content=substr_replace(...) WHERE path=?`

### Fase 5: Eliminar Semaphore (escalado horizontal)
- La capa de memoria permite 100+ tareas sin semáforo
- Los locks de archivo reemplazan al semáforo como mecanismo de control de concurrencia
- El límite real son los recursos del sistema (CPU, RAM, disco)

---

## 6. Métricas Esperadas

| Métrica | Antes (Xavier para todo) | Después (3 capas) | Mejora |
|---------|-------------------------|-------------------|--------|
| Transición de estado | 50-200ms (HTTP POST) | ~0.1ms (DashMap) + ~1ms (SQLite flush) | **~2000x** |
| Adquirir lock de archivo | No implementable | ~0.1ms (DashMap) + ~0.5ms (SQLite INSERT OR IGNORE) | **Nuevo** |
| Insertar timeline event | 10-30ms (fsync JSONL) | ~0.1ms (DashMap buffer) + ~0.5ms (SQLite batch INSERT) | **~100x** |
| Consultar timeline (50 eventos) | 10-50ms (parsear JSONL) | ~0.3ms (SQLite SELECT) | **~100x** |
| Búsqueda semántica (pre-run) | 50-200ms (aceptable) | 50-200ms (Xavier, igual) | Sin cambio |
| Almacenar resultado (post-run) | 50-200ms (aceptable) | 50-200ms (Xavier, igual) | Sin cambio |
| Runs simultáneos | N (limitado por semáforo) | 100+ (sin semáforo, solo locks) | **~10x** |
| Time to detect overlap | Fin del run (post-hoc) | En tiempo real (lock conflict) | **Inmediato** |

---

## 7. Mitigación de Riesgos

| Riesgo | Probabilidad | Impacto | Mitigación |
|--------|-------------|---------|------------|
| SQLite contention con 100+ escritores concurrentes | Media | Alto | WAL mode + batch writes (cada 100ms) + DashMap absorbe writes de alta frecuencia |
| Locks huérfanos por crash de agente | Alta | Medio | TTL automático (30s) + cleanup task periódico + lock heartbeat |
| Inconsistencia DashMap ↔ SQLite | Media | Medio | Write-through síncrono para operaciones críticas (locks, transiciones de estado); batch eventual para timeline |
| Xavier caído en pre/post-run | Baja | Bajo (non-fatal) | Ya manejado como `Option<XavierClient>` — warning y sigue |
| Migración de datos existentes | Baja | Medio | JsonlEventLog → SQLite migration script; Re-play de eventos para reconstruir estado |
| DashMap memory explosion con 100+ runs | Baja | Medio | Límite de entradas por run; purge de runs completados después de N minutos |

---

## 8. Conclusión y Recomendación

**Xavier NO debe usarse como backend de estado en tiempo real para Gestalt.** La razón no es sobreingeniería en el sentido de "demasiada arquitectura", sino **uso incorrecto de las herramientas**: Xavier es un motor de memoria semántica (embeddings + búsqueda + persistencia searchable), no una base de datos operacional (transacciones atómicas + lecturas rápidas + constraints + TTL).

### Lo que Gestalt necesita:

1. **DashMap en memoria** → estado caliente con latencia cero (~0ns) para ~1000 operaciones/segundo
2. **SQLite local (WAL mode)** → persistencia operacional con atomicidad SQL (~0.1-1ms por operación)
3. **Xavier (externo)** → contexto semántico pre-run y archivo post-run (~50-200ms, aceptable)

### Lo que Xavier SÍ debe seguir siendo para Gestalt:

| Función | Endpoint | Latencia |
|---------|----------|----------|
| Recuperar contexto relevante antes de un run | `POST /v1/memories/search` | 50-200ms ✅ |
| Archivar resultados de run completado | `POST /v1/memories` | 50-200ms ✅ |
| Buscar runs similares históricamente | `POST /v1/memories/search` | 50-200ms ✅ |
| Almacenar decisiones arquitectónicas (kind=decision) | `POST /v1/memories` | 50-200ms ✅ |

### Lo que Xavier NUNCA debe ser para Gestalt:

| Función | Alternativa Correcta |
|---------|---------------------|
| Almacenar estado de agente activo | DashMap + SQLite `agent_states` |
| File locks con TTL | DashMap `file_locks` + SQLite `file_locks` con `ttl_seconds` |
| Timeline en tiempo real | DashMap `timeline_buffer` + SQLite `timeline` |
| Virtual file versioning | SQLite `virtual_files` (path indexado, blob storage) |
| Transition atómica de estado | DashMap `alter_if` + SQLite `UPDATE...WHERE status=?` |

**Arquitectura ganadora: 3 capas complementarias, no una competencia entre Xavier y una base de datos propia.**
