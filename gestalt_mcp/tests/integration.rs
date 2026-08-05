//! Integration tests for `gestalt_mcp` standalone server.
//!
//! ## Test strategy
//!
//! Because MCP is a client-server protocol, these tests create an
//! `McpServer` in-process, register tools the same way the binary does,
//! and exercise the `list_tools` and `call_tool` APIs directly.
//!
//! For end-to-end HTTP/stdio transport tests, use the helper scripts in
//! `tests/` or run the binary and connect with `mcp-cli`.

use std::collections::HashMap;
use std::sync::Arc;

use gestalt_mcp::app_context::GestaltAppContext;
use gestalt_mcp::gestalt_tools;
use gestalt_mcp::tools;
use mcp_protocol_sdk::protocol::types::ContentBlock;
use mcp_protocol_sdk::server::McpServer;
use serde_json::json;

/// Helper: create a fully-registered McpServer for test use.
async fn test_server() -> McpServer {
    let ctx = Arc::new(GestaltAppContext::new());
    let server = McpServer::new("test-server".into(), "1.0.0".into());

    tools::register_standard_tools(&server)
        .await
        .expect("standard tools should register");
    gestalt_tools::register_gestalt_tools(&server, ctx)
        .await
        .expect("gestalt tools should register");

    server
}

#[tokio::test]
async fn test_tools_list() {
    let server = test_server().await;
    let tools = server
        .list_tools()
        .await
        .expect("list_tools should succeed");

    // We should have all 12 standard + 6 gestalt = 18 tools
    assert!(
        tools.len() >= 18,
        "Expected >=18 tools, got {}",
        tools.len()
    );

    // Check a few known tools exist
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"echo"), "echo tool should be registered");
    assert!(
        names.contains(&"server_status"),
        "server_status tool should be registered"
    );
    assert!(
        names.contains(&"gestalt_belief_query"),
        "gestalt_belief_query tool should be registered"
    );
    assert!(
        names.contains(&"gestalt_project_analyze"),
        "gestalt_project_analyze tool should be registered"
    );
}

#[tokio::test]
async fn test_echo_tool() {
    let server = test_server().await;

    let args = Some(
        [("message".to_string(), json!("Hello MCP!"))]
            .into_iter()
            .collect(),
    );

    let result = server
        .call_tool("echo", args)
        .await
        .expect("echo should succeed");

    assert!(
        !result.is_error.unwrap_or(false),
        "echo should not return error"
    );
    assert!(!result.content.is_empty(), "echo should return content");

    // Check the text content
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        assert_eq!(text, "Hello MCP!");
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_system_info_tool() {
    let server = test_server().await;

    let result = server
        .call_tool("system_info", None)
        .await
        .expect("system_info should succeed");

    assert!(!result.is_error.unwrap_or(false));
    assert!(!result.content.is_empty());

    // Should return JSON with os/arch fields
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("system_info should return valid JSON");
        assert!(parsed.get("os").is_some(), "JSON should contain 'os' field");
        assert!(
            parsed.get("arch").is_some(),
            "JSON should contain 'arch' field"
        );
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_nonexistent_tool() {
    let server = test_server().await;

    let result = server.call_tool("nonexistent_tool_xyz", None).await;

    assert!(
        result.is_err(),
        "Calling nonexistent tool should return error"
    );
}

#[tokio::test]
async fn test_git_status_tool_no_repo() {
    let server = test_server().await;

    // Pass a non-existent path to trigger an error gracefully
    let args = Some(
        [("path".to_string(), json!("/nonexistent_path_xyz_12345"))]
            .into_iter()
            .collect(),
    );

    let result = server
        .call_tool("git_status", args)
        .await
        .expect("git_status should not panic");
    // Should still return a result (may be an error status, but not a panic)
    assert!(!result.content.is_empty());
}

#[tokio::test]
async fn test_gestalt_project_analyze() {
    let server = test_server().await;

    // Analyze the gestalt_mcp directory itself
    let args = Some(
        [("path".to_string(), json!("."))] // relative to workspace root
            .into_iter()
            .collect(),
    );

    let result = server
        .call_tool("gestalt_project_analyze", args)
        .await
        .expect("gestalt_project_analyze should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        // Should have analyzed the current directory
        assert!(
            parsed.get("total_files").is_some(),
            "JSON should contain total_files"
        );
    }
}

#[tokio::test]
async fn test_gestalt_registry_info() {
    let server = test_server().await;

    let result = server
        .call_tool("gestalt_registry_info", None)
        .await
        .expect("gestalt_registry_info should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        assert!(
            parsed.get("instance_id").is_some(),
            "JSON should contain instance_id"
        );
        assert!(
            parsed.get("registry_available").is_some(),
            "JSON should contain registry_available"
        );
    }
}

// ---------------------------------------------------------------------------
// NEW INTEGRATION TESTS ADDED BELOW
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_gestalt_search_valid() {
    let server = test_server().await;

    let args = Some([("query".to_string(), json!("rust"))].into_iter().collect());

    let result = server
        .call_tool("gestalt_search", args)
        .await
        .expect("gestalt_search should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        assert_eq!(parsed.get("query").and_then(|v| v.as_str()), Some("rust"));
        assert_eq!(
            parsed.get("supported").and_then(|v| v.as_bool()),
            Some(false)
        );
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_gestalt_search_missing_query() {
    let server = test_server().await;

    let args: Option<HashMap<String, serde_json::Value>> = Some(HashMap::new());

    let result = server
        .call_tool("gestalt_search", args)
        .await
        .expect("call_tool should return Ok but with an internal error state");

    assert!(
        result.is_error.unwrap_or(false),
        "Expected error when query is missing"
    );
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        assert!(
            text.contains("Error: query is required"),
            "Unexpected error response: {}",
            text
        );
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_server_status_tool() {
    let server = test_server().await;

    let result = server
        .call_tool("server_status", None)
        .await
        .expect("server_status should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        assert!(parsed.get("instance_id").is_some());
        assert!(parsed.get("started_at").is_some());
        assert!(parsed.get("uptime_secs").is_some());
        assert!(parsed.get("has_registry").is_some());
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_gestalt_belief_query_tool() {
    let server = test_server().await;

    let args = Some(
        [
            ("subject".to_string(), json!("Agent007")),
            ("predicate".to_string(), json!("owns")),
        ]
        .into_iter()
        .collect(),
    );

    let result = server
        .call_tool("gestalt_belief_query", args)
        .await
        .expect("gestalt_belief_query should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        assert_eq!(
            parsed.get("subject").and_then(|v| v.as_str()),
            Some("Agent007")
        );
        assert_eq!(
            parsed.get("predicate").and_then(|v| v.as_str()),
            Some("owns")
        );
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_tool_schema_validation() {
    let server = test_server().await;
    let tools = server
        .list_tools()
        .await
        .expect("list_tools should succeed");

    // Find gestalt_search tool and validate schema parameters and properties
    let search_tool = tools
        .iter()
        .find(|t| t.name == "gestalt_search")
        .expect("gestalt_search tool should be registered");

    assert_eq!(search_tool.input_schema.schema_type, "object");

    let properties = search_tool
        .input_schema
        .properties
        .as_ref()
        .expect("properties field should be present");

    assert!(
        properties.contains_key("query"),
        "Properties must contain query"
    );
    assert!(
        properties.contains_key("limit"),
        "Properties must contain limit"
    );

    let required = search_tool
        .input_schema
        .required
        .as_ref()
        .expect("required field should be present");

    assert!(
        required.contains(&"query".to_string()),
        "query should be required"
    );
}

#[tokio::test]
async fn test_app_context_status() {
    let ctx = GestaltAppContext::new();
    let status = ctx.status();

    assert!(
        status.get("instance_id").is_some(),
        "instance_id should be present"
    );
    assert!(
        status.get("started_at").is_some(),
        "started_at should be present"
    );
    assert!(
        status.get("uptime_secs").is_some(),
        "uptime_secs should be present"
    );
    assert_eq!(
        status.get("has_registry").and_then(|v| v.as_bool()),
        Some(false),
        "has_registry should be false by default"
    );
}

#[tokio::test]
async fn test_gestalt_agent_run_valid() {
    let server = test_server().await;

    let args = Some(
        [
            ("question".to_string(), json!("How do I use gestalt?")),
            ("repo".to_string(), json!(".")),
        ]
        .into_iter()
        .collect(),
    );

    let result = server
        .call_tool("gestalt_agent_run", args)
        .await
        .expect("gestalt_agent_run should succeed");

    assert!(!result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        let parsed: serde_json::Value =
            serde_json::from_str(text).expect("should return valid JSON");
        assert_eq!(
            parsed.get("question").and_then(|v| v.as_str()),
            Some("How do I use gestalt?")
        );
        assert_eq!(
            parsed.get("status").and_then(|v| v.as_str()),
            Some("not_implemented")
        );
    } else {
        panic!("Expected Text content block");
    }
}

#[tokio::test]
async fn test_gestalt_agent_run_missing_question() {
    let server = test_server().await;

    let args: Option<HashMap<String, serde_json::Value>> = Some(HashMap::new());

    let result = server
        .call_tool("gestalt_agent_run", args)
        .await
        .expect("gestalt_agent_run should return Ok with error state");

    assert!(result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &result.content[0] {
        assert!(text.contains("Error: question is required"));
    } else {
        panic!("Expected Text content block");
    }
}
