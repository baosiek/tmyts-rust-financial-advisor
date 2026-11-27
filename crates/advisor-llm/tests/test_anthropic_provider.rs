use advisor_core::Secret;
use advisor_llm::{AnthropicConfig, AnthropicProvider, LlmError, LlmProvider, types::ChatRequest};
use serde_json::json;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

fn provider_for(server: &MockServer) -> AnthropicProvider {
    let config = AnthropicConfig {
        api_key: Secret::new("test-key"),
        model: "test-model".to_string(),
        models_url: Url::parse(&format!("{}/v1/models", server.uri())).expect("valid URL"),
        version: "2023-06-01".to_string(),
        url: Url::parse(&format!("{}/v1/messages", server.uri())).expect("valid URL"),
        timeout: std::time::Duration::from_secs(5),
    };

    AnthropicProvider::new(config).expect("provider should be build")
}

#[tokio::test]
async fn successful_response_becomes_chat_response() {
    // 1. Start a fake server on a random local port.
    let server = MockServer::start().await;

    // 2. A POST to /v1/messages with these headers gets this answer.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(
            {
                "id": "msg_01",
                "type": "message",
                "role": "assistant",
                "model": "test-model",
                "content": [{ "type": "text", "text": "Hello!" }],
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 12, "output_tokens": 3 }
            }
        )))
        .mount(&server)
        .await;

    // 3. Call the provider exactly as the agent will.
    let provider = provider_for(&server);
    let resp = provider.complete(&ChatRequest::new("Hi")).await.unwrap();

    // 4. Check the conversion.
    assert_eq!(resp.text(), "Hello!");
    assert_eq!(resp.usage.input_tokens, 12);
    assert_eq!(resp.usage.output_tokens, 3);
}

#[tokio::test]
async fn rate_limit_becomes_rate_limited_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "2")
                .set_body_json(json!({
                    "type": "error",
                    "error": { "type": "rate_limit_error", "message": "Too many requests" }
                })),
        )
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            LlmError::RateLimited {
                retry_after_ms: 2000
            }
        ),
        "expected RateLimited with 2000 ms, got {err:?}"
    );
    assert!(err.is_retryable());
}

#[tokio::test]
async fn unauthorized_becomes_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "type": "error",
            "error": { "type": "authentication_error", "message": "invalid x-api-key" }
        })))
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();

    assert!(
        matches!(&err, LlmError::Auth(body) if body.contains("invalid x-api-key")),
        "expected Auth error, got {err:?}"
    );
    assert!(!err.is_retryable());
    assert!(
        !err.to_string().contains("test-key"),
        "error message leaks the API key"
    );
}

#[tokio::test]
async fn malformed_response_becomes_parse_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "msg_01",
            "content": [{ "type": "text", "text": "Hello!" }]
        })))
        .mount(&server)
        .await;

    let provider = provider_for(&server);
    let err = provider
        .complete(&ChatRequest::new("Hi"))
        .await
        .unwrap_err();

    assert!(
        matches!(err, LlmError::Parse(_)),
        "expected Parse error, got {err:?}"
    );
    assert!(!err.is_retryable());
}

// ---- health check: key valid and configured model exists -------------------

use advisor_core::HealthCheck;

#[tokio::test]
async fn health_check_asks_for_the_configured_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/test-model"))
        .and(header("x-api-key", "test-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "type": "model", "id": "test-model", "display_name": "Test Model"
        })))
        .mount(&server)
        .await;

    let detail = provider_for(&server).check().await.unwrap();
    assert!(detail.contains("test-model"), "got {detail}");
}

#[tokio::test]
async fn health_check_reports_a_missing_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/test-model"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "type": "error", "error": { "type": "not_found_error", "message": "model: test-model" }
        })))
        .mount(&server)
        .await;

    let err = provider_for(&server).check().await.unwrap_err();
    let err = err.downcast::<LlmError>().expect("an LlmError");
    assert!(matches!(*err, LlmError::ModelNotFound(ref m) if m == "test-model"));
}

#[tokio::test]
async fn health_check_reports_a_bad_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models/test-model"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid x-api-key"))
        .mount(&server)
        .await;

    let err = provider_for(&server).check().await.unwrap_err();
    assert!(matches!(
        *err.downcast::<LlmError>().unwrap(),
        LlmError::Auth(_)
    ));
}
