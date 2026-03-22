//! Agent Registry Service
//!
//! Manages connected agents and their sessions in the timeline system.

use anyhow::Result;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tracing::{debug, info};

use crate::db::SurrealClient;
use crate::models::EventType;
use crate::models::FlexibleTimestamp;
use crate::services::TimelineService;

/// Represents a connected agent in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent identifier - stored as record ID in SurrealDB, not in content
    #[serde(skip)]
    pub id: Option<Thing>,

    /// Human-readable agent name
    pub name: String,

    /// Agent type (cli, copilot, antigravity, etc.)
    pub agent_type: AgentType,

    /// Connection status
    pub status: AgentStatus,

    /// When the agent connected
    #[serde(with = "crate::models::timestamp")]
    pub connected_at: FlexibleTimestamp,

    /// Last activity timestamp
    #[serde(with = "crate::models::timestamp")]
    pub last_seen: FlexibleTimestamp,

    /// Number of commands executed
    pub command_count: u64,

    /// Custom system prompt for this agent (Persona)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Specific model ID (e.g., "anthropic.claude-3-sonnet-20240229-v1:0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

/// Types of agents that can connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentType {
    Cli,
    VsCodeCopilot,
    Antigravity,
    GeminiCli,
    Custom(String),
}

impl serde::Serialize for AgentType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for AgentType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "cli" => Ok(AgentType::Cli),
            "vscode_copilot" => Ok(AgentType::VsCodeCopilot),
            "antigravity" => Ok(AgentType::Antigravity),
            "gemini_cli" => Ok(AgentType::GeminiCli),
            other => {
                if let Some(rest) = other.strip_prefix("custom:") {
                    Ok(AgentType::Custom(rest.to_string()))
                } else if other.starts_with("custom:") {
                    Ok(AgentType::Custom(other["custom:".len()..].to_string()))
                } else {
                    // Try to handle legacy format
                    Ok(AgentType::Custom(other.to_string()))
                }
            }
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::Cli => write!(f, "cli"),
            AgentType::VsCodeCopilot => write!(f, "vscode_copilot"),
            AgentType::Antigravity => write!(f, "antigravity"),
            AgentType::GeminiCli => write!(f, "gemini_cli"),
            AgentType::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// Agent connection status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Online,
    Idle,
    Busy,
    Offline,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Busy => write!(f, "busy"),
            AgentStatus::Offline => write!(f, "offline"),
        }
    }
}

impl Agent {
    /// Create a new agent.
    pub fn new(_id: &str, name: &str, agent_type: AgentType) -> Self {
        let now = FlexibleTimestamp::now();
        Self {
            id: None, // SurrealDB auto-generates the record ID
            name: name.to_string(),
            agent_type,
            status: AgentStatus::Online,
            connected_at: now.clone(),
            last_seen: now,
            command_count: 0,
            system_prompt: None,
            model_id: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Detect agent type from environment or name.
    pub fn detect_type(name: &str) -> AgentType {
        let lower = name.to_lowercase();
        if lower.contains("copilot") {
            AgentType::VsCodeCopilot
        } else if lower.contains("antigravity") {
            AgentType::Antigravity
        } else if lower.contains("gemini") {
            AgentType::GeminiCli
        } else if lower.contains("cli") {
            AgentType::Cli
        } else {
            AgentType::Custom(name.to_string())
        }
    }
}

/// Service for managing agent connections.
#[derive(Clone)]
pub struct AgentService {
    db: SurrealClient,
    timeline: TimelineService,
}

impl AgentService {
    /// Create a new AgentService.
    pub fn new(db: SurrealClient, timeline: TimelineService) -> Self {
        Self { db, timeline }
    }

    /// Register a new agent connection.
    pub async fn connect(&self, agent_id: &str, name: Option<&str>) -> Result<Agent> {
        let agent_name = name.unwrap_or(agent_id);
        let agent_type = Agent::detect_type(agent_name);

        info!("🤖 Agent connecting: {} ({})", agent_id, agent_type);

        // Check if agent already exists
        if let Some(mut existing) = self.get_agent(agent_id).await? {
            // Update existing agent
            existing.status = AgentStatus::Online;
            existing.last_seen = FlexibleTimestamp::now();
            let updated = self.db.update("agents", agent_id, &existing).await?;

            self.timeline
                .emit(agent_id, EventType::AgentConnected)
                .await?;

            return Ok(updated);
        }

        // Create new agent
        let agent = Agent::new(agent_id, agent_name, agent_type);
        let created: Agent = self.db.upsert("agents", agent_id, &agent).await?;

        self.timeline
            .emit(agent_id, EventType::AgentConnected)
            .await?;

        Ok(created)
    }

    /// Disconnect an agent.
    pub async fn disconnect(&self, agent_id: &str) -> Result<Option<Agent>> {
        info!("👋 Agent disconnecting: {}", agent_id);

        if let Some(mut agent) = self.get_agent(agent_id).await? {
            agent.status = AgentStatus::Offline;
            agent.last_seen = FlexibleTimestamp::now();

            let updated = self.db.update("agents", agent_id, &agent).await?;

            self.timeline
                .emit(agent_id, EventType::AgentDisconnected)
                .await?;

            return Ok(Some(updated));
        }

        Ok(None)
    }

    /// Get an agent by ID.
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<Agent>> {
        self.db.select_by_id("agents", agent_id).await
    }

    /// List all agents.
    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        self.db.select_all("agents").await
    }

    /// List only online agents.
    pub async fn list_online_agents(&self) -> Result<Vec<Agent>> {
        let query = "SELECT * FROM agents WHERE status = 'online' ORDER BY last_seen DESC";
        let mut response = self.db.query(query).await?;
        let agents: Vec<Agent> = response.take(0)?;
        Ok(agents)
    }

    /// Update agent's last seen timestamp.
    pub async fn heartbeat(&self, agent_id: &str) -> Result<()> {
        debug!("Heartbeat from agent: {}", agent_id);

        if let Some(mut agent) = self.get_agent(agent_id).await? {
            agent.last_seen = FlexibleTimestamp::now();
            agent.command_count += 1;
            self.db.update("agents", agent_id, &agent).await?;
        }

        Ok(())
    }

    /// Set agent status.
    pub async fn set_status(&self, agent_id: &str, status: AgentStatus) -> Result<Option<Agent>> {
        if let Some(mut agent) = self.get_agent(agent_id).await? {
            agent.status = status;
            agent.last_seen = FlexibleTimestamp::now();
            let updated = self.db.update("agents", agent_id, &agent).await?;
            return Ok(Some(updated));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::TimelineService;

    async fn setup() -> (AgentService, SurrealClient) {
        let db = SurrealClient::connect_mem().await.unwrap();
        let timeline = TimelineService::new(db.clone());
        let service = AgentService::new(db.clone(), timeline);
        (service, db)
    }

    #[tokio::test]
    async fn test_get_agent_not_found() -> Result<()> {
        let (service, _) = setup().await;
        let agent = service.get_agent("non-existent").await?;
        assert!(agent.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_connect_and_get_agent() -> Result<()> {
        let (service, _) = setup().await;
        let agent_id = "test-agent";

        let created = service.connect(agent_id, Some("Test Agent")).await?;
        assert_eq!(created.name, "Test Agent");
        assert_eq!(created.status, AgentStatus::Online);

        let retrieved = service.get_agent(agent_id).await?.unwrap();
        assert_eq!(retrieved.name, "Test Agent");
        assert_eq!(retrieved.status, AgentStatus::Online);
        Ok(())
    }

    #[tokio::test]
    async fn test_connect_existing_agent() -> Result<()> {
        let (service, _) = setup().await;
        let agent_id = "test-agent";

        // Connect first time
        let first = service.connect(agent_id, None).await?;
        let first_seen = first.last_seen;

        // Wait a bit to ensure timestamp would change
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Connect second time
        let second = service.connect(agent_id, None).await?;

        assert_eq!(second.status, AgentStatus::Online);
        assert!(second.last_seen > first_seen);
        Ok(())
    }

    #[tokio::test]
    async fn test_disconnect() -> Result<()> {
        let (service, _) = setup().await;
        let agent_id = "test-agent";

        service.connect(agent_id, None).await?;
        let disconnected = service.disconnect(agent_id).await?.unwrap();

        assert_eq!(disconnected.status, AgentStatus::Offline);

        let retrieved = service.get_agent(agent_id).await?.unwrap();
        assert_eq!(retrieved.status, AgentStatus::Offline);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_agents() -> Result<()> {
        let (service, _) = setup().await;

        service.connect("agent-1", None).await?;
        service.connect("agent-2", None).await?;

        let agents = service.list_agents().await?;
        assert_eq!(agents.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn test_list_online_agents() -> Result<()> {
        let (service, _) = setup().await;

        service.connect("agent-1", None).await?;
        service.connect("agent-2", None).await?;
        service.disconnect("agent-1").await?;

        let online = service.list_online_agents().await?;
        assert_eq!(online.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_heartbeat() -> Result<()> {
        let (service, _) = setup().await;
        let agent_id = "test-agent";

        let agent = service.connect(agent_id, None).await?;
        let initial_seen = agent.last_seen;
        assert_eq!(agent.command_count, 0);

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        service.heartbeat(agent_id).await?;

        let updated = service.get_agent(agent_id).await?.unwrap();
        assert_eq!(updated.command_count, 1);
        assert!(updated.last_seen > initial_seen);
        Ok(())
    }

    #[tokio::test]
    async fn test_set_status() -> Result<()> {
        let (service, _) = setup().await;
        let agent_id = "test-agent";

        service.connect(agent_id, None).await?;
        let updated = service
            .set_status(agent_id, AgentStatus::Busy)
            .await?
            .unwrap();

        assert_eq!(updated.status, AgentStatus::Busy);

        let retrieved = service.get_agent(agent_id).await?.unwrap();
        assert_eq!(retrieved.status, AgentStatus::Busy);
        Ok(())
    }
}
