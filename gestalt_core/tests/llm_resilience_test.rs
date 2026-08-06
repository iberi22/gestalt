use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use async_trait::async_trait;
use gestalt_core::ports::outbound::llm_resilience::{LlmResilience, LlmResilienceError};
use synapse_agentic::providers::LLMProvider;

struct MockProvider {
    name: String,
    call_count: Arc<AtomicUsize>,
    behavior: Arc<dyn Fn(usize) -> anyhow::Result<String> + Send + Sync>,
    delay: Option<Duration>,
}

impl std::fmt::Debug for MockProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockProvider")
            .field("name", &self.name)
            .field("call_count", &self.call_count)
            .field("delay", &self.delay)
            .finish()
    }
}

#[async_trait]
impl LLMProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn cost_per_1k_tokens(&self) -> f64 {
        0.0
    }

    async fn generate(&self, _prompt: &str) -> anyhow::Result<String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        let current_count = self.call_count.load(Ordering::SeqCst);
        (self.behavior)(current_count)
    }
}

/// Test Scenario 1: Primary provider returns error → fallback provider called.
#[tokio::test]
async fn test_primary_down_fallback_used() {
    let primary_calls = Arc::new(AtomicUsize::new(0));
    let fallback_calls = Arc::new(AtomicUsize::new(0));

    let primary = Arc::new(MockProvider {
        name: "Primary_Provider".to_string(),
        call_count: primary_calls.clone(),
        behavior: Arc::new(|_| Err(anyhow::anyhow!("Service Unavailable"))),
        delay: None,
    });

    let fallback = Arc::new(MockProvider {
        name: "Fallback_Provider".to_string(),
        call_count: fallback_calls.clone(),
        behavior: Arc::new(|_| Ok("Hello from Fallback!".to_string())),
        delay: None,
    });

    let resilience = LlmResilience::new(
        primary.clone(),
        vec![fallback.clone()],
        1, // 1 attempt, no retry
        Duration::from_millis(1),
        Duration::from_secs(1),
    );

    let result = resilience.call_with_failover("test prompt").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello from Fallback!");
    assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
    assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
}

/// Test Scenario 2: Rate limit error triggers backoff retries, then still fails, returning a clean structured Crashed error.
#[tokio::test]
async fn test_rate_limit_retry_and_crashed_error() {
    let primary_calls = Arc::new(AtomicUsize::new(0));

    let primary = Arc::new(MockProvider {
        name: "Rate_Limited_Provider".to_string(),
        call_count: primary_calls.clone(),
        behavior: Arc::new(|_| Err(anyhow::anyhow!("Rate limit exceeded: 429 Too Many Requests"))),
        delay: None,
    });

    let resilience = LlmResilience::new(
        primary.clone(),
        vec![],
        3, // 3 attempts
        Duration::from_millis(2),
        Duration::from_secs(1),
    );

    let result = resilience.call_with_failover("test prompt").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, LlmResilienceError::Crashed(_)),
        "Expected Crashed error, got: {:?}",
        err
    );
    assert!(err.to_string().contains("Rate limit exceeded"));
    assert_eq!(primary_calls.load(Ordering::SeqCst), 3);
}

/// Test Scenario 3: Timeout error on providers, returning a clean structured Timeout error.
#[tokio::test]
async fn test_timeout_to_timeout_error() {
    let primary_calls = Arc::new(AtomicUsize::new(0));

    let primary = Arc::new(MockProvider {
        name: "Slow_Provider".to_string(),
        call_count: primary_calls.clone(),
        behavior: Arc::new(|_| Ok("Too late...".to_string())),
        // Sleep longer than timeout
        delay: Some(Duration::from_millis(100)),
    });

    let resilience = LlmResilience::new(
        primary.clone(),
        vec![],
        2, // 2 attempts
        Duration::from_millis(2),
        Duration::from_millis(15), // enforce 15ms timeout
    );

    let result = resilience.call_with_failover("test prompt").await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, LlmResilienceError::Timeout(_)),
        "Expected Timeout error, got: {:?}",
        err
    );
    assert!(err.to_string().contains("timed out after"));
    assert_eq!(primary_calls.load(Ordering::SeqCst), 2);
}

/// Test Scenario 4: Primary provider fails on first attempt, but succeeds on the second attempt.
#[tokio::test]
async fn test_retry_success_on_second_attempt() {
    let primary_calls = Arc::new(AtomicUsize::new(0));

    let primary = Arc::new(MockProvider {
        name: "Flaky_Provider".to_string(),
        call_count: primary_calls.clone(),
        behavior: Arc::new(|count| {
            if count == 1 {
                Err(anyhow::anyhow!("First call fails transiently"))
            } else {
                Ok("Success on second try!".to_string())
            }
        }),
        delay: None,
    });

    let resilience = LlmResilience::new(
        primary.clone(),
        vec![],
        3, // up to 3 attempts
        Duration::from_millis(2),
        Duration::from_secs(1),
    );

    let result = resilience.call_with_failover("test prompt").await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Success on second try!");
    assert_eq!(primary_calls.load(Ordering::SeqCst), 2);
}
