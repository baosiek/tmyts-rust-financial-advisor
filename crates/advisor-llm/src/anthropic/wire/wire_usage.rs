use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct WireUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Only present when prompt caching is in play; may also be `null`.
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
}
