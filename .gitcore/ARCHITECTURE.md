# .gitcore/ARCHITECTURE.md — Gestalt Architecture

## Overview

Gestalt is a Rust workspace for AI agent orchestration. Active workspace members (7 crates); `gestalt_swarm` is **excluded**.

```
gestalt_cli (bin)
    ├── gestalt_core (lib)
    │       └── synapse-agentic (lib)
    ├── gestalt-router
    │       ├── gestalt-merge
    │       └── gestalt-state
    └── gestalt-ws
```

## Crates

| Crate | Role | Workspace |
|-------|------|-----------|
| gestalt_core | Hexagonal core: VFS, auth, LLM, tools; shared DTOs in `models` | member |
| gestalt_cli | REPL + swarm CLI binary | member |
| gestalt-router | Orchestration, worktrees, run states | member |
| gestalt-merge | Isolated branch merge | member |
| gestalt-state | SQLite StateDB + MemState | member |
| gestalt-ws | WebSocket timeline broadcast | member |
| synapse-agentic | Tool registry + Hive | member |
| gestalt_swarm | Legacy swarm coordinator | **EXCLUDED** |

## Key Traits

- `VfsPort` — VFS abstraction
- `Agent` — agent trait
- `Tool` — tool trait
- `LLMProvider` — LLM abstraction

## VFS

```
VfsPort
├── MemoryFs
└── OverlayFs (upper + lower layers)
```

## Swarm (current)

Prefer `gestalt_cli` swarm commands or `scripts/swarm_bridge.py`.
Do not `cargo -p gestalt_swarm` (not a workspace member).

## Removed / excluded

- Removed (2026-04-16): gestalt_app, gestalt_terminal, gestalt_ui, gestalt_mcp, gestaltctl, gestalt_infra_*, benchmarks/
- Excluded from workspace (2026+): gestalt_swarm (legacy; keep off `Cargo.toml` members)
