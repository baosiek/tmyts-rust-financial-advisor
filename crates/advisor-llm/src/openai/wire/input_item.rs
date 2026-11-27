use serde::Serialize;
use serde_json::Value;

use super::PROVIDER;
use crate::types::{ContentBlock, Message, Role};

/// One entry of the request's `input` list. The Responses API takes a flat list
/// of typed items instead of messages with blocks, so one neutral message can
/// become several items (e.g. text plus two function calls).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum InputEntry {
    Item(InputItem),
    /// A reasoning item from an earlier response, sent back exactly as received.
    Raw(Value),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum InputItem {
    Message {
        role: &'static str,
        content: String,
    },
    /// The model's earlier request to run a tool.
    FunctionCall {
        call_id: String,
        name: String,
        /// The arguments as a JSON **string**, the way the API returned them.
        arguments: String,
    },
    /// Our answer to that request, linked by `call_id`.
    FunctionCallOutput {
        call_id: String,
        output: String,
    },
}

/// Flattens the neutral history into input items.
pub(crate) fn to_input(messages: &[Message]) -> Vec<InputEntry> {
    messages
        .iter()
        .flat_map(|message| {
            message
                .content
                .iter()
                .filter_map(move |block| to_entry(message.role, block))
        })
        .collect()
}

fn to_entry(role: Role, block: &ContentBlock) -> Option<InputEntry> {
    let item = match block {
        ContentBlock::Text { text } => InputItem::Message {
            role: match role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: text.clone(),
        },
        ContentBlock::ToolUse { id, name, input } => InputItem::FunctionCall {
            call_id: id.clone(),
            name: name.clone(),
            // A string input is an argument string the model got wrong: send it back as is.
            arguments: match input {
                Value::String(raw) => raw.clone(),
                other => other.to_string(),
            },
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => InputItem::FunctionCallOutput {
            call_id: tool_use_id.clone(),
            // The API has no error flag, so the error is said in the text.
            output: if *is_error {
                format!("Error: {content}")
            } else {
                content.clone()
            },
        },
        // Our own reasoning goes back as received; other providers' state is skipped.
        ContentBlock::ProviderState { provider, data } => {
            return (provider == PROVIDER).then(|| InputEntry::Raw(data.clone()));
        }
    };
    Some(InputEntry::Item(item))
}
