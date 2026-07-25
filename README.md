# Gestalt

> ⚡ Universal AI Agent Orchestration Platform — CLI-first, Swarm-powered.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org)
[![Build Status](https://github.com/iberi22/gestalt-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/iberi22/gestalt-rust/actions/workflows/ci.yml)

**Gestalt** is a high-performance Rust workspace for orchestrating AI agents. It provides VFS isolation, parallel agent routing, state management, and tool registries — all controlled through a REPL or direct CLI.

## 🚀 Quick Start

### Build and Install CLI
```bash
# Build the CLI
cargo build --release -p gestalt_cli

# Run the CLI REPL
cargo run -p gestalt_cli -- repl
```

### Running Multi-Agent Orchestration Router
Run agents in parallel or route them specifically:
```bash
# Run a specific task across multiple agents
gestalt run --agents agy,claude "implement custom logger in gestalt-router"

# Run with custom agent selections for a specialized task
gestalt run --agents agy,claude "audit security-events workflow in CI"
```

## 🧩 Crates

| Crate | Type | Description |
|-------|------|-------------|
| `gestalt_core` | lib | VFS, auth, LLM adapters, agent tools, MCP client |
| `gestalt_cli` | bin | REPL + CLI commands |
| `gestalt-router` | lib | Multi-agent orchestration routing & state |
| `gestalt-merge` | lib | Code integration & branch merging algorithms |
| `synapse-agentic` | lib | Tool registry + agentic primitives (Hive, LLM providers) |

## 📂 Project Structure

```
gestalt-rust/
├── Cargo.toml                  # Workspace root
├── gestalt_core/               # Core domain: VFS, auth, LLM, tools
│   └── src/
│       ├── adapters/           # MCP client, auth (Google OAuth/PKCE)
│       ├── application/        # Agent tools, config, indexer
│       ├── domain/             # Rag embeddings, models
│       ├── mcp/                # MCP client + registry
│       └── ports/              # Inbound/outbound port traits
│           └── outbound/vfs.rs # VFS trait + OverlayFs
├── gestalt_cli/                # Standalone CLI & REPL binary
├── gestalt-router/             # Multi-agent orchestrator router
│   └── src/
│       ├── lib.rs              # Router entrypoint
│       ├── run.rs              # Run spec and execution types
│       └── run_state.rs        # Orchestrator & agent state transitions
├── gestalt-merge/              # Code integration & merge handlers
├── synapse-agentic/            # Tool registry + Hive actor model
├── skills/                     # OpenClaw skill docs
├── docs/                       # Architecture & guides
└── .gitcore/                   # Git-Core planning docs
```

## 🔑 Key Features

- **VFS Overlay** — Isolated file system per agent with merge semantics
- **Multi-Agent Routing** — Route specific tasks to specialized agents (e.g., `agy`, `claude`)
- **Parallel Execution** — Parallel agent orchestration via `gestalt-router`
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
