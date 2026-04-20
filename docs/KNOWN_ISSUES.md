# ⚠️ Known Issues

## Security Vulnerabilities (Dependabot)

### RUSTSEC-2026-0049: rustls-pemfile (Unmaintained)
**Package:** `rustls-pemfile` | **Status:** Pending fix from upstream
**Impact:** TLS connections through rustls ecosystem.

### RUSTSEC-2026-0002: lru IterMut Soundness
**Package:** `lru` `<0.16.3` | **Status:** Pending upstream fix
**Impact:** Soundness issue in concurrent scenarios.

### jsonwebtoken, rand, rustls-webpki
**Status:** 5 Dependabot alerts (1 moderate, 4 low) — auto-update PRs pending.

---

## Known Limitations

### unwrap() in Production Code
Locations:
- `gestalt_core/src/application/indexer.rs` (~line 341)
- `gestalt_cli/src/repl.rs` — `Default::default().unwrap()` blocks on init failure

**Fix:** Replace with `?` + `thiserror` enum.

### No Long-Term Memory
Gestalt has no built-in persistent memory. Relies on external vector DB (SurrealDB).

### No Streaming for LLM Adapters
OpenAI + Anthropic adapters don't support streaming responses yet.

### MCP Client Only (No Standalone Server)
Removed `gestalt_mcp` standalone server (2026-04-16). The MCP **client** in `gestalt_core/adapters/mcp/` can still connect to external MCP servers.

### Config Hot-Reload Not Implemented
Config is read at startup only. No runtime config updates.

---

## CI Status

✅ All checks passing on main (clippy, fmt, build, benchmarks, guardian)

**Last CI:** `24525196446` — `fix(clippy): collapse nested if in file_manager.rs watch loop`
