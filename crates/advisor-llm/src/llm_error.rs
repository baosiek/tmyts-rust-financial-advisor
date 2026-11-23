use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("rate limited, retry after {retry_after_ms} ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("could not parse response: {0}")]
    Parse(String),

    #[error("no response within {after_ms} ms")]
    Timeout { after_ms: u64 },
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::RateLimited { .. } | LlmError::Network(_) | LlmError::Timeout { .. } => true,
            LlmError::Api { status, .. } => matches!(status, 408 | 500 | 502..=504 | 529),
            LlmError::Auth(_) | LlmError::Parse(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_decisions() {
        assert!(
            LlmError::RateLimited {
                retry_after_ms: 1000
            }
            .is_retryable()
        );
        assert!(
            LlmError::Api {
                status: 529,
                message: "overloaded".into()
            }
            .is_retryable()
        );
        assert!(
            !LlmError::Api {
                status: 400,
                message: "bad request".into()
            }
            .is_retryable()
        );
        assert!(!LlmError::Auth("invalid API key".into()).is_retryable());
    }
}
