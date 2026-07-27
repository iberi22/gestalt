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
├── gestalt-router/                 # Multi-agent orchestration engine
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
| gestalt-router | lib | 12 modules (13 .rs) | **Wave 1 — COMPLETE** |
| gestalt_swarm | bin | excluded | Legacy |
| synapse-agentic | lib | 1 .rs | Stable |

## Test Coverage

| Crate | Test files | Tests count | Compiles? | Real % passing |
|-------|-----------|-------------|-----------|---------------|
| gestalt_core | — | — | ✅ | Unknown |
| gestalt_cli | — | — | ✅ | N/A (binary) |
| gestalt-router | 7 test files | 119 tests | ✅ | **100%** (119/119 pass) |
| synapse-agentic | — | — | ✅ | Unknown |

All 119 tests in `gestalt-router` pass. Test categories:
- agent_tests: 15/15 — AgentSpec, AgentResult, SubprocessRunner construction
- checkpoint_tests: 16/16 — git checkpoint creation, symlink detection, binary files
- doctor_tests: 12/12 — orphaned run detection, pruning, manifest handling
- integration_test: 16/16 — full pipeline integration, event log, overlap info
- overlap_tests: 13/13 — overlap detection, mergeability, 50+ branch performance
- router_tests: 46/46 — comprehensive integration tests across all modules
- worktree unit: 1/1 — worktree lifecycle

> **Note:** `test_doctor_pruning_and_orphans` sets `GESTALT_HOME` env var; run with `--test-threads=1` to avoid parallel-test env interference.

## Known Issues (Fixed)

### ~~find_overlaps hardcoded `"."` path~~ ✅ FIXED
`router.rs:282` previously passed `Path::new(".")` to `find_overlaps()` instead of the actual repository path. The Router now resolves `std::env::current_dir()` properly, matching the pattern used by `WorktreeManager`.

### ~~Test compilation errors~~ ✅ FIXED
`router_tests.rs` imported `find_overlaps_in_repo` but the test functions called `find_overlaps` (the wrapper function). Import changed to `find_overlaps`.

### ~~integrate_branches tree vs commit SHA mismatch~~ ✅ FIXED
`integrate.rs` step 3 passed `current_commit` (a commit SHA) as the tree argument to `git commit-tree`. Now resolves the tree SHA from the intermediate commit via `git rev-parse <commit>:`.

## Build / Run

```bash
# Check compilation
PKG_CONFIG_PATH=<openssl-pkgconfig> cargo check --workspace

# Run all tests (sequential to avoid env interference)
PKG_CONFIG_PATH=<openssl-pkgconfig> cargo test -p gestalt-router -- --test-threads=1

# Run multi-agent orchestration
cargo run -p gestalt_cli -- run --task "..." --agents "cmd1,cmd2" --base-ref main

# Run REPL
cargo run -p gestalt_cli -- repl
```

## Status

✅ Project active — iberi22/gestalt — SouthWest AI Labs
✅ Wave 1 complete (Router Foundation) — 119 tests passing
📅 Last verified: 2026-07-25
