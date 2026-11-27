//! HTTP helpers shared by the provider adapters.

use reqwest::header::HeaderMap;
use url::Url;

/// Seconds from a `retry-after` header, in milliseconds; `default_ms` when absent.
pub(crate) fn retry_after_ms(headers: &HeaderMap, default_ms: u64) -> u64 {
    headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|secs| secs * 1000)
        .unwrap_or(default_ms)
}

/// `https://host/v1/models` + `claude-x` -> `https://host/v1/models/claude-x`.
/// The model id is pushed as one path segment, so it is escaped properly.
pub(crate) fn model_url(models_url: &Url, model: &str) -> Url {
    let mut url = models_url.clone();
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.pop_if_empty().push(model);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_url_appends_one_segment() {
        let base: Url = "https://api.example.com/v1/models".parse().unwrap();
        assert_eq!(
            model_url(&base, "claude-x-5").as_str(),
            "https://api.example.com/v1/models/claude-x-5"
        );
        let trailing: Url = "https://api.example.com/v1/models/".parse().unwrap();
        assert_eq!(
            model_url(&trailing, "m").as_str(),
            "https://api.example.com/v1/models/m"
        );
    }
}
