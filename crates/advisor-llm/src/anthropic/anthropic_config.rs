use advisor_core::Secret;
use url::Url;

pub struct AnthropicConfig {
    pub api_key: Secret,
    pub model: String,
    pub version: String,
    pub url: Url,
}
