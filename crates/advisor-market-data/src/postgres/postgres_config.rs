use std::time::Duration;

use advisor_core::Secret;

/// Connection settings. The composition root fills it from the settings file
/// (everything) and the environment (the password).
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Secret,
    pub max_connections: u32,
    /// How long to wait for a free connection from the pool.
    pub acquire_timeout: Duration,
}
