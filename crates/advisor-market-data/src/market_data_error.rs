use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarketDataError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}
