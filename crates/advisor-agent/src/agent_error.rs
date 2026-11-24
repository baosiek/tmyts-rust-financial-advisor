use advisor_llm::LlmError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error(transparent)]
    Llm(#[from] LlmError),

    #[error("no final answer after {0} tool rounds")]
    TooManyToolRounds(u32),

    #[error(
        "the answer was cut off at max_tokens ({max_tokens}); raise [agent] max_tokens in the settings file"
    )]
    MaxTokensReached { max_tokens: u32 },
}
