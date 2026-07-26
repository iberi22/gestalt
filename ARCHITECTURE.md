# Gestalt Architecture

> Last Updated: 2026-07-26
> Status: v2.0 — State backend refactored to SQLite + MemState

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

| Crate | Purpose |
|-------|---------|
| gestalt-state | SQLite state backend + MemState (new) |
| gestalt-router | Agent orchestration, worktrees, merge |
| gestalt_core | Domain types, VFS, XavierClient |
| gestalt_cli | CLI entry point |
| gestalt-merge | Branch merging |
| synapse-agentic | Agent tool primitives |

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

## Communication Patterns

| Pattern | Mechanism | Use Case |
|---------|-----------|----------|
| **In-Process** | `tokio::spawn` + shared `Arc<>` state | Swarm (parallel agents, same binary) |
| **StateDB Events** | SQLite timeline table + MemState broadcast | All agents (persistent event log) |
| **Process Spawning** | `tokio::process::Command` + stdout/stderr capture | External CLI agents (codex, claude) |

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

synapse-agentic
├── tokio
├── reqwest
└── async-trait
```
