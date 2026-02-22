//! Services module

mod agent;
mod auth;
pub mod dispatcher;
pub mod memory;
mod project;
mod runtime;
mod server;
mod task;
pub mod task_queue;
pub mod telegram;
mod timeline;
mod watch;

pub use agent::{Agent, AgentService, AgentStatus, AgentType};
pub use auth::AuthService;

pub use dispatcher::DispatcherService;
pub use memory::{MemoryFragment, MemoryService};
pub use project::ProjectService;
pub use runtime::{AgentRuntime, OrchestrationAction};
pub use server::start_server;
pub use task::TaskService;
pub use task_queue::{QueuedTask, TaskQueue, TaskSource};
pub use telegram::TelegramService;
pub use timeline::TimelineService;
pub use watch::WatchService;
