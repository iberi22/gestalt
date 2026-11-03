//! Graceful shutdown coordination for the Gestalt CLI.
//!
//! The CLI can be running long-lived work: swarm fan-out, router runs, the
//! observation daemon, the event-bus HTTP server, and spawned MCP servers.
//! On `SIGINT`/`SIGTERM` we stop accepting new work, cancel in-flight
//! sub-agents, let short critical sections finish (for example flushing the
//! `OverlayFs` to disk so nothing is lost) and only then exit.
//!
//! Exit codes follow the shell convention used by `timeout(1)` and friends:
//! `0` for a graceful stop (`SIGTERM`), `130` for `Ctrl+C` (`SIGINT`, 128 + 2).
//!
//! The coordinator is built on `tokio::sync::watch` rather than
//! [`tokio::sync::Notify`] on purpose: `Notify::notify_waiters` drops the wakeup
//! when nobody is registered yet, which would let a late awaiter (a task that
//! registers after the signal arrives) miss the shutdown entirely. A `watch`
//! channel keeps the last value, so every clone of the pristine receiver sees
//! the very first transition.

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tokio::task::JoinSet;

/// How long we wait for in-flight work to finish before giving up on it.
pub const DEFAULT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Upper bound accepted from `GESTALT_SHUTDOWN_TIMEOUT_MS` (10 minutes).
const MAX_DRAIN_TIMEOUT_MS: u64 = 600_000;

/// Which signal (or internal request) asked the process to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// `SIGINT` — `Ctrl+C` on a terminal.
    Interrupt,
    /// `SIGTERM` — service manager / `kill`.
    Terminate,
    /// Requested from inside the process (tests, embedding).
    #[allow(dead_code)] // part of the coordinator API; exercised by unit tests
    Programmatic,
}

impl ShutdownReason {
    /// Process exit code for this reason.
    ///
    /// `Ctrl+C` maps to `130` (128 + `SIGINT`), everything else is a clean `0`.
    pub const fn exit_code(self) -> i32 {
        match self {
            Self::Interrupt => 130,
            Self::Terminate | Self::Programmatic => 0,
        }
    }

    /// Human readable label used in logs and reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Interrupt => "SIGINT (Ctrl+C)",
            Self::Terminate => "SIGTERM",
            Self::Programmatic => "programmatic shutdown",
        }
    }
}

impl fmt::Display for ShutdownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Outcome of a drain, reported to the user before exiting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownReport {
    /// Why we are shutting down (`None` if nothing triggered it).
    pub reason: Option<ShutdownReason>,
    /// Tasks that reached a clean end.
    pub drained: usize,
    /// Tasks cancelled (explicitly aborted, or abandoned at the deadline).
    pub cancelled: usize,
    /// Tasks still running when the drain deadline elapsed.
    pub remaining: usize,
    /// Whether the drain deadline was exceeded.
    pub timed_out: bool,
    /// Wall-clock time spent draining.
    pub drain_elapsed: Duration,
}

impl ShutdownReport {
    /// Exit code this shutdown should produce.
    pub fn exit_code(&self) -> i32 {
        self.reason.map(ShutdownReason::exit_code).unwrap_or(0)
    }

    /// True when every tracked task ended before the deadline.
    pub fn is_clean(&self) -> bool {
        !self.timed_out && self.remaining == 0
    }
}

impl fmt::Display for ShutdownReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = self.reason.map(ShutdownReason::label).unwrap_or("none");
        write!(
            f,
            "reason={reason} drained={} cancelled={} remaining={} timed_out={} elapsed={}ms",
            self.drained,
            self.cancelled,
            self.remaining,
            self.timed_out,
            self.drain_elapsed.as_millis()
        )
    }
}

struct Core {
    reason: Mutex<Option<ShutdownReason>>,
    /// Sender side of the shutdown signal.
    shutdown_tx: watch::Sender<bool>,
    /// Pristine receiver: never polled by the coordinator itself, so every
    /// clone still observes the first transition even if it subscribes late.
    shutdown_rx: watch::Receiver<bool>,
    /// Number of registered in-flight tasks.
    active: AtomicUsize,
    /// Mirror of `active` for waiters (same pristine-receiver trick).
    active_tx: watch::Sender<usize>,
    active_rx: watch::Receiver<usize>,
    /// Tasks that finished cleanly.
    completed: AtomicUsize,
    /// Tasks explicitly cancelled (aborted before they could finish).
    cancelled: AtomicUsize,
}

/// Handle to the process-wide shutdown state. Cheap to clone.
#[derive(Clone)]
pub struct ShutdownCoordinator {
    core: Arc<Core>,
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownCoordinator {
    /// Create a standalone coordinator (used by tests and embeddings).
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (active_tx, active_rx) = watch::channel(0usize);
        Self {
            core: Arc::new(Core {
                reason: Mutex::new(None),
                shutdown_tx,
                shutdown_rx,
                active: AtomicUsize::new(0),
                active_tx,
                active_rx,
                completed: AtomicUsize::new(0),
                cancelled: AtomicUsize::new(0),
            }),
        }
    }

    /// The reason recorded for this shutdown, if any.
    pub fn reason(&self) -> Option<ShutdownReason> {
        *self
            .core
            .reason
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Whether a shutdown has been requested.
    pub fn is_shutting_down(&self) -> bool {
        self.reason().is_some()
    }

    /// Request a shutdown. Idempotent: the first reason wins and later calls
    /// return `false` (that is how the second `Ctrl+C` is detected).
    pub fn trigger(&self, reason: ShutdownReason) -> bool {
        {
            let mut slot = self
                .core
                .reason
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if slot.is_some() {
                return false;
            }
            *slot = Some(reason);
        }
        // `send_replace` never fails (we own a receiver), unlike `send`.
        self.core.shutdown_tx.send_replace(true);
        true
    }

    /// Resolve as soon as a shutdown has been requested.
    ///
    /// Safe to call before *or* after the trigger: a late awaiter still sees
    /// the transition because the channel keeps the last value.
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.core.shutdown_rx.clone();
        if *rx.borrow_and_update() {
            return;
        }
        // A `RecvError` here means the coordinator is gone, which can only
        // happen while the process is tearing down anyway.
        let _ = rx.changed().await;
    }

    /// Number of in-flight registered tasks.
    pub fn active_tasks(&self) -> usize {
        self.core.active.load(Ordering::Acquire)
    }

    /// Number of tasks that finished cleanly so far.
    pub fn completed_tasks(&self) -> usize {
        self.core.completed.load(Ordering::Acquire)
    }

    /// Number of tasks explicitly cancelled so far.
    pub fn cancelled_tasks(&self) -> usize {
        self.core.cancelled.load(Ordering::Acquire)
    }

    /// Register an in-flight task. The returned guard keeps the task counted
    /// until it is finished or dropped.
    #[must_use = "dropping the guard immediately deregisters the task"]
    pub fn register_task(&self, label: impl Into<String>) -> TaskGuard {
        let label = label.into();
        let previous = self.core.active.fetch_add(1, Ordering::AcqRel);
        self.core.active_tx.send_replace(previous + 1);
        tracing::debug!("Shutdown: tracking in-flight task '{}'", label);
        TaskGuard {
            core: Arc::clone(&self.core),
            released: false,
        }
    }

    /// Record tasks that were cancelled without going through a guard (for
    /// example a `JoinSet` whose handles we aborted).
    pub fn record_cancelled(&self, count: usize) -> usize {
        if count == 0 {
            return self.cancelled_tasks();
        }
        self.core.cancelled.fetch_add(count, Ordering::AcqRel) + count
    }

    /// Wait until every registered task has ended, or `timeout` elapses.
    ///
    /// The caller decides what to do with tasks still running afterwards (the
    /// CLI aborts them and exits); this method only reports the outcome.
    pub async fn drain(&self, timeout: Duration) -> ShutdownReport {
        let started = Instant::now();
        let mut rx = self.core.active_rx.clone();
        let remaining_now = *rx.borrow_and_update();

        let timed_out = if remaining_now == 0 {
            false
        } else {
            let wait_for_zero = async {
                while *rx.borrow_and_update() > 0 {
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
            };
            tokio::time::timeout(timeout, wait_for_zero).await.is_err()
        };

        let remaining = self.active_tasks();
        let cancelled = self
            .core
            .cancelled
            .load(Ordering::Acquire)
            .saturating_add(if timed_out { remaining } else { 0 });

        ShutdownReport {
            reason: self.reason(),
            drained: self.completed_tasks(),
            cancelled,
            remaining,
            timed_out,
            drain_elapsed: started.elapsed(),
        }
    }
}

/// Keeps a task counted as in-flight until it finishes or is dropped.
pub struct TaskGuard {
    core: Arc<Core>,
    released: bool,
}

impl TaskGuard {
    /// Mark the task as finished cleanly.
    pub fn finish(mut self) {
        self.release(true);
    }

    fn release(&mut self, completed: bool) {
        if self.released {
            return;
        }
        self.released = true;
        let previous = self.core.active.fetch_sub(1, Ordering::AcqRel);
        self.core.active_tx.send_replace(previous.saturating_sub(1));
        if completed {
            self.core.completed.fetch_add(1, Ordering::AcqRel);
        }
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        // Tasks that return early (error, cancellation) are released without
        // being counted as drained.
        self.release(false);
    }
}

/// Abort every task in `set`, returning how many were still running.
///
/// The aborted handles are drained so the `JoinSet` is left empty (otherwise
/// the tasks would keep running detached).
pub async fn abort_and_drain_join_set<T: 'static>(set: &mut JoinSet<T>) -> usize {
    let pending = set.len();
    if pending == 0 {
        return 0;
    }
    set.abort_all();
    while set.join_next().await.is_some() {}
    pending
}

/// Process-wide coordinator used by `main`.
pub fn global() -> &'static ShutdownCoordinator {
    static GLOBAL: OnceLock<ShutdownCoordinator> = OnceLock::new();
    GLOBAL.get_or_init(ShutdownCoordinator::new)
}

/// Drain timeout, overridable with `GESTALT_SHUTDOWN_TIMEOUT_MS`.
pub fn drain_timeout_from_env() -> Duration {
    parse_drain_timeout(std::env::var("GESTALT_SHUTDOWN_TIMEOUT_MS").ok().as_deref())
}

fn parse_drain_timeout(raw: Option<&str>) -> Duration {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map(|ms| Duration::from_millis(ms.min(MAX_DRAIN_TIMEOUT_MS)))
        .unwrap_or(DEFAULT_DRAIN_TIMEOUT)
}

/// Print the shutdown summary (cancelled task count included) to stdout.
pub fn print_report(report: &ShutdownReport) {
    let reason = report.reason.map(ShutdownReason::label).unwrap_or("none");
    println!("🛑 Shutdown complete ({reason})");
    println!("   Tasks drained:   {}", report.drained);
    println!("   Tasks cancelled: {}", report.cancelled);
    println!(
        "   Status:          {}",
        if report.is_clean() {
            "clean"
        } else {
            "incomplete"
        }
    );
    if report.timed_out {
        println!(
            "   ⚠️  Drain deadline exceeded after {}ms — {} task(s) abandoned",
            report.drain_elapsed.as_millis(),
            report.remaining
        );
    }
    println!("   Drain time:      {}ms", report.drain_elapsed.as_millis());
    println!("   Exit code:       {}", report.exit_code());
}

/// Wait for the next termination signal (`SIGINT` or `SIGTERM` on unix).
#[cfg(unix)]
async fn wait_for_signal() -> ShutdownReason {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(stream) => Some(stream),
        Err(e) => {
            tracing::warn!("Failed to install SIGTERM handler: {}", e);
            None
        },
    };

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(e) = result {
                tracing::warn!("Failed to listen for SIGINT: {}", e);
            }
            ShutdownReason::Interrupt
        },
        _ = async {
            match sigterm.as_mut() {
                Some(stream) => {
                    stream.recv().await;
                },
                None => std::future::pending::<()>().await,
            }
        } => ShutdownReason::Terminate,
    }
}

/// Wait for the next termination signal (`Ctrl+C` is all we get off unix).
#[cfg(not(unix))]
async fn wait_for_signal() -> ShutdownReason {
    if let Err(e) = tokio::signal::ctrl_c().await {
        tracing::warn!("Failed to listen for Ctrl+C: {}", e);
    }
    ShutdownReason::Interrupt
}

/// Install the `SIGINT`/`SIGTERM` listener.
///
/// The first signal triggers a graceful shutdown. A second signal escalates:
/// we stop waiting for the drain and exit immediately, because the user has
/// already asked once and means it.
pub fn spawn_signal_listener(coord: &ShutdownCoordinator) -> tokio::task::JoinHandle<()> {
    let coord = coord.clone();
    tokio::spawn(async move {
        let first = wait_for_signal().await;
        // Kept verbatim: existing CLI tests assert on this exact message.
        eprintln!("Shutting down gracefully... ({} received)", first.label());
        coord.trigger(first);

        let second = wait_for_signal().await;
        eprintln!(
            "⚠️  Second signal ({}) — exiting immediately without draining",
            second.label()
        );
        std::process::exit(second.exit_code());
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_shell_convention() {
        assert_eq!(ShutdownReason::Interrupt.exit_code(), 130);
        assert_eq!(ShutdownReason::Terminate.exit_code(), 0);
        assert_eq!(ShutdownReason::Programmatic.exit_code(), 0);
    }

    #[test]
    fn trigger_is_idempotent_and_first_reason_wins() {
        let coord = ShutdownCoordinator::new();
        assert!(!coord.is_shutting_down());

        assert!(coord.trigger(ShutdownReason::Interrupt));
        assert_eq!(coord.reason(), Some(ShutdownReason::Interrupt));

        // Second call is the "user pressed Ctrl+C twice" signal.
        assert!(!coord.trigger(ShutdownReason::Terminate));
        assert_eq!(coord.reason(), Some(ShutdownReason::Interrupt));
    }

    #[tokio::test]
    async fn wait_for_shutdown_wakes_an_already_waiting_task() {
        let coord = ShutdownCoordinator::new();
        let waiter = coord.clone();
        let handle = tokio::spawn(async move { waiter.wait_for_shutdown().await });

        tokio::time::sleep(Duration::from_millis(20)).await;
        coord.trigger(ShutdownReason::Terminate);

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("waiter should wake on shutdown")
            .expect("waiter task should not panic");
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_for_late_waiters() {
        let coord = ShutdownCoordinator::new();
        coord.trigger(ShutdownReason::Interrupt);

        // Registered *after* the trigger: a `Notify` would hang here.
        tokio::time::timeout(Duration::from_secs(2), coord.wait_for_shutdown())
            .await
            .expect("a late waiter must still observe the shutdown");
    }

    #[tokio::test]
    async fn drain_without_tasks_is_immediate() {
        let coord = ShutdownCoordinator::new();
        coord.trigger(ShutdownReason::Terminate);

        let report = coord.drain(Duration::from_secs(5)).await;
        assert!(!report.timed_out);
        assert!(report.is_clean());
        assert_eq!(report.drained, 0);
        assert_eq!(report.cancelled, 0);
        assert_eq!(report.exit_code(), 0);
    }

    #[tokio::test]
    async fn drain_waits_for_registered_tasks() {
        let coord = ShutdownCoordinator::new();
        let guard = coord.register_task("worker");
        assert_eq!(coord.active_tasks(), 1);

        let worker = coord.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            guard.finish();
            assert_eq!(worker.active_tasks(), 0);
        });

        let report = coord.drain(Duration::from_secs(5)).await;
        assert!(!report.timed_out, "drain should have waited for the task");
        assert_eq!(report.drained, 1);
        assert_eq!(report.cancelled, 0);
        assert_eq!(report.remaining, 0);
    }

    #[tokio::test]
    async fn drain_timeout_reports_cancelled_tasks() {
        let coord = ShutdownCoordinator::new();
        let _stuck = coord.register_task("stuck");
        let _stuck_too = coord.register_task("stuck-too");

        let report = coord.drain(Duration::from_millis(50)).await;
        assert!(report.timed_out);
        assert!(!report.is_clean());
        assert_eq!(report.remaining, 2);
        assert_eq!(report.cancelled, 2);
    }

    #[tokio::test]
    async fn explicitly_cancelled_tasks_are_counted() {
        let coord = ShutdownCoordinator::new();
        coord.record_cancelled(3);
        assert_eq!(coord.cancelled_tasks(), 3);

        let report = coord.drain(Duration::from_millis(10)).await;
        assert_eq!(report.cancelled, 3);
        assert!(!report.timed_out);
    }

    #[tokio::test]
    async fn abort_and_drain_join_set_cancels_and_empties_the_set() {
        let coord = ShutdownCoordinator::new();
        let mut set = JoinSet::new();
        for _ in 0..3 {
            set.spawn(async {
                tokio::time::sleep(Duration::from_secs(30)).await;
            });
        }
        // Give the spawned tasks a chance to be scheduled.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(set.len(), 3);

        let cancelled = abort_and_drain_join_set(&mut set).await;
        assert_eq!(cancelled, 3);
        assert_eq!(set.len(), 0);

        coord.record_cancelled(cancelled);
        assert_eq!(coord.cancelled_tasks(), 3);
    }

    #[test]
    fn drain_timeout_parsing_falls_back_to_default() {
        assert_eq!(parse_drain_timeout(None), DEFAULT_DRAIN_TIMEOUT);
        assert_eq!(
            parse_drain_timeout(Some("not-a-number")),
            DEFAULT_DRAIN_TIMEOUT
        );
        assert_eq!(parse_drain_timeout(Some("")), DEFAULT_DRAIN_TIMEOUT);
        assert_eq!(parse_drain_timeout(Some("250")), Duration::from_millis(250));
        assert_eq!(
            parse_drain_timeout(Some(" 1500 ")),
            Duration::from_millis(1500)
        );
        // Values above the ceiling are clamped instead of hanging forever.
        assert_eq!(
            parse_drain_timeout(Some("99999999")),
            Duration::from_millis(MAX_DRAIN_TIMEOUT_MS)
        );
    }

    #[test]
    fn report_display_mentions_cancelled_count() {
        let report = ShutdownReport {
            reason: Some(ShutdownReason::Interrupt),
            drained: 2,
            cancelled: 5,
            remaining: 0,
            timed_out: false,
            drain_elapsed: Duration::from_millis(7),
        };
        let rendered = report.to_string();
        assert!(rendered.contains("cancelled=5"), "got: {rendered}");
        assert!(
            rendered.contains("reason=SIGINT (Ctrl+C)"),
            "got: {rendered}"
        );
        assert_eq!(report.exit_code(), 130);
    }
}
