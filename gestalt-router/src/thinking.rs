//! Xavier Thinking Loop — synthesize cross-run insights from execution memories.
//!
//! Design: `docs/design/xavier-thinking-bus.md` (P1, section 4.3).
//!
//! The ThinkingLoop is the "thinking" layer of the vision: it periodically
//! pulls recent `kind=execution` memories from Xavier, synthesizes a concise
//! cross-run insight (patterns, blockers, decisions, next steps) via a local
//! LLM, and re-indexes it as `kind=insight` so future agents find it via PRE
//! search. This closes the loop: agents write → Xavier remembers → Xavier
//! thinks → future agents benefit.
//!
//! The loop is idempotent (checks today's `kind=insight` before writing) and
//! gated on minimum signal (≥3 executions) to avoid low-quality insights.

use gestalt_core::application::agent::xavier::XavierClient;
use serde_json::json;
use std::sync::Arc;
use tracing::info;

/// Minimum number of recent executions before synthesizing an insight.
pub const MIN_EXECUTIONS: usize = 3;

/// How far back (minutes) the loop looks for recent executions.
pub const DEFAULT_WINDOW_MINUTES: u64 = 30;

/// Generates an insight text from a set of recent execution memories.
///
/// The router crate stays transport-agnostic: the concrete implementation
/// (Ollama qwen3-coder, OpenAI-compatible endpoint, or a structural fallback)
/// lives in the CLI layer where HTTP clients already exist.
#[async_trait::async_trait]
pub trait InsightSynthesizer: Send + Sync {
    /// Given recent execution contents, produce a concise insight (≤200 words).
    async fn synthesize(&self, executions: &[String]) -> Result<String, String>;
}

/// Xavier Thinking Loop — orchestration of the insight cycle.
pub struct ThinkingLoop {
    xavier: Arc<XavierClient>,
    synthesizer: Arc<dyn InsightSynthesizer>,
    window_minutes: u64,
}

impl ThinkingLoop {
    /// Create a thinking loop over a Xavier client with a synthesizer.
    pub fn new(xavier: Arc<XavierClient>, synthesizer: Arc<dyn InsightSynthesizer>) -> Self {
        Self {
            xavier,
            synthesizer,
            window_minutes: DEFAULT_WINDOW_MINUTES,
        }
    }

    /// Override the look-back window (mainly for tests).
    pub fn with_window(mut self, minutes: u64) -> Self {
        self.window_minutes = minutes;
        self
    }

    /// Pull recent `kind=execution` memories from Xavier.
    ///
    /// The query targets the bus namespace (`gestalt/bus/executions` prefix
    /// lives in the content/metadata of every streamed event), which the
    /// snippet search matches reliably; `kind` is a metadata field, not a
    /// textual token, so it cannot drive the query itself.
    pub async fn recent_executions(&self, limit: usize) -> Result<Vec<String>, String> {
        let resp = self
            .xavier
            .search("gestalt bus executions", limit, "snippet")
            .await
            .map_err(|e| format!("Xavier search failed: {}", e))?;

        Ok(resp
            .results
            .into_iter()
            .map(|r| r.text())
            .filter(|c| !c.trim().is_empty())
            .collect())
    }

    /// Re-index a synthesized insight as `kind=insight` for future PRE context.
    pub async fn index_insight(&self, text: &str) -> Result<String, String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let path = format!("gestalt/thinking/{}", today);
        self.xavier
            .add(text, &path, "insight", json!({"source": "thinking-loop"}))
            .await
            .map(|r| r.id)
            .map_err(|e| format!("Xavier add failed: {}", e))
    }

    /// Check whether an insight for today already exists (idempotency).
    pub async fn has_today_insight(&self) -> Result<bool, String> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let resp = self
            .xavier
            .search(&format!("gestalt/thinking/{}", today), 1, "snippet")
            .await
            .map_err(|e| format!("Xavier search failed: {}", e))?;

        Ok(!resp.results.is_empty())
    }

    /// Run one full thinking cycle.
    ///
    /// 1. Pull recent executions (min-signal gate)
    /// 2. Skip if today's insight already exists (idempotent) — unless forced
    /// 3. Synthesize via local LLM
    /// 4. Re-index as kind=insight
    pub async fn run(&self, force: bool) -> Result<Option<String>, String> {
        let executions = self.recent_executions(50).await?;
        if executions.len() < MIN_EXECUTIONS {
            info!(
                "Thinking loop: only {} executions (need ≥{}) — skipping",
                executions.len(),
                MIN_EXECUTIONS
            );
            return Ok(None);
        }

        if !force && self.has_today_insight().await? {
            info!("Thinking loop: today's insight already exists — skipping");
            return Ok(None);
        }

        let insight = self
            .synthesizer
            .synthesize(&executions)
            .await
            .map_err(|e| format!("Synthesis failed: {}", e))?;

        let id = self.index_insight(&insight).await?;
        info!("Thinking loop: insight indexed (id={})", id);
        Ok(Some(insight))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSynthesizer;

    #[async_trait::async_trait]
    impl InsightSynthesizer for FakeSynthesizer {
        async fn synthesize(&self, executions: &[String]) -> Result<String, String> {
            Ok(format!(
                "PATTERNS: {} executions; BLOCKERS: none; DECISIONS: n/a; NEXT: review",
                executions.len()
            ))
        }
    }

    #[tokio::test]
    async fn synthesizer_receives_executions() {
        let synth = FakeSynthesizer;
        let execs = vec!["run a".to_string(), "run b".to_string()];
        let out = synth.synthesize(&execs).await.unwrap();
        assert!(out.contains("2 executions"));
    }

    #[test]
    fn min_executions_gate_is_three() {
        assert_eq!(MIN_EXECUTIONS, 3);
    }

    #[test]
    fn default_window_is_30_minutes() {
        assert_eq!(DEFAULT_WINDOW_MINUTES, 30);
    }
}
