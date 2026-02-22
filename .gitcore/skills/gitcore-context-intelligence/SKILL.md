---
name: gitcore-context-intelligence
description: Build project state context under Git-Core protocol by correlating `git status`, recent commits, local file-based issues in `.github/issues`, and GitHub issues via `gh` CLI. Use when an agent must infer what is actively being worked on, detect protocol drift, prioritize next issue, or produce a status report before coding/review.
---

# Gitcore Context Intelligence

## Goal
Infer active work quickly and defensibly from observable signals, not assumptions.

## Workflow
1. Run the snapshot script:
```powershell
powershell -ExecutionPolicy Bypass -File .gitcore/skills/gitcore-context-intelligence/scripts/context_snapshot.ps1
```
Optional output paths:
```powershell
powershell -ExecutionPolicy Bypass -File .gitcore/skills/gitcore-context-intelligence/scripts/context_snapshot.ps1 `
  -SnapshotOut .gitcore/reports/context_snapshot.md `
  -StatusOut .gitcore/reports/project_status.md
```
2. Read the output sections in this order:
- `Likely Active Work`
- `Git State`
- `Local Issues`
- `GitHub Issues`
- `Protocol Signals`
3. Cross-check the top hypothesis against `.gitcore/ARCHITECTURE.md` before proposing implementation changes.
4. Report confidence and conflicts explicitly:
- High confidence: changed files and issue text clearly overlap.
- Medium confidence: partial overlap or mixed workstreams.
- Low confidence: no overlap; state blockers.

## Required Signals
- `git status --short --branch`
- `git log --oneline -5`
- Changed file paths and diff stat
- `.github/issues/*.md` (exclude `_TEMPLATE.md`)
- `gh issue list --state all`
- `gh issue list --assignee "@me"`

## Interpretation Rules
- Treat changed paths as primary evidence of current work.
- Treat issue text as intent and acceptance criteria.
- Prefer issues with highest overlap score against changed-path keywords.
- Mark protocol drift when forbidden planning files are active instead of issues.
- If local and online issues disagree, surface mismatch and recommend reconciliation.

## Output Contract
Produce a short report with:
1. `Current Workstream`: one-line hypothesis.
2. `Evidence`: branch, top changed modules, matching issue(s).
3. `Risk/Drift`: protocol or tracking inconsistencies.
4. `Next Action`: concrete next step (issue update, test run, PR prep, cleanup).
5. Write reports to:
- `.gitcore/reports/context_snapshot.md`
- `.gitcore/reports/project_status.md`

## References
- Heuristics and scoring: `references/heuristics.md`
- Snapshot command: `scripts/context_snapshot.ps1`
