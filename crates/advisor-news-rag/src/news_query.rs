use chrono::{DateTime, Utc};

/// A semantic search with optional filters.
#[derive(Debug, Clone, PartialEq)]
pub struct NewsQuery {
    /// The embedded question.
    pub vector: Vec<f32>,
    /// Only articles about this ticker (matched in `ticker` or in the extracted entities).
    pub ticker: Option<String>,
    /// Only articles published at or after this moment.
    pub published_after: Option<DateTime<Utc>>,
    /// Only articles published at or before this moment.
    pub published_before: Option<DateTime<Utc>>,
    pub limit: u64,
}
