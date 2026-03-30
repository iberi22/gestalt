# Gestalt-Rust Architecture

> Last Updated: 2026-03-30
> Reviewer: kimi-k2.5 (architecture subagent)

## Overview

Gestalt-Rust is a **Rust workspace** implementing an autonomous AI agent runtime with timeline-based orchestration, virtual filesystem overlay, and multi-provider LLM resilience. It powers the OpenClaw ↔ Gestalt CLI and the Gestalt Nexus daemon.

**Repository:** `iberi22/gestalt-rust`
**Workspace Members:** `gestalt_core`, `gestalt_infra_github`, `gestalt_infra_embeddings`, `gestalt_cli`, `gestalt_timeline`, `gestalt_mcp`, `gestalt_ui`, `gestaltctl`, `synapse-agentic`

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         gestalt_cli                              │
│                   (Clap + Rustyline REPL)                        │
└────────────────────────┬────────────────────────────────────────┘
                         │ Commands / Requests
┌────────────────────────▼────────────────────────────────────────┐
│                     gestalt_timeline                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐   │
│  │ Timeline  │ │ Project   │ │  Task     │ │  WatchService    │   │
│  │ Service   │ │ Service   │ │  Service  │ │  (broadcast)     │   │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │              AgentRuntime (Think-Act-Observe Loop)       │    │
│  │   ┌─────────────┐  ┌─────────────┐  ┌────────────────┐  │    │
│  │   │ Decision    │  │ Tool        │  │ Context         │  │    │
│  │   │ Engine      │  │ Registry    │  │ Compactor       │  │    │
│  │   └─────────────┘  └─────────────┘  └────────────────┘  │    │
│  └──────────────────────────────────────────────────────────┘    │
│  ┌──────────────┐ ┌─────────────────┐ ┌─────────────────────┐  │
│  │ VirtualFs    │ │ TaskQueue       │ │ ProtocolSyncService │  │
│  │ (FileManager) │ │ (dispatch loop)  │ │ (markdown ↔ DB)    │  │
│  └──────────────┘ └─────────────────┘ └─────────────────────┘  │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                       gestalt_core                               │
│  ┌────────────┐ ┌────────────┐ ┌─────────────┐ ┌────────────┐  │
│  │ Application │ │   Domain    │ │   Context   │ │   Ports    │  │
│  │  (agent,    │ │   (RAG,     │ │  (detector,  │ │  (inbound,  │  │
│  │   config)   │ │   GenUI)    │ │   scanner)   │ │   outbound) │  │
│  └────────────┘ └────────────┘ └─────────────┘ └────────────┘  │
│  ┌────────────┐ ┌────────────┐ ┌────────────────────────────┐  │
│  │   Adapters │ │    MCP      │ │        DB                  │  │
│  │  (auth,    │ │  (client,   │ │  (SurrealDB adapter)      │  │
│  │   storage) │ │   registry) │ │                            │  │
│  └────────────┘ └────────────┘ └────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                    Infrastructure Crates                         │
│  ┌─────────────────────┐  ┌──────────────────┐  ┌───────────┐ │
│  │ gestalt_infra_      │  │ gestalt_infra_    │  │ gestalt   │ │
│  │ github (Octocrab)   │  │ embeddings (BERT) │  │ _mcp      │ │
│  └─────────────────────┘  └──────────────────┘  └───────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

---

## Crate Responsibilities

| Crate | Language | Purpose |
|-------|----------|---------|
| `gestalt_core` | Rust | Domain logic, traits, ports, adapters (no I/O) |
| `gestalt_timeline` | Rust | Tokio runtime, SurrealDB, AgentRuntime, all services |
| `gestalt_cli` | Rust | Clap CLI, REPL interface |
| `gestaltctl` | Rust | MCP bridge CLI |
| `gestalt_mcp` | Rust | MCP server implementation |
| `gestalt_infra_embeddings` | Rust | BERT/candle embedding model (optional, feature-gated) |
| `gestalt_infra_github` | Rust | GitHub API via Octocrab (optional) |
| `gestalt_ui` | Rust | Flutter UI (Flutter-side) |
| `synapse-agentic` | Rust | Resilient LLM providers, Hive actor model |

---

## Key Design Patterns

### 1. Ports & Adapters (Hexagonal)
`gestalt_core` defines **ports** (traits) and `gestalt_timeline` implements the **adapters** (concrete services). This keeps domain logic I/O-free.

### 2. Elastic Autonomous Loops
The `AgentRuntime::run_loop()` implements a Think-Act-Observe cycle with:
- **Context Compaction**: Automatic summarization via `ContextCompactor` when token limits approach
- **Dynamic Delegation**: Goals exceeding step caps auto-split into sub-agent tasks
- **Retry with Repair**: Failed actions request repair decisions from the engine before retrying

### 3. Virtual File System (VFS) Overlay
Agents edit in-memory via `VirtualFs` before flushing to disk. This prevents concurrent edit conflicts between multiple agents.

### 4. Resilient LLM Provider Rotation
`synapse_agentic::StochasticRotator` wraps multiple LLM providers (MiniMax, Gemini) with automatic failover. Configured in `init_decision_engine()`.

### 5. Timeline as Source of Truth
Every action emits `TimelineEvent` records. This provides a unified audit trail and enables inter-agent observation sharing.

---

## Error Handling Analysis

### ✅ Strengths

1. **`ConfigError` enum** in `gestalt_core/src/application/config.rs` — well-typed, exhaustive
2. **`thiserror`** used in configuration module for derive-based error types
3. **`anyhow::Result`** used throughout for ergonomic error propagation
4. **VFS lock conflicts** handled explicitly with `LockStatus` enum and timeline events
5. **Git path validation** — `validate_branch_name()` and `validate_git_path()` prevent injection
6. **Hard step cap** prevents infinite loops in autonomous mode

### ⚠️ Issues & Edge Cases

#### Critical

1. **`gestalt_timeline/src/main.rs` — Fallback Empty Provider**
   ```rust
   // Line ~181: Fallback to empty Gemini provider if no API keys configured
   Arc::new(synapse_agentic::prelude::GeminiProvider::new(
       "".into(),
       "gpt-4o".into(),
   ))
   ```
   **Risk**: The fallback provider will silently fail at runtime when `engine.decide()` is called, causing cryptic API errors.
   **Fix**: Require at least one valid API key at startup, or return a `Result` from `init_decision_engine()`.

2. **`runtime.rs` — Timeline Fetch Silently Swallows Errors**
   ```rust
   if let Ok(events) = self.timeline.get_events_since(last_poll_time).await {
       // process events
   }
   // If Err, silently continues with no events
   ```
   **Risk**: Agent loses context of other agents' actions without any warning.
   **Fix**: Log at `warn!` level on failure.

3. **Context Compactor Returns Unvalidated Fallback**
   ```rust
   let compactor_provider = engine.providers().first().cloned().unwrap_or_else(|| {
       Arc::new(synapse_agentic::prelude::GeminiProvider::new(
           "".into(),
           "gpt-4o".into(),
       ))
   });
   ```
   **Risk**: Same as #1 — compaction will fail silently.

#### Moderate

4. **`gestalt_cli/src/main.rs` — Silent Fallback on Config Load**
   ```rust
   let config = CliConfig::load().unwrap_or_default();
   ```
   **Risk**: Missing config silently defaults; user may not realize API keys aren't loaded.
   **Fix**: Log at `warn!` level when falling back to defaults.

5. **`gestalt_timeline/src/main.rs` — Missing API Key Warnings**
   When providers are configured but API keys are missing, the code silently skips initialization without user-facing warning:
   ```rust
   if let Some(api_key) = settings.minimax_api_key.clone() { ... }
   // No else branch warning user
   ```

6. **HTTP Client in CLI — No Timeout/Retry on Network Failure**
   ```rust
   client.post(format!("{}/mcp", url))
       .json(&payload)
       .send()
       .map_err(|e| e.to_string())?;
   ```
   **Risk**: Single attempt; no retry on transient failures.
   **Fix**: Consider `reqwest::Client` with built-in retry middleware.

7. **Dispatcher Subprocess — No Process Cleanup on Panic**
   `spawn_agent()` spawns detached tasks, but if the main process crashes, orphaned child processes may continue running.

#### Minor

8. **`OrchestrationAction::Chat` — Empty Response Handling**
   If the LLM returns an empty `decision.action`, falls through to generic `Chat { response: "..." }` branch. Works but inelegant.

9. **`git_add` with Empty Paths**
   ```rust
   if paths.is_empty() {
       return Ok(ExecutionResult { observation: "git_add requires...", is_success: false });
   }
   ```
   This is handled, but the error message is returned as an observation rather than propagated as an error.

10. **Timeline Event Timestamps — No Clock Skew Handling**
    `FlexibleTimestamp` assumes monotonically increasing time. If the system clock jumps backward, `get_events_since()` may miss events.

---

## Maintainability Recommendations

### 1. Centralize Error Context (`error-chain` or `thiserror`)

Currently errors lose context as they propagate through `?`. Consider a `ContextError` pattern:
```rust
// Instead of:
fs::read_to_string(path)?  // Loses which path failed

// Use:
.read(path)                //.with_context(|| format!("reading config at {:?}", path))?
```

### 2. Add Structured Logging with Correlation IDs

Each `AgentRuntime` run should have a `run_id: String` (ULID) propagated through all async tasks. This makes distributed tracing possible.

### 3. VFS Lock Timeout

Currently locks are held indefinitely until `release_locks()` is called. Implement a TTL:
```rust
pub enum LockStatus {
    Acquired,
    HeldByOther { owner: String, acquired_at: Instant },
    Expired,
}
```

### 4. API Input Validation

`gestalt_timeline/src/services/server.rs` deserializes JSON directly into structs with `#[derive(Deserialize)]` without custom validation. Add `#[serde(validate)]` or manual checks for:
- `OrchestrateRequest.goal` length limits
- `CreateProjectRequest.name` character whitelist
- `ChatRequest.message` size limits

### 5. Health Check Endpoint

Add `GET /health` to the API server that checks:
- SurrealDB connectivity
- At least one valid LLM provider
- VFS lock table state

### 6. Integration Test Coverage

Critical paths lacking tests:
- `AgentRuntime::run_loop()` with mock decision engine
- `ProtocolSyncService` bidirectional sync
- `TaskQueue::run_dispatch_loop()` with bounded workers
- `ContextCompactor::compact()` token estimation accuracy

---

## Security Considerations

1. **Allowed Tool Whitelist** in `CallAgent`:
   ```rust
   let allowed_tools = ["gh", "aws", "kubectl", "cargo", "git", "docker"];
   ```
   This is a good start, but external tools are invoked via shell on Windows (`powershell -Command`). Consider a command allowlist regex.

2. **VFS Path Traversal**: `validate_git_path()` checks for `..`, but `WriteFile` doesn't explicitly validate paths. A malicious LLM could write outside the workspace.

3. **OAuth Token Storage**: `google_pkce.rs` stores tokens in `~/.gestalt/`. Consider OS keyring integration (e.g., `keyring` crate).

---

## Known Issues (from `docs/KNOWN_ISSUES.md`)

- **RUSTSEC-2026-0049**: `rustls-pemfile` unmaintained (transitive dep)
- **RUSTSEC-2026-0002**: `lru` IterMut soundness issue (transitive dep)
- **MCP Tools Gap**: `gestalt_mcp` tools not wired to `gestalt_core` ToolRegistry
- **Mock LLM Providers**: API keys read but HTTP calls not fully implemented

---

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `GEMINI_API_KEY` | Google AI Studio API key | For Gemini provider |
| `MINIMAX_API_KEY` | MiniMax API key | For MiniMax provider |
| `MINIMAX_GROUP_ID` | MiniMax group ID | Optional |
| `SURREAL_URL` | SurrealDB WebSocket URL | Default: embedded |
| `SURREAL_USER` | SurrealDB username | Default: root |
| `SURREAL_PASS` | SurrealDB password | Default: root |
| `GESTALT_HARD_STEP_CAP` | Max steps per autonomous loop | Default: unlimited |
| `GESTALT_MAX_RETRIES` | Max retries per action | Default: 3 |

---

## Testing Strategy

- **Unit tests**: Core domain logic, config parsing, validation functions
- **Integration tests**: Database operations, service wiring
- **E2E tests**: CLI commands, REPL, Nexus daemon
- **Benchmark tests**: RAG operations, context compaction (`cargo bench`)

Run all tests:
```bash
cargo test --workspace --all-targets
```

---

*This document reflects the architecture as of commit `9e01aae` (feat: real API calls for Gemini/MiniMax providers)*
