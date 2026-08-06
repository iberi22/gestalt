# Software Requirements Specification — Gestalt (Router)

> **Protocol:** GitCore 3.8.0 · **Version:** 2.1.0 · **Updated:** 2026-07-29

## REQ-001: Router MVP

| Field | Value |
|-------|-------|
| Category | Functional |
| Priority | High |
| Status | ✅ **Implemented and verified** — 119/119 tests pass |
| Test coverage | **100%** (119 tests across 7 test files — all passing) |

**Description:** Gestalt lanza N agentes CLI sobre worktrees Git aislados, recolecta cambios, detecta solapamientos y produce ramas mergeables.

**Acceptance (current state):**
- [x] `gestalt run --agents <list> "task"` produces N worktrees (via WorktreeManager)
- [x] Each agent runs with CWD in its worktree (via SubprocessRunner)
- [x] Per-agent checkpoint with symlink-escape detection (via Checkpointer)
- [x] Overlap detection: diff --name-only intersection (via OverlapDetector)
- [x] Sequential merge in integration branch (via integrate_branches)
- [x] Conflicts reported, branches preserved (via IntegrateResult)
- [x] JSONL event log per run (via JsonlEventLog)
- [x] Tests updated to match new API — **119/119 PASSING**

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

---

## REQ-007: Swarm Bridge (feature feat-swarm-bridge)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-swarm-bridge)*

### Description
Gestalt Swarm parallel exec bridge (Python asyncio)

*Feature: `feat-swarm-bridge` · status: pending*

## REQ-008: Swarm Smart Selection (feature feat-swarm-smart-selection)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-swarm-smart-selection)*

### Description
Smart goal to agent selection (keyword routing)

*Feature: `feat-swarm-smart-selection` · status: pending*

## REQ-009: Swarm Dynamic Count (feature feat-swarm-dynamic-count)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-swarm-dynamic-count)*

### Description
Dynamic agent count based on rate limits and complexity

*Feature: `feat-swarm-dynamic-count` · status: pending*

## REQ-010: Swarm Streaming (feature feat-swarm-streaming)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-swarm-streaming)*

### Description
Streaming partial results as agents complete

*Feature: `feat-swarm-streaming` · status: pending*

## REQ-011: Unified Storage (feature feat-unified-storage)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-unified-storage)*

### Description
Unified Storage using SurrealDB for Vector and Graph data

*Feature: `feat-unified-storage` · status: pending*

## REQ-012: Hybrid Search (feature feat-hybrid-search)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-hybrid-search)*

### Description
Hybrid Search combining BM25 keyword matching and Vector retrieval

*Feature: `feat-hybrid-search` · status: pending*

## REQ-013: Belief Graph (feature feat-belief-graph)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-belief-graph)*

### Description
Belief Graph for conceptual relationship mapping

*Feature: `feat-belief-graph` · status: pending*

## REQ-014: Mcp Server (feature feat-mcp-server)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-mcp-server)*

### Description
Model Context Protocol (MCP) Server for OpenClaw integration

*Feature: `feat-mcp-server` · status: pending*

## REQ-015: Src Reference (feature feat-src-reference)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-src-reference)*

### Description
Source Code Reference (SRC) - Comprehensive documentation of all source files

*Feature: `feat-src-reference` · status: pending*

## REQ-016: Router Mvp (feature feat-router-mvp)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-router-mvp)*

### Description
Router MVP — WorktreeManager, SubprocessRunner, Checkpointer, OverlapDetector, integrate_branches, Timeline JSONL, Doctor, Router::execute

*Feature: `feat-router-mvp` · status: pending*

## REQ-017: Cli Run (feature feat-cli-run)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-cli-run)*

### Description
CLI gestalt run command with agent orchestration

*Feature: `feat-cli-run` · status: pending*

## REQ-018: Event Log (feature feat-event-log)

- **Category:** Functional
- **Priority:** P2
- **SRS Status:** `active`
- **Files:** *(see .gitcore/features.json feature feat-event-log)*

### Description
JSONL event log for tracking run lifecycle events

*Feature: `feat-event-log` · status: pending*

## REQ-022: Universal Event Bus (feature feat-event-bus)

- **Category:** Functional
- **Priority:** P1
- **SRS Status:** implemented (PR #520; verified 2026-08-06)
- **Files:** gestalt-router/src/event_bus.rs, gestalt-router/src/xavier_sink.rs, gestalt_cli/src/bus.rs

### Description
El router de Gestalt emite `run_started`/`run_finished` al bus en cada ejecución orquestada, y cualquier agente externo (Hermes, Jules, CLI) puede empujar eventos vía `POST /api/event` (:8081) con trazabilidad completa (agent, event_type, run_id, project, state, ts, metadata{llm, provider, requested_by, decision}). Dedup por hash estable (SHA-256) con ventana de 300s medida con reloj del servidor: retries/replays no duplican ni en StateDb ni en Xavier. Eventos persistidos durablemente en StateDb y streamed a Xavier como `kind=execution` con path único. Fire-and-forget: la caída de Xavier o del bus nunca bloquea al agente emisor (R4/R5 del diseño).

*Feature: `feat-event-bus` · status: stable*

## REQ-023: Xavier Thinking Loop (feature feat-xavier-thinking)

- **Category:** Functional
- **Priority:** P1
- **SRS Status:** implemented (PR #521; verified 2026-08-06)
- **Files:** gestalt-router/src/thinking.rs, gestalt_cli/src/bus.rs (OllamaSynthesizer)

### Description
Loop periódico (`gestalt thinking`) que consulta ejecuciones recientes (`kind=execution`), sintetiza un insight determinista (PATTERNS por estado y agente / BLOCKERS / DECISIONS / NEXT) por agregación del corpus — SIN dependencia de LLM externo (decisión 2026-08-05: no Ollama, cero servicios) — y lo re-indexa como `kind=insight` para que futuros agentes lo encuentren vía PRE search. Idempotente (skip si ya existe insight de hoy), gated a ≥3 ejecuciones.

*Feature: `feat-xavier-thinking` · status: stable*
