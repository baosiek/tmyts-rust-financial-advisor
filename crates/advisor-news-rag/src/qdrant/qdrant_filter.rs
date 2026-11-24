//! Turns a `NewsQuery` into a Qdrant filter. Kept separate (and pure) so it can be
//! tested without a Qdrant server.

use chrono::{DateTime, Utc};
use qdrant_client::qdrant::{Condition, DatetimeRange, Filter};

use crate::NewsQuery;

pub(crate) fn build_filter(query: &NewsQuery) -> Option<Filter> {
    let mut must: Vec<Condition> = Vec::new();

    if let Some(ticker) = &query.ticker {
        let ticker = ticker.trim().to_uppercase();
        // Either field may hold the ticker: `ticker` is indexed but often null,
        // `entities.Stock_Ticker` is filled by the entity extraction.
        must.push(
            Filter::should([
                Condition::matches("ticker", ticker.clone()),
                Condition::matches("entities.Stock_Ticker", ticker),
            ])
            .into(),
        );
    }

    if query.published_after.is_some() || query.published_before.is_some() {
        must.push(Condition::datetime_range(
            "published_at",
            DatetimeRange {
                gte: query.published_after.map(timestamp),
                lte: query.published_before.map(timestamp),
                ..Default::default()
            },
        ));
    }

    (!must.is_empty()).then(|| Filter::must(must))
}

fn timestamp(moment: DateTime<Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: moment.timestamp(),
        nanos: moment.timestamp_subsec_nanos() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn query() -> NewsQuery {
        NewsQuery {
            vector: vec![0.0; 3],
            ticker: None,
            published_after: None,
            published_before: None,
            limit: 5,
        }
    }

    #[test]
    fn no_filters_means_no_filter() {
        assert!(build_filter(&query()).is_none());
    }

    #[test]
    fn ticker_matches_either_field_in_upper_case() {
        let mut q = query();
        q.ticker = Some(" aapl ".to_string());
        let filter = build_filter(&q).unwrap();
        let text = format!("{filter:?}");
        assert!(
            text.contains("\"ticker\"") && text.contains("entities.Stock_Ticker"),
            "{text}"
        );
        assert!(text.contains("AAPL") && !text.contains("aapl"), "{text}");
    }

    #[test]
    fn date_range_uses_published_at() {
        let mut q = query();
        q.published_after = Some(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap());
        let filter = build_filter(&q).unwrap();
        assert_eq!(filter.must.len(), 1);
        let text = format!("{filter:?}");
        assert!(
            text.contains("published_at") && text.contains(&1_793_491_200i64.to_string()),
            "{text}"
        );
    }
}
