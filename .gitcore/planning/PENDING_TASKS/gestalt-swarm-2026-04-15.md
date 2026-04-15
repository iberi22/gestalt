# Pending Tasks — Gestalt Swarm (2026-04-15)

## Summary

Gestalt Swarm bridge (`swarm_bridge.py`) está operativo y elimina `gestalt_tui`.
Los pendientes son integrar como tool de OpenClaw y mejorar la inteligencia del sistema.

## Architecture (Updated)

```
NO HAY TUI — Solo exec bridge
┌─────────────────────────────────────────────┐
│  OpenClaw LLM                               │
│    →gestalt_swarm_launch(goal, max_agents?) │
├─────────────────────────────────────────────┤
│  swarm_bridge.py (asyncio parallel exec)    │
│    → asyncio.gather(N execs)                │
│    → rg, cargo, git, curl, etc.            │
│    → returns JSON {agents: [...]}          │
└─────────────────────────────────────────────┘
```

## Pending Issues

| # | Title | Priority | Effort | Labels |
|---|-------|----------|--------|--------|
| 259 | feat: Register Gestalt Swarm as OpenClaw tool | High | Low | `enhancement`, `swarm` |
| 260 | feat: Smart goal → agent selection | Medium | Medium | `enhancement`, `swarm`, `ai` |
| 261 | feat: Dynamic agent count (rate limit aware) | Medium | Medium | `enhancement`, `swarm` |
| 262 | feat: Streaming partial results | Low | High | `enhancement`, `swarm` |

## Issue 259 — Tool Registration

**Goal:** LLM puede invocar `gestalt_swarm_launch(goal)` directamente.

**Options:**
1. Skill approach — Crear skill que LLM lee y sabe invocar
2. Plugin approach — Registrar tool formal via `plugin-sdk`

**Recommendation:** Skill approach (más simple, menos overhead)

**Acceptance Criteria:**
- [ ] `gestalt_swarm` skill existe en `C:\Users\belal\clawd\skills\gestalt-swarm\`
- [ ] LLM sabe invocar `swarm_bridge.py --goal "..." --max-agents N --json`
- [ ] Documentado en SKILL.md

## Issue 260 — Smart Agent Selection

**Goal:** En vez de correr todos los agentes, analizar el goal y seleccionar los relevantes.

**Logic:**
```
Goal: "analyze codebase" → code_analyzer, file_scanner, security_audit
Goal: "check deps" → dep_check, cargo_check
Goal: "git status" → git_analyzer, git_status
Goal: "test api" → api_tester
```

**Acceptance Criteria:**
- [ ] `select_agents(goal: str) -> list[agent_id]` function
- [ ] Keyword matching para categorizar goals
- [ ] Fallback: run all relevant if ambiguous

## Issue 261 — Dynamic Agent Count

**Goal:** Calcular N óptimo de agentes basado en rate limits.

**Formula:**
```
N = min(
  user_config_max_agents,
  rate_limit_tokens // tokens_per_agent,
  complexity_score(goal)
)
```

**Acceptance Criteria:**
- [ ] Config `max_agents` en skill config
- [ ] Rate limit awareness (configurable)
- [ ] Complexity scoring para goals

## Issue 262 — Streaming Output

**Goal:** Devolver resultados parciales mientras agentes terminan.

**Options:**
1. WebSocket streaming
2. SSE (Server-Sent Events)  
3. Polling con `--watch`

**Recommendation:** `--watch` mode con file output + polling

**Acceptance Criteria:**
- [ ] `--watch` flag en bridge
- [ ] Output file (`/tmp/swarm_{id}.json`)
- [ ] Polling desde skill con timeout

---

## Files

- `E:\scripts-python\gestalt-rust\swarm_bridge.py` — Python bridge (100% functional)
- `C:\Users\belal\clawd\skills\gestalt-swarm\SKILL.md` — Skill documentation

## Completed (do not reopen)

- ✅ Delete gestalt_tui (commit 4f414fb)
- ✅ Create swarm_bridge.py (commit 1622f4e)
- ✅ Fix Cargo.toml (remove gestalt_tui workspace member)
- ✅ 15 real CLI agents defined
- ✅ 91ms parallel execution confirmed
