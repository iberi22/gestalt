# Unified Configuration Guide

This document describes the centralized configuration system for Gestalt Rust.

## Overview

Gestalt Rust uses a unified TOML-based configuration system with environment variable overrides. Configuration is loaded in the following order:

1. **TOML file** (highest priority)
2. **Environment variables**
3. **Defaults** (lowest priority)

## TOML Configuration

### Basic Structure

```toml
[llm]
default_provider = "openai"
temperature = 0.7
max_tokens = 4096

[[llm.providers]]
name = "openai"
type = "openai"
api_key = "${OPENAI_API_KEY}"
model = "gpt-4"

[[llm.providers]]
name = "minimax"
type = "minimax"
api_key = "${MINIMAX_API_KEY}"
model = "MiniMax-M2"

[database]
type = "surreal"
url = "ws://localhost:8000"

[mcp]
enabled = true
server_url = "http://localhost:3000"
default_timeout = 30

[logging]
level = "info"
format = "json"
```

## LLM Configuration

### Providers

```toml
[[llm.providers]]
name = "openai"
type = "openai"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
model = "gpt-4"

[[llm.providers]]
name = "ollama"
type = "ollama"
api_key = ""  # Not required
base_url = "http://localhost:11434"
model = "llama2"
```

### Settings

```toml
[llm]
default_provider = "openai"
temperature = 0.7  # 0.0 to 1.0
max_tokens = 4096
```

## Database Configuration

```toml
[database]
type = "surreal"  # or "memory", "sqlite"
url = "ws://localhost:8000"
path = "./data/surreal"
```

## MCP Configuration

```toml
[mcp]
enabled = true
server_url = "http://localhost:3000"
default_timeout = 30  # seconds
```

## Logging Configuration

```toml
[logging]
level = "debug"  # trace, debug, info, warn, error
format = "json"   # or "pretty"
file = "logs/gestalt.log"
```

## Environment Variables

Environment variables use a nested format with double underscores (`__`) as separators.

### Naming Convention

```
GESTALT_<SECTION>__<KEY>=<VALUE>
```

### Examples

```bash
# LLM settings
export GESTALT_LLM__DEFAULT_PROVIDER=openai
export GESTALT_LLM__TEMPERATURE=0.7
export GESTALT_LLM__MAX_TOKENS=4096
export GESTALT_LLM__PROVIDERS__0__API_KEY=sk-...

# Database settings
export GESTALT_DATABASE__URL=ws://localhost:8000
export GESTALT_DATABASE__TYPE=surreal

# MCP settings
export GESTALT_MCP__ENABLED=true
export GESTALT_MCP__SERVER_URL=http://localhost:3000

# Logging
export GESTALT_LOGGING__LEVEL=info
```

## API Reference

### GestaltConfig

```rust
struct GestaltConfig {
    llm: Option<LlmConfig>,
    database: Option<DatabaseConfig>,
    mcp: Option<McpConfig>,
    logging: Option<LoggingConfig>,
}

impl GestaltConfig {
    pub fn from_toml(path: &Path) -> Result<Self, ConfigError>;
    pub fn from_env() -> Result<Self, ConfigError>;
    pub fn with_defaults(self) -> Self;
}
```

### Loading Configuration

```rust
use gestalt_core::application::config::{load_config, GestaltConfig};

fn main() -> Result<(), Box<dyn Error>> {
    // Load from standard paths
    let config = load_config(&[
        Path::new("gestalt.toml"),
        Path::new("config/gestalt.toml"),
        Path::new("$HOME/.config/gestalt.toml"),
    ])?;
    
    println!("LLM Provider: {}", config.llm.unwrap().default_provider());
    Ok(())
}
```

## Example: Complete Setup

```rust
use gestalt_core::application::config::GestaltConfig;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Try TOML config first, then env vars, then defaults
    let config = Path::new("config.toml")
        .exists()
        .then(|| GestaltConfig::from_toml(Path::new("config.toml")))
        .unwrap_or_else(|| GestaltConfig::from_env())?
        .with_defaults();
    
    // Use configuration
    if let Some(llm) = config.llm {
        println!("Using provider: {}", llm.default_provider());
    }
    
    Ok(())
}
```

## Troubleshooting

### Common Issues

1. **Environment variable not loading**
   - Check prefix: must be `GESTALT_`
   - Use double underscores: `__`
   - Verify variable is set: `echo $GESTALT_LLM__API_KEY`

2. **TOML parsing error**
   - Valid TOML syntax required
   - Use double quotes for strings
   - Arrays use `[[...]]` syntax

3. **Values not overriding**
   - TOML has highest priority
   - Env vars come after
   - Check for typos

### Debug Mode

```bash
# Enable verbose logging
GESTALT_LOGGING__LEVEL=debug ./gestalt
```

## File Locations

| Path | Priority | Use Case |
|------|----------|----------|
| `./gestalt.toml` | Highest | Project-specific |
| `./config/gestalt.toml` | Medium | Local development |
| `$HOME/.config/gestalt.toml` | Low | User settings |
| Environment variables | Lowest | CI/CD, Docker |
