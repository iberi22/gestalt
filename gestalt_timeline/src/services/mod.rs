//! Services module

mod agent;
mod auth;
pub mod context_compaction;
pub mod dispatcher;
pub mod file_manager;
mod index;
pub mod memory;
mod project;
pub mod protocol_sync;
pub mod reviewer_merge_agent;
mod runtime;
mod server;
mod task;
pub mod task_queue;
#[cfg(feature = "telegram")]
pub mod telegram;
mod timeline;
mod watch;

pub use agent::{Agent, AgentService, AgentStatus, AgentType};
pub use auth::AuthService;

pub use context_compaction::{CompactionOutcome, ContextCompactor};
pub use dispatcher::DispatcherService;
pub use file_manager::{FileManager, FileManagerActor, FileState};
pub use gestalt_core::ports::outbound::vfs::{
    FileEventType, FileWatchEvent, FileWatcher, FlushError, FlushReport, LockStatus, OverlayFs,
    PendingChange, VirtualFileSystem as VirtualFs,
};
pub use index::IndexService;
pub use memory::{MemoryFragment, MemoryService};
pub use project::ProjectService;
pub use protocol_sync::ProtocolSyncService;
pub use reviewer_merge_agent::{
    spawn_reviewer_agent, ReviewResult, ReviewerMergeAgent, ReviewerMessage,
};
pub use runtime::{AgentRuntime, OrchestrationAction};
pub use server::start_server;
pub use task::TaskService;
pub use task_queue::{QueuedTask, TaskQueue, TaskSource};
#[cfg(feature = "telegram")]
pub use telegram::TelegramService;
pub use timeline::TimelineService;
pub use watch::WatchService;
