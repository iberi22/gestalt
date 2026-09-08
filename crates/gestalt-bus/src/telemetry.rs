use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AgentEvent {
    StepProgress {
        step: usize,
        total_steps: usize,
        message: String,
        #[serde(default = "Utc::now")]
        timestamp: DateTime<Utc>,
    },
    ToolInvoked {
        tool: String,
        input: serde_json::Value,
        status: String,
        #[serde(default = "Utc::now")]
        timestamp: DateTime<Utc>,
    },
    ErrorCaptured {
        error: String,
        code: Option<String>,
        #[serde(default = "Utc::now")]
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Default, Clone)]
pub struct TelemetryEmitter;

impl TelemetryEmitter {
    pub fn new() -> Self {
        Self
    }

    pub fn emit(&self, event: &AgentEvent) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(event)?;
        println!("{}", json);
        Ok(json)
    }
}
