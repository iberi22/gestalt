## Objective
Make `swarm_bridge.py` smarter — analyze the goal and select only relevant agents instead of running all.

## Context
Currently all 15 agents run (or attempt to) regardless of goal. 
Need intelligent routing based on goal keywords.

## Logic
```
Goal: "analyze codebase" → code_analyzer, file_scanner, security_audit
Goal: "check deps" → dep_check, cargo_check
Goal: "git status" → git_analyzer, git_status
Goal: "test api" → api_tester
Goal: "find todos" → find_todos
Goal: "security audit" → security_audit, find_todos
Goal: "quick check" → git_analyzer, git_status
```

## Acceptance Criteria
- [ ] `select_agents(goal: str) -> list[agent_id]` function
- [ ] Keyword matching for goal categorization
- [ ] Fallback: run all relevant if ambiguous
- [ ] `--dry-run` flag to test agent selection without executing

## Priority
**MEDIUM** — Improves relevance and speed.
