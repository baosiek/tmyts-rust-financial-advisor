use serde::Serialize;

use super::{WireMessage, WireTool};
use crate::types::ChatRequest;

#[derive(Debug, Serialize)]
pub(crate) struct MessageRequest {
    pub model: String,
    pub messages: Vec<WireMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<WireTool>,
}

impl MessageRequest {
    pub(crate) fn from_chat(req: &ChatRequest, default_model: &str) -> Self {
        Self {
            model: req
                .model
                .clone()
                .unwrap_or_else(|| default_model.to_string()),
            messages: req
                .messages
                .iter()
                .filter_map(WireMessage::from_neutral)
                .collect(),
            max_tokens: req.max_tokens,
            system: req.system.clone(),
            temperature: req.temperature,
            tools: req.tools.iter().map(WireTool::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, Message, ToolDefinition};
    use serde_json::json;

    #[test]
    fn tools_and_tool_results_serialize_in_anthropic_shape() {
        let mut req = ChatRequest::new("What is the date?");
        req.tools.push(ToolDefinition {
            name: "current_date".into(),
            description: "Today's date".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        });
        req.messages
            .push(Message::assistant_blocks(vec![ContentBlock::ToolUse {
                id: "toolu_1".into(),
                name: "current_date".into(),
                input: json!({}),
            }]));
        req.messages
            .push(Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_1".into(),
                content: "2026-11-23".into(),
                is_error: false,
            }]));

        let json = serde_json::to_value(MessageRequest::from_chat(&req, "m")).unwrap();

        assert_eq!(json["tools"][0]["name"], "current_date");
        assert_eq!(json["messages"][0]["content"][0]["type"], "text");
        assert_eq!(json["messages"][1]["role"], "assistant");
        assert_eq!(json["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(json["messages"][2]["role"], "user");
        assert_eq!(json["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(json["messages"][2]["content"][0]["tool_use_id"], "toolu_1");
        // is_error is only sent when true
        assert!(json["messages"][2]["content"][0].get("is_error").is_none());
    }

    #[test]
    fn no_tools_means_no_tools_field() {
        let json =
            serde_json::to_value(MessageRequest::from_chat(&ChatRequest::new("Hi"), "m")).unwrap();
        assert!(json.get("tools").is_none());
    }

    #[test]
    fn own_state_is_sent_back_and_other_providers_state_is_skipped() {
        let mut req = ChatRequest::new("Hi");
        req.messages.push(Message::assistant_blocks(vec![
            ContentBlock::ProviderState {
                provider: "anthropic".into(),
                data: json!({ "type": "thinking", "thinking": "…", "signature": "sig" }),
            },
            ContentBlock::ProviderState {
                provider: "openai".into(),
                data: json!({ "type": "reasoning", "id": "rs_1" }),
            },
            ContentBlock::text("Hello"),
        ]));
        // A message holding only another provider's state is left out entirely.
        req.messages.push(Message::assistant_blocks(vec![
            ContentBlock::ProviderState {
                provider: "openai".into(),
                data: json!({}),
            },
        ]));

        let json = serde_json::to_value(MessageRequest::from_chat(&req, "m")).unwrap();

        let messages = json["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        let blocks = messages[1]["content"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["type"], "thinking");
        assert_eq!(blocks[0]["signature"], "sig");
        assert_eq!(blocks[1]["type"], "text");
    }
}
