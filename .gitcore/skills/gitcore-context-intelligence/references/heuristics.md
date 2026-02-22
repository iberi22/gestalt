# Heuristics

Use these rules to infer active work from repository evidence.

## Priority Order
1. Changed files in `git status`.
2. Issue acceptance criteria and task lists.
3. Recent commit messages.
4. Branch name.
5. Assignee/open-state from GitHub issues.

## Scoring
- Build keywords from changed paths (`agent`, `runtime`, `repl`, `config`, etc.).
- Score each local issue by keyword overlap in body/title.
- Add signal boosts for strategic words:
- `agentic`, `synapse`: +3 each.
- `repl`: +2.
- `config`: +2.

Interpretation:
- `>=8`: high-confidence alignment.
- `4..7`: moderate alignment; verify manually.
- `<4`: weak alignment; check for missing/obsolete issue tracking.

## Drift Detection
Mark drift when:
- Work is active but issue tasks remain stale.
- GitHub issue state does not reflect local issue intent.
- Forbidden planning files (`TODO.md`, `TASKS.md`, `PLANNING.md`, `NOTES.md`) replace issue tracking.

## Report Template
Use this compact output:

1. Current Workstream
2. Evidence (branch, files, issue matches)
3. Drift/Risk
4. Immediate Next Action

