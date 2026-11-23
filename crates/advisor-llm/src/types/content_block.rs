use serde_json::Value;

/// One piece of a message. A message can mix several kinds, e.g. some text
/// followed by a tool call.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    /// Plain text written by the user or the model.
    Text { text: String },

    /// The model asks to run a tool. `id` links the call to its result.
    ToolUse { id: String, name: String, input: Value },

    /// The result of running a tool, sent back in a user message.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }
}
