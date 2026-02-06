# MiniMax API Configuration

Add to your `.env` or environment:

```bash
# MiniMax API
MINIMAX_API_KEY=your_api_key_here
MINIMAX_MODEL=MiniMax-M2
MINIMAX_API_BASE=https://api.minimax.chat/v1/text
```

## Usage

```rust
use gestalt_core::adapters::llm::minimax::MiniMaxProvider;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = MiniMaxProvider::from_env()?;
    Ok(())
}
```

## Supported Models

- `MiniMax-M2` (default)
- `MiniMax-M2.1` (latest)
