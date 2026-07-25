# SRC.md — Gestalt Rust

> AI Agent Orchestration Platform — CLI-first, Swarm-powered

## Proyecto

- **Nombre:** gestalt-rust
- **Tipo:** Rust workspace (5 crates)
- **Descripción:** Plataforma de orquestación de agentes AI via CLI. VFS + Swarm + Timeline + Tools.
- **Tech Stack:** Rust, SurrealDB, tokio

## Estructura

```
gestalt-rust/
├── Cargo.toml              # Workspace (5 crates)
├── gestalt_core/           # Core: VFS, auth, LLM, tools, MCP client
├── gestalt_timeline/        # Orchestrator bin (gestalt)
├── gestalt_cli/            # REPL bin
├── gestalt_swarm/          # Swarm coordinator bin
├── synapse-agentic/        # Tool registry + Hive actor model
├── skills/                 # OpenClaw skill docs
├── docs/                   # Architecture & guides
└── .gitcore/              # Git-Core planning
```

## Crates

| Crate | Type | Props |
|-------|------|-------|
| gestalt_core | lib | 42 .rs files |
| gestalt_timeline | bin | 37 .rs files |
| gestalt_cli | bin | 4 .rs files |
| gestalt_swarm | bin | 1 .rs file |
| synapse-agentic | lib | tool registry |

## Estado

✅ Proyecto activo — iberi22/gestalt-rust — SouthWest AI Labs

*Última actualización: 2026-04-16*
