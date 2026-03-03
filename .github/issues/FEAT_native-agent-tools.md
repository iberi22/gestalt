---
github_issue: 19
title: "Feat (Backend): Native Agent Tools Implementation (Shell, FS, Git)"
labels:
  - backend
  - enhancement
assignees: []
status: open
last_reviewed: 2026-03-03
---

## Description
Implement concrete runtime tools for agents to operate on the real environment.

## Checklist
- [x] `ExecuteShell` implemented.
- [x] `WriteFile` / `ReadFile` implemented.
- [x] Tool registry integration in runtime.
- [ ] `GitCommands` tool set (status/log/branch/commit/push with guardrails).
- [ ] Integration tests for git command tooling in runtime context.

## Notes
- Kept open intentionally: FS and shell are done, git command surface is still pending.
