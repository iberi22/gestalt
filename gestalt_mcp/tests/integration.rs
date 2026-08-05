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
    let tools = server.list_tools().await.expect("list_tools should succeed");

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

    let result = server
        .call_tool("nonexistent_tool_xyz", None)
        .await;

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
