# .gitcore/ARCHITECTURE.md — Gestalt Architecture

## Overview

Gestalt is a Rust workspace for AI agent orchestration. 5 crates.

```
gestalt_timeline (bin)
    └── gestalt_core (lib)
            ├── synapse-agentic (lib)
            └── gestalt_swarm (bin)
```

## Crates

| Crate | Files | Role |
|-------|-------|------|
| gestalt_core | 42 | Hexagonal core: VFS, auth, LLM, tools |
| gestalt_timeline | 37 | Orchestrator + timeline service |
| gestalt_cli | 4 | REPL binary |
| gestalt_swarm | 1 | Swarm coordinator |
| synapse-agentic | 1 | Tool registry + Hive |

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

## Swarm

```
SwarmCoordinator
├── TaskQueue
├── AgentRegistry
└── HealthMonitor
```

## Removed (2026-04-16)

- gestalt_app, gestalt_terminal, gestalt_ui, gestalt_mcp, gestaltctl, gestalt_infra_*, benchmarks/
