# MiniMax Provider Integration Task

## Objective
Implement MiniMaxProvider in `gestalt_core/src/adapters/llm/minimax.rs`

## Files to Modify
- `gestalt_core/src/adapters/llm/minimax.rs` (NEW)
- `gestalt_core/src/adapters/llm/mod.rs` (UPDATE)

## Reference
Use `gestalt_core/src/adapters/llm/openai.rs` as template

## API Information
```json
Endpoint: POST https://api.minimax.chat/v1/text/chatcompletion_v2
Headers:
  Authorization: Bearer {API_KEY}
  Content-Type: application/json

Body:
{
    "model": "MiniMax-M2",
    "messages": [...],
    "temperature": 0.7,
    "max_tokens": 4096
}
```

## Acceptance Criteria
- [ ] MiniMaxProvider implements LlmProvider trait
- [ ] Tests passing
- [ ] Registered in mod.rs
- [ ] Documentation updated

## Testing
```bash
cargo test -p gestalt_core
```
