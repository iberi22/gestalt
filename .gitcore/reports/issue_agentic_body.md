## Description
Evolve the current `AgentOrchestrator` from a stateless query wrapper into a fully autonomous system by adopting the patterns and components of the `synapse-agentic` framework.

## Tasks
- [x] Add `synapse-agentic` as a local dependency.
- [x] Implement a `GestaltAgent` using the `synapse_agentic::Agent` trait.
- [ ] Transition from single-shot LLM requests to a stateful autonomous loop.
- [ ] Integrate the existing `LlmProvider` and `RepoManager` into the `Tool` system.
- [ ] Implement structured state persistence in SurrealDB.

## Progress Notes (2026-02-21)
- `synapse-agentic` dependency is present in `gestalt_core/Cargo.toml` and `gestalt_timeline/Cargo.toml`.
- `GestaltAgent` already implements `synapse_agentic::Agent` in `gestalt_core/src/application/agent/gestalt_agent.rs`.
- Autonomous loop exists in `gestalt_timeline/src/services/runtime.rs`, but mapping from decision output to concrete actions still has placeholders and needs completion.
- `RepoManager` is already wired into tool creation in `gestalt_core/src/application/agent/tools.rs`.
- Legacy `LlmProvider` path was removed; a replacement integration strategy inside the Tool system is still pending definition.

