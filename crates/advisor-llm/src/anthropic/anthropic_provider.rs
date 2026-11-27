use url::Url;

use crate::anthropic::anthropic_config::AnthropicConfig;
use crate::anthropic::wire::{MessageRequest, MessageResponse, ModelInfo};
use crate::http::{model_url, retry_after_ms};
use crate::types::{ChatRequest, ChatResponse};
use crate::{llm_error::LlmError, llm_provider::LlmProvider};
use advisor_core::{HealthCheck, HealthError, Secret};
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder, Response, StatusCode};

/// Wait used when a 429 carries no `retry-after` header.
const DEFAULT_RETRY_AFTER_MS: u64 = 1000;

#[derive(Clone, Debug)]
pub struct AnthropicProvider {
    name: String,
    model: String,
    api_key: Secret,
    version: String,
    api_url: Url,
    models_url: Url,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Result<Self, LlmError> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| LlmError::Network(e.to_string()))?;

        Ok(Self {
            name: "Anthropic".to_string(),
            model: config.model,
            models_url: config.models_url,
            api_key: config.api_key,
            version: config.version,
            api_url: config.url,
            client,
        })
    }

    /// The headers every Anthropic request needs.
    fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.version)
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let message_request = MessageRequest::from_chat(req, &self.model);

        let resp = self
            .authorized(self.client.post(self.api_url.clone()))
            .json(&message_request)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(error_from_response(resp, &self.model).await);
        }

        let parsed: MessageResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;
        Ok(ChatResponse::from(parsed))
    }
}

/// Checks that the key is valid **and** that the configured model exists, by
/// asking for that one model (`GET /v1/models/{model}`). Asking for the model
/// directly avoids paging through the model list.
#[async_trait]
impl HealthCheck for AnthropicProvider {
    fn service_name(&self) -> &'static str {
        "anthropic_provider"
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
        Ok(match model.display_name {
            Some(display_name) => format!("model {} ({display_name}) available", model.id),
            None => format!("model {} available", model.id),
        })
    }
}

/// Turns a non-success response into the matching `LlmError`.
async fn error_from_response(resp: Response, model: &str) -> LlmError {
    let status = resp.status();
    let retry_after_ms = retry_after_ms(resp.headers(), DEFAULT_RETRY_AFTER_MS);
    let body = resp.text().await.unwrap_or_default();
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::Auth(body),
        StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimited { retry_after_ms },
        StatusCode::NOT_FOUND => LlmError::ModelNotFound(model.to_string()),
        s => LlmError::Api {
            status: s.as_u16(),
            message: body,
        },
    }
}
