# Gestalt — Project State

> **Versión:** 2.0.0 (Router) · **Última actualización:** 2026-07-25
> **Estado:** Diseño completo, MVP en planificación

## Workspace (4 crates activos, 1 planeado)

| Crate | Type | Lines | Estado |
|-------|------|-------|--------|
| `gestalt_core` | Lib | ~1,500 | ✅ Compila |
| `synapse-agentic` | Lib | ~700 | ✅ Compila |
| `gestalt_router` | Lib+Bin | 🚧 En diseño | 📋 Planificado |
| `gestalt_merge` | Lib | 📋 Fase 2 | 📋 Planificado |
| `gestalt_cli` | Binary | ~300 | 🟡 Por reparar |
| `gestalt_swarm` | Binary | ~900 | 🟡 Pendiente migración |
| `gestalt_timeline` | Binary | — | ❌ Eliminado |

## Build Status

| Check | Result |
|-------|--------|
| `cargo check -p gestalt_core` | ✅ Passes |
| `cargo check -p synapse-agentic` | ✅ Passes |
| `cargo check --workspace` | ❌ Falla (timeline eliminado, cli/swarm sin reparar) |

## Logros

- ✅ Diseño completo del router multi-agente validado por AGY + Kimi
- ✅ Decisiones arquitectónicas documentadas en REDESIGN.md
- ✅ SurrealDB eliminado del plan → JSONL event log
- ✅ FUSE eliminado → git worktree como mecanismo de aislamiento
- ✅ gestalt_timeline eliminado como crate

## Pendientes

| Prioridad | Tarea | Fase |
|-----------|-------|------|
| 🔴 ALTA | Crear `gestalt-router` crate con WorktreeManager | Fase 1 |
| 🔴 ALTA | Implementar SubprocessRunner (spawn CLI agents) | Fase 1 |
| 🔴 ALTA | Implementar overlap detection + integrate | Fase 1 |
| 🔴 ALTA | Reparar `gestalt_cli` para usar router | Fase 1 |
| 🟡 MEDIA | Crear `gestalt-merge` con tree-sitter | Fase 2 |
| 🟡 MEDIA | PR creation via gh CLI | Fase 2 |
| 🟡 MEDIA | PathClaims in-process para coordinación preventiva | Fase 3 |
| 🔵 BAJA | Migrar `gestalt_swarm` al nuevo modelo | Fase 3 |
| 🔵 BAJA | FUSE daemon condicional (solo si profiling lo justifica) | Fase 4 |

## Decisiones Arquitectónicas Recientes

| Decisión | Fecha | Detalle |
|----------|-------|---------|
| Reemplazar SurrealDB por JSONL | 2026-07-25 | Timeline era append-only, no necesitaba DB document |
| Reemplazar FUSE por worktree CWD | 2026-07-25 | Worktree da mismo aislamiento a costo 0 |
| Eliminar gestalt_timeline | 2026-07-25 | 31 errores de compilación, reemplazado por módulo |
| Adoptar git merge-tree para v1 | 2026-07-25 | Sin dependencias nuevas, funciona hoy |
| Postergar tree-sitter a Fase 2 | 2026-07-25 | No bloquea el MVP |
