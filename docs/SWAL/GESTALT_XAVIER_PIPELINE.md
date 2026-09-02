# Gestalt Xavier Pipeline Architecture & Operations

## Overview

The Gestalt Xavier Pipeline coordinates multi-repo subagent execution, task delegation, and event tracing across Gestalt components and external subagents.

## PNPM Execution & Worktree Policy

### Problem Context

When subagents execute delegated tasks within Git worktrees (`/tmp/wt-wave10-*`), running workspace filter commands against package hierarchies with symlinked `node_modules` causes PNPM dependency checks (`runDepsStatusCheck`) to fail in non-interactive CI environments (`CI=true`).

This produces the abort error:
`ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY`
which causes 120s stall timeouts repeatedly up to 1800s.

### Pipeline Rules & Guidelines

1. **Direct Tool Directives**:
   - For test suites: execute `npx vitest run` directly inside the package root (`packages/app-pwa`).
   - For frontend builds: execute `npx astro build` directly.
   - For PNPM commands requiring root context: pass `--ignore-workspace-root-check`.

2. **Delegate Prompt Construction**:
   - Ensure `delegate_task` prompts never instruct agents to run `pnpm` filter invocations in worktrees.
   - Verify local `.bin` dependencies prior to test invocation.

3. **Event Bus Tracing**:
   - Task executions inside worktrees should report status events back to `http://127.0.0.1:8081/api/event` using `periferia/project-admin/event.py` with `--worktree`.

Refer to `docs/GESTALT_WORKTREE_PNPM.md` for full technical details and diagnostic procedures.
