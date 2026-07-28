# Gestalt — Architecture Overview

> **Versión:** 2.1.0 (Ola 5) · Jul 2026
> **Ver también:** [REDESIGN.md](../REDESIGN.md)

## System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 USER (CLI)                              │
│   gestalt run --agents agy,claude "task"                 │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌──────────────────────────────────────────────────┐    │
│  │              gestalt-router (orchestrator)        │    │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │    │
│  │  │ Worktree │  │  Subproc  │  │  Overlap     │  │    │
│  │  │ Manager  │  │  Runner   │  │  Detection   │  │    │
│  │  └──────────┘  └───────────┘  └──────────────┘  │    │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │    │
│  │  │ Integrate│  │  Path     │  │  Timeline    │  │    │
│  │  │ (merge)  │  │  Claims   │  │  (JSONL)     │  │    │
│  │  └──────────┘  └───────────┘  └──────────────┘  │    │
│  └──────────────────────────────────────────────────┘    │
│                           │                               │
│  ┌──────────────────────────────────────────────────┐    │
│  │              gestalt_core (reusable)              │    │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │    │
│  │  │ VFS trait │  │ToolRegistry│  │ LLM Adapters│  │    │
│  │  └──────────┘  └───────────┘  └──────────────┘  │    │
│  │  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │    │
│  │  │ MCP Client│  │   Auth    │  │ Agent Logic  │  │    │
│  │  └──────────┘  └───────────┘  └──────────────┘  │    │
│  └──────────────────────────────────────────────────┘    │
│                                                           │
│  ┌──────────────────────────────────────────────────┐    │
│  │         synapse-agentic (actor model)             │    │
│  │         Usado en Fase 3+ para coordinación        │    │
│  └──────────────────────────────────────────────────┘    │
│                                                           │
│  ┌──────────────────────────────────────────────────┐    │
│  │              gestalt-merge (Fase 2)               │    │
│  │  ┌──────────┐  ┌───────────┐                     │    │
│  │  │  git     │  │ tree-     │                     │    │
│  │  │ merge-tree│  │ sitter    │                     │    │
│  │  └──────────┘  └───────────┘                     │    │
│  └──────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## Execution Model

### Phase 1 — Git Worktree + Subprocess

1. Router crea `N` worktrees desde `base_ref`
2. Spawnea agentes CLI con `CWD` en cada worktree
3. Al completar: `git commit` en cada worktree
4. Detecta path overlap entre diffs
5. Merge secuencial en rama de integración
6. Reporta conflictos no resueltos

### Phase 2 — Semantic Merge

- `gestalt-merge` con tree-sitter para merge a nivel AST
- Auto-resuelve conflictos de símbolos no solapados

## Key Design Decisions

| Decisión | Elección | Justificación |
|----------|----------|---------------|
| Aislamiento | Git Worktree (no FUSE) | POSIX nativo, 0 overhead, discard=clean |
| Timeline | JSONL (no SurrealDB) | Append-only, consultable con jq |
| Merge v1 | git merge-tree | Sin dependencias nuevas |
| Merge v2 | tree-sitter AST | Resolución semántica de conflictos |
| Locks v1 | Post-hoc overlap | OCC después de ejecución |
| Locks v3 | PathClaims in-process | Antes de ejecución, en proceso |

## Crate Map

| Crate | Type | Purpose |
|-------|------|---------|
| `gestalt_core` | Library | VFS trait, LLM adapters, tool registry |
| `gestalt_router` | Library + Binary | Orchestration core |
| `gestalt_merge` | Library | 3-way merge engine (Phase 2) |
| `gestalt_cli` | Binary | CLI interface |
| `gestalt_swarm` | Binary | Legacy — pending migration |
| `synapse-agentic` | Library | Actor model (Phase 3+) |

## Previous Architecture (archived)

El diseño anterior incluía SurrealDB Timeline, FUSE Daemon y Lock Server. Fue reemplazado tras validación con AGY + Kimi en Julio 2026. Ver `docs/REDESIGN.md` para el análisis completo.
