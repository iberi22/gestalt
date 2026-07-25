# SRS Index — Gestalt (Router)

> **Protocol:** GitCore 3.8.0 · **Version:** 2.0.0 · **Updated:** 2026-07-25

## Documents

| Document | Path | Status |
|----------|------|--------|
| Architecture Map | `ARCHITECTURE.md` | ✅ Updated |
| Requirements | `REQUIREMENTS.md` | ✅ Updated |
| Full Design | `../../REDESIGN.md` | ✅ New |

## System Context

```
gestalt (L4 — Product App)
Router CLI → Worktree isolation → Agent subprocesses → Git branches → PRs
```

## SWAL Integration

- **Pro features:** Active SWAL node (not Stripe)
- **Memory:** Xavier HTTP/MCP (`:8006`)
- **Multi-instance:** `instance_id` isolation
- **Protocol:** GitCore 3.8.0
