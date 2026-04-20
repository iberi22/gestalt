# 📝 TODO.md — Pending Tasks

> Last Updated: 2026-04-16

## 🔴 Priority: High

- [ ] **Fix unwraps in production** — `gestalt_core/src/application/indexer.rs`, `gestalt_cli/src/repl.rs` — replace with `?` + `thiserror`
- [ ] **SurrealDB v2 deepen** — make v2 the default, migrate indexer queries to v2 syntax
- [ ] **Dependabot alerts** — 5 vulnerabilities (jsonwebtoken, lru, rand, rustls-webpki)

## 🟡 Priority: Medium

- [ ] **VFS integration tests** — test OverlayFs merge in complex workspace structures
- [ ] **Tool registry tests** — add unit tests for git/shell/file tools
- [ ] **Graceful shutdown** — gestalt_swarm needs proper SIGTERM handling
- [ ] **Config hot-reload** — no runtime config update without restart

## 🟢 Priority: Low

- [ ] **cargo doc** — generate API reference for `gestalt_core` traits
- [ ] **CI cache optimization** — reduce GitHub Actions build times
- [ ] **Streaming for LLM adapters** — not implemented yet
- [ ] **Long-term memory** — no persistent memory system (relies on external vector DB)

---

*Scope: CLI/Swarm/Core only. UI, MCP server, infra crates removed.*
