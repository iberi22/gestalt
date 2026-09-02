# Gestalt Worktree & PNPM Integration Guide

## Overview

When executing automated tasks or tests within Git worktrees (e.g., located under `/tmp/wt-wave10-*` or similar isolated paths), repository managers using PNPM (such as PNPM 11.24.0 with legacy overrides or symlinked `node_modules` structure) can encounter execution deadlocks and timeouts.

Specifically, running workspace filter commands (such as `pnpm -F` or workspace filter flags) inside a git worktree where `node_modules` are symlinked to `packages/*/node_modules` triggers PNPM's dependency status check (`runDepsStatusCheck`). In non-interactive or CI environments (`CI=true`), this results in:

```
ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY: Aborted removal of modules directory because TTY is not available.
```

This error causes repetitive 120-second retry cycles that accumulate up to an 1800-second (30-minute) total timeout before aborting the pipeline run.

## Root Cause Analysis

1. **Symlinked node_modules in Worktrees**:
   Git worktrees often link shared `node_modules` directories or sub-packages (e.g., `ln -s packages/app-pwa/node_modules`).
2. **PNPM Dependency Status Checks**:
   PNPM checks workspace tree consistency before executing filtered scripts. When detecting non-standard directory structures or symlinks in headless CI environments without a TTY, PNPM attempts interactive prompts to clean `node_modules`. Because TTY is absent (`CI=true`), PNPM aborts with `ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY`.
3. **Execution Retries**:
   Orchestrators retry the filter command across agents, triggering repeated 120s stalls that sum up to 1800s.

## Recommended Solutions & Workarounds

### Option 1: Direct Tool Invocation (Recommended for Subagents)

Delegate subagents operating inside isolated worktrees should avoid invoking global workspace filter commands. Instead, agents should navigate directly to the target package directory and execute testing or build tools directly:

```bash
# Direct Vitest Execution
cd packages/app-pwa
npx vitest run

# Direct Astro Build Execution
cd packages/app-pwa
npx astro build
```

This bypasses `runDepsStatusCheck` completely, avoiding `node_modules` cleanup prompts and executing in milliseconds rather than timing out.

### Option 2: Worktree Flag / Ignore Workspace Root Check

If `pnpm` must be invoked directly within a workspace sub-path:

```bash
pnpm --ignore-workspace-root-check run test
```

Or pass `--ignore-scripts` / `--no-frozen-lockfile` depending on the operation context.

## Guidelines for Pipeline Prompt Engineering

When delegating tasks to subagents via `delegate_task`:

1. Do NOT include workspace filter options (`pnpm` filter flags) in the prompt instructions.
2. Instruct agents explicitly to run `npx vitest run` or `npx astro build` inside the specific package folder.
3. Ensure prompts verify local `.bin` executables exist before running commands:
   `cat packages/app-pwa/node_modules/.bin/vitest`

## Summary Table

| Execution Mode | Command Pattern | Worktree Safe? | Notes |
|---|---|---|---|
| Workspace Filter | `pnpm -F <target> test` | ❌ No | Triggers TTY abort in CI worktrees |
| Direct Runner | `npx vitest run` | ✅ Yes | Fast, safe, bypasses dependency check |
| Direct Builder | `npx astro build` | ✅ Yes | Direct build without workspace locking |
| Root Bypass | `pnpm --ignore-workspace-root-check` | ✅ Yes | Ignores root worktree mismatch |
