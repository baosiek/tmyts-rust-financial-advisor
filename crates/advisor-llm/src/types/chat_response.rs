use serde_json::Value;

use super::{ContentBlock, StopReason};

/// Tokens of one call, normalised so that every provider means the same thing
/// (cost tracking depends on it):
///
/// - `input_tokens`: input tokens billed at the normal rate, i.e. **without**
///   cache reads and cache writes. (OpenAI and Gemini count cached tokens inside
///   their input total; their adapters subtract them.)
/// - `cache_creation_input_tokens`: input written to the prompt cache (0 for
///   providers that do not bill cache writes).
/// - `cache_read_input_tokens`: input read from the prompt cache.
/// - `output_tokens`: **everything billed as output**, reasoning included.
///   (Gemini reports thinking tokens separately; its adapter adds them.)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: u32,
    pub cache_read_input_tokens: u32,
}

impl Usage {
    /// All input tokens of the call, cached or not.
    pub fn total_input_tokens(&self) -> u32 {
        self.input_tokens + self.cache_creation_input_tokens + self.cache_read_input_tokens
    }
}

/// Lets the agent total the usage of a question: `total += response.usage`.
impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_creation_input_tokens += other.cache_creation_input_tokens;
        self.cache_read_input_tokens += other.cache_read_input_tokens;
    }
}

/// A tool call requested by the model, taken out of a `ContentBlock::ToolUse`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: StopReason,
    pub usage: Usage, // real token count from the API
}

impl ChatResponse {
    /// All text blocks joined together: the part a person reads.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// The tool calls the model asked for, in order.
    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }),
                _ => None,
            })
            .collect()
    }
}
