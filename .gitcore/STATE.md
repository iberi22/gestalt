# STATE.md - Project State

**Project:** Gestalt-Rust
**Last Updated:** 2026-07-27
**Location:** `.gitcore/` (per GitCore Protocol)

## Current Status

| Metric | Value |
|--------|-------|
| Build | `cargo check --workspace` (gestalt_swarm excluded) |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Tests | gestalt_core / gestalt_cli / gestalt-router |
| Documentation | Swarm exclusion documented in AGENTS.md / CLAUDE.md |

## Module Status

| Module | Status | Last Verified |
|--------|--------|--------------|
| gestalt_core | ✅ Active | 2026-07-27 |
| gestalt_cli | ✅ Active | 2026-07-27 |
| gestalt-router | ✅ Active | 2026-07-27 |
| gestalt-merge | ✅ Active | 2026-07-27 |
| gestalt-state | ✅ Active | 2026-07-27 |
| gestalt-ws | ✅ Active | 2026-07-27 |
| synapse-agentic | ✅ Active | 2026-07-27 |
| gestalt_swarm | ⛔ Excluded from workspace | 2026-07-27 |

## Known Issues

- `gestalt_swarm` must stay out of workspace members; use `swarm_bridge.py` / `gestalt_cli` instead.
- Stale CI package names (`gestalt_timeline`, `gestalt_mcp`, …) may still appear in workflows — separate cleanup.

## Recent Changes

- Full feature-scan (grep) @ `28e9209`: 10 pass / 2 fail (hybrid-search, belief-graph); overall features 82.1%; unified-storage → 100.
- Shared domain models in `gestalt_core::models` (f64/u64); `gestalt_swarm` ingest reuses them.
- Confirmed `gestalt_swarm` not in root `Cargo.toml` members.
- Cleaned scripts / Hermes skills that invoked `cargo -p gestalt_swarm`.
- Updated GitCore architecture docs for exclusion.

---
*GitCore Protocol 3.8*
