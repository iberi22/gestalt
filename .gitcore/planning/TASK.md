# 📋 TASK.md - Release Tasks (`v1.0.0`)

_Last update: 2026-03-03_

## Release Objective
Close production blockers for `v1.0.0` (core-first) and keep deferred features in `v1.1`.

## Active Issues (`v1.0.0`)
| Issue | Title | Status |
|---|---|---|
| #33 | Integrate Resilience and Compaction framework improvements | 🔄 In Progress |
| #31 | PLAN: Production-Ready CLI Roadmap | 🔄 In Progress |
| #8 | CLEANUP: Resolve compiler warnings and errors | 🔄 In Progress |

## Deferred Issues (`v1.1`)
| Issue | Title | Status |
|---|---|---|
| #19 | Native Agent Tools Implementation | ⏳ Deferred |
| #21 | Multi-Agent Handoff & Hive Delegation | ⏳ Deferred |
| #82 | Benchmark + Leaderboard system | ⏳ Deferred |
| #83 | VFS binary support + FileWatcher | ⏳ Deferred |

## Gate Checklist
- [x] Benchmark workflow made permission-safe for PR comments.
- [x] Timeline schema initialization hardened for engine compatibility.
- [x] Base-version patch validation hardened in FileManager.
- [x] Runtime file-read observation sanitized.
- [x] CLI HTTP timeout support for MCP calls.
- [x] MCP blocking handlers converted to async-safe execution.
- [ ] Full workspace tests passing in clean CI run.
- [ ] Final warning budget validated for release branch.
- [ ] Tag and release artifacts published as `v1.0.0`.

## Notes
- Any new scope is blocked until the gate checklist is green.
- Use GitHub issue comments for live status; keep this file as synchronized summary.
