# Xavier Thinking Bus — Design Document

**Status:** Draft v0.1
**Author:** Hermes Agent (on behalf of BELA)
**Date:** 2026-08-04
**Target:** Gestalt workspace + Xavier (:8006) + Hermes
**Protocol:** GitCore 3.8.0 · SWAL architecture (cores unificados)

---

## 1. Problem & Scope

### The vision (user statement)

> Hermes should also use Gestalt so that **all agents register their activity on the
> information bus**, enabling Xavier to **index in real time** and feed other agents with
> intelligent context. The purpose of the system is that **Xavier also does the thinking**,
> sharing "memories" — the full analysis of Xavier's executions.

### Current state

Today the loop is **incomplete and manual**:

1. **Agents register activity only in their own silos.** Hermes logs to `~/.hermes/logs/`;
   Jules sessions live on Google's API; agent CLI runs are ephemeral. Gestalt's timeline
   (`StateDbEventLog`) captures *only what Gestalt itself orchestrates*.
2. **Xavier is written to at discrete points** (`gestalt xavier add` POST-phase), never
   streamed. The event bus (`MemState.push_event` + `WsEvent` broadcast) is **not connected
   to Xavier** — events die in SQLite + WebSocket clients.
3. **No "thinking" layer.** Xavier stores memories but nothing *analyzes execution history*
   to produce synthesized insights ("recuerdos") that get re-indexed for future agents.

### Goals

1. **Universal event registration**: any agent (Hermes, Jules, agent CLI, Gestalt) pushes
   structured events to the Gestalt bus via a single lightweight channel.
2. **Real-time Xavier indexing**: every bus event → `POST /v1/memories` (kind=execution)
   with rich metadata (agent, run, project, state, timestamps).
3. **Xavier Thinking loop**: a periodic job queries recent execution memories, synthesizes
   cross-run insights via the local LLM (qwen3-coder), and re-indexes them as
   kind=insight/decision — the "recuerdos" shared with future agents via PRE search.

### Non-goals

- ❌ NOT replacing Xavier's own indexing — Xavier stays the memory/thinking plane.
- ❌ NOT building a new queue system — Gestalt's existing `broadcast::Sender` + SQLite
  timeline are sufficient at SWAL scale.
- ❌ NOT capturing raw file diffs in the bus (too heavy) — events carry *pointers* to
  checkpoints/diffs, which Gestalt already stores.
- ❌ NOT adding auth/encryption to the bus in this iteration (localhost trust domain).

---

## 2. Baseline Architecture

### What exists today (verified 2026-08-04)

```
                    ┌─────────────────────────────────────────┐
                    │  XAVIER :8006                            │
                    │  POST /v1/memories        (add)         │
                    │  POST /v1/memories/search (search)      │
                    │  GET /health                            │
                    └─────────────────────────────────────────┘
                        ▲ add (POST-phase only)
                        │
   ┌────────────────────┴────────────────────────────┐
   │  GESTALT                                         │
   │                                                  │
   │  timeline.rs  ── Event enum (RunStarted,         │
   │  │               AgentStateChanged, Checkpoint,  │
   │  │               Overlap, MergeConflict, ...)    │
   │  │               └─ StateDbEventLog (SQLite)     │
   │  │                                               │
   │  memstate.rs ── push_event() → broadcast tx      │
   │  │              (MemState change events)         │
   │  │                                               │
   │  ws.rs ─────── WsRouterBridge → WsEvent JSON     │
   │               (StateChanged, LockAcquired, ...)  │
   │                                                  │
   │  XavierClient ── search()/add()/stats()/health() │
   │  (gestalt_core/.../xavier/client.rs)             │
   └──────────────────────────────────────────────────┘
```

### Key files

| File | Role | Status |
|------|------|--------|
| `gestalt-router/src/timeline.rs` | `Event` enum + `EventLog` trait + `StateDbEventLog` | Existing |
| `gestalt-state/src/memstate.rs` | `push_event()` → broadcast channel | Existing |
| `gestalt-router/src/ws.rs` | `WsRouterBridge` → `WsEvent` JSON broadcast | Existing |
| `gestalt-ws/src/event.rs` | `WsEvent` enum (StateChanged, RunStarted, ...) | Existing |
| `gestalt_core/src/application/agent/xavier/client.rs` | `XavierClient` (search/add/stats/health, `X-Xavier-Token`) | Existing |
| `gestalt_core/src/application/agent/xavier/agent.rs` | `XavierAgent` (search/add_memory wrappers) | Existing |
| `gestalt_cli/src/main.rs` | `Commands::Xavier` (PRE search / POST index) | Existing |

### Gap analysis

| Capability | Needed | Exists | Gap |
|------------|--------|--------|-----|
| Event capture (run lifecycle) | ✅ | ✅ timeline.rs | — |
| Event broadcast (real-time) | ✅ | ✅ ws.rs | — |
| Event → Xavier streaming | ❌ | — | **🔴 THE GAP** |
| External agents (Hermes/Jules) push events | ❌ | — | **🔴 NEW** |
| Xavier thinking/synthesis loop | ❌ | — | **🔴 NEW** |
| Context feeding (PRE search) | ✅ | ✅ XavierClient.search | — |

---

## 3. Target Architecture

### Topology

```
                    ┌──────────────────────────────────────────────┐
                    │  XAVIER :8006                                │
                    │  ┌────────────────────────────────────────┐  │
                    │  │ memory store (SQLite+vec)              │  │
                    │  │  kind=execution  (streamed events)     │  │
                    │  │  kind=insight    (thinking loop)       │  │
                    │  │  kind=decision   (approved receipts)   │  │
                    │  └────────────────────────────────────────┘  │
                    │  Thinking loop (cron in gestalt)             │
                    └───────▲──────────────────┬──────────────────┘
                            │ POST /v1/memories│ POST /v1/memories/search
              ┌─────────────┴──────┐   ┌───────▼──────────────────┐
              │  GESTALT BUS       │   │  PRE CONTEXT FEED        │
              │                    │   │  (any agent start)       │
              │  EventSink [NEW]   │   │  search(kind=insight)    │
              │  subscribes to     │   │  + kind=execution tail   │
              │  broadcast → POST  │   │                          │
              │  to Xavier         │   └──────────────────────────┘
              │        ▲           │
              │        │ push_event│
              │  ┌─────┴──────┐    │
              │  │ event_bus  │    │   ┌──────────────────────┐
              │  │ :8081      │    │   │  THINKING LOOP [NEW] │
              │  │ HTTP POST  │◄───┼───│  gestalt thinking    │
              │  │ /api/event │    │   │  every N min:        │
              │  └─────▲──────┘    │   │  query recent exec   │
              └────────┼───────────┘   │  → LLM synthesize    │
                       │               │  → index kind=insight│
        ┌──────────────┼──────────────┘  └────────────────────┘
        │              │
  ┌─────┴─────┐  ┌─────┴──────┐  ┌──────────────┐
  │ HERMES    │  │ JULES      │  │ agent CLI    │
  │ (orchest.)│  │ (async)    │  │ (grok/kimi)  │
  │ push via  │  │ push via   │  │ push via     │
  │ /api/event│  │ /api/event │  │ /api/event   │
  └───────────┘  └────────────┘  └──────────────┘
```

### Design decisions

| # | Decision | Rationale |
|---|----------|-----------|
| B1 | One **event bus endpoint** (`POST /api/event`) — HTTP, JSON, fire-and-forget | Works for Hermes (Python), Jules (webhook), agent CLI (bash), Gestalt (native) — zero SDK needed |
| B2 | **EventSink** subscribes to the existing broadcast channel and POSTs to Xavier | Reuses `WsEvent`/`MemState` broadcast; no new event plumbing |
| B3 | Events map to Xavier `kind=execution` with metadata `{agent, run_id, project, state, ts}` | Searchable by agent/project/state via Xavier filters |
| B4 | **Thinking loop** = Gestalt cron (`gestalt thinking` subcommand) using local LLM (qwen3-coder via Ollama) | No external API cost; local-first (REQ-017 pattern) |
| B5 | Insights re-indexed as `kind=insight` with `source_path: gestalt/thinking/<date>` | PRE search surfaces them for any agent starting work |
| B6 | Bus is **append-only, best-effort**: Xavier down → events stay in SQLite timeline, retry on next tick | Never blocks agent execution (fire-and-forget) |
| B7 | Hermes integration = a thin **`event.py`** helper + optional cron | No changes to Hermes core; one import in workflows |

---

## 4. Component-by-Component Design

### 4.1 `event_bus.rs` — [NEW] `gestalt-router/src/event_bus.rs`

**Objective:** Universal ingress for events from ANY agent (Hermes, Jules, CLI, native).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub agent: String,          // "hermes" | "jules" | "agent-cli" | "gestalt"
    pub event_type: String,     // "run_started" | "agent_state" | "checkpoint" | ...
    pub run_id: Option<String>,
    pub project: Option<String>,
    pub state: Option<String>,  // Pending|Running|Success|Timeout|Crashed
    pub summary: String,
    pub metadata: serde_json::Value,
    pub ts: String,             // RFC3339
}

pub async fn handle_event(ev: BusEvent) -> Result<(), RouterError> {
    // 1. Persist to StateDb timeline (durable, survives Xavier outage)
    // 2. Broadcast to WsEvent subscribers (live dashboard)
    // 3. Fire-and-forget: EventSink forwards to Xavier (kind=execution)
    Ok(())
}
```

**Route:** `POST /api/event` (HTTP JSON, no auth on localhost) → `handle_event`.

### 4.2 `xavier_sink.rs` — [NEW] `gestalt-router/src/xavier_sink.rs`

**Objective:** Stream bus events → Xavier memory store in real time.

```rust
pub struct XavierEventSink {
    client: XavierClient,
}

impl XavierEventSink {
    pub fn new(client: XavierClient) -> Self { Self { client } }

    pub async fn sink(&self, ev: &BusEvent) -> anyhow::Result<()> {
        let content = format!(
            "[{}] {} {} — {}",
            ev.event_type, ev.agent,
            ev.run_id.as_deref().unwrap_or("?"),
            ev.summary
        );
        let metadata = json!({
            "agent": ev.agent,
            "run_id": ev.run_id,
            "project": ev.project,
            "state": ev.state,
            "event_type": ev.event_type,
            "ts": ev.ts,
        });
        self.client
            .add(&content, "gestalt/bus/executions", "execution", metadata)
            .await
    }
}
```

**Retry policy:** on failure, log `tracing::warn!` + leave event in StateDb; a sweep job
(`gestalt bus replay`) re-sinks events newer than last-synced cursor.

### 4.3 `thinking.rs` — [NEW] `gestalt-router/src/thinking.rs`

**Objective:** THE core of the vision — Xavier "thinking" over execution memories.

```rust
pub struct ThinkingLoop {
    xavier: XavierClient,
    llm: LlmClient,           // Ollama qwen3-coder (local-first)
    window_minutes: u64,      // default 30
}

impl ThinkingLoop {
    /// 1. Pull recent execution memories from Xavier
    pub async fn recent_executions(&self) -> Vec<MemoryResult> {
        self.xavier.search("kind:execution", 50, "snippet").await...
    }

    /// 2. Synthesize a cross-run insight via local LLM
    pub async fn synthesize(&self, execs: &[MemoryResult]) -> anyhow::Result<String> {
        // prompt: "Given these agent executions, what patterns, blockers,
        //          decisions, and next steps emerge? Be concise."
        // model: qwen3-coder via Ollama :11434 (REQ-017 local-first)
    }

    /// 3. Re-index as kind=insight ("recuerdo") for future PRE context
    pub async fn index_insight(&self, text: &str) -> anyhow::Result<()> {
        self.xavier
            .add(text, "gestalt/thinking/<YYYY-MM-DD>", "insight", json!({"source": "thinking-loop"}))
            .await
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let execs = self.recent_executions().await?;
        if execs.len() < 3 { return Ok(()); }  // not enough signal yet
        let insight = self.synthesize(&execs).await?;
        self.index_insight(&insight).await
    }
}
```

**Trigger:** `gestalt thinking` CLI + optional cron (every 30 min). Idempotent —
searches `kind=insight` for today first, skips if already produced.

### 4.4 `event.py` — [NEW] `~/proyectosSWAL/project-admin/event.py` (Hermes integration)

**Objective:** Let Hermes (and any Python agent) push events to the bus in one line.

```python
#!/usr/bin/env python3
"""Push an event to the Gestalt bus (fire-and-forget)."""
import json, sys, urllib.request

BUS = "http://localhost:8081/api/event"

def push(agent, event_type, summary, run_id=None, project=None, state=None, **meta):
    payload = {
        "agent": agent, "event_type": event_type, "summary": summary,
        "run_id": run_id, "project": project, "state": state,
        "metadata": meta, "ts": __import__("datetime").datetime.now().isoformat() + "Z",
    }
    req = urllib.request.Request(BUS, data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"})
    try:
        urllib.request.urlopen(req, timeout=3)
        return True
    except Exception as e:
        print(f"bus: {e}", file=sys.stderr)
        return False  # never block the caller

if __name__ == "__main__":
    # usage: python3 event.py hermes run_started "audit xavier features" --project xavier
    ...
```

**Hermes wiring (workflow hook):** after each complex task (5+ tool calls), call
`event.py push hermes run_finished "<summary>" --project <name>` — cheap, one import,
survives bus outage.

### 4.5 Bus → Project Admin (live view)

| Change | Detail |
|--------|--------|
| `GET /api/gestalt/events?since=<ts>` | Tail of bus events (from StateDb) |
| WS `/ws` (existing) | Live event stream already broadcast — dashboard subscribes |
| `/api/gestalt/thinking` | Recent insights (kind=insight from Xavier) |

---

## 5. File Island Mapping

| # | File | Status | Change Type | Est. Δ | Risk |
|---|------|--------|-------------|--------|------|
| 1 | `gestalt-router/src/event_bus.rs` | **NEW** | Event ingress + route | +150 | Low |
| 2 | `gestalt-router/src/xavier_sink.rs` | **NEW** | Bus→Xavier streaming | +90 | Low |
| 3 | `gestalt-router/src/thinking.rs` | **NEW** | LLM synthesis loop | +130 | **Medium** (LLM call, prompt quality) |
| 4 | `gestalt-router/src/api.rs` (from review-gates doc) | Modify | Add `POST /api/event`, `GET /api/events`, `GET /api/thinking` | +60 | **Medium** (HTTP surface) |
| 5 | `gestalt-router/src/lib.rs` | Modify | `pub mod event_bus; pub mod xavier_sink; pub mod thinking;` | +5 | Low |
| 6 | `gestalt_cli/src/main.rs` | Modify | `Commands::Bus`, `Commands::Thinking`; `bus replay` sweep | +80 | Medium |
| 7 | `gestalt_core/src/application/agent/xavier/client.rs` | Modify | Add `search_filter(kind, project, since)` helper | +25 | Low |
| 8 | `gestalt-ws/src/event.rs` | Modify | Add `BusEventReceived` variant (dashboard live view) | +10 | Low |
| 9 | `~/proyectosSWAL/project-admin/event.py` | **NEW** | Hermes bus client | +50 | Low |
| 10 | `~/proyectosSWAL/project-admin/server.py` | Modify | `/api/gestalt/events`, `/api/gestalt/thinking` proxies | +40 | Low |
| 11 | `~/proyectosSWAL/project-admin/static/app.js` | Modify | Events + Thinking views | +100 | Low |
| 12 | `.gitcore/features.json` (gestalt) | Modify | `feat-event-bus`, `feat-xavier-thinking` | +2 | Low |
| 13 | `docs/SRS/REQUIREMENTS.md` (gestalt) | Modify | REQ-022 (bus), REQ-023 (thinking) | +50 | Low |

**Files NOT changed:** `timeline.rs` (Event enum kept as-is), `memstate.rs`, `gestalt-merge/`,
`gestalt-state/` core, Xavier itself (no changes to :8006).

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| R1: Event flood → Xavier write amplification | Medium | Medium | Batch sink (drain channel every 500ms, one POST per batch); kind=execution dedup via `mode=dedup` on Xavier |
| R2: LLM synthesis produces low-quality "thinking" | Medium | Medium | Prompt with strict template (patterns/blockers/decisions/next); min 3 executions gate; human review via dashboard |
| R3: Hermes workflow hook forgotten | Medium | Low | Optional cron `bus-replay` reads Hermes session DB tail; low ceremony |
| R4: Bus down (Gestalt :8081) → agents blocked | Low | **High** | `event.py` is fire-and-forget with 3s timeout + stderr log; never raises |
| R5: Xavier down → events lost | Medium | Medium | StateDb timeline is durable source; `bus replay` re-sinks from cursor |
| R6: Thinking loop duplicates insights | Low | Low | Idempotent: check today's `kind=insight` before writing |
| R7: Privacy — execution summaries in shared memory | Medium | Medium | Summaries are agent-state + one-line summary, no secrets; token filter (skip summaries containing `XAVIER_TOKEN`/`password`) |
| R8: Dashboard WS event flood | Low | Low | Client-side cap (render last 200 rows) |

---

## 7. Implementation Phasing

### P0 — Bus + sink (independently deployable, no behavior change)

| Step | Files | Effort |
|------|-------|--------|
| 1. `event_bus.rs` types + `handle_event` | NEW | 45 min |
| 2. `xavier_sink.rs` + batched drain loop | NEW | 45 min |
| 3. Wire into `Commands::Serve` (`POST /api/event`) | api.rs, main.rs | 30 min |
| 4. Unit test: event → StateDb → (mock) Xavier POST | NEW | 30 min |

### P1 — Thinking loop

| Step | Files | Effort |
|------|-------|--------|
| 5. `thinking.rs` (recent → synthesize → index) | NEW | 60 min |
| 6. `gestalt thinking` CLI + cron example | main.rs | 30 min |
| 7. Integration test with Ollama qwen3-coder | tests/ | 30 min |

### P2 — Hermes + dashboard

| Step | Files | Effort |
|------|-------|--------|
| 8. `event.py` helper + Hermes workflow hook | project-admin/event.py | 20 min |
| 9. Dashboard Events + Thinking views | server.py, app.js | 45 min |
| 10. E2E: Hermes task → bus → Xavier → thinking → dashboard | — | 20 min |

### P3 — Hardening + docs

| Step | Files | Effort |
|------|-------|--------|
| 11. `bus replay` cursor + dedup + token filter | event_bus.rs | 30 min |
| 12. features.json + REQUIREMENTS.md (REQ-022/023) | .gitcore, docs/SRS | 20 min |
| 13. Reference docs: how to push from Jules (webhook) | docs/ | 20 min |

Each phase independently deployable; P0-P1 do not require P2 (Hermes) to be useful.

---

## 8. Appendices

### A. BusEvent JSON (wire format)

```json
{
  "agent": "hermes",
  "event_type": "run_finished",
  "run_id": "20260804_1500_abc",
  "project": "xavier",
  "state": "Success",
  "summary": "verify-pipeline 27/27 PASS, features.json reconciled 85.2%",
  "metadata": {"tool_calls": 42, "duration_s": 890},
  "ts": "2026-08-04T15:00:00Z"
}
```

### B. Xavier memory kinds

| kind | Content | Source | Query |
|------|---------|--------|-------|
| `execution` | One event line + metadata | EventSink (real-time) | `kind:execution project:xavier` |
| `insight` | Synthesized thinking (patterns/blockers/decisions/next) | ThinkingLoop | `kind:insight` (PRE feed) |
| `decision` | Approved gate receipts / architectural decisions | review_gate (future) | `kind:decision` |

### C. Thinking prompt template (v1)

```
You are Xavier's thinking layer. Based on these recent agent executions:

{executions}

Produce a concise insight with exactly 4 sections:
- PATTERNS: recurring behaviors or outcomes
- BLOCKERS: repeated failures or stalls
- DECISIONS: choices made and their rationale
- NEXT: recommended next steps

Keep under 200 words. No preamble.
```

### D. Hermes workflow hook (snippet)

```python
# after any complex task in Hermes workflows:
from project_admin.event import push
push("hermes", "run_finished", summary, project="xavier", state="Success",
     tool_calls=n, duration_s=d)
```

---

## Verification Checklist

- [x] Vision captured from user statement (all agents → bus → Xavier real-time → thinking)
- [x] Baseline gap analysis (event capture exists; streaming + thinking missing)
- [x] Target topology (bus :8081, EventSink, ThinkingLoop, Hermes event.py)
- [x] Component designs with compilable Rust + Python sketches (4.1–4.5)
- [x] File island map (13 files, risk-annotated) + Files NOT Changed
- [x] Risk matrix (8 risks, incl. write amplification + LLM quality)
- [x] Independently deployable phases (P0 bus/sink, P1 thinking, P2 Hermes/UI, P3 hardening)
- [x] Appendices: wire format, memory kinds, prompt template, Hermes hook

---

## 9. Implementation Status (2026-08-05)

| Phase | Status | Evidence |
|-------|--------|----------|
| P0 — Bus + sink | ✅ DONE | event_bus.rs, xavier_sink.rs, `gestalt bus serve/push`, recent_timeline; E2E POST → StateDb → Xavier kind=execution verificado (commit 5063c89) |
| P1 — Thinking loop | ✅ DONE | thinking.rs, StructuralSynthesizer (determinista, SIN Ollama — decisión 2026-08-05), `gestalt thinking`; E2E: 27 ejecuciones → insight indexado gestalt/thinking/2026-08-05 |
| P2 — Hermes + dashboard | ✅ DONE | project-admin/event.py, /api/gestalt/events + /api/gestalt/thinking, vistas bus+thinking |
| P3 — Hardening + docs | ✅ DONE | `gestalt bus replay` (cursor, dry-run), MemoryResult tolerante, features.json (3 features), SRS REQ-022/023, AGENTS.md SWAL goal |
| **Bus ↔ Router** | ✅ DONE | **El eslabón faltante**: router.execute() emite `run_started`/`run_finished` al bus en tiempo real (emit_bus_event) — todo run orquestado queda trazado (requested_by, agents, base_sha) |
| **Dedup** | ✅ DONE | SHA-256 sobre identidad semántica del evento + ventana 300s con reloj del SERVIDOR (created_at) — retries/replays no duplican; E2E: push idéntico → deduped=true |

**Decisión (2026-08-05, Belal):** NO usar Ollama ni ningún LLM externo para el thinking
loop — síntesis determinista por agregación. Cero dependencias de servicios.

**Pendiente (mejora continua):** cron 30m de `gestalt thinking`; event.py wiring automático
en workflows de Hermes (hook post-tarea compleja).
