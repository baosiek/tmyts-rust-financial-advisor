use advisor_core::Secret;
use std::time::Duration;
use url::Url;

pub struct AnthropicConfig {
    pub api_key: Secret,
    pub model: String,
    pub version: String,
    /// Full endpoint, e.g. https://api.anthropic.com/v1/messages
    pub url: Url,
    /// Upper bound for one HTTP request, including reading the response.
    pub timeout: Duration,
}
