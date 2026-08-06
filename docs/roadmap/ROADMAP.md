# Gestalt Roadmap — Full-Scope Traceability, Memory & Orchestration

> **Status:** Living document · **Created:** 2026-08-06 · **Owner:** BELA / SWAL
> **Scope:** Every capability Gestalt needs to be the definitive multi-agent
> traceability / memory / orchestration system of the SWAL ecosystem.
> **Sources (verified, do not contradict):** `GLOBAL_GOAL.md`, `ARCHITECTURE.md`,
> `docs/design/xavier-thinking-bus.md`, `agent-registry.toml`, `Cargo.toml`,
> `.gitcore/features.json`, `docs/SRS/REQUIREMENTS.md`,
> `~/.hermes/plans/gestalt-roadmap-context.md` (findings 2026-08-06),
> `~/.hermes/plans/2026-08-06-gestalt-observe-daemon.md`.

**Legend:** `[DONE]` verified working · `[PARTIAL]` code exists, incomplete/unverified · `[PENDING]` not started.

---

## 1. Vision

Gestalt is the **public technical showcase (vitrina técnica)** of the SWAL ecosystem:
a local-first, Rust multi-agent orchestrator where **every AI agent execution in the
system — orchestrated or not — is observed, recorded, synthesized, and shared**.

The end state:

1. **One timeline.** All agents (agy, kimi, opencode, claude, codex, hermes, jules,
   orca-managed or not) report state, tasks, and decisions through the Gestalt
   timeline — never directly to each other.
2. **Total interception.** Gestalt logic intercepts everything about execution
   (start, state changes, checkpoints, decisions, finish) and writes it to the
   real-time database stack: `StateDb` (SQLite, operational truth) + Xavier
   (semantic, durable, searchable).
3. **Shared working memory.** Every agent consumes that execution memory
   (PRE-run context: `kind=insight` + recent `kind=execution`) before it works,
   and contributes to it after it works.
4. **Thinking system.** Xavier is not just storage: the Thinking Loop synthesizes
   cross-run insights (`kind=insight`) that feed future agents.
5. **Real execution only.** The system operates only when there is real LLM
   execution — no heartbeat cronjobs, no empty ticks.

Gestalt is **not** a business product; it is the ecosystem's tool and its
technical proof that execution + unified timeline + shared semantic memory +
synthesis can coexist in one local system (a combination no surveyed competitor
offers — see §10).

---

## 2. Guiding Principles & Non-Negotiables

From `AGENTS.md` and `GLOBAL_GOAL.md`:

| # | Non-negotiable | Consequence for this roadmap |
|---|----------------|------------------------------|
| N1 | **VFS isolation** — agents never write outside their overlay; all I/O via `gestalt_core::ports::outbound::vfs`; no path escaping | Observe/ingest features must be read-only on foreign artifacts |
| N2 | **Deterministic state** — every run maps to `AgentState` (`Pending/Running/Success/Timeout/Crashed/NoChanges/Quarantined`); state lives in `RunManifest`/StateDb, never only in memory | Every new event source must normalize to `AgentState` |
| N3 | **Isolated integration** — agents merge via `gestalt-merge`/router flow; no direct main commits | Unchanged by observability work |
| N4 | **LLM resilience** — automatic provider failover; never panic on rate-limit/outage; clean failure states | Thinking loop is deterministic today (no LLM); any future LLM use needs the failover layer |
| N5 | **Workspace hygiene** — `gestalt_swarm` excluded (legacy, do not re-add); members: `gestalt_core`, `gestalt_cli`, `gestalt-router`, `gestalt-merge`, `gestalt-state`, `gestalt-ws`, `gestalt_mcp`, `gestalt-search`, `synapse-agentic` | New crates must be added to `Cargo.toml` deliberately |
| N6 | **Fire-and-forget ingestion** — bus must never block or crash an agent (3s timeout, fail-open, exit 0) | Observe daemon hooks follow the Orca hook guards pattern |
| N7 | **Xavier = permanent memory only** — operational state stays in StateDb | Bus events persist to StateDb first, sink to Xavier second |

**BELA requirement (verbatim intent):** *"Todos los agentes se comunican NO
directamente sino vía el timeline de gestalt: informar estados, tareas y todo;
transparente; las lógicas de gestalt interceptan TODO sobre la ejecución para
escribirlo en la realtime database (StateDb+Xavier); todos los agentes usan esa
memoria de trabajo/ejecución."*

---

## 3. System Overview (verified baseline)

| Component | Location | Status |
|-----------|----------|--------|
| Event bus server | `gestalt bus serve` → axum `:8081` (`POST /api/event`, `GET /api/events`, `/healthz`) | `[DONE]` |
| Canonical event | `BusEvent {agent, event_type, run_id, project, state, summary, metadata{llm, provider, requested_by, decision, tool_calls}, ts}` in `gestalt-router/src/event_bus.rs` | `[DONE]` |
| Durable timeline | StateDb SQLite WAL: `runs`, `agents`, `locks`, `timeline`, `file_versions` (`gestalt-state/src/statedb.rs`) | `[DONE]` |
| Xavier sink | `gestalt-router/src/xavier_sink.rs` → `kind=execution`, unique path `gestalt/bus/executions/<ts>-<run_id>` (avoids Xavier upsert-by-path collision) | `[DONE]` |
| Live broadcast | `WsRouterBridge` (`gestalt-router/src/ws.rs`) → `gestalt_ws::WsServer` `:3001` | `[DONE]` |
| Thinking loop | `gestalt-router/src/thinking.rs` (`ThinkingLoop`, `MIN_EXECUTIONS=3`, 30-min window, deterministic `InsightSynthesizer`), `gestalt thinking` | `[DONE]` |
| Dedup | SHA-256 over semantic identity + 300s window on server `created_at` (`event_bus.rs`) | `[DONE]` |
| Replay | `gestalt bus replay --after-seq <n> [--dry-run]` (`gestalt_cli/src/main.rs`) | `[DONE]` |
| Router↔bus | `router.execute()` emits `run_started`/`run_finished` (`emit_bus_event`, incl. `requested_by`, agents, `base_sha`) | `[DONE]` |
| Orchestration | Router + worktrees + VFS + Ola-5 patterns: `AtomicCheckpointer`, `ProcessReaper`, `SerialMergeQueue`, `CleanSlateRetry`, `WriteSetValidator`, `TransactionalStateDb` | `[DONE]` |
| MCP | `gestalt_mcp` (18 tools: `gestalt_agent_run`, `analyze_project`, `search_code` BM25, `task_create/list`, `shell_execute`, git ops, belief graph) — stdio + HTTP | `[DONE]` |
| Agent registry | `agent-registry.toml` (agy, kimi, agent-cli-high/low, opencode, hermes, gestalt, tiny agents; providers openrouter/local/opencode/hermes) | `[DONE]` |
| Xavier | HTTP `:8006` + MCP stdio + CLI; DB `data/vec-store.sqlite3` | `[DONE]` (external) |

Known-good verification commands:

```bash
cargo test -p gestalt-router
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
gestalt bus serve            # :8081
curl -s localhost:8081/healthz
gestalt bus push --agent test --event run_finished --summary "e2e probe"
gestalt thinking             # requires Xavier :8006 + ≥3 executions in window
```

---

## 4. Pillars & Features

Five pillars, feature IDs `FEAT-GT-XXX`. Paths and commands are concrete.

### Pillar A — Orchestration & Isolated Execution

The engine: concurrent agent runs, file isolation, conflict control, merge.

| ID | Feature | Status | Evidence / Target |
|----|---------|--------|-------------------|
| FEAT-GT-001 | Run lifecycle (`create_run` → execute → `finalize_run`) with `RunSpec`/`AgentSpec` | `[DONE]` | `gestalt-router/src/router.rs`, `run.rs`, `run_state.rs` |
| FEAT-GT-002 | 3-layer state backend (MemState DashMap → StateDB SQLite WAL → Xavier) | `[DONE]` | `gestalt-state/src/{statedb,memstate,schema}.rs` |
| FEAT-GT-003 | VFS isolation: `VirtualFS` trait, `MemoryFs`, `OverlayFs`, `StateDbVfs` (SHA-256 versions, block edits, diffs) | `[DONE]` | `gestalt_core/src/ports/outbound/vfs.rs`, `gestalt-state/src/virtual_fs.rs` |
| FEAT-GT-004 | File locks + overlap/conflict detection (MemState locks, `OverlapDetector`, `LiveConflictDetector`) | `[DONE]` | `gestalt-router/src/overlap.rs` |
| FEAT-GT-005 | Isolated integration: `SerialMergeQueue`, `CleanSlateRetry`, `WriteSetValidator`, `AtomicCheckpointer` (git-aware rollback), `ProcessReaper` (cgroups SIGTERM) | `[DONE]` | `router.rs`, `integrate.rs`, `worktree.rs`, `checkpoint.rs`, `agent.rs` |
| FEAT-GT-006 | Capability-based routing (`CapabilityMatch`) from `agent-registry.toml` | `[DONE]` | `agent-registry.toml` `[routing]` |
| FEAT-GT-007 | Doctor / orphaned-run cleanup (`gestalt doctor`) | `[DONE]` | `gestalt-router/src/doctor.rs` |
| FEAT-GT-008 | Real-time conflict detection hardening (issue G4: timed out) | `[PENDING]` | `GLOBAL_GOAL.md` known issues |
| FEAT-GT-009 | Broken test recovery (issue T1: `agent_tests`, `doctor_tests`, `router_tests` imports; T2: clippy in `gestalt-merge`/`synapse-agentic`; G3: ignored `test_try_lock_exclusive`) | `[PENDING]` | `GLOBAL_GOAL.md` known issues |
| FEAT-GT-010 | LLM provider resilience layer: automatic failover (OpenRouter → local/Ollama → alt), structured error capture, clean `Timeout`/`Crashed` states | `[PENDING]` | AGENTS.md non-negotiable #4 |

### Pillar B — Universal Event Bus & Traceability

The nervous system: one ingress for every agent event, durable + semantic.

| ID | Feature | Status | Evidence / Target |
|----|---------|--------|-------------------|
| FEAT-GT-011 | Bus ingress `gestalt bus serve` (`POST /api/event`, `GET /api/events`, `/healthz` on `:8081`) | `[DONE]` | `event_bus.rs`; verified 2026-08-06 (6 events in Xavier `vec-store.sqlite3`) |
| FEAT-GT-012 | Canonical `BusEvent` schema (event types: `run_started`, `run_finished`, `agent_state`, `checkpoint`, `decision`) | `[DONE]` | `event_bus.rs` |
| FEAT-GT-013 | Xavier real-time sink (`kind=execution`, unique path per event) | `[DONE]` | `xavier_sink.rs` (commit `5063c89` E2E) |
| FEAT-GT-014 | Router↔bus integration (orchestrated runs auto-traced) | `[DONE]` | `router.rs` `emit_bus_event` |
| FEAT-GT-015 | Dedup (SHA-256 semantic identity, 300s server-clock window) | `[DONE]` | `event_bus.rs`; E2E identical push → `deduped=true` |
| FEAT-GT-016 | Replay cursor (`gestalt bus replay --after-seq`, `--dry-run`) | `[DONE]` | `gestalt_cli/src/main.rs` |
| FEAT-GT-017 | Secret/token filter: skip or redact summaries containing `XAVIER_TOKEN`, `password`, key material before sink/WS | `[PENDING]` | Design doc risk R7 mitigation |
| FEAT-GT-018 | SRS + feature tracking sync: REQ-022/REQ-023 present (`docs/SRS/REQUIREMENTS.md`), `feat-event-bus`/`feat-xavier-thinking` present (`.gitcore/features.json`); keep both current as features land | `[PARTIAL]` | Exists; needs updates per phase |
| FEAT-GT-019 | Multi-project scoping: `project` as first-class filter on `GET /api/events`, per-project namespaces in Xavier paths | `[PENDING]` | — |
| FEAT-GT-020 | Bus auth (optional localhost token) — only if bus is exposed beyond localhost | `[PENDING]` | Design doc non-goal today |

### Pillar C — Memory & Thinking (Xavier)

The brain: execution memory in, synthesized insight out, context fed back.

| ID | Feature | Status | Evidence / Target |
|----|---------|--------|-------------------|
| FEAT-GT-021 | Thinking Loop: recent executions → deterministic synthesis → idempotent `kind=insight` at `gestalt/thinking/<date>`; gate `MIN_EXECUTIONS=3`, 30-min window; `gestalt thinking` | `[DONE]` | `thinking.rs`; E2E 27 executions → insight indexed 2026-08-05 |
| FEAT-GT-022 | Deterministic synthesizer (no external LLM — BELA decision 2026-08-05: zero service dependencies) | `[DONE]` | `StructuralSynthesizer` behind `InsightSynthesizer` trait |
| FEAT-GT-023 | Xavier client (`search`/`add`/`stats`/`health`, `X-Xavier-Token`) | `[DONE]` | `gestalt_core/src/application/agent/xavier/client.rs` |
| FEAT-GT-024 | PRE-run context feed: `search(kind=insight + execution tail)` → `XAVIER_CONTEXT` env for launched agents (`gestalt xavier cycle "task" --agent "cmd"`) | `[PARTIAL]` | Works for Gestalt-launched runs; not yet universal |
| FEAT-GT-025 | POST-run archival (`kind=run_result`) | `[DONE]` | `XavierClient.archive_run` |
| FEAT-GT-026 | `kind=decision` memories (approved gate receipts, architectural decisions) | `[PENDING]` | Design doc appendix B |
| FEAT-GT-027 | Thinking trigger policy: execution-gated cron (run only when ≥ `MIN_EXECUTIONS` new signal since last insight — never empty ticks) | `[PENDING]` | SWAL philosophy §1.5 |
| FEAT-GT-028 | Insight quality loop: dashboard review/approve of insights; promote approved → `kind=decision` | `[PENDING]` | — |

### Pillar D — Real-Time Observability

The eyes: live timeline, dashboards, debug-grade history, interop.

| ID | Feature | Status | Evidence / Target |
|----|---------|--------|-------------------|
| FEAT-GT-029 | WebSocket live broadcast of timeline + bus events (`:3001`) | `[DONE]` | `gestalt-ws`, `WsRouterBridge` |
| FEAT-GT-030 | StateDb timeline (`StateDbEventLog`, `Event` enum: RunStarted, AgentStateChanged, Checkpoint, Overlap, MergeConflict…) | `[DONE]` | `gestalt-router/src/timeline.rs` |
| FEAT-GT-031 | Dashboard Events view (project-admin): `GET /api/gestalt/events?since=<ts>` proxy + live WS panel | `[PENDING]` | Design doc §4.5 |
| FEAT-GT-032 | Dashboard Thinking view: `/api/gestalt/thinking` (recent `kind=insight`) | `[PENDING]` | Design doc §4.5 |
| FEAT-GT-033 | Flight-recorder timeline: reconstruct one run/conversation end-to-end by `run_id` (Honeycomb pattern) — `gestalt timeline show <run_id>` | `[PENDING]` | Data already in StateDb; needs query/CLI |
| FEAT-GT-034 | OTel GenAI interop: map `BusEvent` → `metadata.otel` (span types `invoke_agent`/`execute_tool`/`chat`, `gen_ai.*` attrs, conversation ID = `run_id`) | `[PENDING]` | OTel GenAI semconv (Development status) |
| FEAT-GT-035 | WS auth (token) for `:3001` | `[PENDING]` | `GLOBAL_GOAL.md` recommended steps |

### Pillar E — Ecosystem Integration & Autonomous Observation

The reach: see every agent on the machine, with or without its cooperation.

| ID | Feature | Status | Evidence / Target |
|----|---------|--------|-------------------|
| FEAT-GT-036 | Agent registry (capabilities, rate limits, providers, tiny agents) | `[DONE]` | `agent-registry.toml` |
| FEAT-GT-037 | MCP server (18 tools, stdio + HTTP) | `[DONE]` | `gestalt_mcp` → `~/.local/bin/gestalt-mcp` |
| FEAT-GT-038 | Hermes↔Gestalt protocol (`POST /v1/orchestrate`) | `[DONE]` | `docs/hermes-gestalt-protocol.md` |
| FEAT-GT-039 | **Observe daemon** `gestalt observe` — system-wide agent detection without agent instruction. Plan: `~/.hermes/plans/2026-08-06-gestalt-observe-daemon.md` | `[PENDING]` | Phase 0 verified (below) |
| FEAT-GT-040 | Observe source 1: discovery + merge-safe hook injection (opencode plugin JS, codex `hooks.json`, claude `settings.json`; guards: timeout ≤10s, fail-open, exit 0) | `[PENDING]` | 9 agents in PATH, 7 config dirs verified |
| FEAT-GT-041 | Observe source 2: `/proc` poll (5s) with exact-cmdline matching (filters ~10 generic node/python processes) | `[PENDING]` | Phase 0 noise census |
| FEAT-GT-042 | Observe source 3: Orca bridge (`~/.config/orca/agent-hooks/endpoint.env` → `ORCA_AGENT_HOOK_PORT=42423` + token; Orca already hooks claude/codex/kimi/copilot/cursor/antigravity in `~/.orca/agent-hooks/`) | `[PENDING]` | Verified on disk |
| FEAT-GT-043 | Observe source 4: artifact ingest (Claude `projects/*.jsonl`, Hermes session DB, Jules via GitHub API) | `[PENDING]` | — |
| FEAT-GT-044 | `event.py` one-line push helper in project-admin (Hermes/Python agents → bus, fire-and-forget, 3s timeout, never raises) | `[PENDING]` | Design doc §4.4 |
| FEAT-GT-045 | WASM runner: `gestalt-wasm` → `swal-agent-runner` (WebContainer browser node); currently on disk, not a workspace member | `[PENDING]` | `gestalt-wasm/` |
| FEAT-GT-046 | GitHub sync (force-push clean main; CI visibility for the showcase) | `[PENDING]` | `GLOBAL_GOAL.md` Fase 3 |

---

## 5. User Stories

**Orchestrator operator (BELA):**
- US-01: As the ecosystem operator, I want every agent run on the machine to appear in one timeline, so that I can audit who/which-llm/which-provider made every decision without opening each agent's silo.
- US-02: As the operator, I want the timeline to flow into Xavier automatically, so that execution history becomes searchable semantic memory with zero manual steps.
- US-03: As the operator, I want `gestalt observe` to detect agents I did not launch, so that shadow/ad-hoc agent activity is never invisible.
- US-04: As the operator, I want a flight recorder per run (`gestalt timeline show <run_id>`), so that I can debug a failed run step by step.
- US-05: As the operator, I want the system idle when nothing runs (no heartbeat ticks), so that memory and logs contain only real LLM activity.

**Agents (agy, kimi, opencode, claude, codex, jules, hermes…):**
- US-06: As an agent, I want recent insights and execution context injected before I start (`XAVIER_CONTEXT`), so that I benefit from everything previous runs learned.
- US-07: As an agent, I want to push my state changes with one fire-and-forget call, so that reporting never blocks or fails my actual work.
- US-08: As an agent orchestrated by Gestalt, I want my work isolated in a VFS overlay with declared write-scope, so that I cannot corrupt other agents or the main branch.

**Hermes (peer orchestrator):**
- US-09: As Hermes, I want `POST /v1/orchestrate` to delegate parallel work to Gestalt, so that I keep conversation flow while Gestalt handles isolation and merge.
- US-10: As Hermes, I want a one-line `event.py push(...)` in my workflows, so that my complex tasks land in the shared timeline.

**Dashboard viewer:**
- US-11: As a viewer, I want live Events and Thinking panels in project-admin, so that I can watch executions and synthesized insights in real time.

**Future maintainer:**
- US-12: As a maintainer, I want BusEvents exportable as OTel GenAI spans, so that Gestalt interops with standard observability tooling.
- US-13: As a maintainer, I want provider failover built into every LLM call, so that an upstream outage degrades gracefully to `Timeout` instead of crashing runs.

---

## 6. Critical Paths (end-to-end)

**CP-1 — Orchestrated run → memory → insight (the spine, DONE today):**
`gestalt xavier cycle "task" --agent "cmd"` → Router `create_run` (StateDb `runs`) →
worktree + OverlayFs + `WriteSetValidator` → agent executes (PRE `XAVIER_CONTEXT`) →
`AtomicCheckpointer` commit → `emit_bus_event(run_started/run_finished)` →
StateDb `timeline` + WS `:3001` + `XavierEventSink` (`kind=execution`) →
`gestalt thinking` (≥3 execs/30min) → `kind=insight` → next run's PRE search.

**CP-2 — Unmanaged agent observation (PENDING, the missing half):**
`gestalt observe` starts → discovery finds agent (PATH/config scan) →
source fires (injected hook | `/proc` cmdline match | Orca `:42423` | artifact tail) →
normalizes to `BusEvent` + `AgentState` → `POST :8081/api/event` → CP-1 tail
(StateDb → Xavier). Guards: ≤10s, fail-open, exit 0 — the observed agent must
never be harmed by being observed.

**CP-3 — Outage recovery:**
Xavier down → sink fails → events stay durable in StateDb →
`gestalt bus replay --after-seq <cursor>` re-sinks → SHA-256 dedup drops duplicates →
Xavier consistent. (Mechanism `[DONE]`; operational automation `[PENDING]`.)

**CP-4 — Parallel execution → safe integration:**
N agents in isolated worktrees → MemState locks + `LiveConflictDetector` →
`SerialMergeQueue` merges branches one-by-one into integration branch →
on conflict: rollback + `CleanSlateRetry` from clean base → PR/merge flow.
No direct commits to main (N3).

**CP-5 — Peer orchestration:**
Hermes `POST /v1/orchestrate` → Gestalt plans per `agent-registry.toml`
capabilities → CP-1 executes → results + timeline back to Hermes.

---

## 7. Phases (dependencies & exit criteria)

### P0 — Universal Event Bus `[DONE]`
Scope: FEAT-GT-011..013. Dependencies: StateDb timeline (foundation, pre-roadmap DONE).
Delivered: `event_bus.rs`, `xavier_sink.rs`, `gestalt bus serve/push`, StateDb-durable
timeline, E2E POST → StateDb → Xavier `kind=execution` (commit `5063c89`).

### P1 — Thinking Loop `[DONE]`
Scope: FEAT-GT-021..023. Dependencies: P0 sink (needs `kind=execution` corpus).
Delivered: `thinking.rs`, deterministic synthesis (no external LLM), idempotent
`kind=insight`, `gestalt thinking` (requires Xavier `:8006`).

### P2 — Autonomous Observation + Ecosystem Push `[NEXT]`
Scope: FEAT-GT-039..044, 024 (universal PRE feed), 009, 008.
Dependencies: P0 (bus ingress), Orca endpoint config on disk, hook-injection targets verified.
Work:
1. `gestalt observe` skeleton + source 2 (`/proc` poll, exact cmdline) — fastest win, zero agent changes.
2. Source 3 (Orca bridge `:42423`) — reuse existing hooks for 6 agents.
3. Source 1 (merge-safe hook injection: opencode plugin JS, codex `hooks.json`, claude `settings.json`).
4. `event.py` in project-admin + Hermes workflow hook (post complex task).
5. Fix T1/T2/G3 test debt; harden G4 real-time conflict detection.
Exit criteria:
- Running `opencode`/`claude` outside Gestalt produces a `BusEvent` in `GET /api/events` within ≤10s.
- Hooked agents pass their own test suites; hooks fail-open (kill bus → agent unaffected).
- `cargo test -p gestalt-router` green; `cargo clippy --all-targets -- -D warnings` clean.

### P3 — Visibility, Interop & Hardening
Scope: FEAT-GT-031..034, 017, 019, 026, 027, 028, 046.
Dependencies: P2 (needs event volume to be worth visualizing).
Work:
1. Dashboard Events + Thinking views (project-admin `server.py` proxies + `app.js` panels, WS live).
2. Secret/token filter in sink + WS path (R7).
3. `gestalt timeline show <run_id>` flight recorder.
4. OTel GenAI mapping (`metadata.otel`, `gen_ai.*`, span-per-step).
5. `kind=decision` + execution-gated thinking cron (no empty ticks).
6. GitHub sync / clean main push.
Exit criteria:
- Dashboard renders live bus events + latest insight from real data.
- A push containing `XAVIER_TOKEN` is redacted/skipped (test).
- `gestalt timeline show` reconstructs a real run's full event sequence from StateDb.
- BusEvent→OTel export validates against GenAI semconv attribute names.

### P4 — Full Autonomy, Scale & Showcase
Scope: FEAT-GT-043, 045, 010, 020, 035, 008-hardening.
Dependencies: P3 (visibility proves the loop before scaling it).
Work:
1. Observe source 4: artifact ingest (Claude `projects/*.jsonl` tail, Hermes session DB, Jules GitHub API).
2. LLM provider failover layer for any future LLM caller (N4).
3. Multi-project namespaces + optional bus/WS auth.
4. `gestalt-wasm` workspace integration → `swal-agent-runner` (WebContainer).
5. Public showcase polish: README/QUICKSTART E2E demo, reproducible NixOS build notes
   (system openssl `PKG_CONFIG_PATH`, RAM target dir workaround).
Exit criteria:
- A Claude Code session and a Jules task appear in the timeline with zero user action.
- Provider outage simulation yields clean `Timeout`/failover, never panic.
- `gestalt-wasm` builds in workspace and runs a minimal agent in browser node.

---

## 8. Global Acceptance Criteria

1. **Traceability completeness:** for any run (orchestrated or observed), StateDb contains
   `run_started → agent_state* → run_finished` with `agent`, `llm`, `provider`, `state`;
   Xavier contains the matching `kind=execution` memories.
2. **No agent harm:** ingestion is fire-and-forget (≤3s bus push, ≤10s hooks, fail-open, exit 0);
   no observed agent ever fails because the bus/observe/Xavier is down.
3. **Durability:** kill Xavier → zero events lost (StateDb is source of truth; replay reconciles).
4. **Determinism:** every run maps to exactly one terminal `AgentState`; insights are
   reproducible (deterministic synthesizer; idempotent indexing).
5. **Real-activity-only:** no cronjob produces events/insights without preceding real executions.
6. **Quality gates:** `cargo test -p gestalt-router`, `cargo fmt --all --check`,
   `cargo clippy --all-targets -- -D warnings` all green on every phase exit.
7. **Privacy:** no secret material reaches Xavier or WS (token filter tested).

---

## 9. Risks

| # | Risk | L | I | Mitigation |
|---|------|---|---|------------|
| R1 | Event flood → Xavier write amplification | M | M | Batched sink drain; dedup window; per-agent rate caps in registry |
| R2 | Low-quality synthesized insights | M | M | Deterministic synthesizer (no LLM variance); `MIN_EXECUTIONS=3` gate; dashboard human review (FEAT-GT-028) |
| R3 | Agents forget to push events | M | L | Observe daemon sources 1–4 make reporting automatic; `event.py` is one line |
| R4 | Bus down (`:8081`) → agents blocked | L | H | Fire-and-forget clients, 3s timeout, never raise (N6) |
| R5 | Xavier down → events lost | M | M | StateDb durable + `gestalt bus replay` cursor (E2E proven) |
| R6 | Duplicate insights | L | L | Idempotency check (today's `kind=insight` before write) |
| R7 | Secrets leak into shared memory | M | M | Token/secret filter (FEAT-GT-017); summaries carry state + one line, never payloads |
| R8 | Dashboard WS flood | L | L | Client-side cap (last 200 rows) |
| R9 | `/proc` poll noise (~10 generic node/python processes) → false agent detections | H | M | Exact-cmdline allowlist from discovery; registry-driven matching |
| R10 | Agent config drift breaks injected hooks (opencode/codex/claude formats change) | M | M | Merge-safe injection (never clobber), format-version tests, fail-open guards |
| R11 | Orca endpoint (`:42423`) unavailable/version skew | M | L | Bridge is optional source; degrade to other sources |
| R12 | NixOS build pitfalls (system openssl `PKG_CONFIG_PATH`, glibc 2.42 PLT ABI) block contributors | M | M | Documented in QUICKSTART; RAM target dir workaround |
| R13 | Legacy `~/.local/bin/gestalt` (740MB) `serve` panics — mistaken for current binary | M | M | Do not use as MCP; rebuild/replace in P4; docs warning |
| R14 | Scope creep beyond verified facts | M | M | This roadmap only lists features traceable to context/design-doc/repo evidence |

---

## 10. Gap Analysis vs. the Field (research 2026-08-06)

| System | What it does | Gestalt position | Action |
|--------|--------------|------------------|--------|
| **Langfuse** (OSS, self-hosted) | Traces Claude Code/Codex via lifecycle hooks, Copilot via OTel, Cursor/OpenCode via integrations | Direct competitor of the observability layer; Gestalt wins on data sovereignty (local-first, SWAL-aligned) and on having orchestration + memory in the same system | Close hook-coverage parity via FEAT-GT-040..043 |
| **OTel GenAI SemConv** (Development) | Standard spans `invoke_agent`/`execute_tool`/`chat`/`create_agent`, `gen_ai.*` attrs, span-per-step + conversation ID | Gestalt `BusEvent` is richer than a span but not interoperable | FEAT-GT-034: emit `metadata.otel` mapping (`run_id` = conversation ID) |
| **Honeycomb Agent Timeline** | Flight recorder tied to conversation ID; timeline as debugging tool | Gestalt stores the data but has no reconstruction UX | FEAT-GT-033 `gestalt timeline show <run_id>` |
| **AgentOps** | Decision tracking — captures the *why* | Gestalt has `decision` event type + metadata but no durable decision memory | FEAT-GT-026 `kind=decision` |
| **Orca** (ADE) | Worktrees + UI + mobile, 25+ agents, MCP/hooks/skills; observes only its own managed agents (`:42423`) | Complementary, not competitor: Orca = UX/worktrees layer; Gestalt = VFS/conflicts/merge + system-wide bus. Orca is an event *source* | FEAT-GT-042 bridge; keep layers separate |
| **Claude Squad / cmux / agent-orchestrator / Vibe Kanban** | TUI/tmux/worktrees, terminal multiplexing for coding agents, kanban UX | UX alternatives; none has shared semantic memory or synthesis | No action; differentiate on memory+traceability |
| **Cursor** | Cannot orchestrate parallel subagents (validated limitation) | Confirms the market gap Gestalt fills | Keep parallel isolated execution as core strength |

**Confirmed unique gap:** no surveyed system combines **execution + unified timeline +
shared semantic memory + automatic synthesis** in one local system. That combination
*is* Gestalt's roadmap — this document.

---

## 11. Feature Index (status rollup)

- **DONE (22):** FEAT-GT-001..007, 011..016, 021..023, 025, 029, 030, 036..038
- **PARTIAL (2):** FEAT-GT-018, 024
- **PENDING (22):** FEAT-GT-008..010, 017, 019, 020, 026..028, 031..035, 039..046

(Total: 46 features = 22 DONE + 2 PARTIAL + 22 PENDING.)

Priority order for PENDING: **FEAT-GT-039 (observe daemon)** > FEAT-GT-009 (test debt)
> FEAT-GT-040..042 (observe sources) > FEAT-GT-044 (event.py) > FEAT-GT-031/032 (dashboards)
> FEAT-GT-017 (secret filter) > FEAT-GT-033 (flight recorder) > FEAT-GT-034 (OTel).

*End of roadmap.*
