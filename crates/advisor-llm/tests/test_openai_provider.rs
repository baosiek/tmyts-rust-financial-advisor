//! The OpenAI adapter against a fake Responses API.

use advisor_core::{HealthCheck, Secret};
use advisor_llm::types::{ChatRequest, ContentBlock, Message, StopReason};
use advisor_llm::{LlmError, LlmProvider, OpenaiConfig, OpenaiProvider};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn provider_for(server: &MockServer) -> OpenaiProvider {
    OpenaiProvider::new(OpenaiConfig {
        api_key: Secret::new("sk-test"),
        model: "test-model".to_string(),
        responses_url: Url::parse(&format!("{}/v1/responses", server.uri())).unwrap(),
        models_url: Url::parse(&format!("{}/v1/models", server.uri())).unwrap(),
        timeout: std::time::Duration::from_secs(5),
        replay_reasoning: true,
    })
    .unwrap()
}

fn error_body(code: &str) -> serde_json::Value {
    json!({ "error": { "message": "nope", "type": "error", "code": code } })
}

#[tokio::test]
async fn tool_loop_round_trip_uses_bearer_auth_and_flat_items() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_partial_json(json!({
            "model": "test-model",
            "store": false,
            "input": [
                { "type": "message", "role": "user", "content": "Price of AAPL?" },
                { "type": "function_call", "call_id": "call_1", "name": "price_history",
                  "arguments": "{\"ticker\":\"AAPL\"}" },
                { "type": "function_call_output", "call_id": "call_1", "output": "+4.2%" }
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "resp_1",
            "model": "test-model-2026-10-01",
            "status": "completed",
            "output": [{ "type": "message", "role": "assistant",
                "content": [{ "type": "output_text", "text": "AAPL rose 4.2%." }] }],
            "usage": { "input_tokens": 120, "output_tokens": 8,
                       "input_tokens_details": { "cached_tokens": 100 } }
        })))
        .mount(&server)
        .await;

    let mut req = ChatRequest::new("Price of AAPL?");
    req.messages
        .push(Message::assistant_blocks(vec![ContentBlock::ToolUse {
            id: "call_1".into(),
            name: "price_history".into(),
            input: json!({ "ticker": "AAPL" }),
        }]));
    req.messages
        .push(Message::tool_results(vec![ContentBlock::ToolResult {
            tool_use_id: "call_1".into(),
            content: "+4.2%".into(),
            is_error: false,
        }]));

    let resp = provider_for(&server).complete(&req).await.unwrap();

    assert_eq!(resp.text(), "AAPL rose 4.2%.");
    assert_eq!(resp.stop_reason, StopReason::EndTurn);
    assert_eq!(resp.model, "test-model-2026-10-01");
    assert_eq!(resp.usage.input_tokens, 20);
    assert_eq!(resp.usage.cache_read_input_tokens, 100);
}

#[tokio::test]
async fn empty_account_is_quota_exceeded_not_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(429).set_body_json(error_body("insufficient_quota")))
        .mount(&server)
        .await;

    let err = provider_for(&server)
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();
    assert!(matches!(err, LlmError::QuotaExceeded(_)), "got {err:?}");
    assert!(!err.is_retryable(), "waiting does not add credit");
}

#[tokio::test]
async fn rate_limit_is_retryable_with_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "3")
                .set_body_json(error_body("rate_limit_exceeded")),
        )
        .mount(&server)
        .await;

    let err = provider_for(&server)
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            LlmError::RateLimited {
                retry_after_ms: 3000
            }
        ),
        "got {err:?}"
    );
}

#[tokio::test]
async fn bad_key_is_auth_error_without_leaking_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(401).set_body_json(error_body("invalid_api_key")))
        .mount(&server)
        .await;

    let err = provider_for(&server)
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();
    assert!(matches!(err, LlmError::Auth(_)));
    assert!(!err.to_string().contains("sk-test"));
}

#[tokio::test]
async fn health_check_asks_for_the_configured_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/test-model"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "test-model", "object": "model", "created": 1, "owned_by": "openai"
        })))
        .mount(&server)
        .await;

    let detail = provider_for(&server).check().await.unwrap();
    assert!(detail.contains("test-model"), "got {detail}");
    assert_eq!(provider_for(&server).service_name(), "openai_provider");
}

#[tokio::test]
async fn health_check_reports_a_missing_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/test-model"))
        .respond_with(ResponseTemplate::new(404).set_body_json(error_body("model_not_found")))
        .mount(&server)
        .await;

    let err = provider_for(&server).check().await.unwrap_err();
    assert!(matches!(
        *err.downcast::<LlmError>().unwrap(),
        LlmError::ModelNotFound(ref m) if m == "test-model"
    ));
}
