//! Runtime discovery and ranking of agent provider CLIs.
//!
//! # Port / adapter role
//!
//! In the hexagonal layout of `gestalt_core`, *what a provider can do* is a
//! domain concern ([`crate::domain::capabilities`]), while *which provider
//! binaries exist on this machine* is an environment concern. This module is
//! the **adapter** half of that pair: it probes the local filesystem, builds the
//! matching domain descriptors, and answers "who can do X, and who is the
//! cheapest at it?".
//!
//! The registry is deliberately:
//!
//! - **passive** — it never spawns a process, it only checks that a file
//!   exists in a `PATH` entry;
//! - **stateless** — it holds no file handles or sockets, so it is
//!   `Send + Sync + Clone` and cheap to share;
//! - **testable** — [`ProviderRegistry::from_path_list`] and
//!   [`ProviderRegistry::register`] let callers inject an explicit environment
//!   or hand-written mocks instead of depending on the host's `PATH`.
//!
//! See [`crate::domain::capabilities::ProviderCapabilities`] for the descriptor
//! itself and [`crate::domain::capabilities::Capability`] for the requirement
//! vocabulary.

pub mod registry;

pub use registry::{ProviderRegistry, RegisteredProvider};
