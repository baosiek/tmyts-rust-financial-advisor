//! TimedProvider tested with a mock provider whose delay and outcome we control.

use std::sync::Arc;
use std::time::Duration;

use advisor_core::{HealthCheck, HealthError};
use advisor_llm::types::{ChatRequest, ChatResponse, ContentBlock, StopReason, Usage};
use advisor_llm::{LlmError, LlmProvider, TimedProvider};

struct SlowProvider {
    delay: Duration,
    fail: bool,
}

#[async_trait::async_trait]
impl HealthCheck for SlowProvider {
    fn service_name(&self) -> &'static str {
        "slow"
    }

    async fn check(&self) -> Result<String, HealthError> {
        Ok("ok".to_string())
    }
}

#[async_trait::async_trait]
impl LlmProvider for SlowProvider {
    fn name(&self) -> &str {
        "slow"
    }

    fn model(&self) -> &str {
        "slow-model"
    }

    async fn complete(&self, _req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        tokio::time::sleep(self.delay).await;
        if self.fail {
            return Err(LlmError::Auth("bad key".into()));
        }
        Ok(ChatResponse {
            content: vec![ContentBlock::text("done")],
            model: "slow-model".to_string(),
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
        })
    }
}

fn wrapped(delay_ms: u64, fail: bool, limit_ms: u64) -> TimedProvider {
    let inner = Arc::new(SlowProvider {
        delay: Duration::from_millis(delay_ms),
        fail,
    });
    TimedProvider::new(inner, Duration::from_millis(limit_ms))
}

#[tokio::test]
async fn fast_call_passes_through() {
    let provider = wrapped(5, false, 1_000);
    let resp = provider.complete(&ChatRequest::new("Hi")).await.unwrap();
    assert_eq!(resp.text(), "done");
    assert_eq!(provider.name(), "slow");
}

#[tokio::test]
async fn provider_error_passes_through_unchanged() {
    let provider = wrapped(5, true, 1_000);
    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();
    assert!(matches!(err, LlmError::Auth(_)), "got {err:?}");
}

#[tokio::test]
async fn slow_call_becomes_timeout_error() {
    let provider = wrapped(5_000, false, 20);
    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, LlmError::Timeout { after_ms: 20 }),
        "got {err:?}"
    );
    assert!(err.is_retryable());
}
