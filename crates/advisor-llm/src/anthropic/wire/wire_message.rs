use serde::Serialize;

use super::ContentBlock;
use crate::types::{Message, Role};

/// Anthropic's JSON shape of a message: `{"role": "user", "content": [ ... ]}`.
#[derive(Debug, Serialize)]
pub(crate) struct WireMessage {
    pub role: &'static str,
    pub content: Vec<ContentBlock>,
}

impl From<&Message> for WireMessage {
    fn from(message: &Message) -> Self {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        Self {
            role,
            content: message.content.iter().map(ContentBlock::from).collect(),
        }
    }
}
