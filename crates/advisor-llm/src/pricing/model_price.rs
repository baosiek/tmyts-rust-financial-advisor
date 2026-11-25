use serde::Deserialize;

use crate::types::Usage;

const PER_MILLION: f64 = 1_000_000.0;

/// Prices of one model, in US dollars per million tokens ("per MTok"), the unit
/// providers publish them in.
///
/// Money is `f64` here on purpose: this is an estimate for logs, budgets and
/// eval reports, not accounting. The provider's invoice stays the source of truth.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPrice {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    /// Writing a prompt prefix into the cache (more than a normal input token).
    pub cache_write_per_mtok: f64,
    /// Reading a cached prefix (a fraction of a normal input token).
    pub cache_read_per_mtok: f64,
    /// Some models charge more for every token of a request once its input
    /// passes a threshold. Leave it out for flat-priced models.
    #[serde(default)]
    pub long_context: Option<LongContextPrice>,
}

/// Prices that apply to a whole request whose input is above `above_input_tokens`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LongContextPrice {
    pub above_input_tokens: u32,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_write_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

impl ModelPrice {
    /// Cost of one call. Tiers are chosen per call, because the provider bills
    /// each request on its own input size.
    pub fn cost_usd(&self, usage: &Usage) -> f64 {
        let (input, output, cache_write, cache_read) = match &self.long_context {
            Some(long) if usage.total_input_tokens() > long.above_input_tokens => (
                long.input_per_mtok,
                long.output_per_mtok,
                long.cache_write_per_mtok,
                long.cache_read_per_mtok,
            ),
            _ => (
                self.input_per_mtok,
                self.output_per_mtok,
                self.cache_write_per_mtok,
                self.cache_read_per_mtok,
            ),
        };
        (f64::from(usage.input_tokens) * input
            + f64::from(usage.output_tokens) * output
            + f64::from(usage.cache_creation_input_tokens) * cache_write
            + f64::from(usage.cache_read_input_tokens) * cache_read)
            / PER_MILLION
    }
}
