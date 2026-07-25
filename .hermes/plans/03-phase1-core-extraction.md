# Phase 1: Core Extraction (eliminar SurrealDB)

## Task 1.1: Eliminar SurrealDB de gestalt_timeline

**Archivos a modificar:**
- `gestalt_timeline/Cargo.toml` — quitar `surrealdb` dependency
- `gestalt_timeline/src/db/mod.rs` — eliminar o simplificar
- `gestalt_timeline/src/db/surreal.rs` — eliminar

**Qué reemplaza:** Nada. Timeline events se vuelven opcionales
(log a stdout o archivo JSON). El estado se pasa por contexto
desde las tasks/skills o Xavier CLI.

```toml
# Cargo.toml — antes
surrealdb = { version = "2.6.1", features = ["kv-mem"] }

# Cargo.toml — después
# (eliminado)
```

## Task 1.2: Eliminar SurrealDB de gestalt_core

- `gestalt_core/src/db/mod.rs` — eliminar
- `gestalt_core/src/db/surreal.rs` — eliminar
- `gestalt_core/src/adapters/persistence/surreal_db.rs` — eliminar
- `gestalt_core/Cargo.toml` — quitar `surrealdb` feature

## Task 1.3: Simplificar servicios que usaban DB

**Servicios a refactorizar** (12 archivos):
- `services/timeline.rs` → log a archivo JSON o stdout
- `services/project.rs` → eliminar (no needed)
- `services/task.rs` → eliminar (tasks vienen de issues)
- `services/task_queue.rs` → eliminar (no needed)
- `services/agent.rs` → eliminar (agentes viven en sus worktrees)
- `services/memory.rs` → reemplazar por Xavier CLI bridge
- `services/feedback_loop.rs` → eliminar
- `services/protocol_sync.rs` → eliminar
- `services/watch.rs` → simplificar
- `services/index.rs` → eliminar
- `services/telegram.rs` → eliminar
- `services/context_compaction.rs` → mantener pero simplificar

## Task 1.4: Mantener FileManager + VFS intactos

**NO TOCAR:**
- `services/file_manager.rs` ✅ — core de change control
- `gestalt_core/src/ports/outbound/vfs.rs` ✅ — overlay filesystem
- `gestalt_core/src/ports/outbound/vfs.rs` (FileWatcher) ✅

## Task 1.5: Simplificar modelos

- Eliminar `models/execution_metrics.rs`
- Eliminar `models/project.rs`
- Eliminar `models/task.rs`
- Simplificar `models/runtime_state.rs`
- Simplificar `models/timeline_event.rs`
- Simplificar `models/timestamp.rs`

## Task 1.6: Agregar dependency threeway_merge

```toml
# gestalt_core/Cargo.toml
[dependencies]
threeway_merge = "0.1.19"
```

## Task 1.7: Simplificar gestalt_timeline/src/main.rs

Quitar toda la inicialización de SurrealDB, ProjectService,
TaskService, TimelineService. Dejar solo lo mínimo para
orquestar waves.

## Verification Phase 1

```bash
cargo check -p gestalt_core   # 0 errors
cargo check -p gestalt_timeline  # 0 errors
cargo check --workspace  # 0 errors
cargo test --workspace  # tests de FileManager siguen pasando
```
