//! Application context for the Gestalt MCP server.
//!
//! Holds shared state — configuration, registry instances, and service handles
//! that tool handlers access via `Arc<GestaltAppContext>`.

use gestalt_core::mcp::McpRegistry;

/// Lazy-initialised shared state for the MCP server.
///
/// Every field is optional — the server degrades gracefully when a
/// subsystem isn't available (e.g. no SurrealDB, no Tantivy index).
#[derive(Clone, Default)]
pub struct GestaltAppContext {
    /// Gestalt's MCP registry (tool registry + client).
    pub mcp_registry: Option<McpRegistry>,

    /// Server start timestamp (unix epoch seconds).
    pub started_at: u64,

    /// Unique run id for this server instance.
    pub instance_id: String,
}

impl GestaltAppContext {
    /// Create a fresh app context with a unique instance id.
    pub fn new() -> Self {
        Self {
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            instance_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        }
    }

    /// Return a summary JSON object describing the server status.
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "instance_id": self.instance_id,
            "started_at": self.started_at,
            "uptime_secs": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_sub(self.started_at),
            "has_registry": self.mcp_registry.is_some(),
        })
    }
}
