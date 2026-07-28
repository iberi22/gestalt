# 📋 TASK.md — Session: shared domain models (gestalt_core::models)

_Last update: 2026-07-27_

## Objective
Unify duplicated swarm ingest structs into `gestalt_core::models` (f64/u64) and reuse from `gestalt_swarm`.

## Pattern (research)
Canonical shared domain crate / module: one source of truth for serde DTOs; dependents `use gestalt_core::models::*`. Prefer wider numeric types (`f64`, `u64`) at the boundary. Avoid type forks (`f32`/`u32`) across crates.

## Done
- [x] `gestalt_core/src/models.rs` — `ExecutionMetrics`, `NextStep`, `PriorityUpdate`, `AgentStats`, `categorize_error` (f64/u64)
- [x] `gestalt_core/src/lib.rs` — `pub mod models`
- [x] `gestalt_swarm/src/ingest.rs` — import from `gestalt_core::models`; local duplicates removed
- [x] Fix `Ordering::Equal` typo in `generate_next_steps` sort
- [ ] `cargo check -p gestalt_swarm` — shell blocked in agent session; run from `gestalt_swarm/` (own `[workspace]`)

## Do not
- Create GitHub issues
- Re-add `gestalt_swarm` to root workspace members
