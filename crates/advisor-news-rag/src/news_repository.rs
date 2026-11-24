use async_trait::async_trait;

use crate::{NewsArticle, NewsError, NewsQuery};

/// Searches stored news. The Qdrant implementation is `QdrantNewsRepository`;
/// tests use an in-memory one.
#[async_trait]
pub trait NewsRepository: Send + Sync {
    /// The most similar articles first.
    async fn search(&self, query: &NewsQuery) -> Result<Vec<NewsArticle>, NewsError>;
}
