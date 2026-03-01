use crate::db::SurrealClient;
use crate::models::{EventType, TimelineEvent};
use crate::services::task_queue::{QueuedTask, TaskQueue, TaskSource};
use crate::services::watch::WatchMessage;
use crate::services::WatchService;
use std::sync::Arc;
use synapse_agentic::prelude::*;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct TelegramService {
    token: String,
    engine: Arc<DecisionEngine>,
    allowed_users: Option<Vec<String>>,
    /// If present, messages that start with /task or /run will be pushed to the task queue
    /// instead of being handled as direct chat — enabling multi-agent task execution.
    task_queue: Option<Arc<TaskQueue>>,
    watch: Arc<WatchService>,
    db: SurrealClient,
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
        watch: Arc<WatchService>,
        db: SurrealClient,
    ) -> Self {
        Self {
            token,
            engine,
            allowed_users,
            task_queue: None,
            watch,
            db,
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

        // Spawn background listener for WatchService events
        let bot_clone = bot.clone();
        let service_clone = self.clone();
        tokio::spawn(async move {
            let mut rx = service_clone.watch.subscribe();
            info!("📡 Telegram background listener active");

            while let Ok(msg) = rx.recv().await {
                match msg {
                    WatchMessage::Event(event) => {
                        let _ = service_clone.handle_timeline_event(bot_clone.clone(), *event).await;
                    }
                    WatchMessage::Shutdown => break,
                    _ => {}
                }
            }
        });

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

    async fn handle_timeline_event(&self, bot: Bot, event: TimelineEvent) -> anyhow::Result<()> {
        // Only handle Chat messages or Task completion/failure for now
        let message = match &event.event_type {
            EventType::Chat => event.payload["message"].as_str().map(|s| s.to_string()),
            EventType::TaskStarted => Some(format!("🎯 Task Started: `{}`", event.agent_id)),
            EventType::TaskCompleted => Some(format!("✅ Task Completed: `{}`", event.agent_id)),
            EventType::TaskFailed => Some(format!("❌ Task Failed: `{}`", event.agent_id)),
            _ => None,
        };

        if let Some(text) = message {
            // 1. Try to find if this agent_id is mapped to a chat_id
            let chat_id: Option<String> = self
                .db
                .query_with::<Value>(
                    "SELECT chat_id FROM telegram_chats WHERE id = $id LIMIT 1",
                    serde_json::json!({ "id": event.agent_id }),
                )
                .await?
                .get(0)
                .and_then(|v| v["chat_id"].as_str().map(|s| s.to_string()));

            // 2. Fallback: If it's a task event, try to find the task to see its source
            let chat_id = match (chat_id, &event.task_id) {
                (Some(id), _) => Some(id),
                (None, Some(tid)) => {
                    // Query task_queue for the source chat_id
                    self.db
                        .query_with::<Value>(
                            "SELECT source.Telegram.chat_id as cid FROM task_queue WHERE id = $id LIMIT 1",
                            serde_json::json!({ "id": tid }),
                        )
                        .await?
                        .get(0)
                        .and_then(|v| v["cid"].as_str().map(|s| s.to_string()))
                }
                (None, None) => None,
            };

            if let Some(cid) = chat_id {
                if let Ok(id) = cid.parse::<i64>() {
                    // Escape for MarkdownV2?
                    // Let's defer robust escaping to step 5, but use it here as well
                    let escaped = self.escape_markdown(&text);
                    let _ = bot.send_message(ChatId(id), escaped)
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await;
                }
            }
        }

        Ok(())
    }

    fn escape_markdown(&self, text: &str) -> String {
        let reserved = [
            '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
            '\\',
        ];
        let mut escaped = String::with_capacity(text.len() * 2);
        for c in text.chars() {
            if reserved.contains(&c) {
                escaped.push('\\');
            }
            escaped.push(c);
        }
        escaped
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
                let text = self.escape_markdown(&Command::descriptions().to_string());
                bot.send_message(msg.chat.id, text)
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .await?;
            }
            Command::Start => {
                bot.send_message(
                    msg.chat.id,
                    "👋 *Gestalt Nexus* online\\!\n\n\
                    I can:\n\
                    • Answer questions directly \\(/chat\\)\n\
                    • Queue autonomous tasks for background agents \\(/task\\)\n\
                    • Report system status \\(/status\\)\n\n\
                    For complex work, use /task and I\\'ll launch an agent loop to handle it.",
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
            }
            Command::Chat(text) => {
                if text.trim().is_empty() {
                    bot.send_message(msg.chat.id, "Usage: /chat \\<message\\>")
                        .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                        .await?;
                    return Ok(());
                }

                bot.send_message(msg.chat.id, "🤔 Thinking\\.\\.\\.")
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                    .await?;

                // Map this specific chat session to an agent_id (for conversational direct chat)
                let agent_id = format!("telegram-chat-{}", msg.chat.id);
                let _ = self.db.query_with::<Value>(
                    "INSERT INTO telegram_chats { id: $id, chat_id: $chat_id } ON DUPLICATE KEY UPDATE chat_id = $chat_id",
                    serde_json::json!({ "id": agent_id, "chat_id": msg.chat.id.to_string() })
                ).await;

                let context = DecisionContext::new("telegram")
                    .with_summary(&text)
                    .with_data(serde_json::json!({ "chat_id": msg.chat.id.to_string() }));

                match self.engine.decide(&context).await {
                    Ok(decision) => {
                        // Chunk for Telegram's 4096 char limit
                        let escaped = self.escape_markdown(&decision.reasoning);
                        let chars: Vec<char> = escaped.chars().collect();
                        for chunk in chars.chunks(4000) {
                            let chunk_str: String = chunk.iter().collect();
                            bot.send_message(msg.chat.id, chunk_str)
                                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                                .await?;
                        }
                    }
                    Err(e) => {
                        error!("Chat error: {:?}", e);
                        bot.send_message(msg.chat.id, format!("❌ Error: {}", self.escape_markdown(&e.to_string())))
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                    }
                }
            }
            Command::Task(goal) => {
                if goal.trim().is_empty() {
                    bot.send_message(
                        msg.chat.id,
                        "Usage: /task \\<describe your goal in natural language\\>",
                    )
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
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

                    match queue.enqueue(queued.clone()).await {
                        Ok(_) => {
                            bot.send_message(
                                msg.chat.id,
                                format!(
                                    "🚀 Task queued\\!\n\n*Goal:* {}\n\nAn agent will pick this up shortly and work on it autonomously\\. I\\'ll update when done\\.",
                                    self.escape_markdown(&goal[..goal.len().min(200)])
                                ),
                            )
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                        }
                        Err(e) => {
                            error!("Failed to enqueue task: {}", e);
                            bot.send_message(
                                msg.chat.id,
                                format!("❌ Failed to queue task: {}", self.escape_markdown(&e.to_string())),
                            )
                            .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                            .await?;
                        }
                    }
                } else {
                    // No task queue configured — fall back to direct chat
                    bot.send_message(
                        msg.chat.id,
                        "⚠️ Task queue not configured\\. Use /chat for direct queries\\.",
                    )
                    .parse_mode(teloxide::types::ParseMode::MarkdownV2)
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
                        • Spawning: Parallel sub\\-agents enabled\n\
                        • Memory: SurrealDB persistent",
                        self.escape_markdown(queue_status)
                    ),
                )
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
            }
        }

        Ok(())
    }
}
