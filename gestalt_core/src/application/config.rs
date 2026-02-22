//! Configuration Module
//!
//! Centralized configuration system with environment variable override.
//! Uses the `config` crate for TOML/YAML/JSON support.

use serde::Deserialize;
use std::env;
use std::path::Path;
use thiserror::Error;
use config::Config as ConfigBuilder;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Environment variable error: {0}")]
    EnvError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),
}

/// LLM Provider configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
}

/// LLM configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub providers: Vec<ProviderConfig>,
    pub default_provider: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl LlmConfig {
    pub fn default_provider(&self) -> &str {
        self.default_provider.as_deref().unwrap_or("openai")
    }
}

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
}

/// MCP configuration
#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    pub enabled: Option<bool>,
    pub server_url: Option<String>,
    pub default_timeout: Option<u64>,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: Option<String>,
    pub format: Option<String>,
    pub file: Option<String>,
}

/// Gestalt main configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GestaltConfig {
    pub llm: Option<LlmConfig>,
    pub database: Option<DatabaseConfig>,
    pub mcp: Option<McpConfig>,
    pub logging: Option<LoggingConfig>,
}

impl GestaltConfig {
    /// Load configuration from file (TOML, YAML, or JSON)
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.to_string_lossy().to_string()));
        }
        
        let settings = ConfigBuilder::builder()
            .add_source(config::File::with_name(&path.to_string_lossy()))
            .build()?;
        
        settings.try_deserialize::<GestaltConfig>().map_err(|e| ConfigError::ParseError(e.to_string()))
    }
    
    /// Load configuration from environment variables
    /// Uses GESTALT_* prefix with double underscores for nesting
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = GestaltConfig::default();
        
        // LLM configuration from env
        if let Ok(api_key) = env::var("GESTALT_LLM__API_KEY") {
            config.llm = Some(LlmConfig {
                providers: vec![ProviderConfig {
                    name: "default".to_string(),
                    provider_type: "default".to_string(),
                    api_key: Some(api_key),
                    base_url: None,
                    model: None,
                }],
                default_provider: env::var("GESTALT_LLM__DEFAULT_PROVIDER").ok(),
                temperature: env::var("GESTALT_LLM__TEMPERATURE")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                max_tokens: env::var("GESTALT_LLM__MAX_TOKENS")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            });
        }
        
        // Database configuration from env
        if let Ok(url) = env::var("GESTALT_DATABASE__URL") {
            config.database = Some(DatabaseConfig {
                db_type: env::var("GESTALT_DATABASE__TYPE").ok(),
                url: Some(url),
                path: env::var("GESTALT_DATABASE__PATH").ok(),
            });
        }
        
        // MCP configuration from env
        if let Ok(server_url) = env::var("GESTALT_MCP__SERVER_URL") {
            config.mcp = Some(McpConfig {
                enabled: env::var("GESTALT_MCP__ENABLED")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                server_url: Some(server_url),
                default_timeout: env::var("GESTALT_MCP__DEFAULT_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            });
        }
        
        // Logging configuration from env
        if let Ok(level) = env::var("GESTALT_LOGGING__LEVEL") {
            config.logging = Some(LoggingConfig {
                level: Some(level),
                format: env::var("GESTALT_LOGGING__FORMAT").ok(),
                file: env::var("GESTALT_LOGGING__FILE").ok(),
            });
        }
        
        Ok(config)
    }
    
    /// Merge with defaults
    pub fn with_defaults(self) -> Self {
        Self {
            llm: self.llm.or(Some(LlmConfig {
                providers: vec![],
                default_provider: Some("openai".to_string()),
                temperature: Some(0.7),
                max_tokens: Some(4096),
            })),
            database: self.database.or(Some(DatabaseConfig {
                db_type: Some("memory".to_string()),
                url: None,
                path: None,
            })),
            mcp: self.mcp.or(Some(McpConfig {
                enabled: Some(false),
                server_url: None,
                default_timeout: Some(30),
            })),
            logging: self.logging.or(Some(LoggingConfig {
                level: Some("info".to_string()),
                format: Some("json".to_string()),
                file: None,
            })),
        }
    }
}

/// Load configuration with fallback chain
pub fn load_config(config_paths: &[&Path]) -> Result<GestaltConfig, ConfigError> {
    // Try each path
    for path in config_paths {
        if path.exists() {
            return GestaltConfig::from_file(path);
        }
    }
    
    // Fall back to environment
    let config = GestaltConfig::from_env()?;
    
    // Apply defaults
    Ok(config.with_defaults())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = GestaltConfig::default();
        assert!(config.llm.is_none());
        assert!(config.database.is_none());
    }
    
    #[test]
    fn test_config_with_defaults() {
        let config = GestaltConfig::default().with_defaults();
        assert!(config.llm.is_some());
        assert!(config.database.is_some());
    }
    
    #[test]
    fn test_env_override() {
        // Set environment variable
        env::set_var("GESTALT_LLM__TEMPERATURE", "0.5");
        
        let config = GestaltConfig::from_env();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert!(config.llm.is_some());
        assert_eq!(config.llm.unwrap().temperature, Some(0.5));
        
        // Clean up
        env::remove_var("GESTALT_LLM__TEMPERATURE");
    }
}
