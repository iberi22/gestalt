# AGENTS.md — Gestalt Router Design Non-Negotiables

Welcome! This document outlines the absolute design non-negotiables for the **gestalt-router** crate.

## 🛑 Design Non-Negotiables

### 1. State Backend via gestalt-state
- **Always use gestalt-state**: All agent state, locks, and timeline events must go through `gestalt-state` (StateDb + MemState).
- **No direct Semaphore**: Concurrency control is achieved via file-level locks in MemState, not `tokio::sync::Semaphore`.
- **No JSON state files**: All persistent state uses SQLite WAL mode via `StateDb`. Old JSON-based state files are deprecated.

### 2. Deterministic State Management
- **Strict State Transitions**: Every agent execution run must map cleanly onto `AgentState` enum values (`Pending`, `Running`, `Success`, `Timeout`, `Crashed`, `NoChanges`, `Quarantined`).
- **StateDb Persistence**: Agent state changes must be persisted atomically in `StateDb`. State must never be kept solely in-memory or in transient variables (ephemeral MemState caches are acceptable for reads, but writes must go to StateDb).

### 3. Timeline via StateDbEventLog
- **StateDbEventLog Required**: All event logging for agent runs must use `StateDbEventLog`. The old `JsonlEventLog` is deprecated and must not be used in new code.

### 4. VFS Isolation & Sandboxing
- **Always Use OverlayFs**: Agents must never write directly to the local host filesystem outside of their designated, isolated VFS overlay.
- **No Path Escaping**: Path traversal or escaping above the virtual root is strictly prohibited.

### 5. Feature Branch Integration
- Agents work in isolated worktree branches. Integration to the target or `integration_branch` must go through the proper router merge flow (`gestalt-merge`).

### 6. LLM Provider Resilience & Failover
- **Automatic Fallbacks**: Any LLM caller must utilize the resilience layer to gracefully fall back on credential errors or API timeouts.
- **Fail Gracefully**: Never panic or crash due to rate-limiting or service-down events.

### 7. Workspace Crate Exclusion
- **No gestalt_swarm workspace inclusion**: The `gestalt_swarm` package is explicitly excluded from the Cargo workspace members. Keep it excluded.

## 🚀 Key Framework Packages

| Package | Purpose | Path |
|---------|---------|------|
| `gestalt-state` | SQLite StateDB + DashMap MemState | `gestalt-state/` |
| `gestalt_core` | Core domain, VFS, LLM adapters, registry | `gestalt_core/` |
| `gestalt_cli` | CLI REPL and agent commands | `gestalt_cli/` |
| `gestalt-router` | Orchestration specs, routing, run states | `gestalt-router/` |
| `gestalt-merge` | Isolated branch merging and conflict resolution | `gestalt-merge/` |
| `synapse-agentic` | Primitive agent tools and provider clients | `synapse-agentic/` |

## 🛠️ Essential Development Tasks

### Testing the State Backend
```bash
# Run all state-related tests
cargo test -p gestalt-state

# Run router tests
cargo test -p gestalt-router
```

### Formatting & Linting
```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```
