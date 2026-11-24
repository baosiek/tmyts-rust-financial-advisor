//! PriceHistoryTool tested with an in-memory repository: no database needed.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::{Value, json};

use advisor_market_data::{Asset, MarketDataError, PriceBar, PriceRepository};
use advisor_tools::{PriceHistoryTool, ToolError, ToolRegistry};

struct InMemoryRepository {
    assets: Vec<Asset>,
    bars: Vec<(i32, PriceBar)>,
}

#[async_trait]
impl PriceRepository for InMemoryRepository {
    async fn find_assets(&self, symbol: &str) -> Result<Vec<Asset>, MarketDataError> {
        Ok(self
            .assets
            .iter()
            .filter(|asset| asset.symbol.eq_ignore_ascii_case(symbol))
            .cloned()
            .collect())
    }

    async fn price_history(
        &self,
        asset_id: i32,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PriceBar>, MarketDataError> {
        Ok(self
            .bars
            .iter()
            .filter(|(id, bar)| *id == asset_id && bar.date >= from && bar.date <= to)
            .map(|(_, bar)| bar.clone())
            .collect())
    }
}

fn asset(asset_id: i32, symbol: &str, venue: &str) -> Asset {
    Asset {
        asset_id,
        symbol: symbol.to_string(),
        name: format!("{symbol} Inc."),
        kind: "stock".to_string(),
        venue: venue.to_string(),
    }
}

fn bar(day: u32, close: i64) -> PriceBar {
    let price = Decimal::from(close);
    PriceBar {
        date: NaiveDate::from_ymd_opt(2026, 11, day).unwrap(),
        open: price,
        high: price,
        low: price,
        close: price,
        adj_open: price,
        adj_high: price,
        adj_low: price,
        adj_close: price,
        volume: 1_000,
    }
}

fn registry() -> ToolRegistry {
    let repository = InMemoryRepository {
        assets: vec![
            asset(1, "AAPL", "NASDAQ"),
            asset(2, "SHOP", "NYSE"),
            asset(3, "SHOP", "TSX"),
        ],
        bars: vec![(1, bar(2, 100)), (1, bar(3, 105)), (1, bar(4, 110))],
    };
    let mut registry = ToolRegistry::new();
    registry.register(PriceHistoryTool::new(Arc::new(repository)));
    registry
}

async fn call(input: Value) -> Result<Value, ToolError> {
    let output = registry().call("get_price_history", input).await?;
    Ok(serde_json::from_str(&output).expect("the tool returns JSON"))
}

#[tokio::test]
async fn summarizes_the_requested_period() {
    let output = call(json!({ "ticker": "aapl", "from": "2026-11-01", "to": "2026-11-30" }))
        .await
        .unwrap();

    assert_eq!(output["ticker"], "AAPL");
    assert_eq!(output["venue"], "NASDAQ");
    assert_eq!(output["summary"]["trading_days"], 3);
    assert_eq!(output["summary"]["change_pct"], "10.00");
}

#[tokio::test]
async fn period_without_prices_has_no_summary() {
    let output = call(json!({ "ticker": "AAPL", "from": "2025-01-01", "to": "2025-01-31" }))
        .await
        .unwrap();
    assert!(output["summary"].is_null());
}

#[tokio::test]
async fn ambiguous_ticker_lists_the_venues() {
    let err = call(json!({ "ticker": "SHOP", "from": "2026-11-01", "to": "2026-11-30" }))
        .await
        .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("NYSE") && message.contains("TSX"),
        "got {message}"
    );
}

#[tokio::test]
async fn venue_resolves_an_ambiguous_ticker() {
    let output =
        call(json!({ "ticker": "SHOP", "venue": "tsx", "from": "2026-11-01", "to": "2026-11-30" }))
            .await
            .unwrap();
    assert_eq!(output["venue"], "TSX");
}

#[tokio::test]
async fn unknown_ticker_is_an_input_error() {
    let err = call(json!({ "ticker": "NOPE", "from": "2026-11-01", "to": "2026-11-30" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "got {err:?}");
}

#[tokio::test]
async fn reversed_dates_are_an_input_error() {
    let err = call(json!({ "ticker": "AAPL", "from": "2026-11-30", "to": "2026-11-01" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "got {err:?}");
}

#[tokio::test]
async fn invalid_date_format_is_rejected_before_running() {
    let err = call(json!({ "ticker": "AAPL", "from": "last week", "to": "2026-11-30" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "got {err:?}");
}

#[test]
fn schema_describes_dates_and_optional_venue() {
    let definitions = registry().definitions();
    let schema = &definitions[0].input_schema;
    assert_eq!(schema["properties"]["from"]["format"], "date");
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        required.contains(&"ticker") && !required.contains(&"venue"),
        "{schema}"
    );
}
