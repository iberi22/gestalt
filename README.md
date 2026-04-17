# Gestalt

> ⚡ Universal AI Agent Orchestration Platform — CLI-first, Swarm-powered.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org)

**Gestalt** is a high-performance Rust workspace for orchestrating AI agents via CLI. It provides VFS isolation, Swarm parallel execution, timeline-based state, and tool registries — all controlled through a REPL or direct CLI.

## 🚀 Quick Start

```bash
# Build
cargo build --release -p gestalt_timeline

# Run the orchestrator (gestalt binary)
cargo run -p gestalt_timeline --bin gestalt

# Or use the CLI REPL
cargo run -p gestalt_cli
```

## 🐝 Gestalt Swarm

Parallel agent execution bridge. Spawns multiple CLI agents concurrently for high-speed automation.

```bash
# Run with N agents
cargo run --release -p gestalt_swarm -- --agents 4 --goal "analyze codebase security"
```

See [skills/gestalt-swarm.md](skills/gestalt-swarm.md) for full usage.

## 🧩 Crates

| Crate | Type | Description |
|-------|------|-------------|
| `gestalt_core` | lib | VFS, auth, LLM adapters, agent tools, MCP client |
| `gestalt_timeline` | bin | Main orchestrator (`gestalt` binary) + timeline service |
| `gestalt_cli` | bin | REPL + CLI commands |
| `gestalt_swarm` | bin | Swarm coordinator for parallel agent execution |
| `synapse-agentic` | lib | Tool registry + agentic primitives (Hive, LLM providers) |

## 📂 Project Structure

```
gestalt-rust/
├── Cargo.toml                  # Workspace root (5 crates)
├── gestalt_core/               # Core domain: VFS, auth, LLM, tools
│   └── src/
│       ├── adapters/          # MCP client, auth (Google OAuth/PKCE)
│       ├── application/        # Agent tools, config, indexer
│       ├── domain/            # Rag embeddings, models
│       ├── mcp/               # MCP client + registry
│       └── ports/             # Inbound/outbound port traits
│           └── outbound/vfs.rs # VFS trait + OverlayFs
├── gestalt_timeline/           # Orchestrator binary
│   └── src/main.rs            # gestalt CLI entry point
├── gestalt_cli/                # Standalone REPL binary
├── gestalt_swarm/              # Swarm coordinator binary
├── synapse-agentic/            # Tool registry + Hive actor model
├── skills/                     # OpenClaw skill docs
├── docs/                       # Architecture & guides
└── .gitcore/                  # Git-Core planning docs
```

## 🔑 Key Features

- **VFS Overlay** — Isolated file system per agent with merge semantics
- **Swarm Orchestration** — Parallel multi-agent execution
- **Timeline State** — Events persisted in SurrealDB
- **MCP Client** — Connect to external MCP servers (not a standalone server)
- **LLM Resilience** — OpenAI + Anthropic adapters with automatic failover
- **Tool Registry** — 12+ built-in tools (git, shell, file, search, ask_ai, etc.)
- **Auth** — Google OAuth2 + PKCE built-in

## 🔗 Resources

- **Repository:** https://github.com/iberi22/gestalt-rust
- **Issues:** https://github.com/iberi22/gestalt-rust/issues
- **License:** MIT

---

*Gestalt — AI agents that actually execute.*
