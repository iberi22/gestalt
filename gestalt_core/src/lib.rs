//! gestalt_core - Core domain models, adapters, and application logic for Gestalt.
//!
//! This crate is the foundational library of the Gestalt multi-agent orchestration
//! framework. It follows a hexagonal (ports & adapters) layout:
//!
//! - **`domain`** — Core domain models, enums, and error types.
//! - **`adapters`** — Concrete implementations of ports (LLM clients, persistence, auth).
//! - **`application`** — Application services and agent orchestration use cases.
//! - **`context`** — Agent execution context, including declarative memory storage.
//!
//! Additional supporting modules: `ports`, `config`, `db`, `mcp`, and `models`.

// async_trait expands async fns to #[must_use] Future-returning fns, which the
// new clippy::double_must_use lint (stable 1.99) flags as redundant. The
// expansion is outside our control, so allow it crate-wide.
#![allow(clippy::double_must_use)]

pub mod adapters;
pub mod application;
pub mod bus;
pub mod config;
pub mod context;
pub mod db;
pub mod domain;
pub mod mcp;
pub mod models;
pub mod ports;
pub mod search;

pub use domain::error::{CoreError, Result as CoreResult};
