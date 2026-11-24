//! Market data: assets and their daily price history, read from Postgres.
//! Everything above this crate depends on the `PriceRepository` trait, not on Postgres.

mod asset;
mod market_data_error;
mod postgres;
mod price_bar;
mod price_repository;
mod price_summary;

pub use asset::Asset;
pub use market_data_error::MarketDataError;
pub use postgres::{PgPriceRepository, PostgresConfig};
pub use price_bar::PriceBar;
pub use price_repository::PriceRepository;
pub use price_summary::PriceSummary;
