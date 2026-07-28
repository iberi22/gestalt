# Gestalt Architecture

> Last Updated: 2026-07-27
> Status: v2.1 — Ola 5: gestalt_core domain models, gestalt-ws, transactional state, merge patterns

## Overview

Gestalt is a multi-agent orchestrator for local AI agents. It manages concurrent agent execution,
file system isolation, and timeline tracking.

## Core Architecture: 3-Layer State Backend

```
┌─────────────────────────────────────────┐
│           Gestalt Router                 │
│  ┌───────────────────────────────────┐  │
│  │  Tier 1: MemState (DashMap)       │  │
│  │  • agent_states: 0-latency reads  │  │
│  │  • active_locks: file contention   │  │
│  │  • timeline_tx: event broadcast    │  │
│  └───────────────────────────────────┘  │
│  ┌───────────────────────────────────┐  │
│  │  Tier 2: StateDB (SQLite WAL)     │  │
│  │  • runs, agents, locks, timeline  │  │
│  │  • persisted, atomic, queryable   │  │
│  └───────────────────────────────────┘  │
│  ┌───────────────────────────────────┐  │
│  │  Tier 3: Xavier (memory store)    │  │
│  │  • PRE-run context search         │  │
│  │  • POST-run archival storage      │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## Crate Map

|| Crate | Purpose |
||-------|---------|
|| gestalt-state | SQLite StateDB + MemState — transactional with auto-retry, virtual_fs |
|| gestalt-router | Agent orchestration, worktrees, merge (AtomicCheckpointer, ProcessReaper, SerialMergeQueue, WriteSetValidator) |
|| gestalt_core | Domain types (CoreError, Role, Message), VFS, XavierClient |
|| gestalt_cli | CLI entry point |
|| gestalt-merge | Branch merging, CleanSlateRetry integration |
|| gestalt-ws | WebSocket server — timeline event broadcast to clients |
|| synapse-agentic | Agent tool primitives |

## Key Decisions

- **No Semaphore**: Concurrency controlled via file-level locks in MemState
- **No JSON state files**: All state in SQLite WAL mode
- **StateDbEventLog**: Replaces old JsonlEventLog with SQLite-backed timeline
- **Xavier for long-term only**: Memory search PRE-run, archival POST-run

## State Flow

```
Agent Run Start
     │
     ▼
Router.create_run() ──────────────────► StateDB (runs table)
     │
     ├─► MemState.agent_states ───────► 0-latency state reads
     ├─► MemState.active_locks ───────► File contention tracking
     └─► MemState.timeline_tx ────────► Event broadcast to subscribers

Agent Run End
     │
     ▼
Router.finalize_run() ────────────────► StateDB (agents, timeline)
     │
     └─► Xavier.archival() ───────────► Long-term memory storage
```

## VFS Architecture

```
VfsPort (trait)
├── MemoryFs        — in-memory file system
└── OverlayFs       — layered merge (upper + lower)
    ├── read        — upper first, then lower
    ├── write       — upper only
    └── merge       — explicit overlay merge
```

Used for agent workspace isolation. Each agent gets an isolated overlay that is flushed
to disk only after the run completes.

## Domain Module (`gestalt_core/src/domain/`)

The `domain/` module in `gestalt_core` centralises shared domain types used across all crates:

| File | Contents |
|------|----------|
| `error.rs` | `CoreError` enum with typed variants (Vfs, Repository, Mcp, Database, Embedding, Agent, Indexing, Config, Validation, Internal) and a `Result<T>` alias |
| `mod.rs` | Primitive domain models: `Role`, `Message`, `AgentResponse`, `ConsensusResult` |
| `genui.rs` | GenUI interaction models |
| `memory.rs` | Memory/recall models |
| `rag/` | RAG chunking and embedding models (`embeddings.rs`, `mod.rs`) |

## Communication Patterns

| Pattern | Mechanism | Use Case |
|---------|-----------|----------|
| **In-Process** | `tokio::spawn` + shared `Arc<>` state | Swarm (parallel agents, same binary) |
| **StateDB Events** | SQLite timeline table + MemState broadcast | All agents (persistent event log) |
| **Process Spawning** | `tokio::process::Command` + stdout/stderr capture | External CLI agents (codex, claude) |

## Ola 5 Patterns

New concurrency, resilience, and integration patterns introduced in v2.1:

| Pattern | Crate | Purpose |
|---------|-------|---------|
| **AtomicCheckpointer** | `gestalt-router` (`checkpoint.rs`) | Git-aware checkpoint with rollback: on manifest write failure (real or simulated), performs `git reset --mixed` to original SHA, guaranteeing atomic worktree state |
| **ProcessReaper** | `gestalt-router` (`agent.rs`) | Cgroups-aware child-process reaper: on `Drop`, sends `SIGTERM` to the process group (descendants + root), ensuring no orphan agents remain after a run ends |
| **SerialMergeQueue** | `gestalt-router` (`router.rs`) | Sequential branch integration queue: enqueues branches one-at-a-time, accumulating merges; on conflict, rolls back and records the conflict without advancing the queue |
| **CleanSlateRetry** | `gestalt-router` (`integrate.rs`) | Retry policy for `integrate_branches`: on every retry attempt, starts from a clean-slate base commit, re-applying all remaining branches with a fresh `SerialMergeQueue` |
| **WriteSetValidator** | `gestalt-router` (`worktree.rs`) | Write-scope enforcement for agent workspaces: validates every `write_block` against the agent's declared `allowed_paths`, rejecting out-of-scope writes with a structured error before they touch the worktree |
| **TransactionalStateDb** | `gestalt-state` (`statedb.rs`) | Auto-retry wrapper around SQLite `execute_transaction`: uses `Exclusive` transaction behaviour with exponential backoff (50ms × attempt) on `DatabaseBusy`/`DatabaseLocked`, up to a configurable max retries |
| **StateDbVfs** | `gestalt-state` (`virtual_fs.rs`) | SQLite-backed versioned virtual file system implementing the `VirtualFS` trait: stores file versions with SHA-256 content hashing, supports `read_file`, `write_block` (find-replace), `list_versions`, `get_diff`, and delegated file locking |

## Dependencies

```
gestalt_core
├── serde / serde_json
├── tokio = { features = "full" }
├── reqwest (for MCP client, Xavier)
├── git2
└── synapse-agentic

gestalt-state
├── rusqlite (bundled SQLite)
├── dashmap
├── chrono
└── serde / serde_json

gestalt-router
├── gestalt_core
├── gestalt-state
├── tokio
└── uuid

gestalt_cli
├── gestalt_core
├── gestalt-router
├── clap
└── rustyline (REPL)

gestalt-merge
└── thiserror

gestalt-ws
├── tokio
├── tokio-tungstenite
├── futures-util
├── serde / serde_json
├── chrono
└── gestalt-state

synapse-agentic
├── tokio
├── reqwest
└── async-trait
```
