use std::time::Duration;

use advisor_llm::pricing::PriceTable;
use advisor_llm::types::{StopReason, Usage};
use serde_json::Value;

/// What happened while the agent answered one question: every LLM call, every
/// tool call and the time it took. Kept for failed questions too, because a
/// question that ran out of tool rounds still spent tokens.
#[derive(Debug, Clone, Default)]
pub struct RunTrace {
    pub llm_calls: Vec<LlmCallRecord>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub elapsed: Duration,
}

/// One round trip to the model.
#[derive(Debug, Clone)]
pub struct LlmCallRecord {
    /// The model that answered, as reported by the provider (prices are per model).
    pub model: String,
    pub usage: Usage,
    pub stop_reason: StopReason,
}

/// One tool the model asked for.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub input: Value,
    pub is_error: bool,
}

impl RunTrace {
    /// Tokens of all calls together.
    pub fn usage(&self) -> Usage {
        let mut total = Usage::default();
        for call in &self.llm_calls {
            total += call.usage;
        }
        total
    }

    /// Cost of all calls, priced per call (tiers depend on each call's input).
    /// `None` if any call used a model without a configured price: a partial
    /// sum would look like a real total and under-report the cost.
    pub fn cost_usd(&self, prices: &PriceTable) -> Option<f64> {
        self.llm_calls
            .iter()
            .map(|call| prices.cost_usd(&call.model, &call.usage))
            .sum()
    }

    /// Names of the tools that were called, in call order (with repeats).
    pub fn tool_names(&self) -> Vec<&str> {
        self.tool_calls
            .iter()
            .map(|call| call.name.as_str())
            .collect()
    }
}
