# SRC.md - Source Code Reference

**Project:** Gestalt-Rust
**Generated:** 2026-03-30
**Repository:** https://github.com/iberi22/gestalt-rust
**Location:** `.gitcore/` (per GitCore Protocol)

## Directory Structure

```
Gestalt-Rust/
├── .gitcore/                       # Agent documentation (per GitCore Protocol)
├── config/                         # Configuration files for Gestalt agents and environments
├── docs/                           # System design documents, requirements, and guides
├── gestalt-merge/                  # Independent branch/git integration engine crate
├── gestalt-router/                 # Core multi-agent orchestration pipeline engine crate
├── gestalt-search/                 # Hybrid vector and keyword search adapter crate
├── gestalt-state/                  # Memory map and transaction DB backend crate
├── gestalt-wasm/                   # WebAssembly binding definitions for browser/JS execution
├── gestalt-ws/                     # WebSockets push and subscription server crate
├── gestalt_cli/                    # Command-line interface and interactive REPL crate
├── gestalt_core/                   # Foundational domain logic, ports, adapters, and VFS crate
├── gestalt_mcp/                    # Model Context Protocol (MCP) server implementation
├── gestalt_swarm/                  # Pre-warmed parallel agent swarm manager crate
├── hooks/                          # Git pre-commit, post-commit, and validation hooks
├── scripts/                        # Utility automation, bridge, and pipeline orchestration scripts
├── synapse-agentic/                # Lightweight tool registry and agent network crate
├── agent_benchmark/                # (Legacy) Benchmarking utility directory
├── benchmarks/                     # (Legacy) Automated performance workflows
├── gestaltctl/                     # (Legacy) Command-line orchestration utility stub
├── gestalt_app/                    # (Legacy) Embedded UI/app service integration layer
├── gestalt_infra_embeddings/       # (Legacy) Standalone embedding generation adapter
├── gestalt_infra_github/           # (Legacy) Independent GitHub issue and PR tracker
├── gestalt_terminal/               # (Legacy) Terminal UI visual dashboard wrapper
├── gestalt_timeline/               # (Legacy) Standalone agent run timeline event logging
├── gestalt_ui/                     # (Legacy) Web application management interface frontend
├── skills/                         # Declarative skills and semantic memory schemas
└── target/                         # Compiled Rust build artifacts
```

## Modules

### gestalt_core
- **Status:** implemented
- **Purpose:** Core domain model definition, Hexagonal architecture ports, database persistence adapters (SurrealDB), Virtual File System (VFS), vector embeddings search, and core agent orchestration logic.
- **Test count:** 99

### gestalt_cli
- **Status:** implemented
- **Purpose:** Primary command-line interface entry point that parses commands like run, repl, serve, and thinking to manage workspace sessions, run REPL actions, and track event logs.
- **Test count:** 52

### gestalt-router
- **Status:** implemented
- **Purpose:** High-performance multi-agent task runner and workspace manager coordinating Git worktrees, atomic check-pointing, sandboxed subprocess execution, and conflict detection.
- **Test count:** 163

### gestalt-state
- **Status:** implemented
- **Purpose:** Persistent SQLite storage and in-memory event management layer tracking the transactional state and history of execution timelines and Virtual File System (VFS) operations.
- **Test count:** 22

### gestalt-ws
- **Status:** implemented
- **Purpose:** WebSocket communication server responsible for real-time event distribution and subscription channels across Gestalt agents.
- **Test count:** 7

### gestalt-search
- **Status:** partial
- **Purpose:** Semantic and keyword index module implementing local Tantivy indexing and hybrid vector-keyword retrieval capabilities.
- **Test count:** 10

### gestalt-wasm
- **Status:** implemented
- **Purpose:** WebAssembly integration crate providing JS-compatible bindings, event streams, run report mapping, and wasm-bindgen structures.
- **Test count:** 4

### gestalt-merge
- **Status:** partial
- **Purpose:** Specialized utility module implementing isolated Git tree merging and direct git-index based branch integration features.
- **Test count:** 0

### gestalt_mcp
- **Status:** implemented
- **Purpose:** Model Context Protocol (MCP) server endpoints exposing gestalt tools for hybrid search, database status query, and context tracking to external agents.
- **Test count:** 17

### gestalt_swarm
- **Status:** implemented
- **Purpose:** Core agent pool and lifecycle orchestration layer with weak reference tracking and dynamic warmup/cool-down transitions (excluded from workspace compile).
- **Test count:** 12

### synapse-agentic
- **Status:** partial
- **Purpose:** Tool registration, LLM provider integration, and decentralized actor framework supporting resilient Multi-Agent execution.
- **Test count:** 0

### config
- **Status:** implemented
- **Purpose:** Active workspace configuration workspace containing system-wide default, dev, and production TOML files specifying base network and path properties.
- **Test count:** 0

### docs
- **Status:** implemented
- **Purpose:** System requirements specification (SRS), system architectures, operational roadmaps, design proposals, and feature analysis reports.
- **Test count:** 0

### scripts
- **Status:** implemented
- **Purpose:** Python and shell scripts automating agent workflows, swarm bridge runners, performance benchmarks, and development utilities.
- **Test count:** 0

### hooks
- **Status:** implemented
- **Purpose:** Workspace git pre-commit, post-commit, and push automation quality gate scripts to guarantee compilation and formatting standards.
- **Test count:** 0

### agent_benchmark
- **Status:** stub
- **Purpose:** Placeholder directory intended for executing low-overhead benchmarking of LLM agent responses.
- **Test count:** 0

### benchmarks
- **Status:** stub
- **Purpose:** Automated performance benchmarks and baseline workflows directory for profiling Gestalt workspace components.
- **Test count:** 0

### gestalt.db
- **Status:** legacy
- **Purpose:** SQLite relational database file for holding legacy agent transaction execution states (superseded by modern StateDb).
- **Test count:** 0

### gestaltctl
- **Status:** stub
- **Purpose:** Dedicated CLI control panel terminal utility stub (functionality now fully covered by gestalt_cli commands).
- **Test count:** 0

### gestalt_app
- **Status:** stub
- **Purpose:** Alternative application packaging or desktop client interface runner placeholder.
- **Test count:** 0

### gestalt_infra_embeddings
- **Status:** legacy
- **Purpose:** Legacy infrastructure module for embedding calculation and local text vector storage (integrated into gestalt_core).
- **Test count:** 0

### gestalt_infra_github
- **Status:** legacy
- **Purpose:** Historical directory for syncing and ingesting external GitHub issues, comments, and PR records.
- **Test count:** 0

### gestalt_terminal
- **Status:** legacy
- **Purpose:** Legacy visual terminal client dashboard prototype for dashboarding Multi-Agent run executions.
- **Test count:** 0

### gestalt_timeline
- **Status:** legacy
- **Purpose:** Legacy standalone timeline tracking component (refactored into the universal StateDb SQLite backend).
- **Test count:** 0

### gestalt_ui
- **Status:** legacy
- **Purpose:** Legacy graphical user interface web components for visualizing running agent states.
- **Test count:** 0

### skills
- **Status:** implemented
- **Purpose:** Standard YAML definitions specifying the schema of external tools and active declarative skills available to agents.
- **Test count:** 0

### target
- **Status:** implemented
- **Purpose:** Output directory for Cargo workspace compiled files, build targets, libraries, and binaries.
- **Test count:** 0

### __pycache__
- **Status:** implemented
- **Purpose:** Automated Python bytecode generation folder created by the compiler to optimize script warmups and run speeds.
- **Test count:** 0


## Build Commands

```bash
# Check compilation for the active workspace
CARGO_TARGET_DIR=/tmp/cargo-gestalt cargo check --workspace

# Run all relevant tests (sequential to avoid cross-test resource conflicts)
CARGO_TARGET_DIR=/tmp/cargo-gestalt cargo test --workspace -- --test-threads=1

# Format check
cargo fmt --all --check

# Clippy check
cargo clippy --workspace --all-targets -- -D warnings
```

## Entry Points

- **Main Bin / CLI Entrypoint:** `gestalt_cli/src/main.rs` contains the core CLI commands (run, repl, serve, thinking, bus, etc.) and orchestrates task setups.
- **Core Library Entrypoint:** `gestalt_core/src/lib.rs` exports the foundational model types, VFS, and LLM resilience services.
- **Router Orchestration Engine:** `gestalt-router/src/lib.rs` manages worktree isolated executions, check-pointers, and git merges.

---
*Auto-generated by GitCore Auto-Maintainer*
* All docs stored in .gitcore/ per GitCore Protocol v3*
