//! Logging setup, adapted from `advisor-telemetry` in tmyts-fin-advisor.

use thiserror::Error;
use tracing_subscriber::filter::ParseError;
use tracing_subscriber::util::TryInitError;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Error)]
pub enum ObservabilityError {
    #[error("invalid log filter: {0}")]
    Filter(#[from] ParseError),

    #[error("logging is already initialized: {0}")]
    Init(#[from] TryInitError),
}

/// Starts logging. `RUST_LOG`, when set, overrides `default_filter`
/// (which comes from the settings file), e.g. `RUST_LOG=debug cargo run`.
pub fn init_tracing(default_filter: &str) -> Result<(), ObservabilityError> {
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(_) => EnvFilter::try_new(default_filter)?,
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer())
        .try_init()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_filter_is_reported() {
        // Only valid when RUST_LOG is unset, which is the case under `cargo test`
        // unless you set it yourself.
        if std::env::var("RUST_LOG").is_err() {
            assert!(matches!(
                init_tracing("info,=["),
                Err(ObservabilityError::Filter(_))
            ));
        }
    }
}
