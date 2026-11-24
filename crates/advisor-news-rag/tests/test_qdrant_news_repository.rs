//! Searches your real Qdrant collection. Ignored by default.
//! Needs Qdrant and the embedding service running:
//!   TEST_QDRANT_URL=http://localhost:6334 TEST_QDRANT_COLLECTION=news \
//!   TEST_EMBED_URL=http://localhost:8080/embed [TEST_QDRANT_API_KEY=...] \
//!   cargo test -p advisor-news-rag -- --ignored --nocapture

use std::time::Duration;

use advisor_core::{HealthCheck, Secret};
use advisor_news_rag::{NewsQuery, NewsRepository, QdrantConfig, QdrantNewsRepository};
use serde_json::json;
use url::Url;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test"))
}

#[tokio::test]
#[ignore = "needs Qdrant and the embedding service; see the file header"]
async fn searches_the_real_collection() {
    let repo = QdrantNewsRepository::new(QdrantConfig {
        url: Url::parse(&env("TEST_QDRANT_URL")).unwrap(),
        api_key: std::env::var("TEST_QDRANT_API_KEY").ok().map(Secret::new),
        collection: env("TEST_QDRANT_COLLECTION"),
        timeout: Duration::from_secs(10),
        vector_size: 384,
    })
    .unwrap();
    println!("health: {}", repo.check().await.expect("qdrant answers"));

    // Embed the question with the same service the news was stored with.
    let response: serde_json::Value = reqwest::Client::new()
        .post(env("TEST_EMBED_URL"))
        .json(&json!({ "text": ["earnings call transcript"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let vector: Vec<f32> = serde_json::from_value(response["embedding"][0].clone()).unwrap();

    let hits = repo
        .search(&NewsQuery {
            vector,
            ticker: None,
            published_after: None,
            published_before: None,
            limit: 3,
        })
        .await
        .unwrap();
    assert!(!hits.is_empty());
    for hit in hits {
        println!(
            "{:.3} {} {:?} {}",
            hit.relevance, hit.published_at, hit.tickers, hit.title
        );
    }
}
