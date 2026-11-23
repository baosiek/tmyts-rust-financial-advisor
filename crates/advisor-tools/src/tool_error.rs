use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("unknown tool `{0}`")]
    Unknown(String),

    #[error("invalid input for tool `{tool}`: {reason}")]
    InvalidInput { tool: String, reason: String },

    #[error("tool `{tool}` failed: {reason}")]
    Failed { tool: String, reason: String },
}
