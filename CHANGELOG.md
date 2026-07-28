# Changelog — Gestalt

All notable changes to this project are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html)

---

## [Unreleased]

### Added
- **gestalt-router crate**: Multi-agent orchestration engine with full pipeline
  - `Router::execute()` — parallel agent spawning via JoinSet + Semaphore
  - `SubprocessRunner` — CLI agent process management with POSIX process group kill
  - `WorktreeManager` — git worktree lifecycle (create, list, remove, prune)
  - `Checkpointer` — per-agent git commit with symlink escape detection
  - `OverlapDetector` — file intersection analysis between agent branches
  - `integrate_branches()` — sequential merge using `git merge-tree --write-tree`
  - `Timeline/JsonlEventLog` — JSONL event log for run lifecycle (RunStarted, AgentStateChanged, OverlapDetected, MergeConflict, RunFinished)
  - `Doctor` — orphaned run cleanup and recovery
  - `ProcessManager` — CancellationToken-based process lifecycle
- **gestalt run CLI command**: Wire the Router for multi-agent orchestration from terminal
  - `--task`, `--agents`, `--base-ref`, `--max-parallel`, `--timeout` flags
  - Auto-creates WorktreeManager, SubprocessRunner, JsonlEventLog
- **features.json**: Updated to 12 features (11 passing, 1 failing: feat-belief-graph)
- **Documentation**: README rewritten in English with full Router architecture and usage guide

### Changed
- `gestalt_timeline` removed from workspace — replaced by `gestalt-router` timeline module
- File ownership: 13 PRs merged into `gestalt-router` (Wave 1 — Router Foundation)
- Development workflow: GitFlow with `develop` integration branch, `main` stable

### Fixed
- **checkpoint.rs**: Corrupted merge (two versions concatenated) — complete rewrite
- **integrate.rs**: Corrupted merge (two `integrate()` functions nested) — rewrite as `integrate_branches()`
- **timeline.rs**: `Event` enum fields out of sync with router.rs — updated to match
- **run.rs**: Duplicate struct definitions, nested `RunReport`, missing fields — deduplicated
- **overlap.rs**: Missing `find_overlaps()` function and `OverlapInfo` struct — added
- **worktree.rs**: Added `base_dir` field, high-level `create_worktree(run_id, agent_id, base_sha)`, and `cleanup_worktree()`
- **Cargo.toml**: Consolidated duplicate `tempfile` dev-dependency
- **agent.rs**: Fixed `AgentRunner` trait signature, `EventLog` trait unification
- **Type mismatches**: 143 compilation errors reduced to 0 across the workspace

### Infrastructure
- Continuous integration: `cargo check --workspace` passing
- 9 fix commits on develop, 13 PRs integrated total
- Branch cleanup pending (stale Jules feature branches remain on remote)

## [2026-07-27] Ola 5 — Harden + Warm + Polish

### Added
- **HARDEN P0** — 6 security issues resolved across Router and Core
- **WARM-AGENTS** — agent pool with health checks and feedback loop
- **INTEGRATION** — StateDB + WebSocket wiring for real-time run state
- **Kimi K3 Cycle Auditor** — 3-subagent audit pipeline for implementation review
- **SerialMergeQueue** in `router.rs` — serial FIFO integration with `dry_run` mode (PR #438)
- **AtomicCheckpointer** — pre-checkpoint HEAD save + atomic rollback on manifest write failure (PR #430)
- **`docs/gestalt-run-guide.md`** — functional walkthrough, CLI reference, architecture Mermaid diagram (PR #433)
- **cgroups v2** — dynamic discovery via systemd delegated slices + `/proc` process tree traversal (PR #435)
- **`ClassifiedMergeError` + `RetryPolicy` + `CleanSlate`** — conflict retry from clean base_sha (PR #434)
- **`IndependentRunStatus`, `AgentRunReport`, `IndependentMergeResult`** — per-agent independent merge (PR #426)
- **`CliAdapter` trait + `ExternalCliAdapter` + thread-safe `AdapterRegistry`** — external CLI tool interface (PR #427)
- **`CoreError` enum** with `thiserror` — precise classification of VFS, Repo, MCP, DB, Embedding, Config, Validation errors (PR #431)
- **Exclusive transactions with retry loops** in `StateDb` / `StateDbVfs` — database concurrency safety (PR #437)
- **`fd-lock` file-level locking** on `swarm_metrics.json` — exclusive for writes, shared for reads (PR #455)
- **Self-contained `ingest.rs`** with `MetricDrivenPriorities`, `ExecutionHistory`, `SelfTuning`, `FeedbackReport` (PR #432)

### Changed
- **`Router::execute`** — now uses `SerialMergeQueue` for serial FIFO integration of completed agent branches
- **Event ordering** in `router_tests.rs` — re-adjusted with documented architectural reasoning
- **`StateDbEventLog::read_events`** — reversed output ordering for chronological timeline
- **GestaltAgent registry** — updated agent entries with correct model assignments, rate limiter, tiny agent inventory
- **gestalt_swarm** — removed from Cargo workspace members, lockfile regenerated (PR #457)
- **AGENTS.md Section 5** — updated to reflect gestalt_swarm exclusion (PR #457)
- **`agent.rs` unsafe refactor** — complete rewrite with `// SAFETY:` docs, PID validation, and cgroup isolation
- **9 `unsafe` blocks** — documented with `// SAFETY:` explanations, `pid > 1` validation, `unsafe pre_exec` removed (PR #456)
- **`ingest.rs` persistence** — thread-safe writes with file-level locking
- **VFS doc-tests** — fixed to compile out-of-the-box (PR #431)

### Fixed
- **`thread::sleep` false positive** in audit rules
- **Mutable borrow in `load_test.rs`** + missing `GroqProvider` in `main.rs` (PR #432)
- **Shell command validation** — typo allowing `$` variable expansion (PR #431)
- **Race condition** on `swarm_metrics.json` — write clobbering via `fd-lock` (PR #455)
- **Core domain trait contracts** — documented and enforced with 80%+ test coverage (PR #431)
- **Binary merge conflicts** — auto-rollback preserving history (PR #438)

### Infrastructure
- 13+ Ola 5 merges integrated (PRs #426–#438, #455–#457)
- gitcore score: 95.8%
- features.json retired during cleanup
- `.gitignore` hardened with `*.patch`, `audit_notes.md`, `/existing`

## [1.0.0] — 2026-03-03

### Added
- Initial Gestalt workspace with CLI, Core, Swarm, and Timeline crates
- SurrealDB integration for timeline events
- VFS OverlayFs for agent file isolation
- MCP client for external tool discovery
- LLM adapters (Gemini, MiniMax)
- 12+ built-in tools (git, shell, file, search, ask_ai)

### Fixed
- VFS patch application stability
- Runtime file-read observation boundaries
- Benchmark and release workflow permissions

---

*Built with ❤ at SouthWest AI Labs*
