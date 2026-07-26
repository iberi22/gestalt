use crate::schema::{FileLock, TimelineEvent};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// In-memory state store for agent orchestration.
///
/// Uses DashMap for concurrent access and a broadcast channel for
/// timeline event streaming. Suitable for testing, single-node setups,
/// or as a cache layer in front of [`StateDb`](crate::StateDb).
#[derive(Clone)]
pub struct MemState {
    /// Map from `(run_id, agent_id)` to agent state string.
    agent_states: Arc<DashMap<String, String>>,
    /// Map from lock path to [`FileLock`].
    active_locks: Arc<DashMap<String, FileLock>>,
    /// Broadcast channel sender — all subscribers receive live events.
    event_tx: broadcast::Sender<TimelineEvent>,
}

impl Default for MemState {
    fn default() -> Self {
        Self::new()
    }
}

impl MemState {
    /// Create a new empty `MemState` with a 1024-event broadcast channel.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self {
            agent_states: Arc::new(DashMap::new()),
            active_locks: Arc::new(DashMap::new()),
            event_tx: tx,
        }
    }

    /// Create a new `MemState` with a custom broadcast channel capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self {
            agent_states: Arc::new(DashMap::new()),
            active_locks: Arc::new(DashMap::new()),
            event_tx: tx,
        }
    }

    // ── Agent State ───────────────────────────────────────────────────

    /// Get the current state of an agent within a run.
    ///
    /// Returns `None` if the agent has not been registered.
    pub fn get_agent_state(&self, run_id: &str, agent_id: &str) -> Option<String> {
        let key = format!("{run_id}:{agent_id}");
        self.agent_states.get(&key).map(|v| v.clone())
    }

    /// Set the state of an agent within a run and broadcast a timeline event.
    ///
    /// The broadcast event carries the new state as the payload.
    pub fn set_agent_state(&self, run_id: &str, agent_id: &str, state: &str) {
        let key = format!("{run_id}:{agent_id}");
        self.agent_states.insert(key, state.to_string());

        let event = TimelineEvent {
            seq: None,
            run_id: run_id.to_string(),
            agent_id: Some(agent_id.to_string()),
            event_type: "state_change".to_string(),
            payload: serde_json::json!({ "state": state }).to_string(),
            created_at: chrono::Utc::now(),
        };

        let _ = self.event_tx.send(event);
    }

    /// Remove an agent's state tracking.
    pub fn remove_agent_state(&self, run_id: &str, agent_id: &str) {
        let key = format!("{run_id}:{agent_id}");
        self.agent_states.remove(&key);
    }

    // ── File Locks ────────────────────────────────────────────────────

    /// Try to acquire a lock on `path` for `agent_id`.
    ///
    /// Returns `true` if the lock was acquired. Automatically cleans up
    /// expired locks (based on `ttl_secs`) before attempting acquisition.
    pub fn try_lock(&self, path: &str, agent_id: &str, run_id: &str, ttl_secs: i64) -> bool {
        // Clean up expired locks before attempting acquisition
        self.cleanup_expired_locks();
        let now = chrono::Utc::now();

        // Try to insert
        let lock = FileLock {
            path: path.to_string(),
            agent_id: agent_id.to_string(),
            run_id: run_id.to_string(),
            acquired_at: now,
            ttl_secs,
        };

        // `entry` API: insert if vacant
        use dashmap::mapref::entry::Entry;
        let acquired = match self.active_locks.entry(path.to_string()) {
            Entry::Vacant(v) => {
                v.insert(lock);
                true
            }
            Entry::Occupied(_) => false,
        };

        // Broadcast lock event
        if acquired {
            let payload = serde_json::json!({
                "path": path,
                "agent_id": agent_id,
                "ttl_secs": ttl_secs,
            })
            .to_string();
            self.push_event(run_id, Some(agent_id), "lock_acquired", &payload);
        } else {
            // Lock acquisition failed — check who holds it and emit a conflict event
            if let Some(existing) = self.active_locks.get(path) {
                if existing.agent_id != agent_id {
                    let payload = serde_json::json!({
                        "path": path,
                        "agent_a": existing.agent_id,
                        "agent_b": agent_id,
                        "held_by": existing.agent_id,
                    })
                    .to_string();
                    self.push_event(run_id, Some(agent_id), "lock_conflict", &payload);
                }
            }
        }

        acquired
    }

    /// Release a lock previously acquired by `agent_id` on `path`.
    ///
    /// Returns `true` if a lock was actually released.
    pub fn release_lock(&self, path: &str, agent_id: &str) -> bool {
        // Check ownership first (read lock), then remove separately (write lock)
        let is_owner = self
            .active_locks
            .get(path)
            .map(|lock| lock.agent_id == agent_id)
            .unwrap_or(false);

        let released = if is_owner {
            self.active_locks.remove(path);
            true
        } else {
            false
        };

        // Broadcast lock release event
        if released {
            let payload = serde_json::json!({
                "path": path,
                "agent_id": agent_id,
            })
            .to_string();
            // Only send if there are active subscribers (avoid blocking send with 0 receivers)
            if self.event_tx.receiver_count() > 0 {
                let _ = self.event_tx.send(TimelineEvent {
                    seq: None,
                    run_id: String::new(),
                    agent_id: Some(agent_id.to_string()),
                    event_type: "lock_released".to_string(),
                    payload,
                    created_at: chrono::Utc::now(),
                });
            }
        }

        released
    }

    /// Renew a lock previously acquired by `agent_id` on `path`.
    ///
    /// Resets the `acquired_at` timestamp to now and optionally updates the TTL.
    /// Returns `true` if the lock was renewed (i.e. it existed and was owned by `agent_id`).
    pub fn renew_lock(&self, path: &str, agent_id: &str, ttl_secs: i64) -> bool {
        if let Some(mut lock) = self.active_locks.get_mut(path) {
            if lock.agent_id == agent_id {
                lock.acquired_at = chrono::Utc::now();
                lock.ttl_secs = ttl_secs;
                return true;
            }
        }
        false
    }

    /// Remove all expired locks from the map.
    ///
    /// This is called internally by [`try_lock`](Self::try_lock) but can also
    /// be invoked externally for periodic cleanup.
    pub fn cleanup_expired_locks(&self) {
        let now = chrono::Utc::now();
        self.active_locks.retain(|_, lock| {
            let expires = lock.acquired_at + chrono::Duration::seconds(lock.ttl_secs);
            expires > now
        });
    }

    /// List all currently held file locks.
    pub fn get_locks(&self) -> Vec<FileLock> {
        self.active_locks
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    // ── Timeline / Subscription ───────────────────────────────────────

    /// Subscribe to all timeline events broadcast by this state store.
    ///
    /// Events are sent on every [`set_agent_state`](Self::set_agent_state) call.
    pub fn subscribe(&self) -> broadcast::Receiver<TimelineEvent> {
        self.event_tx.subscribe()
    }

    /// Push a custom timeline event via the broadcast channel.
    ///
    /// Returns the number of active subscribers that received the event.
    pub fn push_event(
        &self,
        run_id: &str,
        agent_id: Option<&str>,
        event_type: &str,
        payload: &str,
    ) -> usize {
        let event = TimelineEvent {
            seq: None,
            run_id: run_id.to_string(),
            agent_id: agent_id.map(|s| s.to_string()),
            event_type: event_type.to_string(),
            payload: payload.to_string(),
            created_at: chrono::Utc::now(),
        };

        let subscriber_count = self.event_tx.receiver_count();
        let _ = self.event_tx.send(event);
        subscriber_count
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_agent_state_lifecycle() {
        let mem = MemState::new();

        // Initially no state
        assert!(mem.get_agent_state("run-1", "agent-1").is_none());

        // Set state
        mem.set_agent_state("run-1", "agent-1", "running");
        assert_eq!(
            mem.get_agent_state("run-1", "agent-1"),
            Some("running".to_string())
        );

        // Update state
        mem.set_agent_state("run-1", "agent-1", "success");
        assert_eq!(
            mem.get_agent_state("run-1", "agent-1"),
            Some("success".to_string())
        );

        // Different agent is independent
        assert!(mem.get_agent_state("run-1", "agent-2").is_none());

        // Remove
        mem.remove_agent_state("run-1", "agent-1");
        assert!(mem.get_agent_state("run-1", "agent-1").is_none());
    }

    #[test]
    fn test_try_lock_exclusive() {
        let mem = MemState::new();

        // First acquire succeeds
        assert!(mem.try_lock("test.lock", "agent-1", "run-1", 30));

        // Second acquire on same path fails
        assert!(!mem.try_lock("test.lock", "agent-2", "run-1", 30));

        // Release by correct owner
        assert!(mem.release_lock("test.lock", "agent-1"));

        // After release, another can acquire
        assert!(mem.try_lock("test.lock", "agent-2", "run-2", 30));

        // Wrong agent can't release
        assert!(!mem.release_lock("test.lock", "agent-1"));
    }

    #[test]
    fn test_lock_expiry() {
        let mem = MemState::new();

        // Acquire with 0-second TTL (effectively immediate expiry)
        assert!(mem.try_lock("/tmp/expire.lock", "agent-1", "run-1", 0));

        // Give time for the 0-second TTL to expire
        std::thread::sleep(Duration::from_millis(10));

        // After expiry, a different agent should be able to acquire
        assert!(mem.try_lock("/tmp/expire.lock", "agent-2", "run-2", 30));
    }

    #[test]
    fn test_broadcast_events() {
        let mem = MemState::new();
        let mut rx = mem.subscribe();

        // Send event via set_agent_state
        mem.set_agent_state("run-1", "agent-1", "running");

        let event = rx.try_recv().expect("Should receive an event");
        assert_eq!(event.run_id, "run-1");
        assert_eq!(event.agent_id, Some("agent-1".to_string()));
        assert_eq!(event.event_type, "state_change");

        // Send event via push_event
        mem.push_event("run-1", Some("agent-1"), "custom", r#"{"msg":"hello"}"#);

        let event = rx.try_recv().expect("Should receive push_event");
        assert_eq!(event.event_type, "custom");
        assert_eq!(event.payload, r#"{"msg":"hello"}"#);
    }

    #[test]
    fn test_locks_listing() {
        let mem = MemState::new();

        mem.try_lock("/tmp/a.lock", "agent-1", "run-1", 30);
        mem.try_lock("/tmp/b.lock", "agent-2", "run-1", 30);

        let locks = mem.get_locks();
        assert_eq!(locks.len(), 2);

        let paths: Vec<String> = locks.into_iter().map(|l| l.path).collect();
        assert!(paths.contains(&"/tmp/a.lock".to_string()));
        assert!(paths.contains(&"/tmp/b.lock".to_string()));
    }
}
