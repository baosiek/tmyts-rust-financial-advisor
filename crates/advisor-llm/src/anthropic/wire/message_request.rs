use crate::types::{ChatRequest, Message};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct MessageRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

impl MessageRequest {
    pub(crate) fn from_chat(req: &ChatRequest, default_model: &str) -> Self {
        Self {
            model: req
                .model
                .clone()
                .unwrap_or_else(|| default_model.to_string()),
            messages: req.messages.clone(),
            max_tokens: req.max_tokens,
            system: req.system.clone(),
            temperature: req.temperature,
        }
    }
}
