//! Gestalt-specific MCP tool handlers.
//!
//! These tools expose Gestalt core capabilities (belief graph, search, agent,
//! project analysis) through the MCP interface.  Each handler holds an
//! `Arc<GestaltAppContext>` so it can access shared server state.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use mcp_protocol_sdk::core::error::McpResult;
use mcp_protocol_sdk::core::tool::ToolHandler;
use mcp_protocol_sdk::protocol::types::{ContentBlock, ToolResult};
use serde_json::{json, Value};

use crate::app_context::GestaltAppContext;

// ---------------------------------------------------------------------------
// Helper: build ToolResult responses
// ---------------------------------------------------------------------------

fn ok_result(text: String) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::text(text)],
        is_error: Some(false),
        structured_content: None,
        meta: None,
    }
}

fn err_result(text: String) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::text(text)],
        is_error: Some(true),
        structured_content: None,
        meta: None,
    }
}

// ---------------------------------------------------------------------------
// server_status — report server health + metadata
// ---------------------------------------------------------------------------

pub struct ServerStatusHandler {
    ctx: Arc<GestaltAppContext>,
}

impl ServerStatusHandler {
    pub fn new(ctx: Arc<GestaltAppContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl ToolHandler for ServerStatusHandler {
    async fn call(&self, _arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let status = self.ctx.status();
        Ok(ok_result(serde_json::to_string_pretty(&status).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// gestalt_belief_query — query the belief graph
// ---------------------------------------------------------------------------

pub struct BeliefQueryHandler {
    ctx: Arc<GestaltAppContext>,
}

impl BeliefQueryHandler {
    pub fn new(ctx: Arc<GestaltAppContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl ToolHandler for BeliefQueryHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let _subject = arguments.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let _predicate = arguments.get("predicate").and_then(|v| v.as_str()).unwrap_or("");

        // In a full implementation, this would query the actual BeliefGraph
        // stored in `self.ctx`.  For now, we return a stub response that
        // demonstrates the integration surface.

        let info = json!({
            "note": "Belief graph query endpoint",
            "subject": _subject,
            "predicate": _predicate,
            "supported": false,
            "believe_registry": self.ctx.mcp_registry.is_some(),
        });

        Ok(ok_result(serde_json::to_string_pretty(&info).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// gestalt_search — full-text search via gestalt-search / Tantivy
// ---------------------------------------------------------------------------

pub struct SearchHandler {
    #[allow(dead_code)]
    ctx: Arc<GestaltAppContext>,
}

impl SearchHandler {
    pub fn new(ctx: Arc<GestaltAppContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl ToolHandler for SearchHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let query = arguments.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let _limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);

        if query.is_empty() {
            return Ok(err_result("Error: query is required".to_string()));
        }

        // Stub — in the full implementation this creates a TantivySearchEngine
        // and calls engine.search(query, limit).
        let result = json!({
            "query": query,
            "limit": _limit,
            "supported": false,
            "note": "Search requires Tantivy index path.  Configure with --search-index <path>",
        });

        Ok(ok_result(serde_json::to_string_pretty(&result).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// gestalt_project_analyze — deep project analysis using gestalt_core's scanner
// ---------------------------------------------------------------------------

pub struct ProjectAnalyzeHandler;

#[async_trait]
impl ToolHandler for ProjectAnalyzeHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let path = std::path::Path::new(path);

        if !path.exists() {
            return Ok(err_result(format!("Error: path does not exist: {}", path.display())));
        }

        // Use gestalt_core's ProjectContext types via the scanner
        let mut total_files = 0u64;
        let mut total_dirs = 0u64;
        let mut languages: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();

        if path.is_dir() {
            for entry in ignore::WalkBuilder::new(path)
                .standard_filters(true)
                .max_depth(Some(10))
                .build()
            {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    total_dirs += 1;
                    continue;
                }
                total_files += 1;
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    let lang = match ext {
                        "rs" => "Rust",
                        "ts" | "tsx" => "TypeScript",
                        "js" | "jsx" => "JavaScript",
                        "py" => "Python",
                        "go" => "Go",
                        "dart" => "Dart",
                        "toml" | "yaml" | "yml" | "json" | "md" => "Config/Docs",
                        _ => "Other",
                    };
                    *languages.entry(lang.to_string()).or_insert(0) += 1;
                }
            }
        } else {
            return Ok(err_result(format!("Error: not a directory: {}", path.display())));
        }

        let analysis = json!({
            "path": path.display().to_string(),
            "total_files": total_files,
            "total_dirs": total_dirs,
            "languages": languages,
        });

        Ok(ok_result(serde_json::to_string_pretty(&analysis).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// gestalt_registry_info — show MCP registry status
// ---------------------------------------------------------------------------

pub struct RegistryInfoHandler {
    ctx: Arc<GestaltAppContext>,
}

impl RegistryInfoHandler {
    pub fn new(ctx: Arc<GestaltAppContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl ToolHandler for RegistryInfoHandler {
    async fn call(&self, _arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let info = json!({
            "registry_available": self.ctx.mcp_registry.is_some(),
            "instance_id": self.ctx.instance_id,
            "uptime_secs": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .saturating_sub(self.ctx.started_at),
        });
        Ok(ok_result(serde_json::to_string_pretty(&info).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// gestalt_agent_run — stub for agent execution
// ---------------------------------------------------------------------------

pub struct AgentRunHandler;

#[async_trait]
impl ToolHandler for AgentRunHandler {
    async fn call(&self, arguments: HashMap<String, Value>) -> McpResult<ToolResult> {
        let _question = arguments.get("question").and_then(|v| v.as_str()).unwrap_or("");
        let _repo = arguments.get("repo").and_then(|v| v.as_str()).unwrap_or(".");

        if _question.is_empty() {
            return Ok(err_result("Error: question is required".to_string()));
        }

        // Stub — in the full implementation this would create a GestaltAgent
        // and run it with the given question.
        let result = json!({
            "note": "Agent execution stub.  Configure LLM provider + vector DB for full operation.",
            "question": _question,
            "repo": _repo,
            "status": "not_implemented",
        });

        Ok(ok_result(serde_json::to_string_pretty(&result).unwrap_or_default()))
    }
}

// ---------------------------------------------------------------------------
// Registration helper — add all stateful handlers to a list
// ---------------------------------------------------------------------------

/// Register Gestalt-specific tools on an MCP server.
pub async fn register_gestalt_tools(
    server: &mcp_protocol_sdk::server::McpServer,
    ctx: Arc<GestaltAppContext>,
) -> anyhow::Result<()> {
    // Stateful handlers
    let status = ServerStatusHandler::new(ctx.clone());
    server
        .add_tool(
            "server_status".to_string(),
            Some("Report Gestalt MCP server health and metadata".to_string()),
            json!({ "type": "object", "properties": {} }),
            status,
        )
        .await?;

    let belief = BeliefQueryHandler::new(ctx.clone());
    server
        .add_tool(
            "gestalt_belief_query".to_string(),
            Some("Query the Gestalt belief graph for known facts".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "subject": { "type": "string", "description": "Subject to query (optional)" },
                    "predicate": { "type": "string", "description": "Predicate to filter by (optional)" },
                }
            }),
            belief,
        )
        .await?;

    let search = SearchHandler::new(ctx.clone());
    server
        .add_tool(
            "gestalt_search".to_string(),
            Some("Full-text search across indexed code / documents (BM25 / Tantivy)".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results (default 10)" },
                },
                "required": ["query"]
            }),
            search,
        )
        .await?;

    // Stateless handlers
    server
        .add_tool(
            "gestalt_project_analyze".to_string(),
            Some("Analyze a project directory structure and language composition".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to project directory" },
                },
                "required": ["path"]
            }),
            ProjectAnalyzeHandler,
        )
        .await?;

    let reg_info = RegistryInfoHandler::new(ctx.clone());
    server
        .add_tool(
            "gestalt_registry_info".to_string(),
            Some("Show MCP registry status and server instance info".to_string()),
            json!({ "type": "object", "properties": {} }),
            reg_info,
        )
        .await?;

    server
        .add_tool(
            "gestalt_agent_run".to_string(),
            Some("Run a Gestalt agent against a repository with a question (stub)".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "Question or task for the agent" },
                    "repo": { "type": "string", "description": "Path to git repository" },
                },
                "required": ["question"]
            }),
            AgentRunHandler,
        )
        .await?;

    Ok(())
}
