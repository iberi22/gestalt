//!
//! # Hermes Memory Patterns (MEM-001, MEM-003, MEM-004)
//!
//! Provides memory guidance constants and sanitization utilities for
//! proper memory handling in the Gestalt agent system.
//!

/// Memory Guidelines for Agent Behavior
///
/// This constant defines how memory should be treated by agents:
/// - Memory = DECLARATIVE FACTS (not imperative instructions)
/// - Skills = procedures/workflows, NOT memory
/// - Save durable facts: user preferences, environment details, tool quirks
/// - Do NOT save task progress or temporary TODO state to memory
///
/// # Example
/// ✓ Good: "User prefers concise responses"
/// ✗ Bad: "Always respond concisely" (imperative, not declarative)
pub const MEMORY_GUIDANCE: &str = r#"Memory Guidelines:
- Memory = DECLARATIVE FACTS (not imperative instructions)
- Example: "User prefers concise responses" ✓ — "Always respond concisely" ✗
- Skills = procedures/workflows, NOT memory
- Save durable facts: user preferences, environment details, tool quirks
- Do NOT save task progress or temporary TODO state to memory
"#;

/// System note added when returning recalled memory context to agent.
/// This helps the agent distinguish recalled memory from new user input.
pub const MEMORY_CONTEXT_SYSTEM_NOTE: &str =
    "[System note: The following is recalled memory context, NOT new user input]";

/// Opening fence tag for memory context wrapping.
pub const MEMORY_CONTEXT_OPEN: &str = "<memory-context>";

/// Closing fence tag for memory context wrapping.
pub const MEMORY_CONTEXT_CLOSE: &str = "</memory-context>";

/// Wrap content with memory context fence tags.
///
/// Used when prefetching memory to give to an agent - the fence tags
/// signal to the agent that this is recalled context, not new input.
///
/// # Arguments
/// * `content` - The memory content to wrap
///
/// # Returns
/// A string with memory context fences and system note
pub fn wrap_memory_context(content: &str) -> String {
    if content.is_empty() {
        String::new()
    } else {
        format!(
            "{}\n{}\n{}\n{}",
            MEMORY_CONTEXT_OPEN,
            MEMORY_CONTEXT_SYSTEM_NOTE,
            content,
            MEMORY_CONTEXT_CLOSE
        )
    }
}

/// Strip memory context fence tags from provider output.
///
/// This sanitization removes fence tags that might have been
/// inadvertently included in LLM provider responses.
///
/// # Arguments
/// * `text` - The text to sanitize
///
/// # Returns
/// Cleaned text with fence tags removed
pub fn sanitize_memory_context(text: &str) -> String {
    let mut result = text.to_string();

    // Remove opening fence tag
    result = result.replace(MEMORY_CONTEXT_OPEN, "");

    // Remove closing fence tag
    result = result.replace(MEMORY_CONTEXT_CLOSE, "");

    // Remove system note
    result = result.replace(MEMORY_CONTEXT_SYSTEM_NOTE, "");

    // Clean up any extra whitespace from removed tags
    result = result
        .replace("  \n", "\n")
        .replace("\n\n\n", "\n\n")
        .trim()
        .to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_memory_context() {
        let content = "User prefers concise responses";
        let wrapped = wrap_memory_context(content);
        assert!(wrapped.contains(MEMORY_CONTEXT_OPEN));
        assert!(wrapped.contains(MEMORY_CONTEXT_CLOSE));
        assert!(wrapped.contains(MEMORY_CONTEXT_SYSTEM_NOTE));
        assert!(wrapped.contains(content));
    }

    #[test]
    fn test_wrap_empty_memory_context() {
        let wrapped = wrap_memory_context("");
        assert!(wrapped.is_empty());
    }

    #[test]
    fn test_sanitize_memory_context() {
        let input = format!(
            "{}\n{}\nUser prefers concise responses\n{}",
            MEMORY_CONTEXT_OPEN,
            MEMORY_CONTEXT_SYSTEM_NOTE,
            MEMORY_CONTEXT_CLOSE
        );
        let sanitized = sanitize_memory_context(&input);
        assert!(!sanitized.contains("<memory-context>"));
        assert!(!sanitized.contains("</memory-context>"));
        assert!(!sanitized.contains("[System note:"));
        assert!(sanitized.contains("User prefers concise responses"));
    }

    #[test]
    fn test_sanitize_preserves_normal_content() {
        let input = "This is normal content without any memory tags";
        let sanitized = sanitize_memory_context(input);
        assert_eq!(sanitized, "This is normal content without any memory tags");
    }
}