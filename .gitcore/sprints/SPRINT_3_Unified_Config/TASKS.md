# Sprint 3: Unified Config

Generated: 2026-02-06 09:06 UTC

## Objective
Implement centralized TOML configuration system.

---

## Tasks

### S3-1: TOML Config Parsing

**Agent:** Codex
**Priority:** HIGH

**Task:**
Implement TOML config parsing in `gestalt_core/src/application/config.rs`

```rust
// config.rs

#[derive(Debug, Deserialize)]
pub struct GestaltConfig {
    pub llm: LlmConfig,
    pub database: DatabaseConfig,
    pub mcp: McpConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    pub providers: Vec<ProviderConfig>,
    pub default_provider: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub r#type: String,  // openai, gemini, qwen, ollama, minimax, etc.
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

impl GestaltConfig {
    pub fn from_toml(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(ConfigError::from)
    }
    
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load from environment variables
    }
}
```

**Files:**
- `gestalt_core/src/application/config.rs` (NEW)
- `gestalt_core/src/application/mod.rs` (UPDATE)

**Acceptance Criteria:**
- [ ] Parse TOML files
- [ ] Environment variable override
- [ ] Validation
- [ ] Default values

---

### S3-2: Environment Variable Support

**Agent:** Codex
**Priority:** HIGH

**Task:**
Add environment variable support to config.

```rust
// Support for:
// GESTALT_LLM__PROVIDERS__0__API_KEY
// GESTALT_DATABASE__URL
// GESTALT_MCP__SERVER_URL

pub fn load_from_env(prefix: &str) -> Result<GestaltConfig, ConfigError> {
    // Parse nested env vars
}
```

**Acceptance Criteria:**
- [ ] Nested env var parsing
- [ ] Prefix support
- [ ] Type conversion
- [ ] Error handling

---

### S3-3: Schema Validation

**Agent:** Codex
**Priority:** MEDIUM

**Task:**
Implement config schema validation.

```rust
#[derive(Validate)]
pub struct GestaltConfig {
    #[validate(required)]
    pub llm: LlmConfig,
    #[validate(required)]
    pub database: DatabaseConfig,
}
```

**Acceptance Criteria:**
- [ ] Required fields validated
- [ ] Type validation
- [ ] Custom validators
- [ ] Error messages

---

### S3-4: Configuration Guide

**Agent:** Gemini
**Priority:** MEDIUM

**Task:**
Create configuration guide documentation.

**Sections:**
1. Installation
2. TOML Configuration
3. Environment Variables
4. Examples
5. Troubleshooting

---

## Files to Create

```
gestalt_core/src/application/
├── config.rs           (NEW - 200 lines)
├── schema.rs          (NEW - validation)
└── CONFIG.md          (NEW - docs)
```

**Example config.toml:**

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
server_url = "http://localhost:3000"
```

---

## Definition of Done

- [ ] TOML parsing works
- [ ] Env vars override
- [ ] Validation complete
- [ ] Documentation done
- [ ] Tests passing
