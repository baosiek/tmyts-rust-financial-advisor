//! The payload stored with each point in the news collection, and its conversion
//! to `NewsArticle`. Field names follow the existing collection exactly.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::NewsArticle;

#[derive(Debug, Deserialize)]
pub(crate) struct NewsPayload {
    #[serde(default)]
    title: String,
    #[serde(default)]
    text: String,
    summary: Option<String>,
    source: Option<String>,
    feed: Option<String>,
    link: Option<String>,
    published_at: DateTime<Utc>,
    ticker: Option<String>,
    #[serde(default)]
    entities: Entities,
    sentiment_label: Option<String>,
    sentiment_score: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct Entities {
    #[serde(rename = "Stock_Ticker", default)]
    stock_ticker: Vec<String>,
}

impl NewsPayload {
    pub(crate) fn into_article(self, id: String, relevance: f32) -> NewsArticle {
        // Many points have `ticker: null` but name the ticker in the extracted entities.
        let tickers: BTreeSet<String> = self
            .ticker
            .into_iter()
            .chain(self.entities.stock_ticker)
            .map(|t| t.trim().to_uppercase())
            .filter(|t| !t.is_empty())
            .collect();

        NewsArticle {
            id,
            title: self.title,
            text: self.text,
            summary: self.summary.filter(|s| !s.trim().is_empty()),
            source: self
                .source
                .or(self.feed)
                .unwrap_or_else(|| "unknown".to_string()),
            link: self.link,
            published_at: self.published_at,
            tickers: tickers.into_iter().collect(),
            sentiment_label: self.sentiment_label,
            sentiment_score: self.sentiment_score,
            relevance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The example point from the real collection.
    fn example() -> serde_json::Value {
        json!({
            "summary": null,
            "feed": "seekingAlpha",
            "categories": ["SRI", "SA Transcripts"],
            "author": [],
            "title": "Stoneridge, Inc. (SRI) Q3 2026 Earnings Call Transcript",
            "sentiment_score": 0.9966,
            "sentiment_label": "Neutral",
            "os_id": "d0b913a1a77c42cf2e07610bf0916fcfc417652035a4c50eb4342d9087afa4b1",
            "link": "https://seekingalpha.com/article/4963425",
            "text": "Stoneridge, Inc. (SRI) Q3 2026 Earnings Call Transcript. . ",
            "ticker": null,
            "entities": {
                "Event": ["Earnings Call Transcript"],
                "Stock_Ticker": ["SRI"],
                "Company": ["Stoneridge, Inc."],
                "Date": ["Q3 2026"]
            },
            "fetched_at": "2026-11-05T20:09:36.758247005Z",
            "source": "seekingAlpha",
            "published_at": "2026-11-05T19:54:44Z"
        })
    }

    #[test]
    fn real_payload_becomes_article() {
        let payload: NewsPayload = serde_json::from_value(example()).unwrap();
        let article = payload.into_article("id-1".to_string(), 0.8);

        assert_eq!(
            article.title,
            "Stoneridge, Inc. (SRI) Q3 2026 Earnings Call Transcript"
        );
        assert_eq!(article.source, "seekingAlpha");
        assert_eq!(article.tickers, vec!["SRI"], "ticker taken from entities");
        assert_eq!(article.summary, None);
        assert_eq!(
            article.published_at.to_rfc3339(),
            "2026-11-05T19:54:44+00:00"
        );
        assert_eq!(article.sentiment_label.as_deref(), Some("Neutral"));
    }

    #[test]
    fn ticker_field_and_entities_are_merged_without_duplicates() {
        let mut value = example();
        value["ticker"] = json!("sri");
        value["entities"]["Stock_Ticker"] = json!(["SRI", "AAPL"]);
        let article = serde_json::from_value::<NewsPayload>(value)
            .unwrap()
            .into_article("id".to_string(), 0.5);
        assert_eq!(article.tickers, vec!["AAPL", "SRI"]);
    }

    #[test]
    fn missing_entities_are_fine() {
        let mut value = example();
        value.as_object_mut().unwrap().remove("entities");
        assert!(serde_json::from_value::<NewsPayload>(value).is_ok());
    }
}
