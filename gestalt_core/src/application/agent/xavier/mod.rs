//! Xavier Subagent for Gestalt Swarm
//!
//! Xavier acts as a memory/context subagent within the Gestalt Swarm architecture,
//! targeting the Xavier v0.12.0 API.
//!
//! ## Architecture
//!
//! ```text
//! Gestalt SwarmCoordinator
//!     |
//!     └── XavierAgent (subagent)
//!             |
//!             └── Xavier API (port 8006)
//!                     |
//!                     ├── /v1/memories/search
//!                     ├── /v1/memories
//!                     └── /v1/stats
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use gestalt_core::application::agent::xavier::XavierAgent;
//!
//! let agent = XavierAgent::new(
//!     "http://localhost:8006".into(),
//!     "token".into(),
//! ).await?;
//!
//! let results = agent.search("query", 10, "hybrid").await?;
//! ```

mod agent;
mod client;

pub use agent::XavierAgent;
pub use client::XavierClient;
