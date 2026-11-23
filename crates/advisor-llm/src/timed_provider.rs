//! Decorator that times every call of the provider it wraps, applies an overall
//! time limit, and logs the outcome. It implements `LlmProvider` itself, so the
//! agent cannot tell whether it talks to a wrapped or an unwrapped provider.

use std::sync::Arc;
use std::time::Duration;

use advisor_core::{TimedError, timed};

use crate::types::{ChatRequest, ChatResponse};
use crate::{LlmError, LlmProvider};

pub struct TimedProvider {
    inner: Arc<dyn LlmProvider>,
    limit: Duration,
}

impl TimedProvider {
    /// Wraps `inner`. Calls that take longer than `limit` fail with `LlmError::Timeout`.
    pub fn new(inner: Arc<dyn LlmProvider>, limit: Duration) -> Self {
        Self { inner, limit }
    }
}

#[async_trait::async_trait]
impl LlmProvider for TimedProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let pending = timed("llm.complete", self.limit, self.inner.complete(req)).await;
        let elapsed_ms = pending.elapsed.as_millis() as u64;

        match pending.result {
            Ok(resp) => {
                tracing::info!(
                    provider = self.inner.name(),
                    model = %resp.model,
                    elapsed_ms,
                    input_tokens = resp.usage.input_tokens,
                    output_tokens = resp.usage.output_tokens,
                    "llm call succeeded"
                );
                Ok(resp)
            }
            Err(TimedError::Failed(e)) => {
                tracing::warn!(
                    provider = self.inner.name(),
                    elapsed_ms,
                    retryable = e.is_retryable(),
                    error = %e,
                    "llm call failed"
                );
                Err(e)
            }
            Err(TimedError::TimedOut(limit)) => {
                let after_ms = limit.as_millis() as u64;
                tracing::error!(provider = self.inner.name(), after_ms, "llm call timed out");
                Err(LlmError::Timeout { after_ms })
            }
        }
    }
}
