# Project Admin & Periferia Tools Guide

This module contains peripheral administrative scripts and utilities for managing Gestalt multi-repo pipeline tasks and event bus notifications.

## Key Components

- `event.py`: Python CLI tool for dispatching events to the Gestalt Event Bus (`http://127.0.0.1:8081/api/event`).

## Worktree Execution & `--worktree` Flag

When operating across Git worktrees (e.g. `/tmp/wt-wave10-*`), scripts sending events or managing agent tasks must take into account isolated environment paths.

### Usage with `--worktree` Flag

When dispatching pipeline events or configuring agent sessions operating in git worktrees, supply the `--worktree` flag (or worktree context parameter):

```bash
python3 periferia/project-admin/event.py --worktree /tmp/wt-wave10-04 --kind task_started --agent gara
```

### Best Practices for Worktree Task Delegation

1. **Avoid Workspace Filter Locks**:
   In Git worktrees with symlinked `node_modules`, standard PNPM workspace filter commands can trigger `ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY` in non-interactive environments (`CI=true`).

2. **Direct Tool Directives**:
   Delegated subagents should execute commands directly:
   - Run tests: `npx vitest run` inside package folder
   - Run builds: `npx astro build` inside package folder

3. **Event Bus Reporting**:
   Always dispatch start/finish events via `event.py` with the corresponding worktree path metadata to maintain clear audit logs in `StateDb`.
