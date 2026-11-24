use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("embedding service unreachable: {0}")]
    Network(String),

    #[error("embedding service returned HTTP {status}: {message}")]
    Api { status: u16, message: String },

    #[error("could not parse embedding response: {0}")]
    Parse(String),

    #[error("expected {expected} vectors of size {dimensions}, got {got}")]
    Shape {
        expected: usize,
        dimensions: usize,
        got: String,
    },
}
