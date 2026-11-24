use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::PriceBar;

/// A compact view of a price history: what the model needs to talk about a
/// period, without sending every daily row (each row costs tokens).
/// Uses adjusted prices (splits and dividends accounted for) for comparisons.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriceSummary {
    pub first_date: NaiveDate,
    pub last_date: NaiveDate,
    pub trading_days: usize,
    pub first_adj_close: Decimal,
    pub last_adj_close: Decimal,
    /// Percentage change from the first to the last adjusted close, 2 decimals.
    pub change_pct: Decimal,
    pub period_adj_high: Decimal,
    pub period_adj_low: Decimal,
    /// The last unadjusted close, i.e. the price as it was actually quoted.
    pub last_close: Decimal,
    pub average_volume: i64,
}

impl PriceSummary {
    /// Summarizes bars sorted oldest first. Returns `None` when there are no bars.
    pub fn from_bars(bars: &[PriceBar]) -> Option<Self> {
        let first = bars.first()?;
        let last = bars.last()?;

        let change_pct = if first.adj_close.is_zero() {
            Decimal::ZERO
        } else {
            ((last.adj_close - first.adj_close) / first.adj_close * Decimal::ONE_HUNDRED)
                .round_dp(2)
        };
        let total_volume: i64 = bars.iter().map(|bar| bar.volume).sum();

        Some(Self {
            first_date: first.date,
            last_date: last.date,
            trading_days: bars.len(),
            first_adj_close: first.adj_close,
            last_adj_close: last.adj_close,
            change_pct,
            period_adj_high: bars.iter().map(|bar| bar.adj_high).max()?,
            period_adj_low: bars.iter().map(|bar| bar.adj_low).min()?,
            last_close: last.close,
            average_volume: total_volume / bars.len() as i64,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn bar(
        day: u32,
        adj_close: Decimal,
        adj_high: Decimal,
        adj_low: Decimal,
        volume: i64,
    ) -> PriceBar {
        PriceBar {
            date: NaiveDate::from_ymd_opt(2026, 11, day).unwrap(),
            open: adj_close,
            high: adj_high,
            low: adj_low,
            close: adj_close * dec!(2), // pretend a 2:1 split happened later
            adj_open: adj_close,
            adj_high,
            adj_low,
            adj_close,
            volume,
        }
    }

    #[test]
    fn summary_uses_adjusted_prices() {
        let bars = vec![
            bar(2, dec!(100), dec!(101), dec!(99), 1_000),
            bar(3, dec!(120), dec!(125), dec!(98), 3_000),
            bar(4, dec!(110), dec!(115), dec!(105), 2_000),
        ];
        let summary = PriceSummary::from_bars(&bars).unwrap();

        assert_eq!(summary.trading_days, 3);
        assert_eq!(summary.first_adj_close, dec!(100));
        assert_eq!(summary.last_adj_close, dec!(110));
        assert_eq!(summary.change_pct, dec!(10.00));
        assert_eq!(summary.period_adj_high, dec!(125));
        assert_eq!(summary.period_adj_low, dec!(98));
        assert_eq!(summary.last_close, dec!(220));
        assert_eq!(summary.average_volume, 2_000);
    }

    #[test]
    fn no_bars_means_no_summary() {
        assert!(PriceSummary::from_bars(&[]).is_none());
    }
}
