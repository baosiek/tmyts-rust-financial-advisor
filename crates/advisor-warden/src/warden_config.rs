use thiserror::Error;

/// Score thresholds. Built by the composition root from the settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WardenConfig {
    sanitize_at: f64,
    block_at: f64,
}

#[derive(Debug, Error, PartialEq)]
#[error(
    "invalid warden thresholds: need 0 < sanitize_at ({sanitize_at}) <= block_at ({block_at}) <= 1"
)]
pub struct WardenConfigError {
    pub sanitize_at: f64,
    pub block_at: f64,
}

impl WardenConfig {
    pub fn new(sanitize_at: f64, block_at: f64) -> Result<Self, WardenConfigError> {
        if 0.0 < sanitize_at && sanitize_at <= block_at && block_at <= 1.0 {
            Ok(Self {
                sanitize_at,
                block_at,
            })
        } else {
            Err(WardenConfigError {
                sanitize_at,
                block_at,
            })
        }
    }

    pub fn sanitize_at(&self) -> f64 {
        self.sanitize_at
    }

    pub fn block_at(&self) -> f64 {
        self.block_at
    }
}
