/// One row of `public.assets`: a ticker listed on one venue.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Asset {
    pub asset_id: i32,
    /// The ticker symbol (column `asset`), e.g. "AAPL".
    #[sqlx(rename = "asset")]
    pub symbol: String,
    #[sqlx(rename = "asset_name")]
    pub name: String,
    #[sqlx(rename = "asset_type")]
    pub kind: String,
    pub venue: String,
}
