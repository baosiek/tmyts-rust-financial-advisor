use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types;

/// The provider name stored in `ContentBlock::ProviderState` for Anthropic state.
pub(crate) const PROVIDER: &str = "anthropic";

/// Block types that are model state rather than content: they must be sent
/// back unchanged in the next request (e.g. with extended thinking and tools).
const STATE_BLOCK_TYPES: [&str; 2] = ["thinking", "redacted_thinking"];

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
    /// Block types this code does not handle (e.g. server tool results); ignored.
    #[serde(other)]
    Other,
}

/// Neutral block -> the JSON Anthropic expects. `None` for blocks that do not
/// belong in an Anthropic request (another provider's state).
pub(crate) fn to_wire(block: &types::ContentBlock) -> Option<Value> {
    let typed = match block {
        types::ContentBlock::Text { text } => ContentBlock::Text { text: text.clone() },
        types::ContentBlock::ToolUse { id, name, input } => ContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        types::ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
        },
        // Our own state goes back exactly as it arrived; other providers' is skipped.
        types::ContentBlock::ProviderState { provider, data } => {
            return (provider == PROVIDER).then(|| data.clone());
        }
    };
    Some(serde_json::to_value(typed).expect("a content block always serializes"))
}

/// Anthropic JSON block -> neutral block. Thinking blocks are kept as provider
/// state; types this code does not handle become `None`.
pub(crate) fn from_wire(block: Value) -> Option<types::ContentBlock> {
    let kind = block
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if STATE_BLOCK_TYPES.contains(&kind) {
        return Some(types::ContentBlock::ProviderState {
            provider: PROVIDER.to_string(),
            data: block,
        });
    }
    match serde_json::from_value::<ContentBlock>(block).ok()? {
        ContentBlock::Text { text } => Some(types::ContentBlock::Text { text }),
        ContentBlock::ToolUse { id, name, input } => {
            Some(types::ContentBlock::ToolUse { id, name, input })
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Some(types::ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        }),
        ContentBlock::Other => None,
    }
}
