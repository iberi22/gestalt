//! Xavier2 Subagent for Gestalt Swarm
//! 
//! Xavier2 acts as a memory/context subagent within the Gestalt Swarm architecture.
//! 
//! ## Architecture
//! 
//! ```text
//! Gestalt SwarmCoordinator
//!     |
//!     └── Xavier2Agent (subagent)
//!             |
//!             └── Xavier2 API (port 8006)
//!                     |
//!                     ├── /memory/search
//!                     ├── /memory/add
//!                     └── /code/scan
//! ```
//! 
//! ## Usage
//! 
//! ```rust
//! use gestalt_core::application::agent::xavier2::Xavier2Agent;
//! 
//! let agent = Xavier2Agent::new(
//!     "http://localhost:8006".into(),
//!     "dev-token".into(),
//! ).await?;
//! 
//! let result = agent.execute(task).await?;
//! ```

mod client;
mod agent;

pub use agent::{Xavier2Agent, Xavier2Action, Xavier2Task, Xavier2Message, Xavier2Response};
pub use client::Xavier2Client;

