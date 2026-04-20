# Gestalt-Rust Architecture

> Last Updated: 2026-04-16
> Status: v1.0 — CLI/Swarm/Core only (UI/MCP/infra removed)

## Overview

Gestalt is a high-performance Rust workspace for AI agent orchestration. It provides VFS isolation, Swarm parallel execution, timeline-based state in SurrealDB, and a tool registry — all accessible via CLI/REPL.

**Scope**: CLI-first orchestration. No UI, no standalone MCP server, no Flutter app.

---

## Crates

### gestalt_core (42 .rs files)
The hexagonal core. Contains domain models, port traits, and business logic.

```
src/
├── adapters/
│   ├── auth/          # Google OAuth2 + PKCE
│   └── mcp/           # MCP client (connects to external servers)
├── application/
│   ├── agent/         # Tool implementations (git, shell, file, search...)
│   ├── config.rs      # GestaltConfig from env
│   ├── indexer.rs     # Code indexer with SurrealDB
│   └── mcp_service.rs # MCP server config loader
├── context/           # Scanner, workspace analysis
├── domain/
│   └── rag/          # Embeddings (DummyEmbeddingModel)
├── mcp/
│   ├── client_impl.rs # MCP client implementation
│   └── registry.rs   # MCP tool registry
├── ports/
│   ├── inbound/       # Port traits (incoming)
│   └── outbound/      # Port traits (outgoing)
│       ├── repo_manager.rs  # Repo + VectorDB traits
│       └── vfs.rs     # VfsPort + OverlayFs + MemoryFs
└── swarm/
    ├── coordinator.rs # SwarmCoordinator
    ├── agent.rs       # Agent trait
    ├── health.rs      # HealthMonitor
    └── registry.rs    # AgentRegistry
```

### gestalt_timeline (37 .rs files)
Primary orchestration engine. The `gestalt` binary.

- `TimelineService` — event sourcing with SurrealDB
- `ProjectService` — project CRUD
- `TaskService` — task management
- Initializes: SurrealDB, VFS, LLM providers, tool registry, SwarmCoordinator

### gestalt_cli (4 .rs files)
Standalone CLI binary with REPL.

- `main.rs` — CLI entry point with subcommands
- `repl.rs` — InteractiveRepl with command handling

### gestalt_swarm (1 .rs file)
Swarm coordinator for parallel agent execution.

- `SwarmCoordinator` — dispatch tasks to N agents in parallel
- `AgentRegistry` — register/list agents
- `TaskQueue` — priority queue for work distribution
- `HealthMonitor` — monitor agent heartbeats

### synapse-agentic (1 + generated)
Tool registry and agentic primitives.

- `Tool` + `ToolContext` traits
- `ToolRegistry` — register/dispatch tools
- `Hive` — actor system for sub-agents
- `LLMProvider` trait — OpenAI + Anthropic adapters
- `StochasticRotator` — LLM provider failover

---

## VFS Architecture

```
VfsPort (trait)
├── MemoryFs        — in-memory file system
└── OverlayFs       — layered merge (upper + lower)
    ├── read        — upper first, then lower
    ├── write       — upper only
    └── merge       — explicit overlay merge
```

- FileWatchService — debounced file system watcher
- Used for agent workspace isolation

---

## Swarm Architecture

```
SwarmCoordinator
├── TaskQueue (priority queue)
├── AgentRegistry (registered agents)
└── HealthMonitor (heartbeat tracking)

Agent (trait)
└── execute(task) -> Result<Value>

Agent implementations:
├── CliAgent (execute shell commands)
└── LlmAgent (LLM-powered agent)
```

---

## MCP Client (not standalone server)

`gestalt_core/adapters/mcp/` contains an MCP **client** that:
- Connects to external MCP servers via HTTP
- Loads server configs from `config/mcp.json`
- Exposes tools from remote servers as `Tool` implementations

The **standalone MCP server** (`gestalt_mcp` crate) was removed.

---

## State Management

- **SurrealDB** — timeline events, projects, tasks, agent state
- **VFS** — file system snapshots per agent session
- **In-memory** — tool registry, agent registry, task queues

---

## Removed in 2026-04-16

These crates were removed (out of scope for CLI-first orchestration):

- `gestalt_app` — Flutter app
- `gestalt_terminal` — TUI
- `gestalt_ui` — UI components
- `gestalt_mcp` (server) — standalone MCP server
- `gestaltctl` — standalone admin binary
- `gestalt_infra_github` — GitHub infra adapter
- `gestalt_infra_embeddings` — BERT embedding infra
- `benchmarks` — standalone benchmark suite

---

## Dependencies

```
gestalt_core
├── surrealdb = "2.6.1"         # Database
├── synapse-agentic              # Tool registry
├── tokio = { features = "full" }
├── serde_json
├── reqwest (for MCP client)
└── oauth2 / jsonwebtoken (auth)

gestalt_timeline
├── gestalt_core
├── tokio
├── teloxide (optional, telegram)

gestalt_cli
├── gestalt_core
├── tokio
└── reedline (REPL)

gestalt_swarm
├── gestalt_core
├── tokio
└── tokio-sync

synapse-agentic
├── tokio
└── async-trait
```
