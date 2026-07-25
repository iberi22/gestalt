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
