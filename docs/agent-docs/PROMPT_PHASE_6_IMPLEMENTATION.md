---
title: "Prompt: Implementation of Phase 6 - Resilience and Isolation"
type: PROMPT
id: "prompt-phase-6-start"
created: 2026-02-22
agent: protocol-architect
model: google-gemini-2.0-flash
requested_by: user
summary: |
  Prompt designed to transfer the context of the new architecture (VFS Overlay, Elastic Loops, Synapse)
  to a developer agent to start the technical implementation of Phase 6 in Gestalt-Rust.
keywords: [prompt, phase-6, vfs, elastic-loops, implementation]
---

# 🤖 Developer Agent: Implementation of Phase 6 (Gestalt-Rust)

You have been assigned to execute **Phase 6: Advanced Resilience and Isolation** for the `gestalt-rust` project. Your goal is to evolve the current orchestrator from a reactive tool to a system of locally isolated autonomous agents.

## 📚 Mandatory Context
Before taking any technical action, you MUST read and assimilate these files containing the approved new architecture:
1.  [ARCHITECTURE.md](file:///.gitcore/ARCHITECTURE.md): Review Decisions 5, 6, and 7 regarding VFS, Elastic Loops, and Synapse Hive.
2.  [PLANNING.md](file:///.gitcore/planning/PLANNING.md): Read the Phase 6 section.
3.  [TASK.md](file:///.gitcore/planning/TASK.md): Identify tasks `F6-01` to `F6-07`.

## 🛠️ Implementation Objectives
Your work will be divided into three main fronts:

### 1. Virtual File System (VFS) Overlay
- Replace direct use of `tokio::fs` in agent tools with a `VirtualFs` adapter.
- Allow the agent to read from the real disk but "write" to a volatile in-memory state.
- Implement a `flush` function that the Supervisor will trigger to persist changes only after validation.

### 2. Elastic Loops & Context Compaction
- Refactor `run_loop` in `runtime.rs` to remove the fixed step limit (`max_steps`).
- Implement a compaction routine that summarizes the history when the token window approaches its limit.
- Ensure the agent can automatically delegate complex tasks.

### 3. Synapse-Agentic Integration
- Migrate agent lifecycle management to the `Hive` actor model seen in `synapse-agentic`.
- Implement robust failover strategies for LLM providers.

## ⚖️ Execution Rules
- **Git-Core Protocol**: Keep all task memory in the `.gitcore` folder files.
- **Atomic Commits**: Make small, descriptive commits for each VFS or elastic engine functionality.
- **Zero Conflicts**: The VFS must ensure the physical disk is not modified until the task is "Completed".

**Start by analyzing the current structure of `gestalt_timeline/src/services/runtime.rs` and propose the first technical step in an implementation plan file.**
