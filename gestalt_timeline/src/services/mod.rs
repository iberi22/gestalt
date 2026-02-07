//! Services module

mod agent;
mod auth;
mod gemini;
mod llm;
mod llm_minimax;
mod project;
mod task;
mod timeline;
mod watch;
mod runtime;
mod server;
pub mod telegram;

pub use agent::{Agent, AgentService, AgentStatus, AgentType};
pub use auth::AuthService;
pub use gemini::GeminiService;

// Re-export LLM services
pub use llm::{Cognition, OrchestrationAction};
pub use llm_minimax::{LLMService as MiniMaxLLMService, LLMResponse};

#[cfg(feature = "bedrock")]
pub use llm::LLMService as BedrockLLMService;

pub use project::ProjectService;
pub use task::TaskService;
pub use timeline::TimelineService;
pub use watch::WatchService;
pub use telegram::TelegramService;
pub use runtime::AgentRuntime;
pub use server::start_server;
