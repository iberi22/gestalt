// async_trait expands async fns to #[must_use] Future-returning fns, which the
// new clippy::double_must_use lint (stable 1.99) flags as redundant. The
// expansion is outside our control, so allow it crate-wide.
#![allow(clippy::double_must_use)]

pub mod agent;
pub mod checkpoint;
pub mod doctor;
pub mod event_bus;
pub mod integrate;
pub mod otel;
pub mod overlap;
pub mod router;
pub mod run;
pub mod run_state;
pub mod thinking;
pub mod timeline;
pub mod worktree;
pub mod ws;
pub mod xavier_sink;
