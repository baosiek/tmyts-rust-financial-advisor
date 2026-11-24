use std::sync::Arc;

use advisor_llm::LlmProvider;
use advisor_llm::types::{ChatRequest, ContentBlock, Message, StopReason};
use advisor_tools::ToolRegistry;

use crate::{AgentConfig, AgentError};

/// The agent receives its provider and its tools from outside (dependency
/// injection), so it works with any `LlmProvider` and any set of tools.
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    config: AgentConfig,
    history: Vec<Message>,
}

impl Agent {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            config,
            history: Vec::new(),
        }
    }

    /// Answers a question. When the model asks for tools, the agent runs them,
    /// sends the results back and asks again, until the model gives a final answer.
    pub async fn ask(&mut self, question: impl Into<String>) -> Result<String, AgentError> {
        // If anything fails, the history goes back to how it was before the question,
        // so the conversation never ends up with a half-finished exchange.
        let history_before = self.history.len();
        self.history.push(Message::user(question));

        match self.run_tool_loop().await {
            Ok(answer) => Ok(answer),
            Err(e) => {
                self.history.truncate(history_before);
                Err(e)
            }
        }
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    async fn run_tool_loop(&mut self) -> Result<String, AgentError> {
        for round in 1..=self.config.max_tool_rounds {
            let request = self.request();
            let response = self.provider.complete(&request).await?;

            // The assistant's turn is kept exactly as produced, tool calls included:
            // the next request must contain them, or the tool results make no sense.
            self.history
                .push(Message::assistant_blocks(response.content.clone()));

            match response.stop_reason {
                StopReason::ToolUse => {}
                // A cut-off answer is incomplete (and may be empty, e.g. when the cut
                // happened inside a tool call). Never present it as the final answer.
                StopReason::MaxTokens => {
                    return Err(AgentError::MaxTokensReached {
                        max_tokens: self.config.max_tokens,
                    });
                }
                _ => return Ok(response.text()),
            }

            let mut results = Vec::new();
            for call in response.tool_calls() {
                let (content, is_error) = match self.tools.call(&call.name, call.input).await {
                    Ok(output) => (output, false),
                    // A failing tool is reported to the model, which can often recover
                    // (e.g. by fixing its input). It does not abort the question.
                    Err(e) => (e.to_string(), true),
                };
                tracing::info!(round, tool = %call.name, is_error, "tool call");
                results.push(ContentBlock::ToolResult {
                    tool_use_id: call.id,
                    content,
                    is_error,
                });
            }
            self.history.push(Message::tool_results(results));
        }

        Err(AgentError::TooManyToolRounds(self.config.max_tool_rounds))
    }

    fn request(&self) -> ChatRequest {
        ChatRequest {
            model: None,
            system: Some(self.config.system_prompt.clone()),
            messages: self.history.clone(),
            max_tokens: self.config.max_tokens,
            temperature: None,
            tools: self.tools.definitions(),
        }
    }
}
