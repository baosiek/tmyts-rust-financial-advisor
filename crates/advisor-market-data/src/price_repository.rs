use async_trait::async_trait;
use chrono::NaiveDate;

use crate::{Asset, MarketDataError, PriceBar};

/// Read access to assets and prices. The Postgres implementation is
/// `PgPriceRepository`; tests use an in-memory one.
#[async_trait]
pub trait PriceRepository: Send + Sync {
    /// All assets with this ticker symbol (one per venue). Matching ignores case.
    async fn find_assets(&self, symbol: &str) -> Result<Vec<Asset>, MarketDataError>;

    /// Daily bars for one asset between `from` and `to` (both inclusive), oldest first.
    async fn price_history(
        &self,
        asset_id: i32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PriceBar>, MarketDataError>;
}
