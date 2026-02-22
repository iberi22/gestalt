use anyhow::Result;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tracing::warn;
use std::sync::Arc;

use synapse_agentic::prelude::*;

/// Runs the interactive REPL for the AI Chat.
pub async fn run_repl(_agent_id: &str, engine: Arc<DecisionEngine>) -> Result<()> {
    println!("🤖 Entering Interactive AI Chat (Synapse Decision Engine)");
    println!("📝 Type 'exit' or 'quit' to leave. 'clear' to reset context.");

    let mut rl = DefaultEditor::new()?;

    // Load history if available
    let _ = rl.load_history("history.txt");

    // Maintain conversation history in memory for the LLM context
    // Ideally, we should fetch previous messages if this is a persistent session,
    // but for now we start fresh per CLI run, similar to a shell session.
    let mut messages: Vec<String> = Vec::new();

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                let _ = rl.add_history_entry(input);

                if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                    println!("👋 Goodbye!");
                    break;
                }

                if input.eq_ignore_ascii_case("clear") {
                    messages.clear();
                    let _ = rl.clear_history();
                    println!("🧹 Context cleared.");
                    continue;
                }

                if input.is_empty() {
                    continue;
                }

                // Add user message to history
                // Note: The `chat` method currently takes a single message string.
                // To support true context, we need to modify or use an overload of `chat`
                // that accepts history. Since `Cognition` trait `chat` is:
                // `async fn chat(&self, agent_id: &str, message: &str) -> Result<LLMResponse>;`
                // It seems it might be stateless or assume state is managed internally?
                // Checking `LLMService::chat`: It constructs `conversation_history` but it's local to the function.
                // To support context, we need to pass the history.

                // Hack: We will concatenate history for now if the underlying service doesn't support context passing.
                // OR better: We rely on the fact that `LLMService` (Bedrock) is stateless per request unless we send history.
                // BUT `chat` method signature is limited.

                // Let's mimic what `orchestrate_step` does but for chat.
                // We'll modify the `chat` method in `Cognition` trait or use a new method `chat_with_history`.
                // FOR NOW, to avoid breaking changes in this specific file edit, we will send the NEW message.
                // Wait, if I send just the new message, the LLM won't know previous context.
                // I need to change the `Cognition` trait to support history, or Append history to the prompt manually.

                // Approach: Append history to the prompt manually for now.
                // "History:\nUser: ...\nAI: ...\n\nCurrent: ..."

                let prompt_with_context = if messages.is_empty() {
                    input.to_string()
                } else {
                    format!(
                        "Here is the conversation history:\n{}\n\nUser: {}",
                        messages.join("\n"),
                        input
                    )
                };

                println!("🤖 Thinking...");
                let context = DecisionContext::new("repl")
                    .with_summary(&prompt_with_context);

                match engine.decide(&context).await {
                    Ok(decision) => {
                         println!("{}", decision.reasoning);

                         // Update local history
                         messages.push(format!("User: {}", input));
                         messages.push(format!("AI: {}", decision.reasoning));
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                        warn!("Error in chat REPL: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    // Save history
    let _ = rl.save_history("history.txt");

    Ok(())
}
