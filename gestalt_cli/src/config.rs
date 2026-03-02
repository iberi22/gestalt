use serde::{Deserialize, Serialize};
use config::{Config, ConfigError, File, Environment};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CliConfig {
    pub mcp: McpConfig,
    pub logging: LoggingConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpConfig {
    pub server_url: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseConfig {
    pub r#type: String,
    pub url: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            mcp: McpConfig {
                server_url: "http://127.0.0.1:3000".to_string(),
                enabled: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "text".to_string(),
            },
            database: DatabaseConfig {
                r#type: "surreal".to_string(),
                url: "mem://".to_string(),
            },
        }
    }
}

impl CliConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut s = Config::builder();

        // 1. Add configuration from file
        let config_paths = [
            "gestalt",
            "config/gestalt",
        ];

        for path in config_paths {
            if std::path::Path::new(&format!("{}.toml", path)).exists() {
                s = s.add_source(File::with_name(path));
            }
        }

        // 2. Add environment variables (e.g., GESTALT_MCP__SERVER_URL)
        // Note: use __ for nested fields like mcp.server_url -> GESTALT_MCP__SERVER_URL
        s = s.add_source(Environment::with_prefix("GESTALT").separator("__"));

        let config = s.build()?;

        let mut res: Self = config.try_deserialize().unwrap_or_default();

        // Manual override if it was still "localhost" from the file but we want "127.0.0.1" as default
        if res.mcp.server_url == "http://localhost:3000" {
             res.mcp.server_url = "http://127.0.0.1:3000".to_string();
        }

        Ok(res)
    }
}
