use std::sync::Arc;

use advisor_llm::types::{ChatRequest, Message};
use advisor_llm::{LlmError, LlmProvider};

/// The agent receives its provider from outside (dependency injection), so it
/// works with any `LlmProvider`: Anthropic, another vendor, or a mock in tests.
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    system_prompt: String,
    max_tokens: u32,
    history: Vec<Message>,
}

impl Agent {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        system_prompt: impl Into<String>,
        max_tokens: u32,
    ) -> Self {
        Self {
            provider,
            system_prompt: system_prompt.into(),
            max_tokens,
            history: Vec::new(),
        }
    }

    /// Sends the question with the whole conversation so far, and remembers the answer.
    pub async fn ask(&mut self, question: impl Into<String>) -> Result<String, LlmError> {
        self.history.push(Message::user(question));

        let req = ChatRequest {
            model: None,
            system: Some(self.system_prompt.clone()),
            messages: self.history.clone(),
            max_tokens: self.max_tokens,
            temperature: None,
        };

        match self.provider.complete(&req).await {
            Ok(resp) => {
                self.history.push(Message::assistant(resp.text.clone()));
                Ok(resp.text)
            }
            Err(e) => {
                // Keep roles alternating: drop the question that got no answer.
                self.history.pop();
                Err(e)
            }
        }
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }
}
