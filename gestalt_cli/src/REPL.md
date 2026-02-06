# Interactive REPL

This document describes the Interactive REPL mode for Gestalt Rust.

## Overview

The REPL (Read-Eval-Print Loop) provides a stateful command-line interface for interacting with Gestalt Rust. It supports:

- Command history (with arrow keys)
- Auto-completion
- Variables
- Context management
- Plugin handlers

## Starting the REPL

```bash
# From project root
cargo run --bin gestalt-cli -- repl

# Or with config
gestalt-cli repl --config gestalt.toml
```

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `exit`, `quit` | Exit the REPL | `exit` |
| `help` | Show help | `help` |
| `clear` | Clear screen | `clear` |
| `history` | Show command history | `history` |
| `context [json]` | Get/set context | `context {"key": "value"}` |
| `set VAR=value` | Set variable | `set name=Gestalt` |
| `get VAR` | Get variable | `get name` |
| `run <expr>` | Run expression | `run "hello"` |
| `<expression>` | Direct input | `hello world` |

## Variables

```gestalt
> set model=gpt-4
Set model=gpt-4

> get model
gpt-4

> set temperature=0.7
Set temperature=0.7
```

## Context

```gestalt
> context {"task": "coding", "language": "rust"}

> context
Object {"task": String("coding"), "language": String("rust")}
```

## History

Use arrow keys to navigate command history:

- `↑` (Up arrow) - Previous command
- `↓` (Down arrow) - Next command
- `Ctrl+R` - Reverse search

```gestalt
> history
[0] set name=Gestalt
[1] get name
[2] context {"task": "coding"}
```

## Auto-completion

Press `Tab` for auto-completion:

- Commands: `hel[TAB]` → `help`
- Variables: `na[TAB]` → `name`
- Files: `./[TAB]` shows directory

## Custom Handlers

Implement `ReplHandler` for custom behavior:

```rust
use gestalt_cli::repl::{ReplHandler, ReplError};

struct MyHandler;

#[async_trait]
impl ReplHandler for MyHandler {
    async fn handle_command(
        &mut self,
        command: &str,
        args: &[&str]
    ) -> Result<(), ReplError> {
        match command {
            "mycommand" => println!("My command!"),
            _ => return Err(ReplError::Command("Unknown".to_string())),
        }
        Ok(())
    }
    
    async fn handle_input(&mut self, input: &str) -> Result<String, ReplError> {
        Ok(format!("Processed: {}", input))
    }
}
```

## State Management

```rust
use gestalt_cli::repl::{InteractiveRepl, ReplState, Message};

let mut repl = InteractiveRepl::new().unwrap();

// Access state
let state = repl.state.lock().await;
state.add_message("user", "Hello");

// Variables
state.set_var("model", "gpt-4");
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+C` | Interrupt |
| `Ctrl+D` | EOF (exit) |
| `Ctrl+L` | Clear screen |
| `Ctrl+R` | Reverse search |
| `Tab` | Auto-complete |
| `↑/↓` | History |

## Configuration

```toml
[repl]
prompt = "gestalt> "
history_size = 1000
completion = true
```

## Integration

### In Code

```rust
use gestalt_cli::repl::{InteractiveRepl, EchoHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut repl = InteractiveRepl::with_handler(EchoHandler::default())?;
    repl.run().await?;
    Ok(())
}
```

### As Binary

```bash
# Build
cargo build --bin gestalt-repl

# Run
./target/debug/gestalt-repl
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| No history | Check `~/.gestalt_repl_history` permissions |
| Tab not working | Ensure `rustyline` features enabled |
| Slow completion | Reduce history size |
| Variables lost | REPL variables are session-only |
