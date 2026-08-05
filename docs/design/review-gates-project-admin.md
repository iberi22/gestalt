# Gestalt Review Gates + Project Admin — Design Document

**Status:** Draft v0.1
**Author:** Hermes Agent (on behalf of BELA)
**Date:** 2026-08-04
**Target:** Gestalt workspace (`~/proyectosSWAL/gestalt`) + Project Admin (`~/proyectosSWAL/project-admin`)
**Protocol:** GitCore 3.8.0

---

## 1. Problem & Scope

### Problem

The SWAL ecosystem has a **verification gap** between "agent finished" and "change merged":

1. **No delivery gates.** Gestalt orchestrates agents (`gestalt run` → worktrees → checkpoint → integrate → merge), but nothing *freezes a candidate*, *classifies its risk*, and *requires a receipt* before merge. A low-risk doc change and a 2-line auth change are treated identically.
2. **No per-project reality dashboard.** `features.json` honesty is enforced by per-project `verify-pipeline.sh`, but there is no *single admin surface* that shows all 40+ projects, their real %, SRS traceability, PR state, and Xavier health at a glance.
3. **No skill registry.** `~/.hermes/skills/` has 100+ skills but no index of which apply to which project/task (gentle-ai solved this with `skill-registry refresh`).

### Goals

1. Add **RDD-style review gates** to the Gestalt router: freeze candidate → risk tiers by evidence → bounded review → receipt → gate validation at `pre-merge` (and later `pre-push`/`pre-pr`).
2. Turn **Project Admin** (already built, Python stdlib, read-only) into the **admin surface of the Gestalt control plane**: live runs, gate state, feature reality, SRS traceability, PR status, Xavier health.
3. Add a **skill registry** command to Gestalt CLI that indexes `~/.hermes/skills/` and project conventions.

### Non-goals

- ❌ NOT porting gentle-ai wholesale (923 Go files, 330K LOC). We extract the *review-gate pattern*, not the codebase.
- ❌ NOT replacing Xavier (Engram-style memory) — Xavier stays the memory plane.
- ❌ NOT building a new executor — swal-agent-runner PWA / Jules / agent CLI remain the runners.
- ❌ NOT adding auth to Project Admin in this iteration (localhost only).
- ❌ NOT touching `gestalt_swarm` (excluded crate, legacy-only per AGENTS.md).

---

## 2. Baseline Architecture

### Current Gestalt flow

```
┌─────────────────────────────────────────────────────────────┐
│ GESTALT (Rust workspace)                                    │
│                                                             │
│  gestalt_cli (Commands::Run / Status / Serve / Task*)       │
│       │                                                     │
│       ▼                                                     │
│  gestalt-router                                             │
│   ├─ router.rs      (execute, worktree lifecycle)           │
│   ├─ run.rs         (RunSpec/AgentSpec/RunReport)           │
│   ├─ run_state.rs   (re-export gestalt_state::AgentState)   │
│   ├─ checkpoint.rs  (AtomicCheckpointer, run_checkpoint)    │
│   ├─ integrate.rs   (integrate_branches, merge-tree)        │
│   ├─ overlap.rs     (file overlap detection)                │
│   ├─ doctor.rs      (runs/archive health, GESTALT_HOME)     │
│   └─ ws.rs          (timeline events)                       │
│                                                             │
│  gestalt-state     (SQLite StateDB + DashMap MemState)      │
│  gestalt-ws        (WsServer: broadcast::Sender<String>)    │
│  gestalt-merge     (branch merging, RetryPolicy)            │
└──────────────┬──────────────────────────────────────────────┘
               │
               ▼
  PROJECT ADMIN (separate Python stdlib server, :8090)
   ├─ reads ~/proyectosSWAL/*/features.json directly
   ├─ reads docs/SRS/* directly
   ├─ queries Xavier /health
   └─ queries gh CLI for PRs
```

### Key files (verified 2026-08-04)

| File | Role | Status |
|------|------|--------|
| `gestalt-router/src/router.rs` | Orchestration executor | Existing |
| `gestalt-router/src/run.rs` | RunSpec/AgentSpec/RunReport | Existing |
| `gestalt-router/src/checkpoint.rs` | Atomic checkpoint + commit | Existing |
| `gestalt-router/src/integrate.rs` | `integrate_branches()` merge flow | Existing |
| `gestalt-router/src/doctor.rs` | Runs/archive health | Existing |
| `gestalt-router/src/ws.rs` + `gestalt-ws/src/server.rs` | Event broadcast | Existing |
| `gestalt-state/src/lib.rs` | `AgentState` enum | Existing |
| `gestalt_cli/src/main.rs` | `Commands` enum (Run, Status, Serve, Task*) | Existing |
| `.gitcore/features.json` | 12 features, overall 82.1% (honest, 2026-07-27) | Existing |
| `docs/SRS/REQUIREMENTS.md` | 18 REQ-IDs | Existing |
| `~/proyectosSWAL/project-admin/` | Python dashboard (built today) | New (committed `f3b27d1`) |

### Constraints

- `gestalt_swarm` is excluded from the Cargo workspace — do not re-add.
- Cargo check workspace: `gestalt_core, gestalt_cli, gestalt-router, gestalt-merge, gestalt-state, gestalt-ws, synapse-agentic`.
- AGENTS.md non-negotiables: VFS isolation, deterministic AgentState transitions, no direct main commits, LLM fallback resilience.
- Doc language: **English** for all artifacts (user mandate 2026-08-03).
- RDD pattern to borrow from gentle-ai: *review after the candidate, tier by evidence, gates validate the same receipt* — but implemented as a **Rust module**, not a control plane.

---

## 3. Target Architecture

### Topology

```
┌────────────────────────────────────────────────────────────────────┐
│ GESTALT (control plane — Rust)                                     │
│                                                                    │
│  gestalt run ──▶ worktrees ──▶ agents ──▶ checkpoint ──▶ integrate │
│       │                │                    │            │         │
│       │                ▼                    ▼            ▼         │
│       │          [N] review_gate.rs   RunManifest   [N] gate.rs    │
│       │          freeze candidate ──▶ tier ──▶ receipt ──▶ pre-merge
│       │                                                           │
│       │  [N] skill_registry.rs  (index ~/.hermes/skills + project)│
│       │  doctor.rs (exists) · ws.rs (exists) · api.rs [N]         │
└──────────────┬─────────────────────────────────────────────────────┘
               │ REST :8081 (api.rs) + WebSocket (gestalt-ws)
┌──────────────▼─────────────────────────────────────────────────────┐
│ PROJECT ADMIN (Python :8090 — MODIFIED)                            │
│  ├─ /api/projects  (unchanged: features.json + SRS direct read)    │
│  ├─ /api/xavier    (unchanged)                                     │
│  ├─ /api/prs       (unchanged)                                     │
│  ├─ /api/gestalt/runs   [NEW] → Gestalt :8081/api/runs             │
│  ├─ /api/gestalt/gates  [NEW] → Gestalt :8081/api/gates            │
│  └─ WebSocket client [NEW] → live run/gate events                  │
└────────────────────────────────────────────────────────────────────┘
```

### Design decisions

| # | Decision | Rationale |
|---|----------|-----------|
| D1 | Review gates live in **gestalt-router**, not a new crate | Reuses `checkpoint.rs`, `integrate.rs`, `AgentState`; no new workspace member (AGENTS.md constraint) |
| D2 | Receipt = `sha256` of frozen candidate + tier + lens results, stored in `$GESTALT_HOME/gates/<run_id>.json` | Follows doctor.rs `get_runs_dir()` convention; immutable, replayable |
| D3 | Tier by **evidence**, never size (gentle-ai rule) | `tier_0` = no review; `tier_1` = single lens; `tier_2` = auth/security/crypto → 2+ lenses |
| D4 | Project Admin stays Python stdlib (no framework rewrite) | Zero deps, works today; only adds a Gestalt API client |
| D5 | Gestalt exposes a **read-only admin API** (`api.rs`, :8081) | `Serve` command already exists (`Commands::Serve`); extend it — no new binary |
| D6 | Skill registry = CLI command + JSON index file | `gestalt skill-registry refresh` → `~/.hermes/skills/index.json` |
| D7 | Gates are **opt-in per run** (`--review-gate`) | Backward compatible: existing `gestalt run` calls unchanged |

---

## 4. Component-by-Component Design

### 4.1 `review_gate.rs` — [NEW] `gestalt-router/src/review_gate.rs`

**Objective:** After `integrate_branches()` succeeds, freeze the exact candidate bytes, classify risk, run bounded lenses, and emit a receipt that `pre-merge` validates.

**Current state:** nothing — merge proceeds unconditionally (`integrate.rs:117 integrate_branches`).

**Design approach:**

```rust
// review_gate.rs — core types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateReceipt {
    pub run_id: String,
    pub candidate_sha256: String,   // frozen bytes hash
    pub tier: GateTier,
    pub lenses: Vec<LensResult>,
    pub verdict: GateVerdict,       // approved | denied | pending
    pub issued_at: String,          // RFC3339
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GateTier { Tier0, Tier1, Tier2 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LensResult {
    pub lens: String,               // "diff-shape" | "auth-surface" | "test-evidence"
    pub pass: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GateVerdict { Approved, Denied, Pending }

pub fn classify_tier(files: &[PathBuf], diff_stat: &str) -> GateTier {
    // Tier2: touches auth/security/crypto/secrets paths OR contains
    //        secrets pattern (XAVIER_TOKEN, api_key, password).
    // Tier1: touches test/impl of core modules, or diff > 200 lines.
    // Tier0: docs, config-only, mechanical.
}

pub fn freeze_candidate(worktree: &Path) -> Result<String, RouterError> {
    // git hash-object of the staged tree → candidate_sha256
}

pub fn emit_receipt(receipt: &GateReceipt) -> Result<(), RouterError> {
    // write $GESTALT_HOME/gates/<run_id>.json (atomic write + rename)
}

pub fn validate_gate(run_id: &str, gate: DeliveryGate) -> Result<GateVerdict, RouterError> {
    // discover receipt, verify candidate_sha256 still matches, return verdict
}
```

**Lenses (v1):**
1. `diff-shape` — additions/deletions/renames; detects large deletions or binary blobs.
2. `auth-surface` — grep for touched files in `auth|security|crypto|secrets|token`.
3. `test-evidence` — presence of tests for changed modules (reuses `verify-pipeline.sh` signals).

**Integration:** called from `router.rs` after `integrate_branches()`, gated by `--review-gate` flag on `gestalt run`. On `Tier2` → require explicit `gestalt gate approve <run_id>` (or lens result from agent-cli-high) before merge proceeds.

**Backward compatibility:** flag defaults off → existing runs identical.

---

### 4.2 `gate.rs` — [NEW] `gestalt-router/src/gate.rs`

**Objective:** Delivery boundary validation. Mirrors gentle-ai gates (`post-apply`, `pre-commit`, `pre-push`, `pre-pr`, `release`) but for Gestalt: `pre-merge` (v1), `pre-push`, `pre-pr` (v2).

```rust
pub enum DeliveryGate { PreMerge, PrePush, PrePr }

pub fn gate_verdict(receipt: &GateReceipt, gate: DeliveryGate) -> Result<GateVerdict, RouterError> {
    // PreMerge: receipt must exist + verdict Approved + candidate hash matches HEAD tree.
    // PrePush/PrePr: same receipt, re-validated against pushed/PR head.
}
```

**Integration:** `gestalt merge --gate pre-merge` / `gestalt pr --gate pre-pr`. CLI additions in `Commands` enum.

---

### 4.3 `api.rs` — [NEW] `gestalt-router/src/api.rs` (or extend `Commands::Serve`)

**Objective:** Read-only admin API consumed by Project Admin.

| Endpoint | Data |
|----------|------|
| `GET /api/runs` | Recent RunReports (id, task, agents, states, timestamps) |
| `GET /api/runs/<id>` | Full run + checkpoint + gate status |
| `GET /api/gates` | Open/closed gate receipts |
| `GET /api/health` | Mirror of doctor.rs output |
| `GET /api/features` | Read `.gitcore/features.json` of current workspace |

**Implementation:** `Commands::Serve` (main.rs:673) already spawns an HTTP server; add routes + a `RouterState` holding `Arc<RunManifest>` and gates dir.

---

### 4.4 `skill_registry.rs` — [NEW] `gestalt_cli` (or gestalt_core)

**Objective:** `gestalt skill-registry refresh` indexes `~/.hermes/skills/` (100+ skills) + project conventions into `~/.hermes/skills/index.json` for agent routing.

```rust
pub struct SkillIndexEntry {
    pub name: String,
    pub path: String,
    pub description: String,
    pub tags: Vec<String>,
    pub category: Option<String>,
}

pub fn refresh_index(skills_root: &Path) -> Result<Vec<SkillIndexEntry>, RouterError> {
    // walk skills_root/**/SKILL.md, parse YAML frontmatter (name, description, tags)
}

pub fn match_skills(query: &str, index: &[SkillIndexEntry]) -> Vec<SkillIndexEntry> {
    // token-overlap scoring on name+description+tags
}
```

**Integration:** CLI command `skill-registry` in `Commands` enum; used by Project Admin `/api/skills` and by Hermes routing.

---

### 4.5 Project Admin — [MODIFY] `~/proyectosSWAL/project-admin/`

**Objective:** consume Gestalt API; add Runs + Gates views.

| Change | File | Detail |
|--------|------|--------|
| Gestalt client | `server.py` | `gestalt_api()` — fetch `:8081/api/runs`, `/api/gates` with short timeout, cache 5s |
| New endpoints | `server.py` | `/api/gestalt/runs`, `/api/gestalt/gates` (proxy) |
| Runs view | `index.html` + `app.js` | Table: run id, task, agents, state badge, gate status |
| Gates view | `index.html` + `app.js` | Receipts: run, tier, verdict, lenses, timestamp |
| WS live events | `app.js` | `new WebSocket('ws://localhost:8081/ws')` → prepend event rows (best-effort, graceful fallback if closed) |

**Backward compatibility:** all existing endpoints unchanged; Gestalt connection is optional (dashboard works degraded if Gestalt down).

---

## 5. File Island Mapping

| # | File | Status | Change Type | Est. Δ | Risk |
|---|------|--------|-------------|--------|------|
| 1 | `gestalt-router/src/review_gate.rs` | **NEW** | New module | +250 | **Medium** (new logic, isolated) |
| 2 | `gestalt-router/src/gate.rs` | **NEW** | New module | +120 | Low |
| 3 | `gestalt-router/src/api.rs` | **NEW** | New module | +180 | **Medium** (HTTP surface) |
| 4 | `gestalt-router/src/router.rs` | Modify | Call `review_gate` after integrate when `--review-gate` | +25 | **High** (core executor — must not break existing runs) |
| 5 | `gestalt-router/src/lib.rs` | Modify | `pub mod review_gate; pub mod gate; pub mod api;` | +5 | Low |
| 6 | `gestalt_cli/src/main.rs` | Modify | New `Commands` variants: `Gate`, `SkillRegistry`; `--review-gate` flag on Run | +60 | Medium |
| 7 | `gestalt_core/src/application/agent/registry.rs` | Unchanged | — | 0 | — |
| 8 | `gestalt-ws/src/server.rs` | Unchanged | (api.rs may reuse) | 0 | — |
| 9 | `.gitcore/features.json` | Modify | New features: `feat-review-gates`, `feat-admin-api`, `feat-skill-registry` | +3 entries | Low |
| 10 | `docs/SRS/REQUIREMENTS.md` | Modify | REQ-019/020/021 for gates, api, registry | +60 | Low |
| 11 | `~/proyectosSWAL/project-admin/server.py` | Modify | Gestalt client + proxy endpoints | +80 | Low |
| 12 | `~/proyectosSWAL/project-admin/static/app.js` | Modify | Runs/Gates views + WS | +120 | Low |
| 13 | `~/proyectosSWAL/project-admin/static/index.html` | Modify | New sections | +40 | Low |

**Files NOT changed (backward compat):**
- `gestalt-state/` — AgentState untouched
- `gestalt-merge/` — merge flow untouched
- `gestalt_core/` — VFS/ports untouched
- All existing `Commands::Run` consumers — flag defaults off

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| R1: `router.rs` change breaks existing runs | Medium | **High** | Flag `--review-gate` defaults OFF; run full `cargo test -p gestalt-router` before/after; keep call site isolated behind `if` |
| R2: Gate blocks merge with false positive (Tier2 on benign change) | Medium | Medium | Tier classifier whitelists common false positives (config paths); `gestalt gate approve` manual override with reason |
| R3: API server (:8081) conflicts with existing port | Low | Medium | Reuse `Commands::Serve` host/port args; document default; make port configurable |
| R4: Project Admin fetches Gestalt when down → degraded UI | Medium | Low | 5s timeout + cached fallback + graceful "Gestalt offline" badge |
| R5: Receipt hash drifts after amend/force-push | Low | **High** | `validate_gate` re-hashes HEAD tree; mismatch → `Denied` with clear recovery (re-freeze) |
| R6: Skill registry index staleness | Medium | Low | `refresh` on demand + post-commit hook (optional) |
| R7: WebSocket event flood in Project Admin | Low | Low | Client-side dedup + max N rows rendered |
| R8: gentle-ai pattern over-copied (ceremony > value) | Medium | Medium | Tier0 default for docs → zero friction; gates opt-in per run (D7) |

---

## 7. Implementation Phasing

### P0 — Foundation (independently deployable, no behavior change)

| Step | Files | Effort |
|------|-------|--------|
| 1. `review_gate.rs` core types + `classify_tier` + `freeze_candidate` | NEW | 60 min |
| 2. `gate.rs` + `validate_gate` | NEW | 30 min |
| 3. Unit tests for tier classification + receipt round-trip | NEW | 30 min |
| 4. `cargo test -p gestalt-router` green | — | 10 min |

### P1 — Router integration (opt-in)

| Step | Files | Effort |
|------|-------|--------|
| 5. Wire `--review-gate` into `Commands::Run` + router.rs call site | router.rs, main.rs | 40 min |
| 6. `gestalt gate status/approve/deny` CLI | main.rs | 40 min |
| 7. Integration test: run with gate → receipt → approve → merge | tests/ | 40 min |

### P2 — Admin API + dashboard

| Step | Files | Effort |
|------|-------|--------|
| 8. `api.rs` endpoints (runs, gates, health, features) | NEW | 60 min |
| 9. Project Admin: gestalt client + Runs/Gates views + WS | server.py, app.js, index.html | 60 min |
| 10. E2E: dashboard shows live run + gate verdict | — | 20 min |

### P3 — Skill registry + docs

| Step | Files | Effort |
|------|-------|--------|
| 11. `skill-registry refresh/match` CLI | NEW (gestalt_cli) | 40 min |
| 12. features.json + REQUIREMENTS.md (REQ-019..021) + this doc | .gitcore, docs/SRS | 30 min |
| 13. `verify-pipeline.sh` port to gestalt repo | .gitcore/scripts/ | 20 min |

Each phase is independently deployable; P0 leaves the system byte-identical.

---

## 8. Appendices

### A. GateTier classification (v1)

```rust
const TIER2_PATHS: &[&str] = &[
    "auth", "security", "crypto", "secrets", "token", "password", "keyring",
    "pro_gate", "polygon_anchor", "node_identity",
];
const TIER2_PATTERNS: &[&str] = &[
    r"(?i)api[_-]?key", r"(?i)password", r"(?i)secret", r"XAVIER_TOKEN",
];

pub fn classify_tier(files: &[PathBuf], diff_stat: &str) -> GateTier {
    let touches_tier2 = files.iter().any(|f| {
        let s = f.to_string_lossy().to_lowercase();
        TIER2_PATHS.iter().any(|p| s.contains(p))
    });
    if touches_tier2 {
        return GateTier::Tier2;
    }
    let lines_changed = parse_diff_stat(diff_stat);
    if lines_changed > 200 || files.len() > 5 {
        GateTier::Tier1
    } else {
        GateTier::Tier0
    }
}
```

### B. Receipt JSON schema

```json
{
  "run_id": "run_20260804_1234",
  "candidate_sha256": "9f86d081884c7d659a2feaa0c55ad015...",
  "tier": "Tier1",
  "lenses": [
    {"lens": "diff-shape", "pass": true, "detail": "3 files, +120/-15"},
    {"lens": "auth-surface", "pass": true, "detail": "no auth paths touched"},
    {"lens": "test-evidence", "pass": false, "detail": "no tests for changed module"}
  ],
  "verdict": "Pending",
  "issued_at": "2026-08-04T12:34:56Z"
}
```

### C. Project Admin API additions

```
GET /api/gestalt/runs     → proxy :8081/api/runs      (5s timeout, 5s cache)
GET /api/gestalt/gates    → proxy :8081/api/gates
WS   ws://localhost:8081/ws → live run/gate events (optional client)
```

---

## Verification Checklist

- [x] Problem grounded in code (`integrate.rs:117` unconditional merge; `doctor.rs` runs dir convention)
- [x] Baseline diagram with real modules
- [x] Target topology with new components
- [x] Component-by-component design (4.1–4.5) with Rust sketches
- [x] File island mapping (13 files, risk-annotated) + Files NOT Changed
- [x] Risk matrix (8 risks with mitigations)
- [x] Independently deployable phases (P0–P3)
- [x] Appendices with compilable Rust types + JSON schema
- [x] Saved at `docs/design/` (project documentation, not `.hermes/plans/`)
