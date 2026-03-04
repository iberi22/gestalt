# Handoff Prompt - Workflow Stabilization Continuation

Use this prompt with the next agent:

---
You are continuing workflow stabilization for `iberi22/gestalt-rust` on branch `main`.

## Context
- Date baseline: 2026-03-04
- Source issue: https://github.com/iberi22/gestalt-rust/issues/87
- Latest manual fixes already merged:
  - `220df21` fix(ci): stabilize dispatcher triggers and Android release toolchain
  - `f2a4162` fix(ci): set GH_REPO for agent-dispatcher gh commands
  - `eb12dec` fix(ci): provide GH_TOKEN to agent-dispatcher gh calls

## What is already fixed
1. `Agent Dispatcher` now has `GH_TOKEN` + `GH_REPO` and executes successfully.
2. Dispatcher trigger noise reduced from `issues:[opened,labeled]` to `issues:[opened]`.
3. `Build and Release` Android job upgraded Flutter to `3.41.3` to satisfy Dart SDK constraints (`^3.10.0`).

## Required Continuation Tasks
1. Inspect latest workflow runs:
   - `gh run list --limit 30 --json databaseId,workflowName,displayTitle,status,conclusion,event,headBranch,headSha,url`
2. Confirm green for latest `main` SHA on:
   - `Agent Dispatcher`
   - `CI`
   - `Benchmarks`
   - `Build and Release`
3. If any run fails:
   - open run logs (`gh run view <id> --log`)
   - identify exact failing step
   - implement minimal targeted fix on `main`
4. Post RCA summary and links on issue #87.
5. If all green, close issue #87 with evidence links.

## Guardrails
- Do not revert prior protocol sync work.
- Keep changes minimal and workflow-focused.
- Avoid unrelated refactors.
---
