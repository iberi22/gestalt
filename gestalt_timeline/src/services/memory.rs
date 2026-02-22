//! Memory Service - Persistent memory for agents using SurrealDB.
//!
//! Provides short-term (session) and long-term (persistent) memory storage
//! with basic content-based search. Sprint 3 will add pgvector embeddings.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use chrono::Utc;

use crate::db::SurrealClient;

/// A fragment of memory stored by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFragment {
    /// Unique ID for this memory
    pub id: Option<surrealdb::sql::Thing>,
    /// The agent that created this memory
    pub agent_id: String,
    /// The content of the memory
    pub content: String,
    /// Context category (e.g. "conversation", "task_result", "observation")
    pub context: String,
    /// Searchable tags
    pub tags: Vec<String>,
    /// When this memory was created (UTC)
    pub created_at: chrono::DateTime<Utc>,
    /// Importance score 0.0 - 1.0 (used for compaction priority)
    pub importance: f32,
}

impl MemoryFragment {
    pub fn new(
        agent_id: impl Into<String>,
        content: impl Into<String>,
        context: impl Into<String>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: None,
            agent_id: agent_id.into(),
            content: content.into(),
            context: context.into(),
            tags,
            created_at: Utc::now(),
            importance: 0.5,
        }
    }

    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }
}

/// The MemoryService manages saving and retrieving agent memories.
///
/// # Design Notes
/// - Sprint 2 (current): SurrealDB-backed, keyword/tag search
/// - Sprint 3 (planned): Add pgvector embeddings for semantic similarity search
#[derive(Clone)]
pub struct MemoryService {
    db: SurrealClient,
    /// In-memory cache of the most recent N fragments per agent (short-term memory)
    short_term: Arc<RwLock<std::collections::VecDeque<MemoryFragment>>>,
    short_term_capacity: usize,
}

impl MemoryService {
    pub fn new(db: SurrealClient) -> Self {
        Self {
            db,
            short_term: Arc::new(RwLock::new(std::collections::VecDeque::new())),
            short_term_capacity: 50,
        }
    }

    /// Save a memory fragment to both short-term cache and SurrealDB.
    pub async fn save(
        &self,
        agent_id: &str,
        content: impl Into<String>,
        context: impl Into<String>,
        tags: Vec<String>,
    ) -> Result<MemoryFragment> {
        let fragment = MemoryFragment::new(agent_id, content, context, tags);

        // Persist to SurrealDB
        let saved = self.db
            .create("memories", &fragment)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to save memory: {}", e))?;

        // Add to short-term cache
        let mut stm = self.short_term.write().await;
        stm.push_back(saved.clone());
        if stm.len() > self.short_term_capacity {
            stm.pop_front();
        }

        info!("🧠 Memory saved [agent={}] context='{}' tags={:?}",
              saved.agent_id, saved.context, saved.tags);

        Ok(saved)
    }

    /// Search memories by tags and content substring.
    /// Returns the most recent matching fragments.
    pub async fn search(
        &self,
        query: &str,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryFragment>> {
        info!("🔍 Searching memories for '{}' (agent={:?}, limit={})", query, agent_id, limit);

        // First check short-term cache for quick hits
        let stm = self.short_term.read().await;
        let query_lower = query.to_lowercase();

        let cached: Vec<MemoryFragment> = stm.iter()
            .filter(|m| {
                let agent_match = agent_id.is_none_or(|a| m.agent_id == a);
                let content_match = m.content.to_lowercase().contains(&query_lower)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&query_lower));
                agent_match && content_match
            })
            .take(limit)
            .cloned()
            .collect();

        if !cached.is_empty() {
            return Ok(cached);
        }

        // Fall back to SurrealDB query
        let fragments: Vec<MemoryFragment> = self.db
            .query_with("SELECT * FROM memories WHERE string::contains(string::lowercase(content), $q) ORDER BY created_at DESC LIMIT $limit",
                serde_json::json!({ "q": query_lower, "limit": limit as i64 }))
            .await
            .map_err(|e| anyhow::anyhow!("Memory search query failed: {}", e))?;

        Ok(fragments)
    }

    /// Retrieve recent memories for an agent (most recent first).
    pub async fn recent(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<MemoryFragment>> {
        let stm = self.short_term.read().await;
        let cached: Vec<MemoryFragment> = stm.iter()
            .rev()
            .filter(|m| m.agent_id == agent_id)
            .take(limit)
            .cloned()
            .collect();

        if cached.len() >= limit {
            return Ok(cached);
        }

        // Query SurrealDB for more
        let fragments: Vec<MemoryFragment> = self.db
            .query_with("SELECT * FROM memories WHERE agent_id = $agent ORDER BY created_at DESC LIMIT $limit",
                serde_json::json!({ "agent": agent_id, "limit": limit as i64 }))
            .await
            .map_err(|e| anyhow::anyhow!("Recent memories query failed: {}", e))?;

        Ok(fragments)
    }

    /// Build a context string from recent memories to inject into LLM prompts.
    pub async fn build_context_string(
        &self,
        agent_id: &str,
        query: Option<&str>,
        max_chars: usize,
    ) -> String {
        let fragments = if let Some(q) = query {
            self.search(q, Some(agent_id), 10).await.unwrap_or_default()
        } else {
            self.recent(agent_id, 10).await.unwrap_or_default()
        };

        if fragments.is_empty() {
            return String::new();
        }

        let mut ctx = String::from("## Relevant Memories\n");
        for f in &fragments {
            let line = format!(
                "- [{}] ({}): {}\n",
                f.context,
                f.created_at.format("%Y-%m-%d %H:%M"),
                &f.content[..f.content.len().min(300)]
            );
            if ctx.len() + line.len() > max_chars {
                break;
            }
            ctx.push_str(&line);
        }
        ctx
    }
}
