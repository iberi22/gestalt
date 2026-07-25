//! Skills Index & Two-Layer Skills Cache
//!
//! Pattern 4: Skills Index — Err on Side of Loading
//!   Load skill even if task seems simple, because skill defines HOW IT SHOULD BE DONE.
//! Pattern 8: Two-Layer Skills Cache
//!   Layer 1: In-process LRU (keyed by tools/platform)
//!   Layer 2: Disk snapshot (.skills_prompt_snapshot.json)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// A loaded skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub tags: Vec<String>,
    pub platform: String,
}

impl Skill {
    pub fn new(name: &str, description: &str, content: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
            platform: "any".to_string(),
        }
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_platform(mut self, platform: &str) -> Self {
        self.platform = platform.to_string();
        self
    }
}

/// Layer 1: In-process LRU cache for skills
#[derive(Debug)]
pub struct SkillsLruCache {
    entries: RwLock<HashMap<String, (Skill, usize)>>, // key -> (skill, last_access_tick)
    capacity: usize,
    tick: RwLock<usize>,
}

impl SkillsLruCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            capacity,
            tick: RwLock::new(0),
        }
    }

    /// Get a skill from the LRU cache, updating its access time
    pub fn get(&self, key: &str) -> Option<Skill> {
        let current_tick = {
            let mut t = self.tick.write().unwrap();
            *t += 1;
            *t
        };

        // Clone skill out first, then update (avoids double-mutable-borrow)
        let skill_opt = {
            let mut entries = self.entries.write().unwrap();
            entries.get_mut(key).map(|(s, tick)| {
                *tick = current_tick;
                s.clone()
            })
        };

        skill_opt
    }

    /// Put a skill into the LRU cache
    pub fn put(&self, key: String, skill: Skill) {
        let mut entries = self.entries.write().unwrap();
        if entries.len() >= self.capacity && !entries.contains_key(&key) {
            // Evict least recently used (lowest tick)
            if let Some((lru_key, _)) = entries
                .iter()
                .min_by_key(|(_, (_, tick))| *tick)
                .map(|(k, v)| (k.clone(), v.1))
            {
                entries.remove(&lru_key);
            }
        }
        let current_tick = *self.tick.read().unwrap();
        entries.insert(key, (skill, current_tick));
    }

    /// Invalidate a cache entry
    pub fn invalidate(&self, key: &str) {
        let mut entries = self.entries.write().unwrap();
        entries.remove(key);
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        entries.clear();
    }

    /// Snapshot all cached skills (for Layer 2)
    pub fn snapshot(&self) -> Vec<Skill> {
        let entries = self.entries.read().unwrap();
        entries.values().map(|(s, _)| s.clone()).collect()
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.read().unwrap();
        entries.len()
    }
}

/// Layer 2: Disk snapshot for skills cache persistence
const SKILLS_SNAPSHOT_FILE: &str = ".skills_prompt_snapshot.json";

#[derive(Debug, Serialize, Deserialize)]
struct SkillsSnapshot {
    skills: Vec<Skill>,
    version: u32,
}

impl Default for SkillsSnapshot {
    fn default() -> Self {
        Self {
            skills: Vec::new(),
            version: 1,
        }
    }
}

/// Two-Layer Skills Cache
///
/// Layer 1: Fast in-process LRU (keyed by "platform:tag" or skill name)
/// Layer 2: Disk snapshot for warm restarts
#[derive(Debug)]
pub struct TwoLayerSkillsCache {
    l1: Arc<SkillsLruCache>,
    disk_path: PathBuf,
}

impl TwoLayerSkillsCache {
    /// Create a new cache, optionally loading from disk snapshot
    pub fn new(disk_path: PathBuf) -> Self {
        let l1 = Arc::new(SkillsLruCache::new(50)); // LRU capacity of 50

        // Try to load snapshot from disk
        let snapshot_path = disk_path.join(SKILLS_SNAPSHOT_FILE);
        if snapshot_path.exists() {
            if let Ok(contents) = fs::read_to_string(&snapshot_path) {
                if let Ok(snapshot) = serde_json::from_str::<SkillsSnapshot>(&contents) {
                    let count = snapshot.skills.len();
                    for skill in snapshot.skills {
                        let key = Self::make_key(&skill);
                        l1.put(key, skill);
                    }
                    tracing::info!(
                        "Loaded {} skills from disk snapshot",
                        count
                    );
                }
            }
        }

        Self { l1, disk_path }
    }

    fn make_key(skill: &Skill) -> String {
        format!("{}:{}", skill.platform, skill.name)
    }

    /// Get a skill from cache (L1 first, then L2 via disk is already loaded)
    pub fn get(&self, name: &str, platform: &str) -> Option<Skill> {
        let key = format!("{}:{}", platform, name);
        self.l1.get(&key).or_else(|| {
            // Try "any" platform as fallback
            let any_key = format!("any:{}", name);
            self.l1.get(&any_key)
        })
    }

    /// Put a skill into the cache and persist to disk
    pub fn put(&self, skill: Skill) {
        let key = Self::make_key(&skill);
        self.l1.put(key, skill.clone());
        self.persist_to_disk();
    }

    /// Find skills matching a tag (searches all cached)
    pub fn get_by_tag(&self, tag: &str) -> Vec<Skill> {
        let entries = self.l1.entries.read().unwrap();
        entries
            .values()
            .filter(|(s, _)| s.tags.contains(&tag.to_string()))
            .map(|(s, _)| s.clone())
            .collect()
    }

    /// Find skills matching a platform
    pub fn get_by_platform(&self, platform: &str) -> Vec<Skill> {
        let entries = self.l1.entries.read().unwrap();
        entries
            .values()
            .filter(|(s, _)| s.platform == platform || s.platform == "any")
            .map(|(s, _)| s.clone())
            .collect()
    }

    /// Persist L1 cache to disk snapshot (Layer 2)
    pub fn persist_to_disk(&self) {
        let snapshot = SkillsSnapshot {
            skills: self.l1.snapshot(),
            version: 1,
        };
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let path = self.disk_path.join(SKILLS_SNAPSHOT_FILE);
            if let Err(e) = fs::write(&path, json) {
                tracing::warn!("Failed to persist skills snapshot: {}", e);
            }
        }
    }

    /// Invalidate a cached skill
    pub fn invalidate(&self, name: &str, platform: &str) {
        let key = format!("{}:{}", platform, name);
        self.l1.invalidate(&key);
        self.persist_to_disk();
    }

    /// Build skills index for a given task/platform
    /// Pattern 4: Err on Side of Loading — load skill even if task seems simple
    pub fn build_prompt_for_task(&self, task: &str, platform: &str) -> String {
        // Always load matching skills — don't economize on context
        let matching = self.get_by_platform(platform);
        let tagged: Vec<Skill> = matching
            .into_iter()
            .filter(|s| {
                s.tags.iter().any(|tag| {
                    task.to_lowercase().contains(&tag.to_lowercase())
                })
            })
            .collect();

        if tagged.is_empty() {
            return String::new();
        }

        let sections: Vec<String> = tagged
            .iter()
            .map(|s| format!("## Skill: {}\n{}\n", s.name, s.content))
            .collect();

        format!("<skills-index>\n{}\n</skills-index>", sections.join("\n"))
    }

    pub fn len(&self) -> usize {
        self.l1.len()
    }
}

/// Shared cache type
pub type SharedSkillsCache = Arc<TwoLayerSkillsCache>;

/// Pattern 4: Build shared skills cache
pub fn build_skills_cache(project_root: PathBuf) -> SharedSkillsCache {
    Arc::new(TwoLayerSkillsCache::new(project_root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_basic() {
        let cache = SkillsLruCache::new(3);
        cache.put(
            "k1".to_string(),
            Skill::new("s1", "desc", "content"),
        );
        cache.put(
            "k2".to_string(),
            Skill::new("s2", "desc", "content"),
        );
        assert_eq!(cache.len(), 2);
        assert!(cache.get("k1").is_some());
        assert!(cache.get("unknown").is_none());
    }

    #[test]
    fn test_lru_cache_eviction() {
        let cache = SkillsLruCache::new(2);
        cache.put(
            "k1".to_string(),
            Skill::new("s1", "desc", "content"),
        );
        cache.put(
            "k2".to_string(),
            Skill::new("s2", "desc", "content"),
        );
        // Access k1 to make it recently used
        cache.get("k1");
        // Adding k3 should evict k2 (least recently used)
        cache.put(
            "k3".to_string(),
            Skill::new("s3", "desc", "content"),
        );
        assert!(cache.get("k1").is_some());
        assert!(cache.get("k3").is_some());
        // k2 should be evicted
        let entries = cache.entries.read().unwrap();
        assert!(!entries.contains_key("k2"));
    }

    #[test]
    fn test_skill_with_tags_and_platform() {
        let skill = Skill::new("rust_scan", "Scan Rust projects")
            .with_tags(&["rust", "scanning"])
            .with_platform("rust");
        assert_eq!(skill.platform, "rust");
        assert!(skill.tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_two_layer_cache_get_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = TwoLayerSkillsCache::new(temp_dir.path().to_path_buf());

        let skill = Skill::new("test_skill", "A test skill", "do something")
            .with_platform("rust");
        cache.put(skill);

        // Should find with exact platform match
        assert!(cache.get("test_skill", "rust").is_some());
        // Should not find with different platform
        assert!(cache.get("test_skill", "python").is_none());
    }
}
