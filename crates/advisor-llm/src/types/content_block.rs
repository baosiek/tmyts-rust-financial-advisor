use serde_json::Value;

/// One piece of a message. A message can mix several kinds, e.g. some text
/// followed by a tool call.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    /// Plain text written by the user or the model.
    Text { text: String },

    /// The model asks to run a tool. `id` links the call to its result.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    /// The result of running a tool, sent back in a user message.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },

    /// Opaque state a provider needs to get back in the next request, kept
    /// exactly as it arrived: Anthropic `thinking` blocks, OpenAI `reasoning`
    /// items, later Gemini thought signatures. Only the provider named in
    /// `provider` sends it back; every other provider skips it, so a history
    /// stays valid if the provider is switched.
    ProviderState { provider: String, data: Value },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}
