# Gestalt Timeline & Session Management Analysis

## Executive Summary

The Gestalt Timeline system implements an **event-sourced architecture** using SurrealDB as the persistence layer. It treats the timeline as the central nervous system where every action is recorded as a timestamped event, enabling full traceability and coordination between multiple agents.

---

## 1. Timeline Architecture

### 1.1 Core Philosophy: Timestamp as Primary Variable

**Key Design Principle** (from `timeline_event.rs:3-4`):
```rust
//! The timestamp is the PRIMARY variable of the entire system.
```

Every action in the system is recorded as a `TimelineEvent` with:
- **UTC timestamp** - The immutable ordering mechanism
- **Agent ID** - Who triggered the event
- **Event Type** - What happened (enum with 20+ variants)
- **Project/Task association** - Contextual linkage
- **Payload** - JSON data for extensibility
- **Metadata** - Key-value annotations

### 1.2 Event Types (Complete Taxonomy)

```rust
pub enum EventType {
    // Project lifecycle
    ProjectCreated, ProjectUpdated, ProjectDeleted,
    
    // Task lifecycle
    TaskCreated, TaskStarted, TaskCompleted, TaskFailed, 
    TaskUpdated, TaskDeleted,
    
    // Agent lifecycle
    AgentConnected, AgentDisconnected,
    
    // CLI operations
    CommandExecuted, SubAgentSpawned(String), SubAgentOutput(String),
    SubAgentCompleted(String), SubAgentFailed(String),
    
    // VFS operations
    VfsPatchApplied, VfsLockAcquired, VfsLockConflict,
    VfsFlushStarted, VfsFlushCompleted,
    
    // Communication
    ChatMessage, Retrieval, Chat,
    
    // Extensibility
    Custom(String),
}
```

### 1.3 Event Flow Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     AGENT RUNTIME                           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Agent A    │    │   Agent B    │    │   Agent C    │  │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘  │
└─────────┼──────────────────┼──────────────────┼──────────┘
          │                  │                  │
          ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────┐
│              TIMELINE SERVICE (Event Bus)                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  record_event() → SurrealDB (timeline_events table) │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│              AGENT OBSERVABILITY                            │
│  get_events_since(timestamp) → Poll for cross-agent events  │
└─────────────────────────────────────────────────────────────┘
```

**Critical Pattern**: Agents poll the timeline to discover events from other agents, enabling decentralized coordination through the shared event log.

---

## 2. SurrealDB Schema for Sessions

### 2.1 Database Tables (Complete Schema)

**Location**: `gestalt_timeline/src/db/surreal.rs:70-157`

#### timeline_events (The Event Store)
```sql
DEFINE TABLE timeline_events SCHEMAFULL;
DEFINE FIELD timestamp ON timeline_events TYPE any;
DEFINE FIELD agent_id ON timeline_events TYPE string;
DEFINE FIELD event_type ON timeline_events TYPE string;
DEFINE FIELD project_id ON timeline_events TYPE option<string>;
DEFINE FIELD task_id ON timeline_events TYPE option<string>;
DEFINE FIELD payload ON timeline_events TYPE option<object>;
DEFINE FIELD metadata ON timeline_events TYPE option<object>;

-- Indexes for efficient querying
DEFINE INDEX idx_timestamp ON timeline_events FIELDS timestamp;
DEFINE INDEX idx_project ON timeline_events FIELDS project_id;
DEFINE INDEX idx_agent ON timeline_events FIELDS agent_id;
```

#### agents (Agent Registry)
```sql
DEFINE TABLE agents SCHEMAFULL;
DEFINE FIELD id ON agents TYPE string;
DEFINE FIELD name ON agents TYPE string;
DEFINE FIELD agent_type ON agents TYPE string;  -- cli, vscode_copilot, antigravity, gemini_cli
DEFINE FIELD status ON agents TYPE string;      -- online, idle, busy, offline
DEFINE FIELD connected_at ON agents TYPE any;
DEFINE FIELD last_seen ON agents TYPE any;
DEFINE FIELD command_count ON agents TYPE int;
DEFINE FIELD system_prompt ON agents TYPE option<string>;
DEFINE FIELD model_id ON agents TYPE option<string>;
```

#### agent_runtime_states (Session Persistence)
```sql
DEFINE TABLE agent_runtime_states SCHEMAFULL;
DEFINE FIELD agent_id ON agent_runtime_states TYPE string;
DEFINE FIELD goal ON agent_runtime_states TYPE string;
DEFINE FIELD phase ON agent_runtime_states TYPE string;  -- running, completed, failed
DEFINE FIELD current_step ON agent_runtime_states TYPE int;
DEFINE FIELD max_steps ON agent_runtime_states TYPE int;
DEFINE FIELD last_action ON agent_runtime_states TYPE option<string>;
DEFINE FIELD last_observation ON agent_runtime_states TYPE option<string>;
DEFINE FIELD history_tail ON agent_runtime_states TYPE array;  -- Last 20 messages
DEFINE FIELD error ON agent_runtime_states TYPE option<string>;
DEFINE FIELD started_at ON agent_runtime_states TYPE any;
DEFINE FIELD updated_at ON agent_runtime_states TYPE any;
DEFINE FIELD finished_at ON agent_runtime_states TYPE any;
DEFINE INDEX idx_runtime_agent ON agent_runtime_states FIELDS agent_id UNIQUE;
```

#### projects
```sql
DEFINE TABLE projects SCHEMAFULL;
DEFINE FIELD name ON projects TYPE string;
DEFINE FIELD status ON projects TYPE string;
DEFINE FIELD priority ON projects TYPE int;
DEFINE FIELD created_at ON projects TYPE any;
DEFINE FIELD updated_at ON projects TYPE any;
DEFINE FIELD created_by ON projects TYPE string;
DEFINE INDEX idx_name ON projects FIELDS name UNIQUE;
```

#### tasks
```sql
DEFINE TABLE tasks SCHEMAFULL;
DEFINE FIELD project_id ON tasks TYPE string;
DEFINE FIELD description ON tasks TYPE string;
DEFINE FIELD status ON tasks TYPE string;  -- pending, running, completed, failed, cancelled
DEFINE FIELD created_at ON tasks TYPE any;
DEFINE FIELD updated_at ON tasks TYPE any;
DEFINE FIELD completed_at ON tasks TYPE any;
DEFINE FIELD created_by ON tasks TYPE string;
DEFINE FIELD executed_by ON tasks TYPE option<string>;
DEFINE FIELD duration_ms ON tasks TYPE option<int>;
DEFINE FIELD external_id ON tasks TYPE option<string>;  -- Protocol sync (F1-01 format)
DEFINE INDEX idx_project_id ON tasks FIELDS project_id;
DEFINE INDEX idx_status ON tasks FIELDS status;
DEFINE INDEX idx_external_id ON tasks FIELDS external_id;
```

#### Vector Search Tables (RAG)
```sql
DEFINE TABLE repositories SCHEMAFULL;
DEFINE TABLE documents SCHEMAFULL;
DEFINE TABLE chunks SCHEMAFULL;
DEFINE FIELD embedding ON chunks TYPE option<array<float, 384>>;
DEFINE INDEX idx_chunk_embedding ON chunks FIELDS embedding TYPE HNSW DIMENSION 384 DISTANCE COSINE;
```

### 2.2 Timestamp Handling

**Critical Implementation**: `FlexibleTimestamp` wrapper handles SurrealDB's datetime formats:
- RFC3339 strings
- SurrealDB datetime objects
- Various wrapper formats

```rust
pub struct FlexibleTimestamp(pub DateTime<Utc>);

// Handles deserialization from multiple formats:
// - "2024-01-01T00:00:00Z"
// - {"datetime": "2024-01-01T00:00:00Z"}
// - {"$surreal": {"datetime": "..."}}
```

---

## 3. Event Sourcing Approach

### 3.1 Append-Only Event Log

The timeline_events table is **append-only**. State is derived by replaying events:

```rust
// Recording an event (services/timeline.rs:27-30)
pub async fn record_event(&self, event: TimelineEvent) -> Result<TimelineEvent> {
    debug!("Recording timeline event: {:?}", event.event_type);
    self.db.create("timeline_events", &event).await
}
```

### 3.2 State Reconstruction Pattern

Current state is NOT stored directly; it's computed from events:

```rust
// Get events since a timestamp for cross-agent awareness
pub async fn get_events_since(&self, since: DateTime<Utc>) -> Result<Vec<TimelineEvent>> {
    let query = r#"
        SELECT * FROM timeline_events
        WHERE timestamp > $since
        ORDER BY timestamp ASC
    "#;
    self.db.query_with(query, ("since", since)).await
}
```

### 3.3 Snapshot Pattern for Performance

**AgentRuntimeState** provides snapshots of agent execution:
- Persisted after every action
- Contains last 20 history entries
- Enables crash recovery and progress monitoring

```rust
pub struct AgentRuntimeState {
    pub agent_id: String,
    pub goal: String,
    pub phase: RuntimePhase,  // running, completed, failed
    pub current_step: usize,
    pub max_steps: usize,
    pub history_tail: Vec<String>,  // Circular buffer of recent context
    // ... timestamps and error info
}
```

---

## 4. Session Management Lifecycle

### 4.1 Agent Session Creation

**Location**: `services/agent.rs:178-208`

```rust
pub async fn connect(&self, agent_id: &str, name: Option<&str>) -> Result<Agent> {
    // 1. Check if agent exists (reconnection)
    if let Some(mut existing) = self.get_agent(agent_id).await? {
        existing.status = AgentStatus::Online;
        existing.last_seen = FlexibleTimestamp::now();
        let updated = self.db.update("agents", agent_id, &existing).await?;
        
        // Emit reconnection event
        self.timeline.emit(agent_id, EventType::AgentConnected).await?;
        return Ok(updated);
    }
    
    // 2. Create new agent record
    let agent = Agent::new(agent_id, agent_name, agent_type);
    let created: Agent = self.db.upsert("agents", agent_id, &agent).await?;
    
    // 3. Emit connection event to timeline
    self.timeline.emit(agent_id, EventType::AgentConnected).await?;
    
    Ok(created)
}
```

### 4.2 Session Updates (Heartbeat Pattern)

```rust
pub async fn heartbeat(&self, agent_id: &str) -> Result<()> {
    if let Some(mut agent) = self.get_agent(agent_id).await? {
        agent.last_seen = FlexibleTimestamp::now();
        agent.command_count += 1;
        self.db.update("agents", agent_id, &agent).await?;
    }
    Ok(())
}
```

### 4.3 Session Termination

```rust
pub async fn disconnect(&self, agent_id: &str) -> Result<Option<Agent>> {
    if let Some(mut agent) = self.get_agent(agent_id).await? {
        agent.status = AgentStatus::Offline;
        agent.last_seen = FlexibleTimestamp::now();
        let updated = self.db.update("agents", agent_id, &agent).await?;
        
        self.timeline.emit(agent_id, EventType::AgentDisconnected).await?;
        return Ok(Some(updated));
    }
    Ok(None)
}
```

### 4.4 Runtime State Persistence (During Execution)

**Location**: `services/runtime.rs:467-492`

```rust
async fn persist_state(&self, input: PersistStateInput<'_>) -> Result<()> {
    let mut state = AgentRuntimeState::new(&self.agent_id, input.goal, 
                                          self.hard_step_cap.unwrap_or(0));
    state.phase = input.phase;
    state.current_step = input.step;
    state.last_action = input.last_action.map(|a| format!("{:?}", a));
    state.last_observation = input.last_observation.map(ToOwned::to_owned);
    state.history_tail = input.history.iter().rev().take(20).cloned().collect();
    state.history_tail.reverse();  // Maintain chronological order
    state.started_at = input.started_at;
    state.updated_at = FlexibleTimestamp::now();
    state.finished_at = input.finished_at;
    state.error = input.error;
    
    // Upsert to agent_runtime_states table
    let db = self.watch.db();
    let _saved: AgentRuntimeState = db
        .upsert("agent_runtime_states", &self.agent_id, &state)
        .await?;
    Ok(())
}
```

### 4.5 Cross-Agent Event Discovery

**Critical Pattern** (runtime.rs:263-280):
```rust
// Fetch recent events from other agents to maintain context
if let Ok(events) = self.timeline.get_events_since(last_poll_time).await {
    for event in events {
        if event.agent_id != self.agent_id {
            let mut session = self.session.lock().await;
            session.add_message(Message::new(
                MessageRole::User,
                format!("Observation (from {}): {:?} | {:?}",
                    event.agent_id, event.event_type, event.payload)
            ));
            if event.timestamp.0 > last_poll_time {
                last_poll_time = event.timestamp.0;
            }
        }
    }
}
```

---

## 5. Differences from Traditional Session Management

### 5.1 Traditional Session Management

```
┌──────────┐      ┌──────────────┐      ┌──────────────┐
│  Client  │◄────►│   Session    │      │   Database   │
│          │      │   Store      │◄────►│   (State)    │
└──────────┘      └──────────────┘      └──────────────┘
                       │
                       │ Session ID in cookie/token
                       ▼
              ┌─────────────────┐
              │  Session Data   │
              │  - User ID      │
              │  - Preferences  │
              │  - Cart items   │
              └─────────────────┘
```

**Characteristics**:
- Session state is the source of truth
- Events modify session state
- Sessions are ephemeral (expire)
- Single-user focused

### 5.2 Gestalt Timeline Approach

```
┌──────────┐      ┌──────────────┐      ┌──────────────────────┐
│  Agent   │◄────►│   Timeline   │◄────►│   SurrealDB          │
│          │      │   Service    │      │   (Event Store)      │
└────┬─────┘      └──────────────┘      └──────────────────────┘
     │                                              │
     │                                              │ timeline_events
     │                                              ▼
     │                                   ┌──────────────────┐
     │                                   │ Event Log        │
     │                                   │ - Agent A: started
     │                                   │ - Agent B: file_write
     │                                   │ - Agent A: response
     │                                   │ - Agent C: git_commit
     │                                   └──────────────────┘
     │                                              │
     └──────────────────────────────────────────────┘
                   All agents read shared event log
```

**Characteristics**:
- Event log is the source of truth
- State is derived from replaying events
- Timeline is permanent (audit trail)
- Multi-agent by design
- Temporal queries ("what happened yesterday?")
- Cross-agent observability

### 5.3 Key Differentiators

| Aspect | Traditional | Gestalt Timeline |
|--------|-------------|------------------|
| **Source of Truth** | Session state object | Append-only event log |
| **Temporal Awareness** | Limited (expiration) | Full history preserved |
| **Multi-Agent** | Not designed for | Core design principle |
| **Audit Trail** | Optional logging | Intrinsic to design |
| **Recovery** | Session recreation | Event replay possible |
| **Query Pattern** | Current state only | Temporal queries supported |

---

## 6. Technical Issues & Limitations

### 6.1 Critical Issues

#### Issue 1: Polling-Based Event Discovery (Performance)

**Location**: `services/runtime.rs:264`

**Problem**: Agents poll `get_events_since()` on every loop iteration
```rust
// This happens on EVERY elastic step
if let Ok(events) = self.timeline.get_events_since(last_poll_time).await {
```

**Impact**: 
- N+1 query problem with many agents
- Database load increases with agent count
- Latency in cross-agent communication

**Recommendation**: Implement WebSocket/Live Query for real-time event streaming

#### Issue 2: Event Payload Schema is Unstructured

**Location**: `models/timeline_event.rs:41-42`

**Problem**: Payload is generic JSON
```rust
/// Event payload data
#[serde(default)]
pub payload: serde_json::Value,
```

**Impact**:
- No type safety for event data
- Schema evolution is difficult
- Querying events by payload content requires JSON path queries

**Recommendation**: Implement typed event payloads using enums with structured data

#### Issue 3: Missing Event Replay Capabilities

**Problem**: No infrastructure to rebuild state from event history

**Current State**: 
- Events are recorded but not used for state reconstruction
- AgentRuntimeState is stored as snapshots, not derived from events
- No event sourcing projection system

**Recommendation**: Implement projection handlers for read model reconstruction

#### Issue 4: No Event Versioning

**Problem**: Event schema changes will break deserialization

**Current State**: Direct deserialization without version checking

**Recommendation**: Add event schema versioning and migration support

### 6.2 Moderate Issues

#### Issue 5: Limited Query Capabilities

**Location**: `services/timeline.rs:63-83`

Current queries only support:
- Time range filtering
- Project ID filtering
- Agent ID filtering (via index)

**Missing**:
- Event type filtering
- Complex boolean queries
- Aggregation queries (count by type, etc.)
- Full-text search on payload

#### Issue 6: No Event Compaction Strategy

**Problem**: timeline_events table will grow indefinitely

**Current State**: No archiving or compaction strategy

**Recommendation**: Implement event retention policies and compaction for old events

#### Issue 7: Race Condition in Agent Status Updates

**Location**: `services/agent.rs:186-207`

```rust
// Check if agent exists
if let Some(mut existing) = self.get_agent(agent_id).await? {
    // Update existing agent
    existing.status = AgentStatus::Online;
    let updated = self.db.update("agents", agent_id, &existing).await?;
```

**Problem**: Not atomic - read-then-update pattern can lose updates

### 6.3 Architectural Limitations

#### Limitation 1: Single Database Bottleneck

All agents connect to single SurrealDB instance. No sharding or federation support.

#### Limitation 2: Synchronous Event Recording

```rust
self.db.create("timeline_events", &event).await  // Blocks until persisted
```

Events are recorded synchronously, adding latency to agent operations.

#### Limitation 3: No Event Streaming/Broadcasting

New events are not pushed to subscribers; they must poll.

---

## 7. Recommendations for SWAL Agent Bus

### 7.1 Immediate Improvements (Phase 1)

#### 7.1.1 Implement Live Query for Real-Time Events

```rust
// Add to TimelineService
pub async fn subscribe_to_events(&self, since: DateTime<Utc>) -> Result<EventStream> {
    let query = r#"
        LIVE SELECT * FROM timeline_events
        WHERE timestamp > $since
        ORDER BY timestamp ASC
    "#;
    self.db.live_query(query, ("since", since)).await
}
```

**Benefit**: Eliminates polling, reduces DB load, enables real-time coordination

#### 7.1.2 Add Typed Event Payloads

```rust
pub enum EventPayload {
    TaskCreated { task_id: String, description: String },
    FileModified { path: String, checksum: String },
    AgentMessage { content: String, recipient: Option<String> },
    // ... etc
}

pub struct TimelineEvent {
    // ... existing fields
    pub payload: EventPayload,  // Typed instead of JSON
}
```

#### 7.1.3 Implement Event Versioning

```rust
pub struct TimelineEvent {
    pub schema_version: u32,  // Add version field
    // ... existing fields
}

// Deserialization with migration
impl TimelineEvent {
    pub fn from_json(json: Value) -> Result<Self> {
        let version = json.get("schema_version").and_then(|v| v.as_u64()).unwrap_or(1);
        match version {
            1 => Self::from_v1(json),
            2 => Self::from_v2(json),
            // ... migrations
        }
    }
}
```

### 7.2 Core Bus Enhancements (Phase 2)

#### 7.2.1 Implement Message Bus Abstraction

```rust
#[async_trait]
pub trait AgentBus: Send + Sync {
    /// Publish an event to the bus
    async fn publish(&self, event: BusEvent) -> Result<()>;
    
    /// Subscribe to events matching a pattern
    async fn subscribe(&self, filter: EventFilter) -> Result<EventStream>;
    
    /// Request-response pattern for direct agent communication
    async fn request(&self, target: &str, request: Request) -> Result<Response>;
    
    /// Get event history with cursor-based pagination
    async fn history(&self, cursor: Option<EventCursor>, limit: usize) -> Result<EventPage>;
}

pub struct BusEvent {
    pub id: Ulid,  // Lexicographically sortable
    pub timestamp: DateTime<Utc>,
    pub source: AgentId,
    pub target: Option<AgentId>,  // None = broadcast
    pub topic: String,  // For filtering: "vfs.changes", "agent.messages", etc.
    pub payload: EventPayload,
    pub correlation_id: Option<String>,  // For tracing request chains
}
```

#### 7.2.2 Add Event Projections for Fast Queries

```rust
// Projection that maintains current state derived from events
pub struct ProjectProjection {
    pub project_id: String,
    pub name: String,
    pub status: ProjectStatus,
    pub task_count: usize,
    pub completed_tasks: usize,
    pub last_activity: DateTime<Utc>,
}

// Auto-updated by event handlers
pub async fn handle_project_created(event: TimelineEvent) -> Result<()> {
    let projection = ProjectProjection::from_event(&event)?;
    db.upsert("project_projections", &projection.project_id, &projection).await?;
    Ok(())
}
```

#### 7.2.3 Implement Agent Discovery Service

```rust
pub struct AgentDiscovery {
    db: SurrealClient,
}

impl AgentDiscovery {
    /// Find agents by capability
    pub async fn find_agents_with_capability(&self, capability: &str) -> Result<Vec<Agent>> {
        let query = r#"
            SELECT * FROM agents 
            WHERE status = 'online' 
            AND capabilities CONTAINS $capability
            ORDER BY last_seen DESC
        "#;
        self.db.query_with(query, ("capability", capability)).await
    }
    
    /// Get agent load metrics
    pub async fn get_agent_load(&self, agent_id: &str) -> Result<AgentLoad> {
        // Aggregate from timeline events
        let query = r#"
            SELECT count() as active_tasks 
            FROM timeline_events 
            WHERE agent_id = $agent_id 
            AND event_type = 'task_started'
            AND timestamp > $recent
        "#;
        // ... implementation
    }
}
```

### 7.3 Advanced Features (Phase 3)

#### 7.3.1 Implement Event Sourcing with CQRS

```rust
// Command side - append events
pub async fn execute_command(cmd: Command) -> Result<Vec<TimelineEvent>> {
    let events = cmd.execute().await?;
    for event in &events {
        timeline.record_event(event.clone()).await?;
    }
    Ok(events)
}

// Query side - read from projections
pub async fn get_project_status(project_id: &str) -> Result<ProjectStatus> {
    // Read from projection, not event log
    db.select_by_id("project_projections", project_id).await
}
```

#### 7.3.2 Add Event Replay Capabilities

```rust
pub async fn replay_events(
    &self,
    since: DateTime<Utc>,
    handler: impl Fn(TimelineEvent) -> Result<()>
) -> Result<()> {
    let events = self.get_events_since(since).await?;
    for event in events {
        handler(event)?;
    }
    Ok(())
}

// Usage: Rebuild read models from scratch
pub async fn rebuild_projections(&self) -> Result<()> {
    self.clear_projections().await?;
    self.replay_events(EPOCH, |event| {
        projection_handlers.handle(event)
    }).await?;
}
```

#### 7.3.3 Implement Distributed Timeline

```rust
pub struct DistributedTimeline {
    local_node: NodeId,
    nodes: Vec<NodeId>,
    replication: ReplicationStrategy,
}

impl DistributedTimeline {
    /// Replicate event to other nodes
    async fn replicate_event(&self, event: &TimelineEvent) -> Result<()> {
        for node in &self.nodes {
            if node != &self.local_node {
                node.send_event(event).await?;
            }
        }
        Ok(())
    }
    
    /// Resolve conflicts using vector clocks
    async fn resolve_conflicts(&self) -> Result<()> {
        // ... CRDT or vector clock implementation
    }
}
```

### 7.4 SWAL Integration Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      SWAL AGENT BUS                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────┐ │
│  │  Agent A    │  │  Agent B    │  │  Agent C    │  │  ...    │ │
│  │  (Copilot)  │  │  (Codex)    │  │  (Claude)   │  │         │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └────┬────┘ │
│         │                │                │              │      │
│         └────────────────┴────────────────┴──────────────┘      │
│                                │                                 │
│                    ┌───────────▼────────────┐                   │
│                    │   Message Broker       │                   │
│                    │   (SurrealDB Live      │                   │
│                    │    Query / WebSocket)  │                   │
│                    └───────────┬────────────┘                   │
│                                │                                 │
│         ┌──────────────────────┼──────────────────────┐         │
│         ▼                      ▼                      ▼         │
│  ┌──────────────┐   ┌──────────────────┐   ┌──────────────┐    │
│  │ Event Store  │   │   Projections    │   │  Discovery   │    │
│  │ (Timeline)   │   │   (Read Models)  │   │  Service     │    │
│  └──────────────┘   └──────────────────┘   └──────────────┘    │
│                                                                │
└─────────────────────────────────────────────────────────────────┘
```

**Key Capabilities**:
1. **Publish-Subscribe**: Agents publish events, subscribe to relevant topics
2. **Request-Response**: Direct agent-to-agent communication with correlation IDs
3. **Event Persistence**: Complete audit trail of all agent interactions
4. **Discovery**: Find agents by capabilities, status, or load
5. **Observability**: Real-time monitoring of agent activities

### 7.5 Implementation Roadmap

| Phase | Feature | Priority | Est. Effort |
|-------|---------|----------|-------------|
| 1a | Live Query Integration | Critical | 2 days |
| 1b | Typed Event Payloads | High | 3 days |
| 1c | Event Versioning | High | 2 days |
| 2a | Message Bus Abstraction | High | 4 days |
| 2b | Read Projections | Medium | 3 days |
| 2c | Agent Discovery | Medium | 2 days |
| 3a | CQRS Implementation | Medium | 5 days |
| 3b | Event Replay | Medium | 3 days |
| 3c | Distributed Timeline | Low | 8 days |

---

## 8. Conclusion

The Gestalt Timeline system provides a solid foundation for event-sourced agent coordination. Its append-only event log, timestamp-centric design, and SurrealDB persistence create a reliable audit trail and enable cross-agent observability.

**Strengths**:
- Clean event-driven architecture
- Flexible schema with JSON payloads
- Comprehensive event taxonomy
- Runtime state persistence for recovery
- Vector search integration for RAG

**Critical Gaps for SWAL Bus**:
1. Polling-based event discovery needs replacement with push-based delivery
2. Event schema needs to be typed and versioned
3. Read projections needed for performance
4. Agent discovery service missing
5. No request-response pattern for direct communication

The timeline should indeed become the central nervous system for SWAL agent communication, but requires Phase 1 and Phase 2 improvements before it can serve as a production-grade message bus.

---

*Analysis completed: March 31, 2026*
*Scope: gestalt_timeline module (Rust)*
*Database: SurrealDB*
