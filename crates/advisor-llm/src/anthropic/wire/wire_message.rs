use serde::Serialize;
use serde_json::Value;

use super::content_block::to_wire;
use crate::types::{Message, Role};

/// Anthropic's JSON shape of a message: `{"role": "user", "content": [ ... ]}`.
/// Blocks are JSON values, because provider state is sent back verbatim.
#[derive(Debug, Serialize)]
pub(crate) struct WireMessage {
    pub role: &'static str,
    pub content: Vec<Value>,
}

impl WireMessage {
    /// `None` when nothing in the message belongs in an Anthropic request
    /// (e.g. it only held another provider's state): an empty message is invalid.
    pub(crate) fn from_neutral(message: &Message) -> Option<Self> {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        let content: Vec<Value> = message.content.iter().filter_map(to_wire).collect();
        (!content.is_empty()).then_some(Self { role, content })
    }
}
