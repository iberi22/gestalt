use gestalt_core::adapters::persistence::surreal_db::SurrealDbAdapter;
use gestalt_core::ports::outbound::repo_manager::VectorDb;
use serde_json::json;

#[tokio::test]
async fn test_vector_search_integration() {
    let adapter = SurrealDbAdapter::new().await.unwrap();

    // Store some items
    adapter.store_embedding("code", "1", vec![1.0, 0.0, 0.0], json!({"content": "item 1"})).await.unwrap();
    adapter.store_embedding("code", "2", vec![0.0, 1.0, 0.0], json!({"content": "item 2"})).await.unwrap();

    // Search with similarity to item 1
    let results = adapter.search_similar("code", vec![0.9, 0.1, 0.0], 10).await.unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].metadata["content"], "item 1");
    assert!(results[0].score > 0.8);
}

#[tokio::test]
async fn test_lexical_fallback() {
    let adapter = SurrealDbAdapter::new().await.unwrap();

    // Store something
    adapter.store_embedding("code", "1", vec![1.0, 0.0, 0.0], json!({"content": "item 1"})).await.unwrap();

    // Search with a vector that might not match well but should return results via fallback
    // (In our current impl, search_similar always returns something if table is not empty)
    let results = adapter.search_similar("code", vec![0.0, 0.0, 1.0], 10).await.unwrap();
    assert!(!results.is_empty());
}
