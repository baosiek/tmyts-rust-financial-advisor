use std::collections::BTreeMap;

use super::ModelPrice;
use crate::types::Usage;

/// Prices of every configured model, keyed by model id.
#[derive(Debug, Clone, Default)]
pub struct PriceTable {
    models: BTreeMap<String, ModelPrice>,
}

impl PriceTable {
    pub fn new(models: BTreeMap<String, ModelPrice>) -> Self {
        Self { models }
    }

    /// The price of `model`. The API may answer with a longer id than the one
    /// requested (e.g. a dated snapshot `claude-x-5-20261001` for `claude-x-5`), so
    /// when there is no exact match the longest configured id that is a prefix wins.
    pub fn price(&self, model: &str) -> Option<&ModelPrice> {
        self.models.get(model).or_else(|| {
            self.models
                .iter()
                .filter(|(id, _)| model.starts_with(id.as_str()))
                .max_by_key(|(id, _)| id.len())
                .map(|(_, price)| price)
        })
    }

    /// Cost of one call, or `None` when the model has no configured price.
    /// Unknown is not the same as free, so this never guesses 0.
    pub fn cost_usd(&self, model: &str, usage: &Usage) -> Option<f64> {
        self.price(model).map(|price| price.cost_usd(usage))
    }
}
