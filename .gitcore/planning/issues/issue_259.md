## Objective
Register `swarm_bridge.py` as an OpenClaw tool so the LLM can invoke `gestalt_swarm_launch(goal, max_agents)` directly.

## Context
- `swarm_bridge.py` is functional — 3 agents run in ~91ms parallel
- `gestalt_tui` was deleted — no TUI, only exec bridge
- Skill exists at `C:\Users\belal\clawd\skills\gestalt-swarm\SKILL.md`

## Approach
**Skill approach (recommended):** Create skill that LLM reads and knows how to invoke.

Alternative: Plugin approach via `plugin-sdk` for formal tool registration.

## Acceptance Criteria
- [ ] `gestalt_swarm` skill auto-loaded by OpenClaw
- [ ] LLM can invoke bridge via exec
- [ ] Results parsed and returned to LLM context
- [ ] Documented in `skills/gestalt-swarm/SKILL.md`

## Files
- `E:\scripts-python\gestalt-rust\swarm_bridge.py` (already exists, commit 1622f4e)
- `C:\Users\belal\clawd\skills\gestalt-swarm\SKILL.md`

## Priority
**HIGH** — Without this, the LLM doesn't know swarm exists.
