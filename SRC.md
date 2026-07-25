# SRC.md — Gestalt Rust

> AI Agent Orchestration Platform — CLI-first, Swarm-powered

## Proyecto

- **Nombre:** gestalt-rust
- **Tipo:** Rust workspace (5 crates)
- **Descripción:** Plataforma de orquestación de agentes AI via CLI. VFS + Multi-agent Routing + Tools.
- **Tech Stack:** Rust, SurrealDB, tokio

## Estructura

```
gestalt-rust/
├── Cargo.toml              # Workspace (5 crates)
├── gestalt_core/           # Core: VFS, auth, LLM, tools, MCP client
│   └── src/                # Core domain, ports, adapters, application
├── gestalt_cli/            # CLI binary & REPL
│   └── src/                # CLI entry point, config, repl
├── gestalt-router/         # Orchestration Router
│   └── src/
│       ├── lib.rs          # Module declarations
│       ├── run.rs          # Run spec (RunSpec, AgentSpec)
│       └── run_state.rs    # Run state (RunManifest, AgentState)
├── gestalt-merge/          # Code integration & merge algorithms
│   └── src/
│       └── lib.rs          # Merge logic
├── synapse-agentic/        # Tool registry + agentic primitives
│   └── src/
│       └── lib.rs          # Tool registry and primitives
├── skills/                 # OpenClaw skill docs
├── docs/                   # Architecture & guides
└── .gitcore/               # Git-Core planning docs
```

## Crates

| Crate | Type | Props |
|-------|------|-------|
| gestalt_core | lib | 45 .rs files |
| gestalt_cli | bin | 3 .rs files |
| gestalt-router | lib | 3 .rs files |
| gestalt-merge | lib | 1 .rs file |
| synapse-agentic | lib | 1 .rs file |

## Estado

✅ Proyecto activo — iberi22/gestalt-rust — SouthWest AI Labs

*Última actualización: 2026-04-16*
