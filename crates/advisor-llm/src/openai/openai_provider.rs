use serde::Deserialize;
use url::Url;

use crate::http::{model_url, retry_after_ms};
use crate::openai::openai_config::OpenaiConfig;
use crate::openai::wire::{ModelInfo, ResponsesRequest, ResponsesResponse};
use crate::types::{ChatRequest, ChatResponse};
use crate::{llm_error::LlmError, llm_provider::LlmProvider};
use advisor_core::{HealthCheck, HealthError, Secret};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder, Response, StatusCode};

/// Wait used when a 429 carries no `retry-after` header.
const DEFAULT_RETRY_AFTER_MS: u64 = 1000;

/// Adapter for the OpenAI Responses API (`POST /v1/responses`).
#[derive(Clone, Debug)]
pub struct OpenaiProvider {
    name: String,
    model: String,
    api_key: Secret,
    responses_url: Url,
    models_url: Url,
    replay_reasoning: bool,
    client: reqwest::Client,
}

impl OpenaiProvider {
    pub fn new(config: OpenaiConfig) -> Result<Self, LlmError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| LlmError::Network(e.to_string()))?;

        Ok(Self {
            name: "OpenAI".to_string(),
            model: config.model,
            api_key: config.api_key,
            responses_url: config.responses_url,
            models_url: config.models_url,
            replay_reasoning: config.replay_reasoning,
            client,
        })
    }

    /// OpenAI authenticates with a bearer token, not an `x-api-key` header.
    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        request.bearer_auth(self.api_key.expose())
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenaiProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let request = ResponsesRequest::from_chat(req, &self.model, self.replay_reasoning);

        let resp = self
            .authorized(self.client.post(self.responses_url.clone()))
            .json(&request)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(error_from_response(resp, &self.model).await);
        }

        let parsed: ResponsesResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;
        parsed.into_chat(self.replay_reasoning)
    }
}

/// Checks that the key is valid **and** that the configured model exists
/// (`GET /v1/models/{model}`).
#[async_trait]
impl HealthCheck for OpenaiProvider {
    fn service_name(&self) -> &'static str {
        "openai_provider"
    }

    async fn check(&self) -> Result<String, HealthError> {
        let resp = self
            .authorized(self.client.get(model_url(&self.models_url, &self.model)))
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(error_from_response(resp, &self.model).await.into());
        }
        let model: ModelInfo = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;
        Ok(match model.owned_by {
            Some(owner) => format!("model {} (owned by {owner}) available", model.id),
            None => format!("model {} available", model.id),
        })
    }
}

/// OpenAI's error body: `{"error": {"message": "...", "type": "...", "code": "..."}}`.
#[derive(Debug, Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
struct ErrorDetail {
    #[serde(default)]
    code: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

/// Turns a non-success response into the matching `LlmError`. The status is
/// not enough on its own: OpenAI answers an empty account with 429, like a rate
/// limit, and only the error code tells them apart.
async fn error_from_response(resp: Response, model: &str) -> LlmError {
    let status = resp.status();
    let retry_after_ms = retry_after_ms(resp.headers(), DEFAULT_RETRY_AFTER_MS);
    let body = resp.text().await.unwrap_or_default();
    let code = serde_json::from_str::<ErrorBody>(&body)
        .ok()
        .and_then(|parsed| parsed.error.code.or(parsed.error.kind))
        .unwrap_or_default();

    match (status, code.as_str()) {
        (_, "insufficient_quota") => LlmError::QuotaExceeded(body),
        (_, "model_not_found") | (StatusCode::NOT_FOUND, _) => {
            LlmError::ModelNotFound(model.to_string())
        }
        (StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, _) => LlmError::Auth(body),
        (StatusCode::TOO_MANY_REQUESTS, _) => LlmError::RateLimited { retry_after_ms },
        (s, _) => LlmError::Api {
            status: s.as_u16(),
            message: body,
        },
    }
}
