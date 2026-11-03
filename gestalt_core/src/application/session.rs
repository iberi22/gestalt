//! Durable, typed lifecycle model for delegated work.
//!
//! Mirrors the `Session → Thread → Run → ExecutionNode` hierarchy used by
//! T3code-style orchestration:
//!
//! ```text
//! Session (an environment / connection to a provider)
//!   └── Thread (a conversation or workspace thread)
//!         └── Run (one execution attempt)
//!               └── ExecutionNode (one tool call / step inside a run)
//! ```
//!
//! This module provides the types plus an in-memory [`SessionStore`]. There is
//! no persistence layer yet: the store keeps everything behind an
//! `Arc<RwLock<..>>` so it is cheap to clone and share across tasks.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Generates a fresh, globally unique identifier.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            /// Borrows the identifier as a plain string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

define_id! {
    /// Identifier of a [`Session`].
    SessionId
}

define_id! {
    /// Identifier of a [`Thread`].
    ThreadId
}

define_id! {
    /// Identifier of a [`Run`].
    RunId
}

define_id! {
    /// Identifier of an [`ExecutionNode`].
    NodeId
}

// ---------------------------------------------------------------------------
// Timestamp serde helpers
// ---------------------------------------------------------------------------

// `chrono` is available in this crate but its `serde` feature is not enabled,
// so timestamps are encoded as RFC 3339 strings explicitly. This keeps the wire
// format stable and readable without adding a dependency or a feature flag.
mod ts {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &DateTime<Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_rfc3339())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        let raw = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&raw)
            .map(|timestamp| timestamp.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

mod ts_opt {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<DateTime<Utc>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(timestamp) => serializer.serialize_some(&timestamp.to_rfc3339()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<DateTime<Utc>>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|raw| {
                DateTime::parse_from_rfc3339(&raw)
                    .map(|timestamp| timestamp.with_timezone(&Utc))
                    .map_err(serde::de::Error::custom)
            })
            .transpose()
    }
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Why a [`Session`] was closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloseReason {
    /// Every thread finished successfully.
    Completed,
    /// The session ended because of an error.
    Failed { message: String },
    /// The session was closed by the user or operator.
    Cancelled,
}

/// Lifecycle state of a [`Session`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// The session is currently in use.
    Active,
    /// The session is open but has no activity in flight.
    Idle,
    /// The session is finished and will not be reused.
    Closed { reason: CloseReason },
}

/// An environment / connection to a provider (claude, opencode, agy, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    /// CLI/provider name, e.g. `claude`, `opencode`, `agy`, `codex`, `wrangler`.
    pub provider: String,
    /// Working directory the provider was launched in.
    pub cwd: PathBuf,
    #[serde(with = "ts")]
    pub created_at: DateTime<Utc>,
    pub state: SessionState,
    /// Threads spawned inside this session, in creation order.
    pub threads: Vec<ThreadId>,
}

// ---------------------------------------------------------------------------
// Thread
// ---------------------------------------------------------------------------

/// Lifecycle state of a [`Thread`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadState {
    /// Created, no run started yet.
    Pending,
    /// At least one run is in flight.
    Running,
    /// The last run succeeded.
    Succeeded,
    /// The last run failed or timed out.
    Failed,
    /// The last run was cancelled.
    Cancelled,
}

/// A conversation / workspace thread owned by a [`Session`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    pub id: ThreadId,
    pub session_id: SessionId,
    /// Optional human-readable title for the thread.
    pub title: Option<String>,
    /// Every run started on this thread, in attempt order.
    pub runs: Vec<RunId>,
    #[serde(with = "ts")]
    pub created_at: DateTime<Utc>,
    pub state: ThreadState,
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

/// Lifecycle state of a [`Run`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    /// Accepted but not executing yet.
    Queued,
    /// Currently executing.
    Running,
    /// Finished without error.
    Succeeded,
    /// Finished with an error.
    Failed { message: String },
    /// Aborted before completion.
    Cancelled,
    /// Exceeded its deadline.
    TimedOut,
}

impl RunState {
    /// Whether a run in this state will never progress again.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed { .. } | Self::Cancelled | Self::TimedOut
        )
    }
}

/// One execution attempt of a [`Thread`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub thread_id: ThreadId,
    /// 1-based attempt number; increments on every retry.
    pub attempt: u32,
    #[serde(with = "ts")]
    pub started_at: DateTime<Utc>,
    #[serde(default, with = "ts_opt")]
    pub finished_at: Option<DateTime<Utc>>,
    pub state: RunState,
    /// Execution steps recorded for this run, in order.
    pub nodes: Vec<NodeId>,
}

impl Run {
    /// Time elapsed between start and finish.
    ///
    /// Returns `None` while the run has not finished yet.
    #[must_use]
    pub fn duration(&self) -> Option<Duration> {
        self.finished_at
            .and_then(|finished| (finished - self.started_at).to_std().ok())
    }

    /// Whether the run reached a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

// ---------------------------------------------------------------------------
// ExecutionNode
// ---------------------------------------------------------------------------

/// What kind of step an [`ExecutionNode`] represents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// A call to a named tool.
    ToolCall { name: String },
    /// A model or user message.
    Message,
    /// A write to a file in the workspace.
    FileEdit { path: PathBuf },
    /// A shell invocation.
    Shell { command: String },
    /// Internal reasoning without observable side effects.
    Thinking,
}

/// Lifecycle state of an [`ExecutionNode`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    /// Created but not executing yet.
    Pending,
    /// Currently executing.
    Running,
    /// Finished without error.
    Ok,
    /// Finished with an error.
    Error { message: String },
}

impl NodeState {
    /// Whether a node in this state will never progress again.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Ok | Self::Error { .. })
    }
}

/// A single tool call / step inside a [`Run`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: NodeId,
    pub run_id: RunId,
    pub kind: NodeKind,
    #[serde(with = "ts")]
    pub started_at: DateTime<Utc>,
    #[serde(default, with = "ts_opt")]
    pub finished_at: Option<DateTime<Utc>>,
    pub state: NodeState,
}

// ---------------------------------------------------------------------------
// In-memory store
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct StoreInner {
    sessions: HashMap<SessionId, Session>,
    threads: HashMap<ThreadId, Thread>,
    runs: HashMap<RunId, Run>,
    nodes: HashMap<NodeId, ExecutionNode>,
}

/// Cheap-to-clone, thread-safe in-memory store for the session hierarchy.
///
/// Clones share the same `Arc<RwLock<..>>`, so every clone observes the same
/// writes. There is no persistence: dropping the last clone drops the state.
#[derive(Debug, Clone, Default)]
pub struct SessionStore {
    inner: Arc<RwLock<StoreInner>>,
}

fn unknown(kind: &str, id: impl fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("unknown {kind} id: {id}")
}

fn thread_state_for(state: &RunState) -> ThreadState {
    match state {
        RunState::Queued | RunState::Running => ThreadState::Running,
        RunState::Succeeded => ThreadState::Succeeded,
        RunState::Failed { .. } | RunState::TimedOut => ThreadState::Failed,
        RunState::Cancelled => ThreadState::Cancelled,
    }
}

impl SessionStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, StoreInner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, StoreInner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a new session for `provider` running in `cwd`.
    ///
    /// # Errors
    /// Currently never fails; the signature stays fallible so callers do not
    /// change if validation is added later.
    pub fn create_session(
        &self,
        provider: impl Into<String>,
        cwd: impl Into<PathBuf>,
    ) -> Result<SessionId> {
        let id = SessionId::new();
        let session = Session {
            id: id.clone(),
            provider: provider.into(),
            cwd: cwd.into(),
            created_at: Utc::now(),
            state: SessionState::Active,
            threads: Vec::new(),
        };
        self.write().sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Creates a thread inside `session`.
    ///
    /// # Errors
    /// Returns an error when `session` is unknown.
    pub fn create_thread(&self, session: SessionId, title: Option<String>) -> Result<ThreadId> {
        let id = ThreadId::new();
        let thread = Thread {
            id: id.clone(),
            session_id: session.clone(),
            title,
            runs: Vec::new(),
            created_at: Utc::now(),
            state: ThreadState::Pending,
        };

        let mut inner = self.write();
        {
            let parent = inner
                .sessions
                .get_mut(&session)
                .ok_or_else(|| unknown("session", &session))?;
            parent.state = SessionState::Active;
            parent.threads.push(id.clone());
        }
        inner.threads.insert(id.clone(), thread);
        Ok(id)
    }

    /// Starts a new run on `thread`, bumping the attempt counter.
    ///
    /// # Errors
    /// Returns an error when `thread` is unknown.
    pub fn start_run(&self, thread: ThreadId) -> Result<RunId> {
        let id = RunId::new();
        let mut inner = self.write();
        let attempt = {
            let parent = inner
                .threads
                .get_mut(&thread)
                .ok_or_else(|| unknown("thread", &thread))?;
            parent.runs.push(id.clone());
            parent.state = ThreadState::Running;
            u32::try_from(parent.runs.len()).unwrap_or(u32::MAX).max(1)
        };

        let run = Run {
            id: id.clone(),
            thread_id: thread,
            attempt,
            started_at: Utc::now(),
            finished_at: None,
            state: RunState::Queued,
            nodes: Vec::new(),
        };
        inner.runs.insert(id.clone(), run);
        Ok(id)
    }

    /// Moves `run` to a terminal state and records its finish time.
    ///
    /// The owning thread inherits the outcome.
    ///
    /// # Errors
    /// Returns an error when `run` is unknown, `state` is not terminal, or the
    /// run was already finished.
    pub fn finish_run(&self, run: RunId, state: RunState) -> Result<()> {
        if !state.is_terminal() {
            bail!("run state {state:?} is not terminal");
        }

        let mut inner = self.write();
        let thread_id = {
            let entry = inner
                .runs
                .get_mut(&run)
                .ok_or_else(|| unknown("run", &run))?;
            if entry.finished_at.is_some() {
                bail!("run {run} is already finished");
            }
            entry.finished_at = Some(Utc::now());
            entry.state = state.clone();
            entry.thread_id.clone()
        };

        if let Some(thread) = inner.threads.get_mut(&thread_id) {
            thread.state = thread_state_for(&state);
        }
        Ok(())
    }

    /// Appends a pending execution node to `run`.
    ///
    /// # Errors
    /// Returns an error when `run` is unknown or already finished.
    pub fn add_node(&self, run: RunId, kind: NodeKind) -> Result<NodeId> {
        let id = NodeId::new();
        let mut inner = self.write();
        {
            let parent = inner
                .runs
                .get_mut(&run)
                .ok_or_else(|| unknown("run", &run))?;
            if parent.is_terminal() {
                bail!("cannot add a node to finished run {run}");
            }
            parent.nodes.push(id.clone());
        }

        let node = ExecutionNode {
            id: id.clone(),
            run_id: run,
            kind,
            started_at: Utc::now(),
            finished_at: None,
            state: NodeState::Pending,
        };
        inner.nodes.insert(id.clone(), node);
        Ok(id)
    }

    /// Moves `node` to a terminal state and records its finish time.
    ///
    /// # Errors
    /// Returns an error when `node` is unknown, `state` is not terminal, or the
    /// node was already finished.
    pub fn finish_node(&self, node: NodeId, state: NodeState) -> Result<()> {
        if !state.is_terminal() {
            bail!("node state {state:?} is not terminal");
        }

        let mut inner = self.write();
        let entry = inner
            .nodes
            .get_mut(&node)
            .ok_or_else(|| unknown("node", &node))?;
        if entry.finished_at.is_some() {
            bail!("node {node} is already finished");
        }
        entry.finished_at = Some(Utc::now());
        entry.state = state;
        Ok(())
    }

    /// Returns a copy of the session with `id`, if any.
    #[must_use]
    pub fn session(&self, id: &SessionId) -> Option<Session> {
        self.read().sessions.get(id).cloned()
    }

    /// Returns a copy of the thread with `id`, if any.
    #[must_use]
    pub fn thread(&self, id: &ThreadId) -> Option<Thread> {
        self.read().threads.get(id).cloned()
    }

    /// Returns a copy of the run with `id`, if any.
    #[must_use]
    pub fn run(&self, id: &RunId) -> Option<Run> {
        self.read().runs.get(id).cloned()
    }

    /// Returns a copy of the node with `id`, if any.
    #[must_use]
    pub fn node(&self, id: &NodeId) -> Option<ExecutionNode> {
        self.read().nodes.get(id).cloned()
    }

    /// Lists every known session, oldest first.
    #[must_use]
    pub fn sessions(&self) -> Vec<Session> {
        let inner = self.read();
        let mut sessions: Vec<Session> = inner.sessions.values().cloned().collect();
        sessions.sort_by_key(|session| session.created_at);
        sessions
    }

    /// Number of tracked sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.read().sessions.len()
    }

    /// Whether the store tracks no sessions.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>(_: &T) {}

    fn sample_run(state: RunState) -> Run {
        Run {
            id: RunId::new(),
            thread_id: ThreadId::new(),
            attempt: 1,
            started_at: Utc::now(),
            finished_at: None,
            state,
            nodes: Vec::new(),
        }
    }

    #[test]
    fn happy_path_session_thread_run_node() {
        let store = SessionStore::new();
        assert!(store.is_empty());

        let session_id = store.create_session("claude", "/tmp/ws").unwrap();
        let session = store.session(&session_id).unwrap();
        assert_eq!(session.provider, "claude");
        assert_eq!(session.cwd, PathBuf::from("/tmp/ws"));
        assert_eq!(session.state, SessionState::Active);
        assert!(session.threads.is_empty());
        assert_eq!(session.id.as_str(), session_id.as_str());

        let thread_id = store
            .create_thread(session_id.clone(), Some("wave 1".to_string()))
            .unwrap();
        let thread = store.thread(&thread_id).unwrap();
        assert_eq!(thread.session_id, session_id);
        assert_eq!(thread.title.as_deref(), Some("wave 1"));
        assert_eq!(thread.state, ThreadState::Pending);
        assert_eq!(
            store.session(&session_id).unwrap().threads,
            vec![thread_id.clone()]
        );

        let run_id = store.start_run(thread_id.clone()).unwrap();
        let run = store.run(&run_id).unwrap();
        assert_eq!(run.thread_id, thread_id);
        assert_eq!(run.attempt, 1);
        assert_eq!(run.state, RunState::Queued);
        assert!(!run.is_terminal());
        assert!(run.duration().is_none());
        assert_eq!(
            store.thread(&thread_id).unwrap().state,
            ThreadState::Running
        );
        assert_eq!(store.thread(&thread_id).unwrap().runs, vec![run_id.clone()]);

        let node_id = store
            .add_node(
                run_id.clone(),
                NodeKind::Shell {
                    command: "cargo check".to_string(),
                },
            )
            .unwrap();
        let node = store.node(&node_id).unwrap();
        assert_eq!(node.run_id, run_id);
        assert_eq!(node.state, NodeState::Pending);
        assert!(node.finished_at.is_none());
        assert_eq!(store.run(&run_id).unwrap().nodes, vec![node_id.clone()]);

        store.finish_node(node_id.clone(), NodeState::Ok).unwrap();
        let node = store.node(&node_id).unwrap();
        assert_eq!(node.state, NodeState::Ok);
        assert!(node.finished_at.is_some());

        store
            .finish_run(run_id.clone(), RunState::Succeeded)
            .unwrap();
        let run = store.run(&run_id).unwrap();
        assert_eq!(run.state, RunState::Succeeded);
        assert!(run.is_terminal());
        assert!(run.finished_at.is_some());
        assert!(run.duration().is_some());
        assert_eq!(
            store.thread(&thread_id).unwrap().state,
            ThreadState::Succeeded
        );
        assert_eq!(session_id.to_string(), session_id.as_str());
        assert_eq!(store.sessions(), vec![store.session(&session_id).unwrap()]);
    }

    #[test]
    fn run_duration_is_none_until_finished() {
        let store = SessionStore::new();
        let session = store.create_session("agy", "/tmp").unwrap();
        let thread = store.create_thread(session, None).unwrap();
        let run_id = store.start_run(thread).unwrap();

        assert!(store.run(&run_id).unwrap().duration().is_none());

        store
            .finish_run(run_id.clone(), RunState::Succeeded)
            .unwrap();
        let elapsed = store.run(&run_id).unwrap().duration();
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap() < Duration::from_secs(60));
    }

    #[test]
    fn run_terminality_matrix() {
        assert!(!sample_run(RunState::Queued).is_terminal());
        assert!(!sample_run(RunState::Running).is_terminal());
        assert!(sample_run(RunState::Succeeded).is_terminal());
        assert!(sample_run(RunState::Failed {
            message: "boom".to_string()
        })
        .is_terminal());
        assert!(sample_run(RunState::Cancelled).is_terminal());
        assert!(sample_run(RunState::TimedOut).is_terminal());
    }

    #[test]
    fn finish_run_rejects_non_terminal_and_double_finish() {
        let store = SessionStore::new();
        let session = store.create_session("codex", "/tmp").unwrap();
        let thread = store.create_thread(session, None).unwrap();
        let run = store.start_run(thread).unwrap();

        assert!(store.finish_run(run.clone(), RunState::Running).is_err());
        store.finish_run(run.clone(), RunState::Cancelled).unwrap();
        assert!(store.finish_run(run, RunState::Succeeded).is_err());
    }

    #[test]
    fn cannot_add_nodes_to_a_finished_run() {
        let store = SessionStore::new();
        let session = store.create_session("wrangler", "/tmp").unwrap();
        let thread = store.create_thread(session, None).unwrap();
        let run = store.start_run(thread).unwrap();
        store.finish_run(run.clone(), RunState::TimedOut).unwrap();

        assert!(store.add_node(run, NodeKind::Thinking).is_err());
    }

    #[test]
    fn serde_round_trip_run_and_node() {
        let run = Run {
            id: RunId::new(),
            thread_id: ThreadId::new(),
            attempt: 3,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            state: RunState::Failed {
                message: "provider exploded".to_string(),
            },
            nodes: vec![NodeId::new(), NodeId::new()],
        };
        let encoded = serde_json::to_string(&run).unwrap();
        let decoded: Run = serde_json::from_str(&encoded).unwrap();
        assert_eq!(run, decoded);

        let node = ExecutionNode {
            id: NodeId::new(),
            run_id: run.id.clone(),
            kind: NodeKind::FileEdit {
                path: PathBuf::from("/tmp/ws/src/lib.rs"),
            },
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            state: NodeState::Error {
                message: "conflict".to_string(),
            },
        };
        let encoded = serde_json::to_string(&node).unwrap();
        let decoded: ExecutionNode = serde_json::from_str(&encoded).unwrap();
        assert_eq!(node, decoded);

        // Optional timestamps survive the round trip in both directions.
        let pending = ExecutionNode {
            finished_at: None,
            state: NodeState::Pending,
            ..node
        };
        let encoded = serde_json::to_string(&pending).unwrap();
        let decoded: ExecutionNode = serde_json::from_str(&encoded).unwrap();
        assert_eq!(pending, decoded);
        assert!(decoded.finished_at.is_none());
    }

    #[test]
    fn serde_round_trip_session_and_thread() {
        let store = SessionStore::new();
        let session_id = store.create_session("opencode", "/tmp/ws").unwrap();
        let thread_id = store
            .create_thread(session_id.clone(), Some("t".to_string()))
            .unwrap();

        let session = store.session(&session_id).unwrap();
        let decoded: Session =
            serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(session, decoded);

        let thread = store.thread(&thread_id).unwrap();
        let decoded: Thread =
            serde_json::from_str(&serde_json::to_string(&thread).unwrap()).unwrap();
        assert_eq!(thread, decoded);

        let closed = Session {
            state: SessionState::Closed {
                reason: CloseReason::Failed {
                    message: "boom".to_string(),
                },
            },
            ..session
        };
        let decoded: Session =
            serde_json::from_str(&serde_json::to_string(&closed).unwrap()).unwrap();
        assert_eq!(closed, decoded);
    }

    #[test]
    fn unknown_ids_are_reported() {
        let store = SessionStore::new();

        assert!(store.session(&SessionId::new()).is_none());
        assert!(store.thread(&ThreadId::new()).is_none());
        assert!(store.run(&RunId::new()).is_none());
        assert!(store.node(&NodeId::new()).is_none());

        assert!(store.create_thread(SessionId::new(), None).is_err());
        assert!(store.start_run(ThreadId::new()).is_err());
        assert!(store.finish_run(RunId::new(), RunState::Succeeded).is_err());
        assert!(store.add_node(RunId::new(), NodeKind::Message).is_err());
        assert!(store.finish_node(NodeId::new(), NodeState::Ok).is_err());

        // A rejected call must not leave partial state behind.
        assert!(store.is_empty());
    }

    #[test]
    fn clones_share_the_same_state() {
        let store = SessionStore::new();
        let other = store.clone();
        assert_send_sync(&store);
        assert_send_sync(&other);

        let session = store.create_session("claude", "/tmp").unwrap();
        assert!(other.session(&session).is_some());

        let thread = other.create_thread(session.clone(), None).unwrap();
        assert_eq!(
            store.session(&session).unwrap().threads,
            vec![thread.clone()]
        );

        let run = store.start_run(thread.clone()).unwrap();
        assert_eq!(other.thread(&thread).unwrap().runs, vec![run.clone()]);

        let node = other.add_node(run.clone(), NodeKind::Thinking).unwrap();
        assert_eq!(store.run(&run).unwrap().nodes, vec![node.clone()]);

        other.finish_node(node.clone(), NodeState::Ok).unwrap();
        assert_eq!(store.node(&node).unwrap().state, NodeState::Ok);

        // Concurrency smoke test: every handle can write.
        let handles: Vec<_> = (0..4)
            .map(|index| {
                let store = store.clone();
                std::thread::spawn(move || {
                    store.create_session(format!("p{index}"), "/tmp").unwrap()
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(store.sessions().len(), 5);
        assert_eq!(other.len(), 5);
    }

    #[test]
    fn attempt_counter_increments_on_retry() {
        let store = SessionStore::new();
        let session = store.create_session("claude", "/tmp").unwrap();
        let thread = store.create_thread(session, None).unwrap();

        let first = store.start_run(thread.clone()).unwrap();
        assert_eq!(store.run(&first).unwrap().attempt, 1);
        store
            .finish_run(
                first,
                RunState::Failed {
                    message: "flaky".to_string(),
                },
            )
            .unwrap();
        assert_eq!(store.thread(&thread).unwrap().state, ThreadState::Failed);

        let second = store.start_run(thread.clone()).unwrap();
        assert_eq!(store.run(&second).unwrap().attempt, 2);
        store
            .finish_run(second.clone(), RunState::Succeeded)
            .unwrap();

        let third = store.start_run(thread.clone()).unwrap();
        assert_eq!(store.run(&third).unwrap().attempt, 3);

        assert_eq!(store.thread(&thread).unwrap().runs.len(), 3);
    }

    #[test]
    fn ids_are_unique_and_displayable() {
        let a = SessionId::new();
        let b = SessionId::new();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 36);
        assert_eq!(a.to_string(), a.as_str());
        assert_eq!(SessionId::default().as_str().len(), 36);

        let round_tripped: RunId =
            serde_json::from_str(&serde_json::to_string(&RunId::new()).unwrap()).unwrap();
        assert_eq!(round_tripped.as_str().len(), 36);
    }
}
