use advisor_core::{HealthCheck, HealthError};
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::{Asset, MarketDataError, PostgresConfig, PriceBar, PriceRepository};

/// `PriceRepository` backed by your trading app's Postgres tables
/// `public.assets` and `public.assets_price_history`.
#[derive(Clone)]
pub struct PgPriceRepository {
    pool: PgPool,
}

impl PgPriceRepository {
    /// Creates the connection pool. Lazy: nothing connects until the first
    /// query, so startup does not fail here; the health check connects.
    pub fn new(config: &PostgresConfig) -> Self {
        let options = PgConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .database(&config.database)
            .username(&config.username)
            .password(config.password.expose());

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.acquire_timeout)
            .connect_lazy_with(options);

        Self { pool }
    }
}

#[async_trait]
impl PriceRepository for PgPriceRepository {
    async fn find_assets(&self, symbol: &str) -> Result<Vec<Asset>, MarketDataError> {
        let assets = sqlx::query_as::<_, Asset>(
            "SELECT asset_id, asset, asset_name, asset_type, venue
               FROM public.assets
              WHERE upper(asset) = upper($1)
              ORDER BY venue",
        )
        .bind(symbol)
        .fetch_all(&self.pool)
        .await?;
        Ok(assets)
    }

    async fn price_history(
        &self,
        asset_id: i32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PriceBar>, MarketDataError> {
        let bars = sqlx::query_as::<_, PriceBar>(
            "SELECT price_date, price_open, price_high, price_low, price_close,
                    adj_price_open, adj_price_high, adj_price_low, adj_price_close, volume
               FROM public.assets_price_history
              WHERE asset_id = $1 AND price_date BETWEEN $2 AND $3
              ORDER BY price_date",
        )
        .bind(asset_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        Ok(bars)
    }
}

#[async_trait]
impl HealthCheck for PgPriceRepository {
    fn service_name(&self) -> &'static str {
        "postgres"
    }

    async fn check(&self) -> Result<String, HealthError> {
        let (version,): (String,) = sqlx::query_as("SELECT version()")
            .fetch_one(&self.pool)
            .await?;
        // "PostgreSQL 16.4 on x86_64-pc-linux-gnu, ..." -> "PostgreSQL 16.4"
        let short = version.split(" on ").next().unwrap_or(&version).to_string();
        Ok(short)
    }
}
