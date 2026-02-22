---
title: "Refactor: Transition Gestalt Core to Agentic Framework"
labels:
  - enhancement
  - ai-plan
  - architecture
assignees: []
---

## Description
Evolve the current `AgentOrchestrator` from a stateless query wrapper into a fully autonomous system by adopting the patterns and components of the `synapse-agentic` framework.

## Tasks
- [x] Add `synapse-agentic` as a local dependency.
- [x] Implement a `GestaltAgent` using the `synapse_agentic::Agent` trait.
- [x] Transition from single-shot LLM requests to a stateful autonomous loop.
- [x] Integrate the existing `LlmProvider` and `RepoManager` into the `Tool` system.
- [x] Implement structured state persistence in SurrealDB.

## Progress Notes (2026-02-21)
- `synapse-agentic` dependency is present in `gestalt_core/Cargo.toml` and `gestalt_timeline/Cargo.toml`.
- `GestaltAgent` already implements `synapse_agentic::Agent` in `gestalt_core/src/application/agent/gestalt_agent.rs`.
- Autonomous loop is active in `gestalt_timeline/src/services/runtime.rs` with concrete mapping from decision output to executable orchestration actions.
- `RepoManager` is already wired into tool creation in `gestalt_core/src/application/agent/tools.rs`.
- Legacy `LlmProvider` direct path was removed and replaced by DecisionEngine-driven Tool execution (`OrchestrationAction`, including `CallAgent`).
- Runtime decision mapping is now implemented in `gestalt_timeline/src/services/runtime.rs`:
  - Supports direct action-tag mapping (`create_project`, `run_task`, etc.).
  - Supports multi-action payloads via JSON (`parameters.actions` or raw array/object).
  - Supports `call:<tool>` -> `OrchestrationAction::CallAgent` bridge for tool-native execution.
- LLM->Tool integration is now explicit via DecisionEngine output mapped to `OrchestrationAction` (including `CallAgent`) instead of legacy direct `LlmProvider` execution path.
- Runtime loop state is now persisted in SurrealDB (`agent_runtime_states`) with structured snapshots:
  - phase (`running`/`completed`/`failed`)
  - current step and max steps
  - last action and last observation
  - bounded history tail, timestamps, and failure reason
