//! A service the application depends on (database, vector store, ...) that can
//! be checked at startup. Plays the role of `TClient::health` in tmyts-fin-advisor.

use async_trait::async_trait;

/// Why a health check failed, in words an operator can act on.
pub type HealthError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Short name for logs, e.g. "postgres".
    fn service_name(&self) -> &'static str;

    /// Checks that the service answers. Returns a short detail on success.
    async fn check(&self) -> Result<String, HealthError>;
}
