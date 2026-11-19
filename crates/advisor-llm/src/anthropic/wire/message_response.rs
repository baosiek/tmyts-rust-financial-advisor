use super::{ContentBlock, WireUsage};
use crate::types::{ChatResponse, Usage};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct MessageResponse {
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: WireUsage, // <- this is where the tokens come from
}

impl From<MessageResponse> for ChatResponse {
    fn from(r: MessageResponse) -> Self {
        let text = r
            .content
            .into_iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text),
                ContentBlock::Other => None,
            })
            .collect::<Vec<_>>()
            .join("");
        ChatResponse {
            text,
            model: r.model,
            stop_reason: r.stop_reason,
            usage: Usage {
                input_tokens: r.usage.input_tokens,
                output_tokens: r.usage.output_tokens,
            },
        }
    }
}
