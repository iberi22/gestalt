# 🧠 PLANNING.md - Gestalt Production Alignment

_Last update: 2026-03-03_

## Objective
Ship **Gestalt v1.0.0** as a stable, production-ready autonomous Rust agent platform with GitHub as protocol source of truth.

## Source of Truth
- GitHub Issues and PRs are canonical.
- `.gitcore` planning/features are synchronized mirrors.

## Release Scope
- `v1.0.0` includes: #33, #31, #8.
- Deferred to `v1.1`: #19, #21, #82, #83.

## Current Architecture Priorities
1. Keep runtime non-blocking (`tokio`) and supervisor-based (`Hive`).
2. Preserve VFS isolation and explicit flush semantics.
3. Keep heavy integrations behind optional infra crates/features.
4. Enforce CI stability over scope expansion.

## Production Gates
1. `cargo fmt --all --check`
2. `cargo test --workspace --all-targets`
3. Benchmark workflow must not fail on PR comment permission restrictions.
4. Release workflow must generate deterministic `v1.0.0` artifacts.

## Workstreams
1. Runtime/CI stabilization (tests + schema compatibility + workflow hardening).
2. Reliability/security fixes from code reviews.
3. Warning cleanup and technical debt reduction.
4. Protocol/admin sync (`features.json`, issue mapping, planning docs).
5. Versioning and release automation alignment.
