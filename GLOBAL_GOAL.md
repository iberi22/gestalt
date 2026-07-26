# 🎯 Gestalt — Global Goal

> **Última actualización:** 2026-07-26
> **Estado:** Fase 1 ✅ + Fase 2 ✅ completadas. Pendiente Fase 3 + GitHub sync.

---

## What is Gestalt?

**Gestalt es un orquestador multi-agente local** que gestiona la ejecución concurrente de agentes AI (agy, cursor-agent, kimi, opencode) con aislamiento de archivos, timeline en tiempo real, y un backend de estado de 3 capas.

---

## 🏗️ Arquitectura (3 Capas + Xavier)

```
┌──────────────────────────────────────────────────────────┐
│  Tier 1: MemState (DashMap, ~0ns)                        │
│  • agent_states: get/set + broadcast                      │
│  • active_locks: try_lock/release_lock/renew_lock         │
│  • event_tx (broadcast::channel): timeline events → WS    │
├──────────────────────────────────────────────────────────┤
│  Tier 2: StateDB (SQLite WAL, ~0.1-1ms)                 │
│  • runs(run_id, spec, status, created_at)                │
│  • agents(run_id, agent_id, state, output, error, ...)   │
│  • locks(path, agent_id, run_id, acquired_at, ttl_secs)  │
│  • timeline(seq, run_id, agent_id, event_type, payload)  │
│  • file_versions(path, version_hash, content, ...)       │
├──────────────────────────────────────────────────────────┤
│  Tier 3: VirtualFS + AgentWrapper                         │
│  • VirtualFS trait: read_file, write_block, versions     │
│  • StateDbVfs: SQLite-backed versioned files             │
│  • WorktreeManager: implements VirtualFS (transitional)  │
│  • AgentWrapper: CLI diff → BlockEdit → VirtualFS        │
├──────────────────────────────────────────────────────────┤
│  Tier 4: Xavier (memoria permanente, ~50-200ms)          │
│  • PRE-run: search_context → XAVIER_CONTEXT en env       │
│  • POST-run: archive_run → kind=run_result               │
│  • Indexado: ARCHITECTURE.md, state-backend.md, AGENTS.md│
└──────────────────────────────────────────────────────────┘
```

---

## ✅ Completado

### Fase 1 — StateDB + MemState Backend ✅

| Componente | Archivos | Tests |
|------------|----------|-------|
| `gestalt-state` crate | `statedb.rs`, `memstate.rs`, `schema.rs` | 14/14 pass |
| Router refactor | `router.rs` — no Semaphore | compila 0 errores |
| StateDbEventLog | `timeline.rs` — reemplaza JsonlEventLog | 1 crate nuevo |
| Docs + Cleanup | `ARCHITECTURE.md`, `.gitignore`, `xavier2/` eliminado | fmt OK |

### Fase 2 — Real-time Streaming + VFS ✅

| Componente | Archivos | Tests |
|------------|----------|-------|
| `gestalt-ws` crate | `server.rs`, `event.rs` — WebSocket :3001 | 6/6 WS tests |
| Timeline broadcast | `ws.rs` (WsRouterBridge) | 6/6 WS tests |
| Lock expiry + renew | `memstate.rs` | 14/14 pass |
| VirtualFS trait | `gestalt_core/src/ports/outbound/vfs.rs` | 6/6 VFS tests |
| StateDbVfs | `gestalt-state/src/virtual_fs.rs` | 6/6 VFS tests |
| Worktree → VirtualFS | `worktree.rs` implements VirtualFS | compila 0 errores |
| AgentWrapper | `gestalt_cli/src/agent_wrapper.rs` | 14/14 pass |
| Xavier sync | `client.rs` — search_context + archive_run | compila 0 errores |

### Sistema de Hooks ✅

| Componente | Archivo | Propósito |
|------------|---------|-----------|
| Pre-commit hook | `.git/hooks/pre-commit`, `hooks/pre-commit.sh` | 5-stage validation |
| Commit-msg hook | `.git/hooks/commit-msg` | Conventional commits |
| Clippy config | `clippy.toml` | 10 args, 30 cognitive complexity |
| Rustfmt config | `rustfmt.toml` | 100 cols, Unix newlines |
| Installer | `hooks/install.sh` | Install hooks in fresh clone |

---

## 📋 Pendiente (Fase 3)

### ⚠️ Issues Conocidos

| # | Issue | Severidad | Estado |
|---|-------|-----------|--------|
| G4 | Real-time conflict detection | 🔴 Media | Timeout (600s, 39 calls) |
| T1 | Tests rotos: agent_tests, doctor_tests, router_tests | 🔴 Alta | Imports rotos (_RunManifest_, JsonlEventLog, tempfile) |
| T2 | `gestalt-merge` + `synapse-agentic` clippy errors | 🟡 Baja | Pre-existing, crates no modificados |
| G3 | `test_try_lock_exclusive` ignorado | 🟡 Baja | Runtime tokio en sync test |

### 🔮 Próximos Pasos Recomendados

1. **Fix test files** — Actualizar agent_tests.rs, doctor_tests.rs, integration_test.rs, router_tests.rs a la nueva API (StateDbEventLog, VirtualFS, no RunManifest)
2. **G4: Conflict detection** — Implementar detección de conflictos en tiempo real usando MemState locks + timeline events
3. **GitHub push** — Force push main → origin (target/ limpiado del historial)
4. **Enable all tests** — Fix `test_try_lock_exclusive` (tokio runtime en test), quitar `#[ignore]`
5. **Extend VirtualFS** — StateDbVfs como impl por defecto, WorktreeManager como legacy
6. **WebSocket auth** — Opcional: token-based auth para WS :3001

---

## 🛑 Design Non-Negotiables

1. **No Semaphore** — Concurrencia via locks por archivo en MemState
2. **No JSON state files** — Todo el estado persistente en SQLite WAL via StateDb
3. **StateDbEventLog** — Reemplazo total de JsonlEventLog
4. **VirtualFS** — Siempre usar VirtualFS, nunca escribir directo al FS
5. **Xavier = solo memoria permanente** — No usar Xavier para estado operativo
6. **Fail gracefully** — Nunca panic por rate-limiting o timeout

---

## 🔗 Enlaces

| Documento | Path | Xavier ID |
|-----------|------|-----------|
| Architecture | `ARCHITECTURE.md` | `01KYG35YJW7SH8N4X6XGF6NFJV` |
| State Backend | `docs/state-backend.md` | `01KYG365Q5FMATB35PRN7826N9` |
| AGENTS.md | `AGENTS.md` | `01KYG3661X1QJPS76AKB0V5MJ0` |
| Clippy Config | `clippy.toml` | `01KYG366CR9RRJ8GR5YBWWWY0H` |
| Plan Fase 1+2 | `.hermes/plans/gestalt-state-backend-refactor.md` | — |
| Plan Análisis | `.hermes/plans/gestalt-vs-xavier-state-backend-analysis.md` | — |
