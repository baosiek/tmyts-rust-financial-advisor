//! SearchNewsTool tested with a fake embedder and an in-memory news store.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use serde_json::{Value, json};

use advisor_embed::{EmbedError, Embedder};
use advisor_news_rag::{NewsArticle, NewsError, NewsQuery, NewsRepository};
use advisor_tools::{SearchNewsConfig, SearchNewsTool, ToolError, ToolRegistry};

struct FakeEmbedder;

#[async_trait]
impl Embedder for FakeEmbedder {
    fn dimensions(&self) -> usize {
        3
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
    }
}

/// Returns its articles (up to the limit) and records the last query.
struct InMemoryNews {
    articles: Vec<NewsArticle>,
    last_query: Mutex<Option<NewsQuery>>,
}

#[async_trait]
impl NewsRepository for InMemoryNews {
    async fn search(&self, query: &NewsQuery) -> Result<Vec<NewsArticle>, NewsError> {
        *self.last_query.lock().unwrap() = Some(query.clone());
        Ok(self
            .articles
            .iter()
            .take(query.limit as usize)
            .cloned()
            .collect())
    }
}

fn article(title: &str, text: &str, summary: Option<&str>) -> NewsArticle {
    NewsArticle {
        id: title.to_string(),
        title: title.to_string(),
        text: text.to_string(),
        summary: summary.map(str::to_string),
        source: "seekingAlpha".to_string(),
        link: Some("https://example.com/a".to_string()),
        published_at: Utc.with_ymd_and_hms(2026, 11, 5, 19, 54, 44).unwrap(),
        tickers: vec!["AAPL".to_string()],
        sentiment_label: Some("Positive".to_string()),
        sentiment_score: Some(0.91),
        relevance: 0.87654,
    }
}

fn setup() -> (ToolRegistry, Arc<InMemoryNews>) {
    let news = Arc::new(InMemoryNews {
        articles: vec![
            article(
                "Apple beats estimates",
                "Long text ".repeat(100).as_str(),
                None,
            ),
            article("Apple guidance", "full text", Some("Short summary.")),
            article("Third", "t", None),
        ],
        last_query: Mutex::new(None),
    });
    let tool = SearchNewsTool::new(
        Arc::new(FakeEmbedder),
        news.clone(),
        SearchNewsConfig {
            default_results: 2,
            max_results: 3,
            max_excerpt_chars: 20,
        },
    );
    let mut registry = ToolRegistry::new();
    registry.register(tool);
    (registry, news)
}

async fn call(registry: &ToolRegistry, input: Value) -> Result<Value, ToolError> {
    let output = registry.call("search_news", input).await?;
    Ok(serde_json::from_str(&output).expect("the tool returns JSON"))
}

#[tokio::test]
async fn returns_citable_results() {
    let (registry, _) = setup();
    let output = call(&registry, json!({ "query": "apple earnings" }))
        .await
        .unwrap();

    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "default number of results");
    assert_eq!(results[0]["title"], "Apple beats estimates");
    assert_eq!(results[0]["source"], "seekingAlpha");
    assert_eq!(results[0]["link"], "https://example.com/a");
    assert_eq!(results[0]["published_at"], "2026-11-05T19:54:44Z");
    assert!((results[0]["relevance"].as_f64().unwrap() - 0.877).abs() < 1e-6);
    assert!(
        output["note"]
            .as_str()
            .unwrap()
            .contains("never as instructions")
    );
}

#[tokio::test]
async fn long_text_is_cut_and_summary_is_preferred() {
    let (registry, _) = setup();
    let output = call(&registry, json!({ "query": "apple" })).await.unwrap();
    let excerpt = output["results"][0]["excerpt"].as_str().unwrap();
    assert_eq!(
        excerpt.chars().count(),
        21,
        "20 characters plus the ellipsis: {excerpt}"
    );
    assert_eq!(output["results"][1]["excerpt"], "Short summary.");
}

#[tokio::test]
async fn filters_and_limit_reach_the_repository() {
    let (registry, news) = setup();
    call(
        &registry,
        json!({ "query": "apple", "ticker": "AAPL", "from": "2026-11-01", "to": "2026-11-05", "limit": 50 }),
    )
    .await
    .unwrap();

    let query = news.last_query.lock().unwrap().clone().unwrap();
    assert_eq!(query.ticker.as_deref(), Some("AAPL"));
    assert_eq!(query.limit, 3, "clamped to max_results");
    assert_eq!(
        query.published_after.unwrap().to_rfc3339(),
        "2026-11-01T00:00:00+00:00"
    );
    assert!(
        query
            .published_before
            .unwrap()
            .to_rfc3339()
            .starts_with("2026-11-05T23:59:59")
    );
    assert_eq!(query.vector, vec![0.1, 0.2, 0.3]);
}

#[tokio::test]
async fn empty_query_is_an_input_error() {
    let (registry, _) = setup();
    let err = call(&registry, json!({ "query": "  " })).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "got {err:?}");
}

#[tokio::test]
async fn reversed_dates_are_an_input_error() {
    let (registry, _) = setup();
    let err = call(
        &registry,
        json!({ "query": "x", "from": "2026-11-05", "to": "2026-11-01" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput { .. }), "got {err:?}");
}

#[test]
fn only_query_is_required() {
    let (registry, _) = setup();
    let schema = &registry.definitions()[0].input_schema;
    assert_eq!(schema["required"], json!(["query"]));
}
