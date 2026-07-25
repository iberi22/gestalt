# Software Requirements Specification — Gestalt (Router)

> **Protocol:** GitCore 3.8.0 · **Version:** 2.0.0 · **Updated:** 2026-07-25

## REQ-001: Router MVP

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | High |
| Status | Design |

**Description:** Gestalt lanza N agentes CLI sobre worktrees Git aislados, recolecta cambios, detecta solapamientos y produce ramas mergeables.

**Acceptance:**
- [ ] `gestalt run --agents agy,claude "task"` produce N worktrees
- [ ] Cada agente corre con CWD en su worktree
- [ ] Al completar: commit por agente + push a branch remota
- [ ] Overlap detection: diff --name-only intersection logged
- [ ] Merge secuencial en rama de integración
- [ ] Conflictos reportados, branches preservados

## REQ-002: Event Log

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | High |
| Status | Design |

**Description:** JSONL event log por run. Reemplaza SurrealDB timeline.

**Acceptance:**
- [ ] Eventos: RunStarted, AgentFinished, OverlapDetected, MergeConflict, BranchPublished
- [ ] Archivo en `~/.gestalt/runs/{run-id}.jsonl`
- [ ] Consultable con `jq`

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
