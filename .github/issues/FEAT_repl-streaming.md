---
title: "FEAT: Interactive REPL & Streaming"
labels:
  - enhancement
  - cli
  - jules
assignees: []
---

## 🎯 Objective
Implement an interactive REPL (Read-Eval-Print Loop) for the Gestalt CLI that supports streaming responses from LLMs. This will allow users to have a continuous conversation with the agent.

## 📁 Files to Modify

### 1. `gestalt_core/src/ports/outbound/llm_provider.rs`
Update the `LlmProvider` trait to support streaming.

```rust
use futures::stream::BoxStream;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;

    // New method for streaming
    async fn stream(&self, request: LlmRequest) -> Result<BoxStream<'static, Result<String, LlmError>>, LlmError>;
}
```

### 2. `gestalt_cli/Cargo.toml`
Add dependencies for REPL functionality.
```toml
[dependencies]
rustyline = "13.0"
futures = "0.3"
tokio = { version = "1.0", features = ["full"] }
```

### 3. `gestalt_cli/src/main.rs`
Refactor `main` to support a REPL mode when no arguments are provided.

- If `args.prompt` is present -> Run single shot (existing logic).
- If `args.prompt` is missing -> Enter REPL loop.
- Implement commands: `/exit`, `/clear` (clear history), `/config` (show config).
- Maintain a `Vec<Message>` history context (if the provider supports it, otherwise just append to prompt).

## ✅ Acceptance Criteria
- [x] Running `gestalt` without arguments starts the REPL.
- [x] Typing a prompt streams the response token-by-token to stdout.
- [x] `/exit` or `Ctrl+C` exits the REPL.
- [x] Streaming behavior implemented in the CLI path for current architecture (`DecisionEngine` providers), replacing legacy `LlmProvider::stream` expectation.
- [x] Existing single-shot functionality (`--prompt`) still works.

## Progress Notes (2026-02-22)
- `gestalt_cli/src/main.rs` was migrated from deleted legacy `gestalt_core` LLM modules to `synapse-agentic::DecisionEngine`.
- Both modes are implemented and active:
  - Single-shot: `gestalt_cli --prompt "..."`
  - Interactive REPL: `gestalt_cli` (no args)
- REPL commands supported: `/exit`, `/clear`, `/config`.
- Response output now streams incrementally to stdout in both REPL and single-shot flows.

## 🧪 Testing
```bash
# Run REPL
cargo run -p gestalt_cli

# Run single shot
cargo run -p gestalt_cli -- --prompt "Hello"
```
