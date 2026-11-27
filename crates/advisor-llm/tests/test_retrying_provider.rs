//! RetryingProvider with a scripted provider that fails a set number of times.
//! Time is paused (`start_paused`), so the backoff waits take no real time but
//! still advance the clock, which the tests measure.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use advisor_core::{HealthCheck, HealthError};
use advisor_llm::types::{ChatRequest, ChatResponse, ContentBlock, StopReason, Usage};
use advisor_llm::{LlmError, LlmProvider, RetryPolicy, RetryingProvider};
use tokio::time::Instant;

struct FlakyProvider {
    script: Mutex<VecDeque<Result<ChatResponse, LlmError>>>,
    calls: Mutex<u32>,
}

impl FlakyProvider {
    fn new(script: Vec<Result<ChatResponse, LlmError>>) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(script.into()),
            calls: Mutex::new(0),
        })
    }

    fn calls(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl HealthCheck for FlakyProvider {
    fn service_name(&self) -> &'static str {
        "flaky"
    }

    async fn check(&self) -> Result<String, HealthError> {
        Err("down".into())
    }
}

#[async_trait::async_trait]
impl LlmProvider for FlakyProvider {
    fn name(&self) -> &str {
        "flaky"
    }

    fn model(&self) -> &str {
        "flaky-model"
    }

    async fn complete(&self, _req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        *self.calls.lock().unwrap() += 1;
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .expect("the provider was called more often than scripted")
    }
}

fn ok() -> Result<ChatResponse, LlmError> {
    Ok(ChatResponse {
        content: vec![ContentBlock::text("ok")],
        model: "flaky-model".to_string(),
        stop_reason: StopReason::EndTurn,
        usage: Usage::default(),
    })
}

fn overloaded() -> Result<ChatResponse, LlmError> {
    Err(LlmError::Api {
        status: 529,
        message: "overloaded".into(),
    })
}

/// 3 attempts, waits of 1 s then 2 s, capped at 10 s, no jitter (exact timings).
fn retrying(inner: Arc<FlakyProvider>, max_attempts: u32) -> RetryingProvider {
    let policy = RetryPolicy::new(
        max_attempts,
        Duration::from_secs(1),
        Duration::from_secs(10),
        2.0,
        0.0,
    )
    .unwrap();
    RetryingProvider::new(inner, policy)
}

#[tokio::test(start_paused = true)]
async fn temporary_failures_are_retried_with_growing_waits() {
    let inner = FlakyProvider::new(vec![overloaded(), overloaded(), ok()]);
    let provider = retrying(inner.clone(), 3);

    let started = Instant::now();
    let resp = provider.complete(&ChatRequest::new("Hi")).await.unwrap();

    assert_eq!(resp.text(), "ok");
    assert_eq!(inner.calls(), 3);
    assert_eq!(
        started.elapsed(),
        Duration::from_secs(3),
        "waited 1 s + 2 s"
    );
}

#[tokio::test(start_paused = true)]
async fn gives_up_after_max_attempts_with_the_last_error() {
    let inner = FlakyProvider::new(vec![overloaded(), overloaded(), overloaded()]);
    let provider = retrying(inner.clone(), 3);

    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();

    assert!(matches!(err, LlmError::Api { status: 529, .. }));
    assert_eq!(inner.calls(), 3);
}

#[tokio::test(start_paused = true)]
async fn permanent_errors_are_not_retried() {
    for error in [
        LlmError::Auth("bad key".into()),
        LlmError::ModelNotFound("m".into()),
        LlmError::QuotaExceeded("no credit".into()),
        LlmError::Parse("bad json".into()),
    ] {
        let inner = FlakyProvider::new(vec![Err(error)]);
        let provider = retrying(inner.clone(), 3);

        let started = Instant::now();
        assert!(provider.complete(&ChatRequest::new("Hi")).await.is_err());
        assert_eq!(inner.calls(), 1);
        assert_eq!(started.elapsed(), Duration::ZERO, "no waiting either");
    }
}

#[tokio::test(start_paused = true)]
async fn rate_limit_waits_as_long_as_the_server_asks() {
    let inner = FlakyProvider::new(vec![
        Err(LlmError::RateLimited {
            retry_after_ms: 5_000,
        }),
        ok(),
    ]);
    let provider = retrying(inner.clone(), 3);

    let started = Instant::now();
    provider.complete(&ChatRequest::new("Hi")).await.unwrap();

    assert_eq!(
        started.elapsed(),
        Duration::from_secs(5),
        "retry-after beats the 1 s backoff"
    );
}

#[tokio::test(start_paused = true)]
async fn rate_limit_longer_than_max_backoff_is_not_waited_for() {
    let inner = FlakyProvider::new(vec![Err(LlmError::RateLimited {
        retry_after_ms: 60_000,
    })]);
    let provider = retrying(inner.clone(), 3);

    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();

    assert!(matches!(err, LlmError::RateLimited { .. }));
    assert_eq!(inner.calls(), 1);
}

#[tokio::test(start_paused = true)]
async fn one_attempt_means_no_retries() {
    let inner = FlakyProvider::new(vec![overloaded()]);
    let provider = retrying(inner.clone(), 1);

    assert!(provider.complete(&ChatRequest::new("Hi")).await.is_err());
    assert_eq!(inner.calls(), 1);
}

#[tokio::test]
async fn health_check_is_delegated_without_retries() {
    let inner = FlakyProvider::new(vec![]);
    let provider = retrying(inner.clone(), 3);

    assert_eq!(provider.service_name(), "flaky");
    assert!(provider.check().await.is_err());
    assert_eq!(provider.name(), "flaky");
    assert_eq!(provider.model(), "flaky-model");
}
