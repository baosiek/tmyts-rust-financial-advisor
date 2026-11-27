use serde::Deserialize;

use crate::types::Usage;

/// `usage` of a response. OpenAI's `input_tokens` **includes** the cached ones.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct WireUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub input_tokens_details: Option<InputTokensDetails>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct InputTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

impl From<WireUsage> for Usage {
    /// Normalises to the neutral meaning: `input_tokens` without cached tokens.
    /// Output already includes reasoning tokens; OpenAI bills no cache writes.
    fn from(usage: WireUsage) -> Self {
        let cached = usage
            .input_tokens_details
            .map(|details| details.cached_tokens)
            .unwrap_or(0);
        Usage {
            input_tokens: usage.input_tokens.saturating_sub(cached),
            output_tokens: usage.output_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: cached,
        }
    }
}
