//! Services module

mod agent;
mod auth;
mod project;
mod task;
mod timeline;
mod watch;
mod runtime;
mod server;
pub mod telegram;
pub mod memory;
pub mod task_queue;
pub mod dispatcher;

pub use agent::{Agent, AgentService, AgentStatus, AgentType};
pub use auth::AuthService;

pub use project::ProjectService;
pub use task::TaskService;
pub use timeline::TimelineService;
pub use watch::WatchService;
pub use telegram::TelegramService;
pub use runtime::{AgentRuntime, OrchestrationAction};
pub use server::start_server;
pub use memory::{MemoryFragment, MemoryService};
pub use task_queue::{TaskQueue, QueuedTask, TaskSource};
pub use dispatcher::DispatcherService;
