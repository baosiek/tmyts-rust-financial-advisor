use crate::LlmError;
use crate::types::{ChatRequest, ChatResponse};

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, LlmError>;
}
