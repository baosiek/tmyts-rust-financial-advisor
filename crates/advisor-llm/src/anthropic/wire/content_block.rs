use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types;

/// Anthropic's JSON shape of a content block: `{"type": "text", "text": "..."}` etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
    /// Block types this code does not handle yet (e.g. "thinking"); ignored.
    #[serde(other)]
    Other,
}

impl From<&types::ContentBlock> for ContentBlock {
    fn from(block: &types::ContentBlock) -> Self {
        match block {
            types::ContentBlock::Text { text } => Self::Text { text: text.clone() },
            types::ContentBlock::ToolUse { id, name, input } => Self::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            },
            types::ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Self::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
                is_error: *is_error,
            },
        }
    }
}

impl ContentBlock {
    /// Converts to the neutral type. `Other` has no neutral form, so it becomes `None`.
    pub(crate) fn into_neutral(self) -> Option<types::ContentBlock> {
        match self {
            Self::Text { text } => Some(types::ContentBlock::Text { text }),
            Self::ToolUse { id, name, input } => {
                Some(types::ContentBlock::ToolUse { id, name, input })
            }
            Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(types::ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            }),
            Self::Other => None,
        }
    }
}
