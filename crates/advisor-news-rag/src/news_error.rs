use thiserror::Error;

#[derive(Debug, Error)]
pub enum NewsError {
    #[error("vector database error: {0}")]
    Database(String),
}
