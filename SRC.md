# SRC.md — Gestalt Rust

> AI Agent Orchestration Platform — CLI-first, Rust-powered.
> Multi-Agent Codebase Router with git worktree isolation.

## Project

- **Name:** gestalt
- **Type:** Rust workspace (4 crates active, 1 excluded)
- **Description:** Orchestrate AI coding agents in parallel via isolated git worktrees, detect file overlaps, and integrate branches automatically.
- **Tech Stack:** Rust 2021, tokio, serde, clap, git2

## Directory Structure

```
gestalt/
├── Cargo.toml                      # Workspace root
├── gestalt_core/                   # Core: VFS, LLM adapters, auth, MCP client, tools
│   └── src/                        # 45 .rs files — adapters, application, domain, ports
├── gestalt_cli/                    # CLI binary: run, repl, serve, exec, git
│   └── src/main.rs                 # CLI entry point (Commands enum, match arms)
├── gestalt-router/                 # **NEW** Multi-agent orchestration engine
│   └── src/
│       ├── lib.rs                  # Module declarations (11 modules)
│       ├── run.rs                  # RunSpec, AgentSpec, RunReport, AgentResult, RouterError, ConflictInfo
│       ├── run_state.rs            # RunManifest, AgentState
│       ├── router.rs               # Router::execute() — main pipeline orchestrator
│       ├── agent.rs                # AgentRunner trait, SubprocessRunner impl
│       ├── worktree.rs             # WorktreeManager (create, list, remove, prune)
│       ├── checkpoint.rs           # Per-agent git commit with symlink-escape detection
│       ├── integrate.rs            # integrate_branches() — sequential merge-tree
│       ├── overlap.rs              # OverlapDetector — file intersection analysis
│       ├── timeline.rs             # JsonlEventLog — JSONL event log, Event enum
│       ├── doctor.rs               # Orphaned run cleanup and recovery
│       └── process.rs              # ProcessManager with CancellationToken
├── gestalt_swarm/                  # **EXCLUDED** Legacy swarm coordinator (not in workspace)
├── synapse-agentic/                # Tool registry + Hive actor model
│   └── src/lib.rs                  # Tool registry, LLM providers
├── docs/                           # SRS, guides, architecture
│   └── SRS/                        # Requirements, architecture map
└── .gitcore/                       # GitCore protocol: features.json, planning
```

## Crate Map

| Crate | Type | Files | Status |
|-------|------|-------|--------|
| gestalt_core | lib | 45 .rs | Stable |
| gestalt_cli | bin | 1 .rs + 2 helpers | Updated — `gestalt run` added |
| gestalt-router | lib | 11 modules (13 .rs) | **Wave 1 — NEW** |
| gestalt_swarm | bin | excluded | Legacy |
| synapse-agentic | lib | 1 .rs | Stable |

## Test Coverage

| Crate | Test files | Tests count | Compiles? | Real % passing |
|-------|-----------|-------------|-----------|---------------|
| gestalt_core | — | — | ✅ | Unknown |
| gestalt_cli | — | — | ✅ | N/A (binary) |
| gestalt-router | 6 test files | 39 tests | ❌ **110 errors** | **0%** (no tests compile) |
| synapse-agentic | — | — | ✅ | Unknown |

All 39 tests in `gestalt-router/tests/` reference old APIs (Checkpointer struct, MergeResult enum, `integrate()` function, AgentStatus, Router from run.rs) that were rewritten during Wave 1 merge. Tests must be updated to match the new API surface.

## Build / Run

```bash
# Check compilation
PKG_CONFIG_PATH=<openssl-pkgconfig> cargo check --workspace

# Run multi-agent orchestration
cargo run -p gestalt_cli -- run --task "..." --agents "cmd1,cmd2" --base-ref main

# Run REPL
cargo run -p gestalt_cli -- repl

# Run tests (once fixed)
cargo test --workspace
```

## Status

✅ Project active — iberi22/gestalt — SouthWest AI Labs
🔄 Wave 1 complete (Router Foundation) — tests pending update
📅 Last verified: 2026-07-25
