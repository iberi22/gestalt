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

use crate::event_bus::BusEvent;
use gestalt_core::application::agent::xavier::XavierClient;
use gestalt_state::StateDb;
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

    /// Pull recent bus events from the local StateDb timeline (authoritative
    /// source — the same store `bus serve` writes to).
    ///
    /// This is more reliable than querying Xavier's semantic search, whose
    /// embeddings index may lag or exclude short bus lines. The StateDb
    /// timeline is always available and contains every event verbatim.
    pub fn recent_executions_from_db(&self, db: &StateDb, limit: usize) -> Vec<String> {
        match db.recent_timeline(limit as i64) {
            Ok(events) => events
                .iter()
                .filter_map(|e| serde_json::from_str::<BusEvent>(&e.payload).ok())
                .map(|e| {
                    format!(
                        "[{}] {} {} — {}",
                        e.event_type,
                        e.agent,
                        e.run_id.as_deref().unwrap_or("?"),
                        e.summary
                    )
                })
                .collect(),
            Err(_) => Vec::new(),
        }
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

    /// Get the timestamp of the last insight from Xavier by searching for
    /// "gestalt/thinking/" path prefix.
    pub async fn last_insight_time(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        let resp = self
            .xavier
            .search("gestalt/thinking/", 100, "snippet")
            .await
            .ok()?;

        let mut latest_date: Option<chrono::DateTime<chrono::Utc>> = None;

        for res in resp.results {
            if res.path.starts_with("gestalt/thinking/") {
                if let Some(date_str) = res.path.strip_prefix("gestalt/thinking/") {
                    if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d") {
                        if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
                            let datetime = naive_datetime.and_utc();
                            if latest_date.is_none() || Some(datetime) > latest_date {
                                latest_date = Some(datetime);
                            }
                        }
                    }
                }
            }
        }
        latest_date
    }

    /// Count the number of pending executions in StateDb since the last index/insight.
    pub async fn pending_executions_since_last_insight(&self, db: &StateDb) -> usize {
        let last_time = self.last_insight_time().await;

        let events = match db.recent_timeline(1000) {
            Ok(evs) => evs,
            Err(_) => return 0,
        };

        events
            .iter()
            .filter(|e| {
                let is_execution = e.event_type == "run_finished" || e.event_type == "run_started";
                let is_newer = last_time.map_or(true, |t| e.created_at > t);
                is_execution && is_newer
            })
            .count()
    }

    /// Determine whether the thinking loop should run.
    pub async fn should_run(&self, db: &StateDb, min_executions: usize) -> bool {
        self.pending_executions_since_last_insight(db).await >= min_executions
    }

    /// Run one full thinking cycle.
    ///
    /// 1. Pull recent executions from the local StateDb timeline
    /// 2. Skip if today's insight already exists (idempotent) — unless forced
    /// 3. Synthesize via the deterministic synthesizer
    /// 4. Re-index as kind=insight
    pub async fn run(&self, db: &StateDb, force: bool) -> Result<Option<String>, String> {
        let executions = self.recent_executions_from_db(db, 100);
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
