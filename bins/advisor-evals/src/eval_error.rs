use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid eval cases file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },

    #[error("invalid eval cases file {path}: {reason}")]
    Invalid { path: PathBuf, reason: String },

    #[error("cannot write report {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}
