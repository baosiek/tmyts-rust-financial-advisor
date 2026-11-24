use chrono::{DateTime, Utc};

/// One search hit, with what the model needs to answer and to cite.
#[derive(Debug, Clone, PartialEq)]
pub struct NewsArticle {
    pub id: String,
    pub title: String,
    pub text: String,
    pub summary: Option<String>,
    pub source: String,
    pub link: Option<String>,
    pub published_at: DateTime<Utc>,
    /// Tickers from the `ticker` field and from the extracted `Stock_Ticker` entities.
    pub tickers: Vec<String>,
    pub sentiment_label: Option<String>,
    pub sentiment_score: Option<f64>,
    /// Cosine similarity to the question: higher is more relevant.
    pub relevance: f32,
}
