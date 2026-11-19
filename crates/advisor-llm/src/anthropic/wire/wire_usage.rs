use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct WireUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
