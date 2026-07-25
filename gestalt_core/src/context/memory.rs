//! Memory Store — Declarative Facts
//!
//! Pattern 1: Memory = Declarative Facts
//! Separates declarative "what is true" (memory) from procedural "how to do" (skills).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// A declarative fact stored in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub key: String,
    pub value: String,
    pub source: String,
    pub tags: Vec<String>,
}

impl Fact {
    pub fn new(key: impl Into<String>, value: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            source: source.into(),
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// Declarative Memory Store
///
/// Stores "what is true" facts separately from skills (procedures).
/// Thread-safe for concurrent reads, write-locked for mutations.
#[derive(Debug, Default)]
pub struct MemoryStore {
    facts: RwLock<HashMap<String, Fact>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            facts: RwLock::new(HashMap::new()),
        }
    }

    /// Store a declarative fact
    pub fn store(&self, fact: Fact) {
        let mut facts = self.facts.write().unwrap();
        facts.insert(fact.key.clone(), fact);
    }

    /// Retrieve a fact by key
    pub fn get(&self, key: &str) -> Option<Fact> {
        let facts = self.facts.read().unwrap();
        facts.get(key).cloned()
    }

    /// Search facts by tag
    pub fn get_by_tag(&self, tag: &str) -> Vec<Fact> {
        let facts = self.facts.read().unwrap();
        facts
            .values()
            .filter(|f| f.tags.contains(&tag.to_string()))
            .cloned()
            .collect()
    }

    /// Get all facts as a Vec
    pub fn all(&self) -> Vec<Fact> {
        let facts = self.facts.read().unwrap();
        facts.values().cloned().collect()
    }

    /// Get all facts as a formatted string for prompt injection
    pub fn to_declarative_context(&self) -> String {
        let facts = self.facts.read().unwrap();
        if facts.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = facts
            .values()
            .map(|f| format!("  {}: {}", f.key, f.value))
            .collect();
        format!("<memory-context>\n{}\n</memory-context>", lines.join("\n"))
    }

    /// Remove a fact
    pub fn remove(&self, key: &str) -> Option<Fact> {
        let mut facts = self.facts.write().unwrap();
        facts.remove(key)
    }

    /// Count stored facts
    pub fn len(&self) -> usize {
        let facts = self.facts.read().unwrap();
        facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Self-Improvement Loop — Pattern 6
///
/// Tracks task outcomes and periodically extracts patterns
/// to generate new skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutcome {
    pub task: String,
    pub tool_used: String,
    pub success: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub pattern_key: Option<String>,
}

#[derive(Debug, Default)]
pub struct SelfImprover {
    pub outcomes: Vec<TaskOutcome>,
    pub pattern_count: usize,
}

impl SelfImprover {
    pub fn record(&mut self, outcome: TaskOutcome) {
        self.outcomes.push(outcome);
        // Keep last 100 outcomes for pattern analysis
        if self.outcomes.len() > 100 {
            self.outcomes.remove(0);
        }
    }

    /// Extract patterns from recorded outcomes
    /// Returns (tool_name, frequency) pairs for tools used successfully
    pub fn extract_patterns(&self) -> HashMap<String, usize> {
        let mut patterns: HashMap<String, usize> = HashMap::new();
        for outcome in &self.outcomes {
            if outcome.success {
                *patterns.entry(outcome.tool_used.clone()).or_insert(0) += 1;
            }
        }
        patterns
    }

    /// Check if a task should trigger skill generation
    /// (repeated 3+ times with same tool = candidate for skill)
    pub fn skill_candidates(&self) -> Vec<(String, usize)> {
        self.extract_patterns()
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_basic() {
        let store = MemoryStore::new();
        store.store(Fact::new("user.name", "Sebas", "user_profile"));
        store.store(Fact::new("project.type", "Rust", "context_scanner"));

        assert_eq!(store.len(), 2);
        assert_eq!(store.get("user.name").unwrap().value, "Sebas");
    }

    #[test]
    fn test_memory_store_with_tags() {
        let store = MemoryStore::new();
        store.store(
            Fact::new("env.cpu_count", "8", "system")
                .with_tags(&["system", "performance"]),
        );

        let by_tag = store.get_by_tag("system");
        assert_eq!(by_tag.len(), 1);
    }

    #[test]
    fn test_declarative_context() {
        let store = MemoryStore::new();
        store.store(Fact::new("user.name", "Sebas", "profile"));
        let ctx = store.to_declarative_context();
        assert!(ctx.contains("<memory-context>"));
        assert!(ctx.contains("user.name"));
    }

    #[test]
    fn test_self_improver_patterns() {
        let mut improver = SelfImprover::default();
        improver.record(TaskOutcome {
            task: "scan rust project".to_string(),
            tool_used: "scan_workspace".to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
            pattern_key: None,
        });
        improver.record(TaskOutcome {
            task: "scan py project".to_string(),
            tool_used: "scan_workspace".to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
            pattern_key: None,
        });
        improver.record(TaskOutcome {
            task: "scan node project".to_string(),
            tool_used: "scan_workspace".to_string(),
            success: true,
            timestamp: chrono::Utc::now(),
            pattern_key: None,
        });

        let candidates = improver.skill_candidates();
        assert!(candidates.iter().any(|(t, c)| t == "scan_workspace" && *c == 3));
    }
}
