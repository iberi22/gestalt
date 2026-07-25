# gestalt — SRS Architecture map

> **Protocol:** GitCore 3.8.0 · **Version:** 2.0.0 · **Updated:** 2026-07-25

## System context (v2.0 Router)

```
┌─────────────────────────────────────────────────────────┐
│  gestalt (L4 — Multi-Agent Codebase Router)             │
│                                                          │
│  CLI: gestalt run --agents X,Y "task"                    │
│    → WorktreeManager (git worktree per agent)            │
│    → AgentRunner (subprocess CWD=worktree)               │
│    → OverlapDetector (diff intersection)                 │
│    → Integrator (commit → merge → branch → PR)           │
│    → EventLog (JSONL — replaces SurrealDB)               │
│                                                          │
│  Reuses: gestalt_core (VFS, ToolRegistry, LLM/MCP)      │
│          synapse-agentic (actor model, Phase 3+)         │
└───────────────┬─────────────────────────┬────────────────┘
                │                         │
                ▼                         ▼
         Xavier (L3)               edge-mesh (L2)
         memory / MCP              P2P data sync
```

## Non-negotiables (SWAL)

1. Business data ≠ mesh bulk storage ≠ chain blobs
2. Pro = SWAL node active (not Stripe)
3. Multi-instance isolation by default
4. Xavier for agentic memory
5. GitCore protocol files always present
6. Git worktree = isolation mechanism
7. JSONL event log (not SurrealDB)

## Components

| Component | Responsibility | REQ |
|-----------|----------------|-----|
| gestalt-router | Orchestration, worktrees, agent lifecycle | REQ-001 |
| gestalt-merge | 3-way merge engine | REQ-003 |
| Event log | Run audit trail | REQ-002 |
| PR automation | Git branch → PR | REQ-004 |
| PathClaims | Preventive coordination | REQ-005 |

## Previous Architecture (archived)

v1.0.0 incluía SurrealDB Timeline, FUSE Daemon, Lock Server y gestalt_timeline. Reemplazado en v2.0.0 tras validación con AGY + Kimi. Ver `docs/REDESIGN.md`.
