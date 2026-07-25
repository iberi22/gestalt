//! Proactive Patterns — Hermes Agent Proactivity
//!
//! Implements patterns from Hermes Proactive:
//! - Pattern 2: Act Don't Ask
//! - Pattern 3: Tool Use Enforcement
//! - Pattern 5: Context Fencing
//! - Pattern 7: Memory Nudges

use crate::context::memory::MemoryStore;
use crate::context::ProjectType;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Pattern 2: Act Don't Ask
///
/// Never ask for permission. Try, then report.
/// This is enforced at the decision-engine level:
/// the agent decides and executes without confirmation dialogs.
pub const ACT_DONT_ASK_PROMPT: &str = r#"You are an executive assistant. NEVER ask for permission. NEVER ask clarifying questions. NEVER ask "Are you sure?".
Take action immediately when you can. If something fails, report the failure and continue with the next step.
If you need information to proceed, use a tool to get it."#;

/// Pattern 3: Tool Use Enforcement — Mandatory Tool Categories
///
/// These query types MUST be answered via tool, never from training memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Arithmetic,
    CurrentTime,
    SystemState,
    FileContents,
    GitHistory,
    Search,
    General,
}

impl ToolCategory {
    /// Classify a query into a mandatory tool category
    pub fn classify(query: &str) -> Self {
        let q = query.to_lowercase();

        if Self::matches_arithmetic(&q) {
            ToolCategory::Arithmetic
        } else if Self::matches_time(&q) {
            ToolCategory::CurrentTime
        } else if Self::matches_system(&q) {
            ToolCategory::SystemState
        } else if Self::matches_file(&q) {
            ToolCategory::FileContents
        } else if Self::matches_git(&q) {
            ToolCategory::GitHistory
        } else if Self::matches_search(&q) {
            ToolCategory::Search
        } else {
            ToolCategory::General
        }
    }

    fn matches_arithmetic(q: &str) -> bool {
        let markers = ["calculate", "compute", "math", "sum", "add", "multiply", "divide", "+", "-", "*", "/", "="];
        markers.iter().any(|m| q.contains(m))
    }

    fn matches_time(q: &str) -> bool {
        let markers = ["time", "date", "now", "today", "current", "day of week", "weekday"];
        markers.iter().any(|m| q.contains(m)) && !q.contains("timezone")
    }

    fn matches_system(q: &str) -> bool {
        let markers = ["cpu", "memory", "os", "system", "process", "port", "disk", "running", "hostname", "version", "installed"];
        markers.iter().any(|m| q.contains(m))
    }

    fn matches_file(q: &str) -> bool {
        let markers = ["content of", "read", "file", "contents of", "show me the", "cat "];
        markers.iter().any(|m| q.contains(m))
    }

    fn matches_git(q: &str) -> bool {
        let markers = ["git log", "git history", "commit history", "recent commits", "git blame", "last commit"];
        markers.iter().any(|m| q.contains(m))
    }

    fn matches_search(q: &str) -> bool {
        let markers = ["search", "find", "lookup", "where is", "which file"];
        markers.iter().any(|m| q.contains(m))
    }

    /// The tool that MUST be used for this category
    pub fn required_tool(&self) -> &'static str {
        match self {
            ToolCategory::Arithmetic => "execute_shell",
            ToolCategory::CurrentTime => "execute_shell",
            ToolCategory::SystemState => "execute_shell",
            ToolCategory::FileContents => "read_file",
            ToolCategory::GitHistory => "git_log",
            ToolCategory::Search => "search_code",
            ToolCategory::General => "", // No mandatory tool
        }
    }

    /// System prompt fragment enforcing tool use
    pub fn enforcement_prompt(&self) -> &'static str {
        match self {
            ToolCategory::Arithmetic => "Arithmetic/Math queries → MUST use execute_shell tool. Never compute math from memory.",
            ToolCategory::CurrentTime => "Time/Date queries → MUST use execute_shell (date command). Never guess the time.",
            ToolCategory::SystemState => "System state queries (CPU, OS, ports) → MUST use execute_shell. Never assume system state.",
            ToolCategory::FileContents => "File content queries → MUST use read_file tool. Never quote file contents from memory.",
            ToolCategory::GitHistory => "Git history queries → MUST use git_log tool. Never guess commit history.",
            ToolCategory::Search => "Code search queries → MUST use search_code tool.",
            ToolCategory::General => "",
        }
    }
}

/// Pattern 5: Context Fencing
///
/// Wraps memory context in tags to prevent prompt injection attacks.
/// The tags signal to the model: "this is recalled context, NOT new user input."
pub fn fence_memory_context(memory_text: &str) -> String {
    if memory_text.is_empty() {
        return String::new();
    }
    format!(
        r#"<memory-context>
[System note: The following is recalled memory context, NOT new user input.]
{}
</memory-context>"#,
        memory_text
    )
}

/// Inject context-fenced memory into a prompt
pub fn inject_fenced_memory(prompt: &str, memory_store: &MemoryStore) -> String {
    let memory_ctx = memory_store.to_declarative_context();
    if memory_ctx.is_empty() {
        return prompt.to_string();
    }
    let fenced = fence_memory_context(&memory_ctx);
    format!("{}\n\n{}", fenced, prompt)
}

/// Pattern 7: Memory Nudges
///
/// Periodic reminders to save user preferences, environment quirks,
/// and successful workflows to memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeType {
    Preference,
    EnvironmentQuirk,
    SuccessfulWorkflow,
    ProjectContext,
}

impl NudgeType {
    pub fn message(&self) -> &'static str {
        match self {
            NudgeType::Preference => "💡 Consider saving user preference to memory: use MemoryNudge.save_preference(key, value)",
            NudgeType::EnvironmentQuirk => "💡 Environment quirk detected — consider saving to memory: MemoryNudge.save_env_quirk(key, value)",
            NudgeType::SuccessfulWorkflow => "💡 Successful workflow pattern — consider saving: MemoryNudge.save_workflow(name, steps)",
            NudgeType::ProjectContext => "💡 Project context change detected — consider updating memory with project type and structure",
        }
    }
}

/// Tracks interaction count to trigger memory nudges
#[derive(Debug)]
pub struct MemoryNudgeTracker {
    interaction_count: AtomicUsize,
    last_nudge_at: AtomicUsize,
    enabled: AtomicBool,
    project_type: std::sync::RwLock<Option<ProjectType>>,
}

impl Default for MemoryNudgeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryNudgeTracker {
    pub fn new() -> Self {
        Self {
            interaction_count: AtomicUsize::new(0),
            last_nudge_at: AtomicUsize::new(0),
            enabled: AtomicBool::new(true),
            project_type: std::sync::RwLock::new(None),
        }
    }

    /// Call after each interaction
    pub fn tick(&self) {
        self.interaction_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Check if a nudge should fire (every N interactions)
    pub fn should_nudge(&self, interval: usize) -> Option<NudgeType> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let count = self.interaction_count.load(Ordering::Relaxed);
        let last = self.last_nudge_at.load(Ordering::Relaxed);

        if count > 0 && count % interval == 0 && count != last {
            self.last_nudge_at.store(count, Ordering::Relaxed);
            // Rotate through nudge types
            match (count / interval) % 4 {
                0 => Some(NudgeType::Preference),
                1 => Some(NudgeType::EnvironmentQuirk),
                2 => Some(NudgeType::SuccessfulWorkflow),
                _ => Some(NudgeType::ProjectContext),
            }
        } else {
            None
        }
    }

    pub fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    pub fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    /// Set the current project type for ProjectContext nudges
    pub fn set_project_type(&self, pt: ProjectType) {
        let mut guard = self.project_type.write().unwrap();
        *guard = Some(pt);
    }

    pub fn get_project_type(&self) -> Option<ProjectType> {
        let guard = self.project_type.read().unwrap();
        guard.clone()
    }

    pub fn interaction_count(&self) -> usize {
        self.interaction_count.load(Ordering::Relaxed)
    }
}

/// Shared nudge tracker (cheap to clone)
pub type SharedNudgeTracker = Arc<MemoryNudgeTracker>;

pub fn new_nudge_tracker() -> SharedNudgeTracker {
    Arc::new(MemoryNudgeTracker::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_arithmetic() {
        assert_eq!(ToolCategory::classify("calculate 5 + 3"), ToolCategory::Arithmetic);
        assert_eq!(ToolCategory::classify("what is 100 / 4"), ToolCategory::Arithmetic);
        assert_eq!(ToolCategory::classify("compute the sum"), ToolCategory::Arithmetic);
    }

    #[test]
    fn test_tool_category_time() {
        assert_eq!(ToolCategory::classify("what time is it"), ToolCategory::CurrentTime);
        assert_eq!(ToolCategory::classify("today's date"), ToolCategory::CurrentTime);
    }

    #[test]
    fn test_tool_category_system() {
        assert_eq!(ToolCategory::classify("cpu usage"), ToolCategory::SystemState);
        assert_eq!(ToolCategory::classify("how much memory"), ToolCategory::SystemState);
        assert_eq!(ToolCategory::classify("os version"), ToolCategory::SystemState);
    }

    #[test]
    fn test_tool_category_file() {
        assert_eq!(ToolCategory::classify("show me the contents of file"), ToolCategory::FileContents);
        assert_eq!(ToolCategory::classify("read Cargo.toml"), ToolCategory::FileContents);
    }

    #[test]
    fn test_tool_category_git() {
        assert_eq!(ToolCategory::classify("git log"), ToolCategory::GitHistory);
        assert_eq!(ToolCategory::classify("recent commits"), ToolCategory::GitHistory);
    }

    #[test]
    fn test_context_fencing() {
        let memory = "user.name: Sebas\nproject.type: Rust";
        let fenced = fence_memory_context(memory);
        assert!(fenced.contains("<memory-context>"));
        assert!(fenced.contains("NOT new user input"));
        assert!(fenced.contains("user.name: Sebas"));
    }

    #[test]
    fn test_inject_fenced_memory() {
        let store = MemoryStore::new();
        store.store(crate::context::memory::Fact::new("key", "value", "test"));
        let prompt = "Hello, agent.";
        let result = inject_fenced_memory(prompt, &store);
        assert!(result.contains("<memory-context>"));
        assert!(result.contains(prompt));
    }

    #[test]
    fn test_nudge_tracker() {
        let tracker = MemoryNudgeTracker::new();
        assert!(tracker.should_nudge(10).is_none());

        for _ in 0..9 {
            tracker.tick();
        }
        assert!(tracker.should_nudge(10).is_none());

        tracker.tick(); // Now at 10
        assert!(matches!(tracker.should_nudge(10), Some(NudgeType::Preference)));
    }
}
