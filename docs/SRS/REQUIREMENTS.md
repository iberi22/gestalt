# Software Requirements Specification — Gestalt (Router)

> **Protocol:** GitCore 3.8.0 · **Version:** 2.0.0 · **Updated:** 2026-07-25

## REQ-001: Router MVP

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | High |
| Status | ✅ **Implemented** — tests need update |
| Test coverage | **0%** (39 tests broken — APIs were rewritten) |

**Description:** Gestalt lanza N agentes CLI sobre worktrees Git aislados, recolecta cambios, detecta solapamientos y produce ramas mergeables.

**Acceptance (current state):**
- [x] `gestalt run --agents <list> "task"` produces N worktrees (via WorktreeManager)
- [x] Each agent runs with CWD in its worktree (via SubprocessRunner)
- [x] Per-agent checkpoint with symlink-escape detection (via Checkpointer)
- [x] Overlap detection: diff --name-only intersection (via OverlapDetector)
- [x] Sequential merge in integration branch (via integrate_branches)
- [x] Conflicts reported, branches preserved (via IntegrateResult)
- [x] JSONL event log per run (via JsonlEventLog)
- [ ] Tests updated to match new API — **PENDING**

## REQ-002: Event Log

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | High |
| Status | ✅ **Implemented** |

**Description:** JSONL event log per run. Replaces SurrealDB timeline.

**Acceptance:**
- [x] Events: RunStarted, AgentStateChanged, OverlapDetected, MergeConflict, RunFinished
- [x] Thread-safe file writing via Mutex<BufWriter>
- [x] EventLog trait with pluggable backend
- [x] File in temp dir per run ID

## REQ-003: Semantic Merge (Phase 2)

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | Medium |
| Status | Planned |

**Description:** Merge engine con tree-sitter AST para resolución semántica de conflictos.

**Acceptance:**
- [ ] git merge-tree wrapper funcional
- [ ] tree-sitter parsers para lenguajes soportados
- [ ] Auto-resolución de conflictos no-overlap en mismo archivo

## REQ-004: PR Automation (Phase 2)

| Field | Value |
|-------|-------|
| Category | Automation |
| Priority | Medium |
| Status | Planned |

**Description:** Auto PR creation via gh CLI.

## REQ-005: Preventive Coordination (Phase 3)

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | Low |
| Status | Planned |

**Description:** PathClaims OCC antes de ejecución, repartición de tareas.

## REQ-006: GitCore Compliance

| Field | Value |
|-------|-------|
| Category | Process |
| Priority | High |
| Status | ✅ Done |

- `.git-core-protocol-version` = 3.8.0
- `SRC.md` completo
- `.gitcore/planning/` actualizado
- `docs/SRS/` presentes
- `features.json` actualizado

---

*Previous requirements (SurrealDB, FUSE, Timeline services) archived — see legacy/* 
