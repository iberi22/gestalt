# Sprint 4: Interactive REPL

Generated: 2026-02-06 09:06 UTC

## Objective
Implement stateful conversation mode with history and auto-completion.

---

## Tasks

### S4-1: REPL Interface

**Agent:** Codex
**Priority:** HIGH

**Task:**
Implement REPL interface in `gestalt_cli/src/repl.rs`

```rust
// gestalt_cli/src/repl.rs

use rustyline::{Editor, config::Config};
use gestalt_core::prelude::*;

pub struct GestaltRepl {
    editor: Editor<()>,
    agent: Box<dyn Agent>,
    history_file: PathBuf,
}

impl GestaltRepl {
    pub fn new(agent: impl Agent) -> Self {
        let editor = Editor::new()
            .with_config(Config::builder()
                .history_ignore_space(true)
                .completion_type(CompletionType::List)
                .build());
        
        Self {
            editor,
            agent: Box::new(agent),
            history_file: home_dir().unwrap().join(".gestalt_history"),
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        self.load_history()?;
        
        loop {
            let readline = self.editor.readline("gestalt> ");
            match readline {
                Ok(line) => {
                    self.editor.add_history_entry(&line);
                    self.handle_command(&line).await?;
                }
                Err(readline::Error::Interrupted) => break,
                Err(e) => return Err(e.into()),
            }
        }
        
        self.save_history()?;
        Ok(())
    }
    
    async fn handle_command(&mut self, input: &str) -> Result<()> {
        // Parse and execute command
    }
}

pub enum ReplCommand {
    Exit,
    Clear,
    History,
    Context(String),
    Agent(String),
}
```

**Files:**
- `gestalt_cli/src/repl.rs` (NEW - 200 lines)
- `gestalt_cli/src/mod.rs` (UPDATE)

---

### S4-2: State Management

**Agent:** Codex
**Priority:** HIGH

**Task:**
Add state management for conversation history.

```rust
// State management for REPL

pub struct ReplState {
    pub messages: Vec<Message>,
    pub context: HashMap<String, Value>,
    pub variables: HashMap<String, String>,
}

impl ReplState {
    pub fn new() -> Self {
        Self {
            messages: vec![],
            context: HashMap::new(),
            variables: HashMap::new(),
        }
    }
    
    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: Utc::now(),
        });
    }
    
    pub fn get_context(&self) -> String {
        // Serialize context for LLM
    }
}
```

---

### S4-3: History and Auto-completion

**Agent:** Codex
**Priority:** MEDIUM

**Task:**
Implement command history and auto-completion.

```rust
// Auto-completion

impl ConfigurableCompletion for GestaltCompleter {
    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &rustyline::Context
    ) -> rustline::Result<(usize, Vec<rustline::hint::Hint>)> {
        let commands = vec![
            "exit", "quit", "clear", "history",
            "context", "agent", "set", "get",
            "help", "load", "save",
        ];
        
        let matches: Vec<Hint> = commands
            .iter()
            .filter(|c| c.starts_with(line))
            .map(|c| Hint::with_text(c))
            .collect();
        
        Ok((line.len(), matches))
    }
}
```

---

### S4-4: REPL Documentation

**Agent:** Gemini
**Priority:** LOW

**Task:**
Document REPL usage guide.

**Sections:**
1. Getting Started
2. Commands Reference
3. Variables and Context
4. Examples
5. Keyboard Shortcuts

---

## Files to Create

```
gestalt_cli/src/
├── repl.rs              (NEW - 250 lines)
├── state.rs            (NEW - 100 lines)
├── completion.rs       (NEW - 80 lines)
└── REPL.md            (NEW - docs)
```

---

## REPL Commands

| Command | Description |
|---------|-------------|
| `exit` / `quit` | Exit REPL |
| `clear` | Clear screen |
| `history` | Show command history |
| `context [json]` | Show/set context |
| `agent [name]` | Switch agent |
| `set VAR=value` | Set variable |
| `get VAR` | Get variable |
| `help` | Show help |
| `load <file>` | Load file |
| `save <file>` | Save session |

---

## Definition of Done

- [ ] REPL starts and runs
- [ ] Commands work correctly
- [ ] History saved/loaded
- [ ] Auto-completion works
- [ ] Documentation complete
- [ ] Tests passing
