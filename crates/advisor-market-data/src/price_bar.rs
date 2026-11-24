use chrono::NaiveDate;
use rust_decimal::Decimal;

/// One day of `public.assets_price_history`. Prices are `numeric` in Postgres,
/// so they are `Decimal` here: exact, unlike `f64`.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PriceBar {
    #[sqlx(rename = "price_date")]
    pub date: NaiveDate,
    #[sqlx(rename = "price_open")]
    pub open: Decimal,
    #[sqlx(rename = "price_high")]
    pub high: Decimal,
    #[sqlx(rename = "price_low")]
    pub low: Decimal,
    #[sqlx(rename = "price_close")]
    pub close: Decimal,
    #[sqlx(rename = "adj_price_open")]
    pub adj_open: Decimal,
    #[sqlx(rename = "adj_price_high")]
    pub adj_high: Decimal,
    #[sqlx(rename = "adj_price_low")]
    pub adj_low: Decimal,
    #[sqlx(rename = "adj_price_close")]
    pub adj_close: Decimal,
    pub volume: i64,
}
