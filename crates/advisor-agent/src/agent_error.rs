use advisor_llm::LlmError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error(transparent)]
    Llm(#[from] LlmError),

    #[error("no final answer after {0} tool rounds")]
    TooManyToolRounds(u32),
}
