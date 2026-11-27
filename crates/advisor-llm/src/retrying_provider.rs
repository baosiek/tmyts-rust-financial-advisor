//! Decorator that sends a failed call again when the error is temporary (rate
//! limit, overload, network, timeout of one HTTP request), waiting longer each
//! time. Like `TimedProvider`, it implements `LlmProvider` itself, so it wraps
//! any provider and the agent cannot tell the difference.
//!
//! Wiring (in advisor-app): `TimedProvider(RetryingProvider(provider))`. The
//! outer time limit therefore covers all attempts together: retries can never
//! make a question wait longer than `call_timeout_secs`.

use std::sync::Arc;
use std::time::Duration;

use advisor_core::{HealthCheck, HealthError};

use crate::types::{ChatRequest, ChatResponse};
use crate::{LlmError, LlmProvider, RetryPolicy};

pub struct RetryingProvider {
    inner: Arc<dyn LlmProvider>,
    policy: RetryPolicy,
}

impl RetryingProvider {
    pub fn new(inner: Arc<dyn LlmProvider>, policy: RetryPolicy) -> Self {
        Self { inner, policy }
    }

    /// How long to wait before `retry`, or `None` to give up on `error` now.
    fn wait_before(&self, retry: u32, error: &LlmError) -> Option<Duration> {
        if !error.is_retryable() || retry >= self.policy.max_attempts() {
            return None;
        }
        match error {
            // The server said how long to wait; sooner would only fail again.
            // If that is longer than this policy allows, give up instead of
            // keeping the question hanging.
            LlmError::RateLimited { retry_after_ms } => {
                let wait = Duration::from_millis(*retry_after_ms)
                    .max(self.policy.backoff_with_jitter(retry));
                (wait <= self.policy.max_backoff()).then_some(wait)
            }
            _ => Some(self.policy.backoff_with_jitter(retry)),
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for RetryingProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let mut attempt = 1;
        loop {
            match self.inner.complete(req).await {
                Ok(resp) => {
                    if attempt > 1 {
                        tracing::info!(
                            provider = self.inner.name(),
                            attempt,
                            "llm call succeeded after retry"
                        );
                    }
                    return Ok(resp);
                }
                Err(error) => match self.wait_before(attempt, &error) {
                    Some(wait) => {
                        tracing::warn!(
                            provider = self.inner.name(),
                            attempt,
                            max_attempts = self.policy.max_attempts(),
                            wait_ms = wait.as_millis() as u64,
                            error = %error,
                            "llm call failed, retrying"
                        );
                        tokio::time::sleep(wait).await;
                        attempt += 1;
                    }
                    None => {
                        if error.is_retryable() {
                            tracing::warn!(
                                provider = self.inner.name(),
                                attempt,
                                error = %error,
                                "llm call failed, giving up"
                            );
                        }
                        return Err(error);
                    }
                },
            }
        }
    }
}

/// Delegates without retrying: a failing service at startup should be reported
/// at once, and the startup check has its own time limit.
#[async_trait::async_trait]
impl HealthCheck for RetryingProvider {
    fn service_name(&self) -> &'static str {
        self.inner.service_name()
    }

    async fn check(&self) -> Result<String, HealthError> {
        self.inner.check().await
    }
}
