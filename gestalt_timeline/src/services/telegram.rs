use crate::services::task_queue::{QueuedTask, TaskQueue, TaskSource};
use std::sync::Arc;
use synapse_agentic::prelude::*;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::{error, info};

#[derive(Clone)]
pub struct TelegramService {
    token: String,
    engine: Arc<DecisionEngine>,
    allowed_users: Option<Vec<String>>,
    /// If present, messages that start with /task or /run will be pushed to the task queue
    /// instead of being handled as direct chat — enabling multi-agent task execution.
    task_queue: Option<Arc<TaskQueue>>,
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "🤖 Gestalt Nexus Bot Commands:"
)]
enum Command {
    #[command(description = "show this help text.")]
    Help,
    #[command(description = "start the bot.")]
    Start,
    #[command(description = "chat directly with the agent.")]
    Chat(String),
    #[command(description = "queue an agentic task for background execution.")]
    Task(String),
    #[command(description = "check system status and active agents.")]
    Status,
}

impl TelegramService {
    pub fn new(
        token: String,
        engine: Arc<DecisionEngine>,
        allowed_users: Option<Vec<String>>,
    ) -> Self {
        Self {
            token,
            engine,
            allowed_users,
            task_queue: None,
        }
    }

    /// Wire a TaskQueue so that /task commands create autonomous agent goals.
    pub fn with_task_queue(mut self, queue: Arc<TaskQueue>) -> Self {
        self.task_queue = Some(queue);
        self
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        info!("🤖 Starting Gestalt Nexus Telegram Bot...");
        let bot = Bot::new(&self.token);
        let service = self.clone();

        let handler = Update::filter_message()
            .filter_command::<Command>()
            .endpoint(
                move |bot: Bot, msg: teloxide::types::Message, cmd: Command| {
                    let service = service.clone();
                    async move { service.answer(bot, msg, cmd).await }
                },
            );

        Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    fn is_allowed(&self, msg: &teloxide::types::Message) -> bool {
        match &self.allowed_users {
            None => true,
            Some(allowed) => {
                let username = msg.chat.username().unwrap_or("unknown");
                allowed.contains(&username.to_string())
            }
        }
    }

    async fn answer(
        &self,
        bot: Bot,
        msg: teloxide::types::Message,
        cmd: Command,
    ) -> ResponseResult<()> {
        if !self.is_allowed(&msg) {
            bot.send_message(msg.chat.id, "⛔ Access denied.").await?;
            return Ok(());
        }

        match cmd {
            Command::Help => {
                bot.send_message(msg.chat.id, Command::descriptions().to_string())
                    .await?;
            }
            Command::Start => {
                bot.send_message(
                    msg.chat.id,
                    "👋 *Gestalt Nexus* online!\n\n\
                    I can:\n\
                    • Answer questions directly (/chat)\n\
                    • Queue autonomous tasks for background agents (/task)\n\
                    • Report system status (/status)\n\n\
                    For complex work, use /task and I'll launch an agent loop to handle it.",
                )
                .await?;
            }
            Command::Chat(text) => {
                if text.trim().is_empty() {
                    bot.send_message(msg.chat.id, "Usage: /chat <message>")
                        .await?;
                    return Ok(());
                }

                bot.send_message(msg.chat.id, "🤔 Thinking...").await?;

                let context = DecisionContext::new("telegram")
                    .with_summary(&text)
                    .with_data(serde_json::json!({ "chat_id": msg.chat.id.to_string() }));

                match self.engine.decide(&context).await {
                    Ok(decision) => {
                        // Chunk for Telegram's 4096 char limit
                        let chars: Vec<char> = decision.reasoning.chars().collect();
                        for chunk in chars.chunks(4000) {
                            let chunk_str: String = chunk.iter().collect();
                            bot.send_message(msg.chat.id, chunk_str).await?;
                        }
                    }
                    Err(e) => {
                        error!("Chat error: {:?}", e);
                        bot.send_message(msg.chat.id, format!("❌ Error: {}", e))
                            .await?;
                    }
                }
            }
            Command::Task(goal) => {
                if goal.trim().is_empty() {
                    bot.send_message(
                        msg.chat.id,
                        "Usage: /task <describe your goal in natural language>",
                    )
                    .await?;
                    return Ok(());
                }

                let user_id = msg.chat.username().unwrap_or("unknown").to_string();
                let chat_id = msg.chat.id.to_string();

                if let Some(queue) = &self.task_queue {
                    let queued = QueuedTask::new(
                        goal.clone(),
                        TaskSource::Telegram {
                            user_id: user_id.clone(),
                            chat_id,
                        },
                        7, // High priority for user-initiated tasks
                    );

                    match queue.enqueue(queued).await {
                        Ok(_) => {
                            bot.send_message(
                                msg.chat.id,
                                format!(
                                    "🚀 Task queued!\n\n*Goal:* {}\n\nAn agent will pick this up shortly and work on it autonomously. I'll update when done.",
                                    &goal[..goal.len().min(200)]
                                ),
                            ).await?;
                        }
                        Err(e) => {
                            error!("Failed to enqueue task: {}", e);
                            bot.send_message(
                                msg.chat.id,
                                format!("❌ Failed to queue task: {}", e),
                            )
                            .await?;
                        }
                    }
                } else {
                    // No task queue configured — fall back to direct chat
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Task queue not configured. Use /chat for direct queries.",
                    )
                    .await?;
                }
            }
            Command::Status => {
                let queue_status = if self.task_queue.is_some() {
                    "✅ TaskQueue: Online"
                } else {
                    "⚠️ TaskQueue: Not configured"
                };
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "🟢 *Gestalt Nexus Status*\n\
                        • LLM Engine: `Synapse DecisionEngine`\n\
                        • {}\n\
                        • Spawning: Parallel sub-agents enabled\n\
                        • Memory: SurrealDB persistent",
                        queue_status
                    ),
                )
                .await?;
            }
        }

        Ok(())
    }
}
