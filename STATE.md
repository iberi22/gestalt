# 📊 STATE.md — Project State

## 🟢 Current Version: `1.1.0`
**Last Update:** 2026-07-25
**Status:** Stable — Router MVP complete, 119 tests passing

## 🎯 Project Purpose
Gestalt is a context-aware AI Agent Orchestration Platform built in Rust. It provides VFS isolation, Swarm parallel execution, timeline-based event logging, git worktree agent isolation, overlap detection, automatic branch integration, and a tool registry — all via CLI/REPL.

## 🏗️ Architecture Summary
- **Execution Model:** Async autonomy via `tokio` + `synapse-agentic` (Hive actor model)
- **State Management:** Timeline events via JSONL event log (`JsonlEventLog`)
- **Orchestration:** `gestalt-router` crate — Router::execute pipeline with worktree isolation
- **Agent Isolation:** Per-agent git worktrees with symlink-escape detection
- **Overlap Detection:** File-intersection analysis via `OverlapDetector` + `git diff --name-only`
- **Branch Integration:** Sequential `git merge-tree --write-tree` with intermediate commit creation
- **Cleanup:** Doctor module for orphaned run detection and pruning
- **Tools:** 12+ built-in (git, shell, file, search, ask_ai, etc.)

## 📦 Workspace Crates (4 active, 1 excluded)

| Crate | Type | Description |
|-------|------|-------------|
| `gestalt_core` | lib | VFS, auth, LLM adapters, agent tools, MCP client |
| `gestalt_cli` | bin | REPL + CLI commands (includes `gestalt run`) |
| `gestalt-router` | lib | **Wave 1 — Router MVP** WorktreeManager, Checkpointer, OverlapDetector, Integrate, Timeline, Doctor, Router::execute |
| `gestalt_swarm` | bin | **EXCLUDED** Legacy swarm coordinator (not in workspace) |
| `synapse-agentic` | lib | Tool registry + agentic primitives |

## ✅ Completed Milestones

- [x] VFS overlay with OverlayFs merge
- [x] Swarm coordinator with TaskQueue + HealthMonitor (legacy)
- [x] LLM adapters (OpenAI + Anthropic) with failover
- [x] Google OAuth2 + PKCE auth
- [x] 12+ agent tools (git, shell, file, search, clone, ask_ai...)
- [x] CLI REPL
- [x] **Wave 1: Router MVP** — WorktreeManager, SubprocessRunner, Checkpointer, OverlapDetector, integrate_branches, Timeline JSONL, Doctor, Router::execute
- [x] 119 unit/integration tests passing for gestalt-router
- [x] `cargo check --workspace` clean

## ⚠️ Known Issues

- `test_doctor_pruning_and_orphans` flaky in parallel (uses `GESTALT_HOME` env var) — run with `--test-threads=1`
- No long-term memory system (relies on external vector DB)
- No telemetry/observability
- `unwrap()` in production paths (config, indexer) — use `expect()` with messages

## 📈 Current Health

- **CI:** ✅ All 119 tests passing (`cargo test -p gestalt-router`)
- **Build:** ✅ `cargo check --workspace` clean
- **Linting:** Zero clippy errors on main
- **Vulnerabilities:** 5 Dependabot alerts pending (jsonwebtoken, lru, rand, rustls-webpki)

## 🗑️ Removed (2026-04-16)

- gestalt_app (Flutter app)
- gestalt_terminal (TUI)
- gestalt_ui (UI components)
- gestalt_mcp (standalone server)
- gestaltctl (admin binary)
- gestalt_infra_github
- gestalt_infra_embeddings
- benchmarks/

---

*Gestalt — AI agents that actually execute.*
