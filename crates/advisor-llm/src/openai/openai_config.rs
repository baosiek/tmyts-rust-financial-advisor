use advisor_core::Secret;
use std::time::Duration;
use url::Url;

pub struct OpenaiConfig {
    pub api_key: Secret,
    pub model: String,
    /// Full endpoint, e.g. https://api.openai.com/v1/responses. Other providers
    /// with an OpenAI-compatible Responses API (e.g. xAI) only need another URL.
    pub responses_url: Url,
    /// Upper bound for one HTTP request, including reading the response.
    pub timeout: Duration,
    /// Model list endpoint, e.g. https://api.openai.com/v1/models. The health
    /// check asks for `{models_url}/{model}`.
    pub models_url: Url,
    /// Send reasoning items back in the next request (asks for their encrypted
    /// content). Helps reasoning models keep their train of thought across tool calls.
    pub replay_reasoning: bool,
}
