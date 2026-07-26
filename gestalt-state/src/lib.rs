//! Gestalt State — SQLite StateDB + DashMap MemState for agent orchestration.
//!
//! Provides two state backends:
//! - [`StateDb`]: Persistent SQLite-based state store with WAL mode, run/agent/lock/timeline tables.
//! - [`MemState`]: In-memory DashMap-backed state store with broadcast channels for events.

pub mod memstate;
pub mod schema;
pub mod statedb;
pub mod virtual_fs;

pub use memstate::MemState;
pub use schema::{AgentRecord, AgentState, FileLock, RunRecord, TimelineEvent};
pub use statedb::StateDb;
pub use virtual_fs::StateDbVfs;
