use gestalt_mcp::tools;
use mcp_protocol_sdk::protocol::types::ContentBlock;
use mcp_protocol_sdk::server::McpServer;
use serde_json::json;
use tempfile::tempdir;

async fn test_server() -> McpServer {
    let server = McpServer::new("test-search-server".into(), "1.0.0".into());
    tools::register_standard_tools(&server)
        .await
        .expect("standard tools should register");
    server
}

#[tokio::test]
async fn test_search_index_and_stats() {
    let server = test_server().await;
    let dir = tempdir().expect("failed to create temp dir");
    let index_path = dir.path().to_string_lossy().to_string();

    // 1. Get stats initially (should be 0)
    let stats_args = Some(
        [("index_path".to_string(), json!(index_path))]
            .into_iter()
            .collect(),
    );
    let stats_result = server
        .call_tool("search_stats", stats_args.clone())
        .await
        .expect("search_stats should succeed");

    assert!(!stats_result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &stats_result.content[0] {
        let stats: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            stats.get("document_count").and_then(|v| v.as_u64()),
            Some(0)
        );
    } else {
        panic!("Expected text block");
    }

    // 2. Index a document
    let index_args = Some(
        [
            ("index_path".to_string(), json!(index_path)),
            ("doc_id".to_string(), json!("doc_abc")),
            ("doc_path".to_string(), json!("src/main.rs")),
            (
                "content".to_string(),
                json!("The quick brown fox jumps over the lazy dog"),
            ),
            ("kind".to_string(), json!("code")),
        ]
        .into_iter()
        .collect(),
    );

    let index_result = server
        .call_tool("search_index", index_args)
        .await
        .expect("search_index should succeed");

    assert!(!index_result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &index_result.content[0] {
        assert!(text.contains("indexed successfully"));
    } else {
        panic!("Expected text block");
    }

    // 3. Verify stats updated to 1
    let stats_result2 = server
        .call_tool("search_stats", stats_args)
        .await
        .expect("search_stats should succeed");

    assert!(!stats_result2.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &stats_result2.content[0] {
        let stats: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            stats.get("document_count").and_then(|v| v.as_u64()),
            Some(1)
        );
    } else {
        panic!("Expected text block");
    }
}

#[tokio::test]
async fn test_bm25_search_tool() {
    let server = test_server().await;
    let dir = tempdir().expect("failed to create temp dir");
    let index_path = dir.path().to_string_lossy().to_string();

    // Index a document
    let index_args = Some(
        [
            ("index_path".to_string(), json!(index_path)),
            ("doc_id".to_string(), json!("doc_rust")),
            ("doc_path".to_string(), json!("lib.rs")),
            (
                "content".to_string(),
                json!("Rust is a beautiful multi-paradigm systems programming language."),
            ),
            ("kind".to_string(), json!("code")),
        ]
        .into_iter()
        .collect(),
    );

    let index_result = server
        .call_tool("search_index", index_args)
        .await
        .expect("search_index should succeed");
    assert!(!index_result.is_error.unwrap_or(false));

    // Perform BM25 search
    let search_args = Some(
        [
            ("index_path".to_string(), json!(index_path)),
            ("query".to_string(), json!("Rust programming")),
            ("limit".to_string(), json!(5)),
        ]
        .into_iter()
        .collect(),
    );

    let search_result = server
        .call_tool("bm25_search", search_args)
        .await
        .expect("bm25_search should succeed");

    assert!(!search_result.is_error.unwrap_or(false));
    if let ContentBlock::Text { text, .. } = &search_result.content[0] {
        let results: serde_json::Value = serde_json::from_str(text).unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("id").and_then(|v| v.as_str()), Some("doc_rust"));
        assert_eq!(arr[0].get("path").and_then(|v| v.as_str()), Some("lib.rs"));
        assert!(arr[0].get("score").and_then(|v| v.as_f64()).unwrap() > 0.0);
    } else {
        panic!("Expected text block");
    }
}
