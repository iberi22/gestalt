# 📝 TODO.md — Pending Tasks

> Last Updated: 2026-07-25

## 🔴 Priority: High

- [ ] **Fix unwraps in production** — `gestalt_core/src/application/indexer.rs`, `gestalt_cli/src/repl.rs` — replace with `?` + `thiserror`
- [ ] **SurrealDB v2 deepen** — make v2 the default, migrate indexer queries to v2 syntax
- [ ] **Dependabot alerts** — 5 vulnerabilities (jsonwebtoken, lru, rand, rustls-webpki)

## 🟡 Priority: Medium

- [ ] **Router Wave 2: production hardening** — add timeout config, retry logic, agent health checks
- [ ] **Router Wave 2: conflict resolution** — auto-merge strategies for common file overlaps
- [ ] **Router Wave 2: streaming output** — real-time agent output via WebSocket/channels
- [ ] **VFS integration tests** — test OverlayFs merge in complex workspace structures
- [ ] **Tool registry tests** — add unit tests for git/shell/file tools
- [ ] **Graceful shutdown** — proper SIGTERM handling
- [ ] **Config hot-reload** — no runtime config update without restart

## 🟢 Priority: Low

- [ ] **cargo doc** — generate API reference for `gestalt_core` traits
- [ ] **CI cache optimization** — reduce GitHub Actions build times
- [ ] **Streaming for LLM adapters** — not implemented yet
- [ ] **Long-term memory** — no persistent memory system (relies on external vector DB)
- [ ] **Test racing** — `test_doctor_pruning_and_orphans` flaky in parallel env due to `GESTALT_HOME` env var

---

*Scope: Router MVP complete (119 tests). Wave 2 planning starts.*
