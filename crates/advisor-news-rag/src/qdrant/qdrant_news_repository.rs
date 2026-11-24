use advisor_core::{HealthCheck, HealthError};
use async_trait::async_trait;
use qdrant_client::qdrant::QueryPointsBuilder;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::vectors_config::Config as VectorsKind;
use qdrant_client::{Payload, Qdrant};

use super::news_payload::NewsPayload;
use super::qdrant_filter::build_filter;
use crate::{NewsArticle, NewsError, NewsQuery, NewsRepository, QdrantConfig};

/// `NewsRepository` backed by your existing Qdrant news collection.
pub struct QdrantNewsRepository {
    client: Qdrant,
    config: QdrantConfig,
}

impl QdrantNewsRepository {
    /// Creates the client. Nothing connects until the first call.
    pub fn new(config: QdrantConfig) -> Result<Self, NewsError> {
        // The client's own version check prints to stderr when the server is down;
        // the startup health check reports that case through the log instead.
        let mut builder = Qdrant::from_url(config.url.as_str())
            .timeout(config.timeout)
            .skip_compatibility_check();
        if let Some(api_key) = &config.api_key {
            builder = builder.api_key(api_key.expose());
        }
        let client = builder.build().map_err(db)?;
        Ok(Self { client, config })
    }
}

#[async_trait]
impl NewsRepository for QdrantNewsRepository {
    async fn search(&self, query: &NewsQuery) -> Result<Vec<NewsArticle>, NewsError> {
        let mut request = QueryPointsBuilder::new(&self.config.collection)
            .query(query.vector.clone())
            .limit(query.limit)
            .with_payload(true);
        if let Some(filter) = build_filter(query) {
            request = request.filter(filter);
        }

        let response = self.client.query(request).await.map_err(db)?;

        let mut articles = Vec::with_capacity(response.result.len());
        for point in response.result {
            let id = match point.id.and_then(|id| id.point_id_options) {
                Some(PointIdOptions::Uuid(uuid)) => uuid,
                Some(PointIdOptions::Num(num)) => num.to_string(),
                None => String::new(),
            };
            let json = serde_json::Value::from(Payload::from(point.payload));
            // A point whose payload does not fit (e.g. no published_at) is skipped,
            // not fatal: one bad record must not break every search.
            match serde_json::from_value::<NewsPayload>(json) {
                Ok(payload) => articles.push(payload.into_article(id, point.score)),
                Err(e) => {
                    tracing::warn!(point = %id, error = %e, "skipping news point with unexpected payload")
                }
            }
        }
        Ok(articles)
    }
}

#[async_trait]
impl HealthCheck for QdrantNewsRepository {
    fn service_name(&self) -> &'static str {
        "qdrant"
    }

    async fn check(&self) -> Result<String, HealthError> {
        let collection = &self.config.collection;
        if !self.client.collection_exists(collection).await? {
            return Err(format!("collection `{collection}` does not exist").into());
        }

        let info = self
            .client
            .collection_info(collection)
            .await?
            .result
            .ok_or("collection_info returned no result")?;

        let points = info.points_count.unwrap_or(0);
        let sizes: Vec<u64> = info
            .config
            .and_then(|config| config.params)
            .and_then(|params| params.vectors_config)
            .and_then(|vectors| vectors.config)
            .map(|kind| match kind {
                VectorsKind::Params(params) => vec![params.size],
                VectorsKind::ParamsMap(map) => map.map.values().map(|params| params.size).collect(),
            })
            .unwrap_or_default();

        // The embedding service and the collection must agree, or every search fails.
        if sizes != [self.config.vector_size] {
            return Err(format!(
                "collection `{collection}` has vector size {sizes:?}, but the settings expect {}",
                self.config.vector_size
            )
            .into());
        }
        Ok(format!(
            "collection `{collection}`: {points} points, vector size {}",
            self.config.vector_size
        ))
    }
}

fn db(error: impl ToString) -> NewsError {
    NewsError::Database(error.to_string())
}
