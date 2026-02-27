//! Services module

mod agent;
mod auth;
pub mod context_compaction;
pub mod dispatcher;
pub mod memory;
mod project;
pub mod reviewer_merge_agent;
mod runtime;
mod index;
pub mod file_manager;
mod server;
mod task;
pub mod task_queue;
pub mod telegram;
mod timeline;
pub mod vfs;
mod watch;

pub use agent::{Agent, AgentService, AgentStatus, AgentType};
pub use auth::AuthService;

pub use context_compaction::{CompactionOutcome, ContextCompactor};
pub use dispatcher::DispatcherService;
pub use memory::{MemoryFragment, MemoryService};
pub use index::IndexService;
pub use file_manager::{FileManager, FileManagerActor, FileState};
pub use project::ProjectService;
pub use reviewer_merge_agent::{
    spawn_reviewer_agent, ReviewResult, ReviewerMergeAgent, ReviewerMessage,
};
pub use runtime::{AgentRuntime, OrchestrationAction};
pub use server::start_server;
pub use task::TaskService;
pub use task_queue::{QueuedTask, TaskQueue, TaskSource};
pub use telegram::TelegramService;
pub use timeline::TimelineService;
pub use vfs::{FlushError, FlushReport, LockStatus, OverlayFs, PendingChange, VirtualFs};
pub use watch::WatchService;
