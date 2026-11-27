use serde::Serialize;

use super::input_item::{InputEntry, to_input};
use super::wire_tool::WireTool;
use crate::types::ChatRequest;

/// What `include` asks for so reasoning items can be replayed without storing
/// the conversation at OpenAI.
const ENCRYPTED_REASONING: &str = "reasoning.encrypted_content";

/// Body of `POST /v1/responses`.
#[derive(Debug, Serialize)]
pub(crate) struct ResponsesRequest {
    pub model: String,
    /// The system prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub input: Vec<InputEntry>,
    pub max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<WireTool>,
    /// Always `false`: the agent sends the whole history every time, so OpenAI
    /// does not need to keep the conversation (and does not store it).
    pub store: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<&'static str>,
}

impl ResponsesRequest {
    pub(crate) fn from_chat(
        req: &ChatRequest,
        default_model: &str,
        replay_reasoning: bool,
    ) -> Self {
        Self {
            model: req
                .model
                .clone()
                .unwrap_or_else(|| default_model.to_string()),
            instructions: req.system.clone(),
            input: to_input(&req.messages),
            max_output_tokens: req.max_tokens,
            temperature: req.temperature,
            tools: req.tools.iter().map(WireTool::from).collect(),
            store: false,
            include: if replay_reasoning {
                vec![ENCRYPTED_REASONING]
            } else {
                Vec::new()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, Message, ToolDefinition};
    use serde_json::json;

    fn history() -> ChatRequest {
        let mut req = ChatRequest::new("Price of AAPL?");
        req.system = Some("You are careful.".into());
        req.tools.push(ToolDefinition {
            name: "price_history".into(),
            description: "Prices.".into(),
            input_schema: json!({ "type": "object", "properties": {} }),
        });
        req.messages.push(Message::assistant_blocks(vec![
            ContentBlock::ProviderState {
                provider: "openai".into(),
                data: json!({ "type": "reasoning", "id": "rs_1", "encrypted_content": "e" }),
            },
            ContentBlock::ProviderState {
                provider: "anthropic".into(),
                data: json!({ "type": "thinking" }),
            },
            ContentBlock::text("Let me check."),
            ContentBlock::ToolUse {
                id: "call_1".into(),
                name: "price_history".into(),
                input: json!({ "ticker": "AAPL" }),
            },
        ]));
        req.messages
            .push(Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "+4.2%".into(),
                is_error: false,
            }]));
        req
    }

    #[test]
    fn history_is_flattened_into_typed_items() {
        let json =
            serde_json::to_value(ResponsesRequest::from_chat(&history(), "m", true)).unwrap();

        assert_eq!(json["instructions"], "You are careful.");
        assert_eq!(json["max_output_tokens"], 1024);
        assert_eq!(json["store"], false);
        assert_eq!(json["include"], json!(["reasoning.encrypted_content"]));
        assert!(json.get("messages").is_none() && json.get("system").is_none());

        let input = json["input"].as_array().unwrap();
        assert_eq!(input.len(), 5, "anthropic state is skipped: {input:?}");
        assert_eq!(
            input[0],
            json!({ "type": "message", "role": "user", "content": "Price of AAPL?" })
        );
        assert_eq!(
            input[1]["type"], "reasoning",
            "own reasoning is replayed verbatim"
        );
        assert_eq!(
            input[2],
            json!({ "type": "message", "role": "assistant", "content": "Let me check." })
        );
        assert_eq!(
            input[3],
            json!({
                "type": "function_call",
                "call_id": "call_1",
                "name": "price_history",
                "arguments": "{\"ticker\":\"AAPL\"}"
            })
        );
        assert_eq!(
            input[4],
            json!({ "type": "function_call_output", "call_id": "call_1", "output": "+4.2%" })
        );
    }

    #[test]
    fn tools_are_flat_and_not_strict() {
        let json =
            serde_json::to_value(ResponsesRequest::from_chat(&history(), "m", false)).unwrap();
        assert_eq!(
            json["tools"][0],
            json!({
                "type": "function",
                "name": "price_history",
                "description": "Prices.",
                "parameters": { "type": "object", "properties": {} },
                "strict": false
            })
        );
        assert!(json.get("include").is_none());
    }

    #[test]
    fn failed_tool_result_says_so_in_the_output() {
        let mut req = ChatRequest::new("q");
        req.messages
            .push(Message::tool_results(vec![ContentBlock::ToolResult {
                tool_use_id: "call_9".into(),
                content: "unknown ticker".into(),
                is_error: true,
            }]));
        let json = serde_json::to_value(ResponsesRequest::from_chat(&req, "m", false)).unwrap();
        assert_eq!(json["input"][1]["output"], "Error: unknown ticker");
    }
}
