//! Runs the real queries against a real Postgres. Ignored by default.
//! Point it at a database that has your two tables, then run:
//!   TEST_PG_HOST=localhost TEST_PG_PORT=5432 TEST_PG_DATABASE=trading \
//!   TEST_PG_USERNAME=advisor_ro TEST_PG_PASSWORD=... TEST_PG_TICKER=AAPL \
//!   cargo test -p advisor-market-data -- --ignored --nocapture

use std::time::Duration;

use advisor_core::{HealthCheck, Secret};
use advisor_market_data::{PgPriceRepository, PostgresConfig, PriceRepository, PriceSummary};
use chrono::{Days, Utc};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

#[tokio::test]
#[ignore = "needs a Postgres with the trading tables; see the file header"]
async fn reads_assets_and_prices_from_postgres() {
    let repo = PgPriceRepository::new(&PostgresConfig {
        host: env("TEST_PG_HOST"),
        port: env("TEST_PG_PORT").parse().expect("port is a number"),
        database: env("TEST_PG_DATABASE"),
        username: env("TEST_PG_USERNAME"),
        password: Secret::new(env("TEST_PG_PASSWORD")),
        max_connections: 2,
        acquire_timeout: Duration::from_secs(5),
    });
    println!("health: {}", repo.check().await.expect("database answers"));

    let ticker = env("TEST_PG_TICKER");
    let assets = repo.find_assets(&ticker.to_lowercase()).await.unwrap();
    assert!(!assets.is_empty(), "no asset found for {ticker}");
    println!("assets: {assets:?}");

    let to = Utc::now().date_naive();
    let from = to.checked_sub_days(Days::new(3650)).unwrap();
    let bars = repo
        .price_history(assets[0].asset_id, from, to)
        .await
        .unwrap();
    assert!(
        bars.windows(2).all(|w| w[0].date < w[1].date),
        "oldest first"
    );
    println!(
        "{} bars; summary: {:?}",
        bars.len(),
        PriceSummary::from_bars(&bars)
    );
}
