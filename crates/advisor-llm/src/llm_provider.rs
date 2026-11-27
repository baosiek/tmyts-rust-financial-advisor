use crate::LlmError;
use crate::types::{ChatRequest, ChatResponse};
use advisor_core::HealthCheck;

#[async_trait::async_trait]
/// Every provider is also a `HealthCheck`, so startup can verify any of them
/// through the same `Arc<dyn LlmProvider>` it hands to the agent.
pub trait LlmProvider: HealthCheck + Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}
