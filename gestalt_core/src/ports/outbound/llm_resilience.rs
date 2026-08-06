//! 🛡️ LLM Provider Resilience Layer
//!
//! Provides automatic failover, retries with exponential backoff, and timeouts for
//! LLM generation requests. Never panics on failure; instead, it returns structured
//! errors that map directly to `Timeout` or `Crashed` agent states.

use std::sync::Arc;
use std::time::Duration;
use synapse_agentic::providers::LLMProvider;
use thiserror::Error;

/// Structured error representing the failure mode of the resilient LLM execution.
/// Maps cleanly to `AgentState::Timeout` or `AgentState::Crashed`.
#[derive(Debug, Error, Clone)]
pub enum LlmResilienceError {
    /// The request or individual retries timed out.
    #[error("LLM execution timed out: {0}")]
    Timeout(String),

    /// The request failed due to provider crashing, outages, rate limits, or all failovers failing.
    #[error("LLM execution crashed/failed: {0}")]
    Crashed(String),
}

/// A centralized resilience layer configuration for LLM calls.
/// Coordinates primary provider execution, retries with exponential backoff, and automatic
/// fallback routing to alternate providers if needed.
#[derive(Clone)]
pub struct LlmResilience {
    /// The primary LLM provider to attempt first.
    pub primary: Arc<dyn LLMProvider>,
    /// Fallback LLM providers to try in order if the primary provider fails.
    pub fallbacks: Vec<Arc<dyn LLMProvider>>,
    /// Maximum number of retries per provider.
    pub max_retries: usize,
    /// Initial duration to wait before the first retry (exponentially doubled after each retry).
    pub initial_backoff: Duration,
    /// Timeout duration enforced on each individual provider call.
    pub timeout: Duration,
}

impl std::fmt::Debug for LlmResilience {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmResilience")
            .field("primary", &self.primary.name())
            .field(
                "fallbacks",
                &self.fallbacks.iter().map(|f| f.name()).collect::<Vec<_>>(),
            )
            .field("max_retries", &self.max_retries)
            .field("initial_backoff", &self.initial_backoff)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl LlmResilience {
    /// Creates a new resilience manager.
    pub fn new(
        primary: Arc<dyn LLMProvider>,
        fallbacks: Vec<Arc<dyn LLMProvider>>,
        max_retries: usize,
        initial_backoff: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            primary,
            fallbacks,
            max_retries,
            initial_backoff,
            timeout,
        }
    }

    /// Executes the prompt using the configured resilience strategy:
    /// - Starts with the primary provider.
    /// - Enforces timeouts on individual calls.
    /// - Retries failures with exponential backoff.
    /// - Gracefully falls back to configured secondary providers if the primary (after retries) continues to fail.
    /// - Returns a structured `LlmResilienceError` indicating `Timeout` or `Crashed` states if all options are exhausted.
    pub async fn call_with_failover(&self, prompt: &str) -> Result<String, LlmResilienceError> {
        let mut last_err = None;

        // Sequence: Primary provider, followed by each fallback provider in order.
        let providers = std::iter::once(&self.primary).chain(self.fallbacks.iter());

        for (provider_idx, provider) in providers.enumerate() {
            let mut backoff = self.initial_backoff;
            let provider_label = if provider_idx == 0 { "primary" } else { "fallback" };

            for attempt in 1..=self.max_retries {
                tracing::info!(
                    "Attempt {}/{} for {} provider '{}' (index {})",
                    attempt,
                    self.max_retries,
                    provider_label,
                    provider.name(),
                    provider_idx
                );

                let result = tokio::time::timeout(self.timeout, provider.generate(prompt)).await;

                match result {
                    Ok(Ok(text)) => {
                        tracing::info!(
                            "Successfully generated content on attempt {} with provider '{}'",
                            attempt,
                            provider.name()
                        );
                        return Ok(text);
                    }
                    Ok(Err(e)) => {
                        let err_msg = e.to_string();
                        tracing::warn!(
                            "Provider '{}' returned error: {}. Attempt {}/{}",
                            provider.name(),
                            err_msg,
                            attempt,
                            self.max_retries
                        );
                        last_err = Some(LlmResilienceError::Crashed(format!(
                            "Provider '{}' crashed: {}",
                            provider.name(),
                            err_msg
                        )));
                    }
                    Err(_) => {
                        tracing::warn!(
                            "Provider '{}' call timed out after {:?}. Attempt {}/{}",
                            provider.name(),
                            self.timeout,
                            attempt,
                            self.max_retries
                        );
                        last_err = Some(LlmResilienceError::Timeout(format!(
                            "Provider '{}' timed out after {:?}",
                            provider.name(),
                            self.timeout
                        )));
                    }
                }

                // If not the last attempt, sleep with backoff
                if attempt < self.max_retries {
                    tracing::info!("Waiting {:?} before next retry...", backoff);
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                }
            }

            tracing::warn!(
                "All {} retries exhausted for {} provider '{}'",
                self.max_retries,
                provider_label,
                provider.name()
            );
        }

        // If we made it here, every provider and retry attempt failed. Return the structured error.
        Err(last_err.unwrap_or_else(|| {
            LlmResilienceError::Crashed("All configured providers failed to execute.".to_string())
        }))
    }
}
