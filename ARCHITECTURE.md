# Gestalt-Rust Architecture

> Last Updated: 2026-04-16
> Status: v1.0 — CLI/Swarm/Core only (UI/MCP/infra removed)

## Overview

Gestalt is a high-performance Rust workspace for AI agent orchestration. It provides VFS isolation, Swarm parallel execution, timeline-based state in SurrealDB, and a tool registry — all accessible via CLI/REPL.

**Scope**: CLI-first orchestration. No UI, no standalone MCP server, no Flutter app.

---

## Crates

### gestalt_core (42 .rs files)
The hexagonal core. Contains domain models, port traits, and business logic.

```
src/
├── adapters/
│   ├── auth/          # Google OAuth2 + PKCE
│   └── mcp/           # MCP client (connects to external servers)
├── application/
│   ├── agent/         # Tool implementations (git, shell, file, search...)
│   ├── config.rs      # GestaltConfig from env
│   ├── indexer.rs     # Code indexer with SurrealDB
│   └── mcp_service.rs # MCP server config loader
├── context/           # Scanner, workspace analysis
├── domain/
│   └── rag/          # Embeddings (DummyEmbeddingModel)
├── mcp/
│   ├── client_impl.rs # MCP client implementation
│   └── registry.rs   # MCP tool registry
├── ports/
│   ├── inbound/       # Port traits (incoming)
│   └── outbound/      # Port traits (outgoing)
│       ├── repo_manager.rs  # Repo + VectorDB traits
│       └── vfs.rs     # VfsPort + OverlayFs + MemoryFs
└── swarm/
    ├── coordinator.rs # SwarmCoordinator
    ├── agent.rs       # Agent trait
    ├── health.rs      # HealthMonitor
    └── registry.rs    # AgentRegistry
```

### gestalt_timeline (37 .rs files)
Primary orchestration engine. The `gestalt` binary.

- `TimelineService` — event sourcing with SurrealDB
- `ProjectService` — project CRUD
- `TaskService` — task management
- Initializes: SurrealDB, VFS, LLM providers, tool registry, SwarmCoordinator

### gestalt_cli (4 .rs files)
Standalone CLI binary with REPL.

- `main.rs` — CLI entry point with subcommands
- `repl.rs` — InteractiveRepl with command handling

### gestalt_swarm (1 .rs file)
Swarm coordinator for parallel agent execution.

- `SwarmCoordinator` — dispatch tasks to N agents in parallel
- `AgentRegistry` — register/list agents
- `TaskQueue` — priority queue for work distribution
- `HealthMonitor` — monitor agent heartbeats

### synapse-agentic (1 + generated)
Tool registry and agentic primitives.

- `Tool` + `ToolContext` traits
- `ToolRegistry` — register/dispatch tools
- `Hive` — actor system for sub-agents
- `LLMProvider` trait — OpenAI + Anthropic adapters
- `StochasticRotator` — LLM provider failover

---

## VFS Architecture

```
VfsPort (trait)
├── MemoryFs        — in-memory file system
└── OverlayFs       — layered merge (upper + lower)
    ├── read        — upper first, then lower
    ├── write       — upper only
    └── merge       — explicit overlay merge
```

- FileWatchService — debounced file system watcher
- Used for agent workspace isolation

---

## Communication Patterns

### Overview

Gestalt Rust implements three distinct communication patterns between agents, depending on the execution context:

| Pattern | Mechanism | Use Case |
|---------|-----------|----------|
| **In-Process** | `tokio::spawn` + shared `Arc<>` state | Swarm (parallel agents, same binary) |
| **Timeline-Based** | SurrealDB events + `TimelineService` | All agents (cross-session, persistent) |
| **Process Spawning** | `tokio::process::Command` + stdout/stderr capture | External CLI agents (codex, claude) |

---

### 1. Swarm Communication (In-Process)

**Context:** `gestalt_swarm/src/main.rs` — multiple agents within the same binary

```
┌─────────────────────────────────────────────────────────────┐
│                      Main Process                           │
│                                                             │
│  ┌──────────────┐     ┌──────────────┐     ┌────────────┐ │
│  │  Semaphore   │     │  RwLock<Vec>  │     │   watch    │ │
│  │  (concurrency │     │  (results)   │     │  channel   │ │
│  │   limiter)   │     └──────┬───────┘     └─────┬───────┘ │
│  └──────────────┘            │                    │         │
│                             │                    │         │
│    ┌────────────────────────┼────────────────────┘         │
│    │                        │                              │
│    ▼                        ▼                              ▼
│  [Agent 0]              [Agent 1]              [Agent N]   │
│  tokio::spawn        tokio::spawn           tokio::spawn     │
│  ┌────────┐         ┌────────┐             ┌────────┐      │
│  │Health  │◄────────│Monitor │             │Recovery│      │
│  │beat    │         │(RwLock)│             │Manager │      │
│  └────────┘         └────────┘             └────────┘      │
│        │                                       ▲           │
│        └───────────────────────────────────────┘ mpsc    │
│                    RecoveryAction                          │
└─────────────────────────────────────────────────────────────┘
```

**Key Primitives:**

| Primitive | Type | Purpose |
|-----------|------|---------|
| `Semaphore` | `tokio::sync::Semaphore` | Bounds concurrent LLM calls |
| `Arc<RwLock<Vec<AgentResult>>>` | Shared results collection | All agents write here |
| `mpsc::channel<RecoveryAction>` | Producer/consumer | Health → Recovery manager |
| `watch::channel<HealthEvent>` | Pub/sub | Health events to subscribers |
| `tokio::spawn` | Task spawning | Each agent runs in isolated task |

**Code pattern:**

```rust
// Shared state (Arc-wrapped for cloning into each task)
let semaphore = Arc::new(Semaphore::new(args.max_concurrency));
let results: Arc<RwLock<Vec<AgentResult>>> = Arc::new(RwLock::new(Vec::new()));

// Health monitor + recovery channel
let (recovery_tx, mut recovery_rx) = mpsc::channel::<RecoveryAction>(100);
let monitor = Arc::new(SwarmHealthMonitor::new(health_config));

// Spawn agent tasks
for agent_id in 0..args.agents {
    let handle = tokio::spawn(async move {
        run_agent(
            agent_id, goal, cwd, provider, model,
            semaphore.clone(),    // Arc<Semaphore> clones cheaply
            results.clone(),      // Arc<RwLock<Vec>> clones cheaply
            quiet, monitor.clone(),
            recovery_tx.clone(),
            restart_counters[agent_id].clone(),
        ).await;
    });
    handles.push(handle);
}
```

**Recovery flow:**

```
HealthChecker (background task)
    │
    ▼ (watch channel)
SwarmHealthMonitor.events_tx.broadcast(HealthEvent::AgentUnhealthy)
    │
    ▼
RecoveryManager.handle_event()
    │
    ▼ (mpsc::Sender)
recovery_tx.send(RecoveryAction { agent_id, action: Restart })
    │
    ▼ (recovery_rx.recv() loop in main)
match action:
  - Restart → increment counter, spawn replacement
  - Kill   → monitor.mark_dead(agent_id)
```

---

### 2. Timeline-Based Communication (Persistent, Cross-Session)

**Context:** `gestalt_timeline/src/services/timeline.rs` — all agents communicate via events in SurrealDB

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│  Agent A    │         │  Timeline    │         │  Agent B    │
│             │ emit()  │  Service     │ poll()  │             │
│             ├────────►│              │◄────────┤             │
│             │         │  (SurrealDB) │         │             │
└─────────────┘         └──────────────┘         └─────────────┘
                              │
                              ▼ (optional)
                        ┌─────────────┐
                        │   Cortex    │  (external memory sync)
                        │  (HTTP)     │
                        └─────────────┘
```

**Core operations:**

```rust
// Record an event
self.timeline.emit(agent_id, EventType::SubAgentCompleted).await?;

// Poll events since last known timestamp (for cross-agent awareness)
let events = self.timeline.get_events_since(last_poll_time).await?;

// Event carries: agent_id, event_type, project_id, task_id, payload, timestamp
```

**Agent loop uses timeline for cross-agent context:**

```rust
// In AgentRuntime::run_loop — fetch events from other agents
if let Ok(events) = self.timeline.get_events_since(last_poll_time).await {
    for event in events {
        if event.agent_id != self.agent_id {
            // Inject other agents' observations into session context
            session.add_message(Message::new(
                MessageRole::User,
                format!("Observation (from {}): {:?}", event.agent_id, event.event_type)
            ));
        }
    }
}
```

**AgentService (gestalt_timeline/src/services/agent.rs)** provides connection management:

```rust
// Agent connects → registered in DB with status, last_seen, command_count
service.connect(agent_id, Some("Name")).await?;

// Agent heartbeat → updates timestamp, increments command_count
service.heartbeat(agent_id).await?;

// Agent disconnects → status = Offline
service.disconnect(agent_id).await?;
```

---

### 3. Process-Spawn Communication (External Agents)

**Context:** `gestalt_timeline/src/services/dispatcher.rs` — spawn headless CLI agents

```
Main Process                  DispatcherService                    Child Process
     │                              │                                    │
     │ spawn_agent()                │                                    │
     │─────────────────────────────►│                                    │
     │                              │                                    │
     │                              │  tokio::spawn(async move {          │
     │                              │    Command::new(agent_type)        │
     │                              │    .arg(prompt)                     │
     │                              │────────────────────────────────────►│
     │                              │                                    │
     │                              │◄──────── stdout/stderr             │
     │                              │    (BufReader lines)               │
     │                              │                                    │
     │  returns task_name           │                                    │
     │◄─────────────────────────────│                                    │
     │                              │                                    │
```

**Code pattern:**

```rust
pub async fn spawn_agent(&self, agent_type: &str, prompt: &str) -> Result<String> {
    // Record spawn event
    self.timeline.emit(agent_type, EventType::SubAgentSpawned).await?;

    tokio::spawn(async move {
        let mut cmd = Command::new(agent_type);
        cmd.arg(prompt);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Capture stdout/stderr, write to log file + timeline
        let mut stdout_reader = BufReader::new(stdout).lines();
        
        loop {
            tokio::select! {
                result = stdout_reader.next_line() => {
                    // Emit SubAgentOutput to timeline
                    timeline.emit(agent_type, EventType::SubAgentOutput(line)).await;
                }
            }
        }
    });

    Ok(task_name) // Returns immediately, process runs in background
}
```

---

### 4. Sub-Agent Delegation (Intra-Process)

**Context:** `AgentRuntime::OrchestrationAction::DelegateTask` — parent spawns child agent

```rust
OrchestrationAction::DelegateTask { agent, goal } => {
    let sub_agent_id = format!("{}-{}", self.agent_id, agent);
    let sub_runtime = AgentRuntime::new(...).with_parent(self.agent_id.clone());
    
    // Spawn sub-agent as background task
    spawn_sub_agent(sub_runtime, goal.clone());
    
    // Emit event to timeline
    timeline.emit(&self.agent_id, EventType::SubAgentSpawned).await;
}

fn spawn_sub_agent(sub_runtime: AgentRuntime, goal: String) {
    tokio::spawn(async move {
        sub_runtime.run_loop(&goal).await;
    });
}
```

Parent receives completion via timeline event polling (SubAgentCompleted/SubAgentFailed).

---

### 5. Hive Actor System (Internal to synapse-agentic)

**Context:** `Hive` from synapse-agentic — used for reviewer agent (see `reviewer_merge_agent.rs`)

```rust
// Reviewer agent uses Hive (actor system) for internal messaging
let (reply_tx, reply_rx) = oneshot::channel();
let mut hive = self.hive.lock().await;
spawn_reviewer_agent(&mut hive);

handle.send(ReviewerMessage::ReviewAndMerge { goal, reply: reply_tx }).await?;
let result = reply_rx.await?;
```

**Hive primitive** (synapse-agentic): actor-based message passing with `Inbox<T>` channels.

---

### Communication Summary Table

| Scenario | Mechanism | Guarantees |
|----------|-----------|------------|
| Swarm agents (parallel) | `Arc<RwLock>` + `mpsc` | No strict ordering; best-effort health |
| Agent ↔ Timeline | SurrealDB + `emit()` | Durable, queryable |
| External sub-agents | `tokio::process::Command` | Fire-and-forget, stdout captured to timeline |
| Parent ↔ Child agent | `tokio::spawn` + timeline events | Child completes asynchronously |
| Reviewer agent | `Hive` actors + `oneshot` | Request/response with reply channel |

---

## Swarm Architecture

```
SwarmCoordinator
├── TaskQueue (priority queue)
├── AgentRegistry (registered agents)
└── HealthMonitor (heartbeat tracking)

Agent (trait)
└── execute(task) -> Result<Value>

Agent implementations:
├── CliAgent (execute shell commands)
└── LlmAgent (LLM-powered agent)
```

---

## MCP Client (not standalone server)

`gestalt_core/adapters/mcp/` contains an MCP **client** that:
- Connects to external MCP servers via HTTP
- Loads server configs from `config/mcp.json`
- Exposes tools from remote servers as `Tool` implementations

The **standalone MCP server** (`gestalt_mcp` crate) was removed.

---

## State Management

- **SurrealDB** — timeline events, projects, tasks, agent state
- **VFS** — file system snapshots per agent session
- **In-memory** — tool registry, agent registry, task queues

---

## Removed in 2026-04-16

These crates were removed (out of scope for CLI-first orchestration):

- `gestalt_app` — Flutter app
- `gestalt_terminal` — TUI
- `gestalt_ui` — UI components
- `gestalt_mcp` (server) — standalone MCP server
- `gestaltctl` — standalone admin binary
- `gestalt_infra_github` — GitHub infra adapter
- `gestalt_infra_embeddings` — BERT embedding infra
- `benchmarks` — standalone benchmark suite

---

## Dependencies

```
gestalt_core
├── surrealdb = "2.6.1"         # Database
├── synapse-agentic              # Tool registry
├── tokio = { features = "full" }
├── serde_json
├── reqwest (for MCP client)
└── oauth2 / jsonwebtoken (auth)

gestalt_timeline
├── gestalt_core
├── tokio
├── teloxide (optional, telegram)

gestalt_cli
├── gestalt_core
├── tokio
└── reedline (REPL)

gestalt_swarm
├── gestalt_core
├── tokio
└── tokio-sync

synapse-agentic
├── tokio
└── async-trait
```
