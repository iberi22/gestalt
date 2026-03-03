# 🧠 PLANNING.md - Gestalt Production Alignment

_Last update: 2026-03-03_

## Objective
Ship and validate **Gestalt v1.0.0** as a stable, production-ready autonomous Rust agent platform with GitHub as protocol source of truth.

## Source of Truth
- GitHub Issues and PRs are canonical.
- `.gitcore` planning/features are synchronized mirrors.

## Release Scope
- `v1.0.0` implemented and closed: #33, #31, #8, #19, #21, #82, #83.
- Open blockers in GitHub: none (as of 2026-03-03).

## Current Architecture Priorities
1. Keep runtime non-blocking (`tokio`) and supervisor-based (`Hive`).
2. Preserve VFS isolation with binary-safe IO and external change watching.
3. Keep heavy integrations behind optional infra crates/features.
4. Enforce CI stability and deterministic release artifacts.

## Production Gates
1. `cargo fmt --all --check`
2. `cargo test --workspace --all-targets`
3. Benchmark workflow must not fail on PR comment permission restrictions and must sync Rust metrics into leaderboard storage.
4. Release workflow must generate deterministic `v1.0.0` artifacts.

## Workstreams
1. Final protocol/admin sync (`features.json`, issue mapping, planning docs).
2. CI benchmark hardening (`agent_benchmark sync-rust` + compare report).
3. Workspace quality verification (`fmt`, tests, warning/error budget).
4. Release cut readiness (`main` only, clean integrated branches, tag prep).
