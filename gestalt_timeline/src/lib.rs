//! Gestalt Timeline Orchestrator
//!
//! Meta-Agent CLI for coordinating projects and tasks with a universal timeline.

pub mod cli;
pub mod config;
pub mod db;
pub mod models;
pub mod services;

#[cfg(test)]
mod tests;

pub use db::SurrealClient;
pub use models::{EventType, Project, Task, TimelineEvent};
pub use services::{
    Agent, AgentRuntime, AgentService, AgentStatus, AgentType, DispatcherService, MemoryService,
    ProjectService, TaskQueue, TaskService, TelegramService, TimelineService, WatchService,
};
