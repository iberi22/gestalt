# RAG Operations Runbook

This document provides operational procedures for running Retrieval-Augmented Generation (RAG) in the Gestalt system.

## 1. Initial Indexing

Initial indexing involves populating the SurrealDB vector database with code fragments and memory fragments.

### Manual Indexing via SurrealDB SQL

Since the CLI `index-repo` command is currently a placeholder for a future autonomous pipeline, production indexing should be performed via direct SQL ingestion or programmatic scripts using the `VectorDb` adapter.

#### Ingesting Code Fragments

To manually ingest a code fragment into the `code` collection:

```sql
-- Connect to the 'neural' namespace and 'link' database
-- (or the namespace/database configured in your environment)
USE NS neural DB link;

CREATE code:unique_fragment_id CONTENT {
    "embedding": [0.1, 0.2, ...], -- 384-dimensional vector for local models
    "metadata": {
        "path": "src/main.rs",
        "repo_url": "https://github.com/user/repo",
        "content": "fn main() { println!(\"Hello\"); }",
        "language": "rust"
    }
};
```

#### Ingesting Memory Fragments

To manually ingest a memory fragment into the `memories` collection:

```sql
USE NS gestalt DB timeline;

CREATE memories:unique_memory_id CONTENT {
    "agent_id": "worker-1",
    "content": "User requested a project named 'Omega'.",
    "context": "conversation",
    "tags": ["omega", "project-creation"],
    "created_at": time::now(),
    "importance": 0.8
};
```

### Programmatic Indexing

Developers can use the `VectorDb` trait in `gestalt_core` to programmatically store embeddings:

```rust
// Using the SurrealDbAdapter
let adapter = SurrealDbAdapter::new().await?;
adapter.store_embedding(
    "code",
    "fragment_id",
    vec![0.1, 0.2, ...],
    serde_json::json!({ "path": "src/lib.rs" })
).await?;
```
## 2. Reindexing and Rollback

### Incremental Reindexing Strategy

The current implementation of `store_embedding` uses the `CREATE` operation, which will fail if a record with the same ID already exists. To perform a reindex or update of a repository, follow these steps:

1. **Identify the records to update:** Determine the `repo_url` or file paths that need reindexing.
2. **Clear existing embeddings:** Use a `DELETE` query to remove old data to avoid "duplicate ID" errors during the new indexing run.
   ```sql
   USE NS neural DB link;
   DELETE code WHERE metadata.repo_url = "https://github.com/user/repo";
   ```
3. **Execute fresh indexing:** Run your indexing script or manual ingestion for the updated content.

### Deduplication

Deduplication is currently handled at the ingestion layer by ensuring unique IDs for fragments (e.g., a hash of the file path and content). If duplicates are found in the database, they can be cleaned up via SQL:

```sql
-- Example: Remove older duplicates based on timestamp
-- This requires a 'timestamp' field in metadata
DELETE code WHERE metadata.repo_url = $url AND metadata.timestamp < $cutoff;
```

### Rollback Procedures

If an indexing run introduces corrupted data or incorrect embeddings, roll back the changes by deleting the affected records:

#### Rollback by Repository
```sql
USE NS neural DB link;
DELETE code WHERE metadata.repo_url = "https://github.com/user/repo";
```

#### Rollback by Agent Session (for Memories)
```sql
USE NS gestalt DB timeline;
DELETE memories WHERE agent_id = "worker-fail-123";
```

#### Total Reset (Warning: Permanent)
To completely clear all indexed code and start over:
```sql
REMOVE TABLE code;
-- The table will be re-defined on next system startup if using SurrealClient::init_schema
```
## 3. Observability

To ensure the health and performance of RAG operations, monitor the following metrics and logs.

### Metrics to Monitor

| Metric | Source | Description |
|--------|--------|-------------|
| **Vector Search Latency** | SurrealDB Logs | Time taken to execute `SELECT` with vector similarity. |
| **Embedding Generation Time** | App Logs | Duration of API calls to LLM providers (MiniMax/Gemini). |
| **Index Size** | SurrealDB | Number of records in `code` and `memories` collections. |
| **Cache Hit Rate** | App Logs | Frequency of retrieval from the `short_term` memory cache. |

### Logs and Tracing

Gestalt uses the `tracing` crate for structured logging. Ensure the log level is set appropriately in your environment:

- **INFO Level:** Provides high-level visibility into memory saving and searches.
  - Look for: `🧠 Memory saved [agent=...]`
  - Look for: `🔍 Searching memories for '...'`
- **DEBUG Level:** Provides detailed database interaction logs.
  - Look for: `Initializing database schema`
  - Look for SurrealDB raw query executions.

To enable detailed logs:
```bash
export RUST_LOG=gestalt_timeline=debug,gestalt_core=debug
gestalt nexus
```
## 4. Troubleshooting Matrix

| Symptom | Potential Cause | Resolution |
|---------|-----------------|------------|
| **Empty search results** | Index not populated or incorrect namespace. | Verify `code` and `memories` collections via SurrealDB CLI. Check `database.namespace` in `config/default.toml`. |
| **High latency in retrieval** | Unoptimized queries or large collection without indexes. | Ensure `DEFINE INDEX` was run for primary fields. Check SurrealDB query plans. |
| **LLM Provider Auth Error** | Missing or expired API Key. | Verify `MINIMAX_API_KEY` or `GEMINI_API_KEY` in environment variables or `config/`. |
| **Data loss on restart** | Using in-memory database (`mem://`). | Switch `database.url` to a persistent path (e.g., `file://gestalt.db`) or a remote server. |
| **Vector dimensionality mismatch** | Inconsistent embedding models used. | Ensure all ingested vectors match the dimensionality expected by the `SearchCodeTool` (default 384). |

## 5. RAG Deployment Release Checklist

- [ ] **Verify Database Schema:** Run a test query to ensure `code` and `memories` tables are defined.
- [ ] **Check LLM Quotas:** Ensure the configured provider (MiniMax/Gemini) has sufficient token quota.
- [ ] **Validate Config:** Ensure `config/default.toml` points to the correct SurrealDB instance.
- [ ] **Pre-index Core Repos:** Run the manual indexing script for essential project repositories.
- [ ] **Smoke Test Search:** Run `gestalt ai-chat "How does X work?"` to verify retrieval is working.
