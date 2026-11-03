//! Persistence adapters.
//!
//! Concrete implementations of the outbound persistence ports:
//!
//! - [`SqliteRepo`] — [`RepoManager`](crate::ports::outbound::repo_manager::RepoManager)
//!   built on the embedded SQLite store, adding validated and idempotent git
//!   cloning on top of it.
//! - [`SqliteVectorDb`] — [`VectorDb`](crate::ports::outbound::repo_manager::VectorDb)
//!   built on the embedded SQLite store with sqlite-vec for vector similarity search.
//!
//! The raw storage layer (schema, connection pool, vector search) lives in
//! [`crate::db::sqlite::SqliteDb`]; adapters here are thin, purpose-built
//! facades over it.

pub mod sqlite_repo;
pub mod sqlite_vector;

pub use sqlite_repo::SqliteRepo;
pub use sqlite_vector::{decode_vector, encode_vector, SqliteVectorDb, F32_BYTES};
