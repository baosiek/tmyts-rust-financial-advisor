//! Client for the embedding service used by tmyts-fin-advisor (`embeddings.rs`):
//! `POST {"text": [...]}` returns `{"embedding": [[...], ...]}`.

use std::time::Duration;

use advisor_core::{HealthCheck, HealthError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{EmbedError, Embedder};

#[derive(Debug, Clone)]
pub struct HttpEmbedderConfig {
    /// Full endpoint, e.g. http://localhost:8080/embed
    pub url: Url,
    pub timeout: Duration,
    /// Expected vector size; must match the Qdrant collection.
    pub dimensions: usize,
}

pub struct HttpEmbedder {
    http: reqwest::Client,
    config: HttpEmbedderConfig,
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    text: &'a [String],
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<Vec<f32>>,
}

impl HttpEmbedder {
    pub fn new(config: HttpEmbedderConfig) -> Result<Self, EmbedError> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| EmbedError::Network(e.to_string()))?;
        Ok(Self { http, config })
    }
}

#[async_trait]
impl Embedder for HttpEmbedder {
    fn dimensions(&self) -> usize {
        self.config.dimensions
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let resp = self
            .http
            .post(self.config.url.clone())
            .json(&EmbedRequest { text: texts })
            .send()
            .await
            .map_err(|e| EmbedError::Network(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let message = resp.text().await.unwrap_or_default();
            return Err(EmbedError::Api {
                status: status.as_u16(),
                message,
            });
        }

        let parsed: EmbedResponse = resp
            .json()
            .await
            .map_err(|e| EmbedError::Parse(e.to_string()))?;

        // A wrong count or size would make Qdrant reject the search, or worse,
        // silently compare vectors from different models. Fail loudly instead.
        let sizes_ok = parsed
            .embedding
            .iter()
            .all(|vector| vector.len() == self.config.dimensions);
        if parsed.embedding.len() != texts.len() || !sizes_ok {
            let sizes: Vec<usize> = parsed.embedding.iter().map(Vec::len).collect();
            return Err(EmbedError::Shape {
                expected: texts.len(),
                dimensions: self.config.dimensions,
                got: format!("{} vectors of sizes {sizes:?}", parsed.embedding.len()),
            });
        }
        Ok(parsed.embedding)
    }
}

#[async_trait]
impl HealthCheck for HttpEmbedder {
    fn service_name(&self) -> &'static str {
        "embeddings"
    }

    async fn check(&self) -> Result<String, HealthError> {
        self.embed(&["health check".to_string()]).await?;
        Ok(format!("{} dimensions", self.config.dimensions))
    }
}
