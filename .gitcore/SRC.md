# SRC.md - Source Code Reference

**Project:** Gestalt-Rust
**Generated:** 2026-08-06 (Updated post-Wave 8 / Wave 9 integration)
**Repository:** https://github.com/iberi22/gestalt-rust
**Location:** `.gitcore/` (per GitCore Protocol)

## Directory Structure

```
gestalt/
├── Cargo.toml                       # Workspace root configuration
├── .gitcore/                        # GitCore Protocol feature/architecture tracking
├── .github/                         # CI/CD workflows and configurations
├── config/                          # Workspace configuration templates
├── data/                            # Local operational and vector storage databases
├── docs/                            # Documentation (SRS, designs, walkthroughs)
│   └── SRS/                         # Software Requirements Specifications (REQUIREMENTS.md)
├── gestalt_core/                    # Core Domain logic, traits, VFS and Xavier/MCP clients
├── gestalt_cli/                     # Main CLI command center and REPL
│   └── src/
│       ├── observe/                 # Observe Daemon (autonomous agent monitoring)
│       │   ├── artifact_ingest.rs   # Claude transcript tailing, SQLite DB session/Jules GitHub polling
│       │   ├── inject.rs            # Idempotent hook injection (Codex, Claude, Opencode)
│       │   ├── orca_bridge.rs       # Orca agent status polling and event mapping
│       │   ├── proc_monitor.rs      # Linux process monitor (PID matching and start/finish events)
│       │   └── mod.rs               # Observe daemon mod and orchestrator loop
│       ├── main.rs                  # CLI command routing and main entry
│       ├── repl.rs                  # CLI REPL shell with autocomplete & command history
│       └── bus.rs                   # Event bus server and push command helpers
├── gestalt-router/                  # Multi-agent orchestrator & execution runtime
├── gestalt-merge/                   # Advanced merge engines and conflict resolution utils
├── gestalt-state/                   # Transactional SQLite WAL database and in-memory lock state
├── gestalt-ws/                      # Real-time WebSocket broadcasting interface
├── gestalt-search/                  # Tantivy-based local BM25 & vector search integrations
├── gestalt-wasm/                    # WebAssembly wasm-bindgen bindings for client runtimes
├── gestalt_mcp/                     # Stateless MCP tool server (search, belief query, etc.)
├── synapse-agentic/                 # Tool registry, Hive actor model, and LLM providers
└── gestalt_swarm/                   # **EXCLUDED** Legacy swarm coordinator (not in workspace)
```

## Active Crate Catalog

### gestalt_core
- **Status:** Stable
- **Purpose:** Declares core VFS traits, VFS overlays, and adapters for calling remote services (Xavier client, MCP client, and LLM resilience wrappers).

### gestalt_cli
- **Status:** Active
- **Purpose:** Implements user-facing commands, the interactive REPL shell with autocomplete and command history, and the `observe` daemon cluster.
- **Observe Cluster (`src/observe/`):**
  - `proc_monitor.rs`: Scans active Linux process command lines to dynamically track agent execution transition events (`run_started`, `run_finished`).
  - `inject.rs`: Automatically injects fail-safe/fail-open event reporting hooks into external agent configurations (Codex, Claude, Opencode).
  - `orca_bridge.rs`: Integrates with Orca daemon to proxy external agent hook notifications.
  - `artifact_ingest.rs`: Tail-polls third-party trace output (Claude JSONL files, Hermes SQLite DB, GitHub Jules issue API).

### gestalt-router
- **Status:** Stable (Wave 1 Complete)
- **Purpose:** Coordinates concurrent, isolated multi-agent runs on virtual workspace branches using isolated Git worktrees and `WriteSetValidator` VFS containment. Features atomic checkpointers and sequential tree-merging with cgroup-based subprocess reaping.

### gestalt-state
- **Status:** Stable
- **Purpose:** Manages system persistent operational truth in SQLite (`StateDb` with WAL journaling and exponential busy retries) and transient transactional in-memory states (`MemState`).

### gestalt-ws
- **Status:** Stable
- **Purpose:** Operates a real-time WebSocket server that broadcasts Gestalt timeline updates and event bus state transitions to connected subscribers.

### gestalt-search
- **Status:** Active
- **Purpose:** Integrates full-text BM25 index matching via Tantivy with vector similarity searches, providing a hybrid search capability.

### gestalt_mcp
- **Status:** Active
- **Purpose:** Exposes Gestalt's internals (VFS, search engines, belief graphs, and agent triggers) as standard Model Context Protocol (MCP) tools for external clients.

### synapse-agentic
- **Status:** Active
- **Purpose:** Modular LLM context tracker, tool registries, actor model workflows, and providers (Gemini, Groq, etc.).

### gestalt-wasm
- **Status:** Active
- **Purpose:** Exposes Gestalt models and API adapters to JS/WASM runtimes for use in browser nodes and progressive web apps.

---
*Auto-updated - Wave 9 Traceability Document*