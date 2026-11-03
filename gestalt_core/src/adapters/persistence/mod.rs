//! Persistence adapters.
//!
//! Concrete implementations of the outbound persistence ports:
//!
//! - [`SqliteRepo`] — [`RepoManager`](crate::ports::outbound::repo_manager::RepoManager)
//!   built on the embedded SQLite store, adding validated and idempotent git
//!   cloning on top of it.
//!
//! The raw storage layer (schema, connection pool, vector search) lives in
//! [`crate::db::sqlite::SqliteDb`]; adapters here are thin, purpose-built
//! facades over it.

pub mod sqlite_repo;

pub use sqlite_repo::SqliteRepo;
