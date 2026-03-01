//! Interactive REPL
//!
//! Stateful conversation mode with history and auto-completion.

use async_trait::async_trait;
use rustyline::{Editor, config::Config};
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use std::fs;

/// REPL errors
#[derive(Debug, thiserror::Error)]
pub enum ReplError {
    #[error("Readline error: {0}")]
    Readline(#[from] ReadlineError),
    
    #[error("Command error: {0}")]
    Command(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Message in conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// REPL state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplState {
    pub messages: Vec<Message>,
    pub variables: std::collections::HashMap<String, String>,
    pub context: Value,
}

impl ReplState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            variables: std::collections::HashMap::new(),
            context: Value::Null,
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        });
    }
    
    pub fn set_var(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }
    
    pub fn get_var(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }
    
    pub fn clear(&mut self) {
        self.messages.clear();
        self.variables.clear();
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<(), ReplError> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn load_from_file(path: &PathBuf) -> Result<Self, ReplError> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let state = serde_json::from_str(&content)?;
            Ok(state)
        } else {
            Ok(Self::new())
        }
    }
}

/// REPL command
#[derive(Debug)]
pub enum ReplCommand {
    Exit,
    Help,
    Clear,
    History,
    Context(Option<Value>),
    Set(String, String),
    Get(String),
    Run(String),
    Unknown(String),
}

/// REPL trait for customization
#[async_trait]
pub trait ReplHandler: Send + Sync {
    async fn handle_command(&mut self, command: &str, args: &[&str]) -> Result<(), ReplError>;
    async fn handle_input(&mut self, input: &str) -> Result<String, ReplError>;
}

/// Default REPL handler
#[async_trait]
impl<H: ReplHandler> ReplHandler for Arc<Mutex<H>> {
    async fn handle_command(&mut self, command: &str, args: &[&str]) -> Result<(), ReplError> {
        let mut handler = self.lock().await;
        handler.handle_command(command, args).await
    }
    
    async fn handle_input(&mut self, input: &str) -> Result<String, ReplError> {
        let mut handler = self.lock().await;
        handler.handle_input(input).await
    }
}

/// Interactive REPL
pub struct InteractiveRepl<H: ReplHandler> {
    editor: Editor<(), FileHistory>,
    handler: Arc<Mutex<H>>,
    state: Arc<Mutex<ReplState>>,
    history_file: PathBuf,
    state_file: PathBuf,
}

impl<H: ReplHandler + Default> InteractiveRepl<H> {
    /// Create new REPL
    pub fn new() -> Result<Self, ReplError> {
        Self::with_handler(H::default())
    }
    
    /// Create with custom handler
    pub fn with_handler(handler: H) -> Result<Self, ReplError> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(rustyline::CompletionType::List)
            .build();
        let editor = Editor::<(), FileHistory>::with_config(config)?;
        
        let home = home::home_dir().unwrap_or(PathBuf::from("."));
        let history_file = home.join(".gestalt_repl_history");
        let state_file = home.join(".gestalt_repl_state.json");
        
        let state = ReplState::load_from_file(&state_file).unwrap_or_default();

        Ok(Self {
            editor,
            handler: Arc::new(Mutex::new(handler)),
            state: Arc::new(Mutex::new(state)),
            history_file,
            state_file,
        })
    }
    
    /// Run the REPL
    pub async fn run(&mut self) -> Result<(), ReplError> {
        // Load history
        if let Err(e) = self.editor.load_history(&self.history_file) {
            // Ignore if file doesn't exist
            if self.history_file.exists() {
                eprintln!("Warning: Could not load history: {}", e);
            }
        }
        
        println!("Gestalt Rust REPL v0.2.0");
        println!("Type 'help' for available commands.");
        println!("Press Ctrl+C or type 'exit' to quit.\n");
        
        loop {
            let readline = self.editor.readline("gestalt> ");
            
            match readline {
                Ok(line) => {
                    self.editor.add_history_entry(line.as_str())?;
                    
                    if line.trim().is_empty() {
                        continue;
                    }
                    
                    match self.process_line(&line).await {
                        Ok(Some(output)) => {
                            // Basic streaming-like output (preserving whitespace)
                            for char in output.chars() {
                                print!("{}", char);
                                std::io::Write::flush(&mut std::io::stdout())?;
                                if char == ' ' || char == '\n' {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                                } else {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
                                }
                            }
                            println!();
                        }
                        Ok(None) => {}
                        Err(ReplError::Command(cmd)) if cmd == "exit" => {
                            break;
                        }
                        Err(e) => eprintln!("Error: {}", e),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("\nUse 'exit' to quit.");
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    return Err(ReplError::Readline(e));
                }
            }
        }
        
        // Save history
        if let Err(e) = self.editor.save_history(&self.history_file) {
            eprintln!("Warning: Could not save history: {}", e);
        }
        
        // Save state
        let state = self.state.lock().await;
        if let Err(e) = state.save_to_file(&self.state_file) {
            eprintln!("Warning: Could not save state: {}", e);
        }

        Ok(())
    }
    
    /// Process input line
    async fn process_line(&mut self, line: &str) -> Result<Option<String>, ReplError> {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        
        if parts.is_empty() {
            return Ok(None);
        }
        
        let command = parts[0].to_lowercase();
        let args = &parts[1..];
        
        match command.as_str() {
            "exit" | "quit" => {
                return Err(ReplError::Command("exit".to_string()));
            }
            
            "help" => {
                Ok(Some(Self::help_text()))
            }
            
            "clear" => {
                let mut state = self.state.lock().await;
                state.clear();
                // Clear screen
                print!("\x1B[2J\x1B[3J\x1B[H");
                Ok(Some("Console cleared.".to_string()))
            }
            
            "history" => {
                let state = self.state.lock().await;
                let mut output = String::new();
                for (i, msg) in state.messages.iter().enumerate() {
                    output.push_str(&format!("[{}] {}: {}\n", i, msg.role, msg.content));
                }
                Ok(Some(output))
            }
            
            "context" => {
                let mut state = self.state.lock().await;
                if let Some(first) = args.first() {
                    // Set context from JSON
                    let json: Value = serde_json::from_str(first)?;
                    state.context = json;
                    Ok(Some("Context updated.".to_string()))
                } else {
                    // Show context
                    Ok(Some(format!("{:?}", state.context)))
                }
            }
            
            "set" => {
                if args.len() < 2 {
                    return Err(ReplError::Command("Usage: set VAR=value".to_string()));
                }
                let var_eq = args[0];
                let value = args[1..].join(" ");
                
                let (name, value) = if let Some(pos) = var_eq.find('=') {
                    let (name, _) = var_eq.split_at(pos);
                    (name, &value[..value.len().min(value.len())])
                } else {
                    (var_eq, &value[..value.len().min(value.len())])
                };
                
                let mut state = self.state.lock().await;
                state.set_var(name, value);
                Ok(Some(format!("Set {}={}", name, value)))
            }
            
            "get" => {
                if args.is_empty() {
                    return Err(ReplError::Command("Usage: get VAR".to_string()));
                }
                let state = self.state.lock().await;
                if let Some(value) = state.get_var(args[0]) {
                    Ok(Some(value.clone()))
                } else {
                    Ok(Some(format!("Variable '{}' not found", args[0])))
                }
            }
            
            "run" => {
                if args.is_empty() {
                    return Err(ReplError::Command("Usage: run <expression>".to_string()));
                }
                let expr = args.join(" ");
                let mut handler = self.handler.lock().await;
                Ok(Some(handler.handle_input(&expr).await?))
            }
            
            _ => {
                // Pass to handler
                let mut handler = self.handler.lock().await;
                let response = handler.handle_input(line).await?;

                let mut state = self.state.lock().await;
                state.add_message("user", line);
                state.add_message("assistant", &response);

                Ok(Some(response))
            }
        }
    }
    
    fn help_text() -> String {
        r#"Available commands:
  exit, quit     Exit the REPL
  help           Show this help message
  clear          Clear the console
  history        Show command history
  context [json] Show or set context
  set VAR=value  Set a variable
  get VAR        Get a variable
  run <expr>     Run an expression
  <expression>   Evaluate expression"#
            .to_string()
    }
}

impl<H: ReplHandler + Default> Default for InteractiveRepl<H> {
    fn default() -> Self {
        InteractiveRepl::new().unwrap()
    }
}

/// Simple handler that just echoes input
#[derive(Debug, Default)]
pub struct EchoHandler;

#[async_trait]
impl ReplHandler for EchoHandler {
    async fn handle_command(&mut self, _command: &str, _args: &[&str]) -> Result<(), ReplError> {
        Ok(())
    }
    
    async fn handle_input(&mut self, input: &str) -> Result<String, ReplError> {
        Ok(format!("Echo: {}", input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_repl_state() {
        let mut state = ReplState::new();
        state.add_message("user", "Hello");
        state.add_message("assistant", "Hi there!");
        
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].role, "user");
    }
    
    #[tokio::test]
    async fn test_variables() {
        let mut state = ReplState::new();
        state.set_var("name", "Gestalt");
        assert_eq!(state.get_var("name"), Some(&"Gestalt".to_string()));
        assert_eq!(state.get_var("unknown"), None);
    }
    
    #[tokio::test]
    async fn test_echo_handler() {
        let mut handler = EchoHandler::default();
        let result = handler.handle_input("test").await.unwrap();
        assert_eq!(result, "Echo: test");
    }
}
