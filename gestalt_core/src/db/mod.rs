//! Database layer for `gestalt_core`.
//!
//! This module provides the embedded persistence implementation that replaces
//! the removed SurrealDB client. It is backed by SQLite (via [`rusqlite`] with
//! the bundled engine) plus the `sqlite-vec` extension for vector similarity
//! search.

pub mod sqlite;

pub use sqlite::SqliteDb;
