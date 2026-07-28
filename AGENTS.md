# AGENTS.md — Gestalt Agent Design Non-Negotiables

Welcome! This document outlines the absolute design non-negotiables (reglas inquebrantables de diseño) for developers creating, extending, or maintaining agents within the Gestalt framework.

## 🛑 Design Non-Negotiables

### 1. VFS Isolation & Sandboxing
- **Always Use OverlayFs:** Agents must never write directly to the local host filesystem outside of their designated, isolated VFS overlay. All reads and writes must go through the VFS port (`gestalt_core::ports::outbound::vfs`).
- **No Path Escaping:** Path traversal or escaping above the virtual root is strictly prohibited. Security and sandboxing are foundational design goals.

### 2. Deterministic State Management
- **Strict State Transitions:** Every agent execution run must map cleanly onto `AgentState` enum values (`Pending`, `Running`, `Success`, `Timeout`, `Crashed`, `NoChanges`, `Quarantined`).
- **Run Manifests:** Agent state changes must be updated deterministically inside the central `RunManifest`. State must never be kept solely in-memory or in local transient variables.

### 3. Isolated Code Integration
- **Feature Branch Integration:** Agents work in isolated feature/worktree branches. Integration to the target or `integration_branch` must go through the proper router merge flow (`gestalt-merge` or orchestration router).
- **No Direct Main Commits:** Direct unvalidated commits to production/main branches of primary workspaces are prohibited.

### 4. LLM Provider Resilience & Failover
- **Automatic Fallbacks:** Any LLM caller must utilize the resilience layer to gracefully fall back (e.g., from OpenAI to Anthropic/DeepSeek) on credential errors or API timeouts.
- **Fail Gracefully:** Never panic or crash due to rate-limiting or service-down events; always capture structured error logs and return a clean failure/timeout agent state.

### 5. Workspace Crate Exclusion
- **`gestalt_swarm`** is excluded from the Cargo workspace members in `Cargo.toml` (resolving compilation/link errors regarding `GroqProvider` or `synapse-agentic`).
- All other crates (`gestalt_core`, `gestalt_cli`, `gestalt-router`, `gestalt-merge`, `gestalt-state`, `gestalt-ws`, `synapse-agentic`) are active workspace members and compile together.

---

## 🚀 Key Framework Packages

| Package | Purpose | Path |
|---------|---------|------|
| `gestalt_core` | Core domain, VFS, LLM adapters, registry | `gestalt_core/` |
| `gestalt_cli` | CLI REPL and agent commands | `gestalt_cli/` |
| `gestalt-router` | Orchestration specs, routing, run states | `gestalt-router/` |
| `gestalt-merge` | Isolated branch merging and conflict resolution | `gestalt-merge/` |
| `synapse-agentic` | Primitive agent tools and provider clients | `synapse-agentic/` |

## 🛠️ Essential Development Tasks

### Running Router Tests
Ensure all orchestration router unit and integration tests are passing:
```bash
cargo test -p gestalt-router
```

### Formatting & Linting
Enforce style guidelines across the workspace:
```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```
