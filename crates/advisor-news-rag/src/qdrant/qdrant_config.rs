use std::time::Duration;

use advisor_core::Secret;
use url::Url;

#[derive(Debug, Clone)]
pub struct QdrantConfig {
    /// gRPC endpoint, usually port 6334, e.g. http://localhost:6334
    pub url: Url,
    /// `None` when Qdrant runs without an API key.
    pub api_key: Option<Secret>,
    pub collection: String,
    pub timeout: Duration,
    /// Expected vector size; checked against the collection at startup.
    pub vector_size: u64,
}
