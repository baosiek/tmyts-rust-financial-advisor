use std::sync::Arc;

use async_trait::async_trait;
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use advisor_market_data::{Asset, PriceRepository, PriceSummary};

use crate::{Tool, ToolError};

/// Summarizes an asset's daily prices over a period, read from the trading
/// database through a `PriceRepository` (injected, so tests need no database).
pub struct PriceHistoryTool {
    repository: Arc<dyn PriceRepository>,
}

impl PriceHistoryTool {
    pub fn new(repository: Arc<dyn PriceRepository>) -> Self {
        Self { repository }
    }
}

// Doc comments below become the schema descriptions the model reads.

/// Which asset and period to summarize.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PriceHistoryInput {
    /// Ticker symbol, e.g. "AAPL".
    pub ticker: String,
    /// Exchange or venue, e.g. "NASDAQ". Only needed when the ticker is listed on several venues.
    pub venue: Option<String>,
    /// First day of the period, inclusive (YYYY-MM-DD).
    pub from: NaiveDate,
    /// Last day of the period, inclusive (YYYY-MM-DD).
    pub to: NaiveDate,
}

/// What the tool returns to the model, as JSON.
#[derive(Debug, Serialize)]
struct PriceHistoryOutput<'a> {
    ticker: &'a str,
    name: &'a str,
    venue: &'a str,
    requested_from: NaiveDate,
    requested_to: NaiveDate,
    /// `None` when the database has no prices in the period.
    summary: Option<PriceSummary>,
}

#[async_trait]
impl Tool for PriceHistoryTool {
    const NAME: &'static str = "get_price_history";
    const DESCRIPTION: &'static str = "Summarizes an asset's daily prices between two dates: \
        first and last close, percentage change, high, low, average volume and number of \
        trading days. Comparisons use adjusted prices (splits and dividends accounted for); \
        `last_close` is the unadjusted price as quoted. Use it for questions about how an \
        asset performed over a period.";
    type Input = PriceHistoryInput;

    async fn run(&self, input: PriceHistoryInput) -> Result<String, ToolError> {
        if input.from > input.to {
            return Err(invalid(format!(
                "`from` ({}) is after `to` ({})",
                input.from, input.to
            )));
        }

        let asset = self
            .resolve_asset(&input.ticker, input.venue.as_deref())
            .await?;

        let bars = self
            .repository
            .price_history(asset.asset_id, input.from, input.to)
            .await
            .map_err(failed)?;

        let output = PriceHistoryOutput {
            ticker: &asset.symbol,
            name: &asset.name,
            venue: &asset.venue,
            requested_from: input.from,
            requested_to: input.to,
            summary: PriceSummary::from_bars(&bars),
        };
        serde_json::to_string(&output).map_err(|e| failed(e.to_string()))
    }
}

impl PriceHistoryTool {
    /// Finds the one asset meant by `ticker` (and `venue`, when given). An unknown
    /// or ambiguous ticker is an input error, so the model can correct its call.
    async fn resolve_asset(&self, ticker: &str, venue: Option<&str>) -> Result<Asset, ToolError> {
        let mut assets = self.repository.find_assets(ticker).await.map_err(failed)?;

        if let Some(venue) = venue {
            assets.retain(|asset| asset.venue.eq_ignore_ascii_case(venue));
        }

        match assets.len() {
            1 => Ok(assets.remove(0)),
            0 => Err(invalid(match venue {
                Some(venue) => format!("no asset `{ticker}` on venue `{venue}`"),
                None => format!("unknown ticker `{ticker}`"),
            })),
            _ => {
                let venues: Vec<&str> = assets.iter().map(|asset| asset.venue.as_str()).collect();
                Err(invalid(format!(
                    "ticker `{ticker}` is listed on several venues ({}); call again with `venue`",
                    venues.join(", ")
                )))
            }
        }
    }
}

fn invalid(reason: String) -> ToolError {
    ToolError::InvalidInput {
        tool: PriceHistoryTool::NAME.to_string(),
        reason,
    }
}

fn failed(error: impl ToString) -> ToolError {
    ToolError::Failed {
        tool: PriceHistoryTool::NAME.to_string(),
        reason: error.to_string(),
    }
}
