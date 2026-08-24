# Gestalt Timeline Orchestrator

> High-performance Rust CLI meta-agent orchestrator and universal bus for multi-agent coordination.

[![CI](https://github.com/southwest-ai-labs/gestalt/actions/workflows/ci.yml/badge.svg)](https://github.com/southwest-ai-labs/gestalt/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021%2B-orange.svg)](https://www.rust-lang.org)

Gestalt solves the coordination overhead, file collision risk, and traceability loss inherent when running multiple autonomous AI coding agents against a shared codebase. By orchestrating external AI agents in isolated git worktrees, Gestalt executes tasks concurrently, tracks real-time execution events, and automatically integrates resultant code changes. It serves as a unified control plane that bridges autonomous developer tools into a single, cohesive workflow.

## Features

- **Multi-Agent Event Bus**: Real-time event ingress and state persistence (`state.db`) capturing `run_started`, `checkpoint`, and `run_finished` lifecycle telemetry across heterogeneous agents.
- **Git Worktree & VFS Sandboxing**: Isolated execution environments for each agent utilizing local git worktrees and overlay file systems to prevent destructive file stomping.
- **Parallel Task Waves & Integration**: Concurrent agent execution bounded by configurable concurrency limits, followed by automated branch overlap analysis and sequential `git merge-tree` integration.
- **Deterministic Protocols & MCP Bridge**: Built-in Model Context Protocol (MCP) server endpoints alongside deterministic memory and thinking loops to power context retrieval and decision archiving.
- **Agent Lifecycle Observer & Injector**: Background daemon that automatically discovers local AI agent CLIs, injects telemetry hooks, and tracks execution transcripts.

## Quickstart

### Installation

Clone the repository and compile the workspace binaries using Cargo:

```bash
git clone https://github.com/southwest-ai-labs/gestalt.git
cd gestalt
cargo build --release --workspace
```

The compiled `gestalt` binary will be available at `./target/release/gestalt`.

### System Health Verification

Verify environment readiness, database paths, and agent connectivity before running orchestrated workflows:

```bash
gestalt doctor
```

### Multi-Agent Orchestration

Orchestrate multiple agents concurrently on a specific engineering task:

```bash
gestalt run \
  --task "Audit Cargo.toml dependencies and optimize build targets" \
  --agents "cargo check" \
  --base-ref main \
  --max-parallel 4 \
  --timeout 300
```

### Event Bus Service

Launch the central event bus HTTP server to receive events from connected agents:

```bash
gestalt bus serve --host 127.0.0.1 --port 8081
```

Push a manual telemetry event to the bus:

```bash
gestalt bus push \
  --agent hermes \
  --event-type run_started \
  --project gestalt \
  --state Running \
  "Started dependency analysis task"
```

### Model Context Protocol (MCP) Server

Serve the MCP tool registry over HTTP or stdio:

```bash
gestalt mcp serve --host 127.0.0.1 --port 3000 --transport http
```

### Thin Agent Launcher with Context Tracing

Execute a single external agent wrapped with automatic Xavier context lookup and bus event emission:

```bash
gestalt agent exec \
  --agent "cargo check" \
  --task "Verify workspace compilation" \
  --project gestalt \
  --timeout 120
```

## Usage Example

The following annotated transcript demonstrates inspecting system health and executing a multi-agent orchestration run using the `gestalt` CLI:

```text
$ gestalt doctor
🔍 Running Gestalt Doctor Environment Check...
=============================================
✅ Xavier reachability: Healthy (endpoint: http://127.0.0.1:8006)
✅ StateDb open: Success (path: ~/.gestalt/state.db)
✅ Agent registry parse: Success (9 agents loaded)
✅ Bus serve reachability: Reachable (port 8081)
=============================================
Verdict: Healthy! All environment components are fully operational.

$ gestalt run --task "Refactor error handling in router" --agents "cargo check" --base-ref main --max-parallel 2 --timeout 180
🚀 Gestalt Router Run
   Task: Refactor error handling in router
   Agents: 1
   Base ref: main
   Max parallel: 2
   Timeout: 180s

⚙️  Executing run...

📊 Run Report:
   Run ID: 01JEX8Q4P8Z9M2K3N5R7V1W8X9
   Success: true
   Agents: 1
   Merged branches: 1
   Conflicts: 0
   Events log: ~/.gestalt/events/01JEX8Q4P8Z9M2K3N5R7V1W8X9.jsonl

   ✅ [agent-0] Success
      Changed files: 2
      Duration: 1420ms

📦 AgentWrapper block-level editing:
   ✅ 1 AgentWrapper(s) configured
```

## Architecture

Gestalt is structured as a modular Rust workspace consisting of domain crates, orchestration engines, protocol adapters, and a unified CLI front-end:

```
                                  ┌────────────────────────┐
                                  │      gestalt_cli       │
                                  │   (CLI / Control)      │
                                  └───────────┬────────────┘
                                              │
                    ┌─────────────────────────┼─────────────────────────┐
                    ▼                         ▼                         ▼
        ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
        │    gestalt-router    │  │     gestalt_core     │  │     gestalt_mcp      │
        │ (Orchestration Engine│  │(Domain / VFS / LLM)  │  │ (MCP Tool Server)    │
        └───────────┬──────────┘  └───────────┬──────────┘  └──────────────────────┘
                    │                         │
                    ▼                         ▼
        ┌──────────────────────┐  ┌──────────────────────┐
        │    gestalt-state     │  │    gestalt-search    │
        │ (SQLite StateDb/Bus) │  │(BM25/Tantivy Engine) │
        └──────────────────────┘  └──────────────────────┘
```

- **`gestalt_cli`**: Primary CLI application exposing subcommands for `run`, `bus`, `mcp`, `agent`, `doctor`, `chain`, and `thinking`.
- **`gestalt-router`**: Multi-agent orchestration engine providing git worktree lifecycle management, process runners, branch integration (`merge-tree`), and event bus handlers.
- **`gestalt_core`**: Core domain logic, virtual file system (VFS) implementations, belief graphs, and LLM resilience wrappers.
- **`gestalt-state`**: SQLite-backed state persistence layer (`StateDb`) managing execution timelines, agent states, and event deduplication.
- **`gestalt_mcp`**: Standalone Model Context Protocol (MCP) server providing standard AI tool interfaces.
- **`gestalt-search`**: Fast local search engine providing BM25 lexical indexing over codebase assets.

## Tech Stack

Rust 2021 edition, Tokio async runtime, SQLite (`rusqlite`), Git `merge-tree`, Clap CLI parser, Tantivy search, Serde JSON, and Tracing.

## Status

**BETA / WIP**: Gestalt is actively developed under Wave 1 release candidate testing. Core CLI workflows, event bus ingress, worktree isolation, and integration routines are stable, while advanced swarm scheduling features remain under active refinement.

## License

Gestalt is distributed under the terms of the MIT License. See [LICENSE](LICENSE) for details.
