use url::Url;

use crate::anthropic::anthropic_config::AnthropicConfig;
use crate::anthropic::wire::Models;
use crate::types::{ChatRequest, ChatResponse};
use crate::{
    anthropic::wire::MessageRequest, anthropic::wire::MessageResponse, llm_error::LlmError,
    llm_provider::LlmProvider,
};
use advisor_core::{HealthCheck, HealthError, Secret};
use async_trait::async_trait;
use reqwest::{Client, StatusCode};

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
            .client
            .post(self.api_url.clone())
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.version)
            .json(&message_request)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = resp.status();

        if !status.is_success() {
            let retry_after_ms = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|s| s * 1000)
                .unwrap_or(1000);
            let body = resp.text().await.unwrap_or_default();
            return Err(match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::Auth(body),
                StatusCode::TOO_MANY_REQUESTS => LlmError::RateLimited { retry_after_ms },
                s => LlmError::Api {
                    status: s.as_u16(),
                    message: body,
                },
            });
        }

        let parsed: MessageResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;
        Ok(ChatResponse::from(parsed))
    }
}

#[async_trait]
impl HealthCheck for AnthropicProvider {
    fn service_name(&self) -> &'static str {
        "anthropic_provider"
    }

    async fn check(&self) -> Result<String, HealthError> {
        let resp = self
            .client
            .get(self.models_url.clone())
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.version)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let status = resp.status();

        let body = resp.text().await.unwrap_or_default();

        if status.is_success() {
            let models: Models = serde_json::from_str(&body)?;
            check_for_model(&models, &self.model)?;
            return Ok(format!("{} reachable", self.model));
        }

        let err = match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::Auth(body),
            s => LlmError::Api {
                status: s.as_u16(),
                message: body,
            },
        };
        Err(err.into())
    }
}

fn check_for_model(models: &Models, model_to_be_used: &str) -> Result<(), LlmError> {
    if models.data.iter().any(|m| m.id == model_to_be_used) {
        Ok(())
    } else {
        Err(LlmError::ModelNotFound(model_to_be_used.to_string()))
    }
}
