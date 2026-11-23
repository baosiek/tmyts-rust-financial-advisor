use super::{ContentBlock, ToolDefinition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// One turn of the conversation. `content` holds one or more blocks:
/// text, tool calls (assistant) or tool results (user).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::text(text)],
        }
    }

    /// The assistant's turn exactly as the model produced it (text and tool calls).
    pub fn assistant_blocks(content: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    /// Tool results go back to the model in a user message.
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::User,
            content: results,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: Option<String>,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    /// Tools the model may call. Empty means no tool calling.
    pub tools: Vec<ToolDefinition>,
}

impl ChatRequest {
    pub fn new(user_text: impl Into<String>) -> Self {
        Self {
            model: None,
            system: None,
            messages: vec![Message::user(user_text)],
            max_tokens: 1024,
            temperature: None,
            tools: Vec::new(),
        }
    }
}
