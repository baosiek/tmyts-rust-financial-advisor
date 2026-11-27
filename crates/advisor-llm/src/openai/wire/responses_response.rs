use serde::Deserialize;
use serde_json::Value;

use super::output_item::convert;
use super::wire_usage::WireUsage;
use crate::LlmError;
use crate::types::{ChatResponse, ContentBlock, StopReason};

/// Body of a `POST /v1/responses` answer. There is no stop reason field: why the
/// model stopped follows from `status`, `incomplete_details` and the output items.
#[derive(Debug, Deserialize)]
pub(crate) struct ResponsesResponse {
    pub model: String,
    pub status: String,
    #[serde(default)]
    pub incomplete_details: Option<IncompleteDetails>,
    #[serde(default)]
    pub error: Option<ResponseError>,
    /// Raw items: reasoning items must be replayed byte for byte.
    #[serde(default)]
    pub output: Vec<Value>,
    #[serde(default)]
    pub usage: Option<WireUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct IncompleteDetails {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResponseError {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ResponsesResponse {
    pub(crate) fn into_chat(self, replay_reasoning: bool) -> Result<ChatResponse, LlmError> {
        if self.status == "failed" {
            let error = self.error.unwrap_or(ResponseError {
                code: None,
                message: None,
            });
            // A response that failed on OpenAI's side after being accepted.
            return Err(LlmError::Api {
                status: 500,
                message: format!(
                    "response failed: {} {}",
                    error.code.unwrap_or_default(),
                    error.message.unwrap_or_default()
                ),
            });
        }

        let mut content = Vec::new();
        let mut refused = false;
        for item in self.output {
            let converted = convert(item, replay_reasoning);
            refused |= converted.refused;
            content.extend(converted.blocks);
        }
        let has_tool_calls = content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }));

        let stop_reason = match self.status.as_str() {
            "completed" if refused => StopReason::Refused,
            "completed" if has_tool_calls => StopReason::ToolUse,
            "completed" => StopReason::EndTurn,
            "incomplete" => {
                match self
                    .incomplete_details
                    .and_then(|details| details.reason)
                    .as_deref()
                {
                    Some("max_output_tokens") => StopReason::MaxTokens,
                    Some("content_filter") => StopReason::Refused,
                    Some(other) => StopReason::Other(format!("incomplete: {other}")),
                    None => StopReason::Other("incomplete".to_string()),
                }
            }
            other => StopReason::Other(other.to_string()),
        };

        Ok(ChatResponse {
            content,
            model: self.model,
            stop_reason,
            usage: self.usage.unwrap_or_default().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(body: Value, replay_reasoning: bool) -> Result<ChatResponse, LlmError> {
        serde_json::from_value::<ResponsesResponse>(body)
            .unwrap()
            .into_chat(replay_reasoning)
    }

    #[test]
    fn function_call_becomes_tool_use_with_call_id() {
        let resp = parse(
            json!({
                "model": "m",
                "status": "completed",
                "output": [
                    { "type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "enc" },
                    { "type": "function_call", "id": "fc_1", "call_id": "call_1",
                      "name": "price_history", "arguments": "{\"ticker\":\"AAPL\"}" }
                ],
                "usage": {
                    "input_tokens": 1000, "output_tokens": 50,
                    "input_tokens_details": { "cached_tokens": 800 }
                }
            }),
            true,
        )
        .unwrap();

        assert_eq!(
            resp.stop_reason,
            StopReason::ToolUse,
            "derived from the items"
        );
        let calls = resp.tool_calls();
        assert_eq!(calls[0].id, "call_1", "call_id, not the item id");
        assert_eq!(calls[0].input, json!({ "ticker": "AAPL" }));
        assert!(
            matches!(&resp.content[0], ContentBlock::ProviderState { provider, .. } if provider == "openai")
        );
        assert_eq!(resp.usage.input_tokens, 200, "cached tokens are subtracted");
        assert_eq!(resp.usage.cache_read_input_tokens, 800);
        assert_eq!(resp.usage.output_tokens, 50);
    }

    #[test]
    fn text_answer_ends_the_turn() {
        let resp = parse(
            json!({
                "model": "m",
                "status": "completed",
                "output": [{ "type": "message", "role": "assistant",
                    "content": [{ "type": "output_text", "text": "Hello", "annotations": [] }] }],
                "usage": { "input_tokens": 5, "output_tokens": 1 }
            }),
            true,
        )
        .unwrap();
        assert_eq!(resp.text(), "Hello");
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn cut_off_response_is_max_tokens_even_with_a_function_call() {
        let resp = parse(
            json!({
                "model": "m",
                "status": "incomplete",
                "incomplete_details": { "reason": "max_output_tokens" },
                "output": [{ "type": "function_call", "id": "fc_1", "call_id": "call_1",
                    "name": "t", "arguments": "{\"ticker\": \"AA" }]
            }),
            true,
        )
        .unwrap();
        assert_eq!(resp.stop_reason, StopReason::MaxTokens);
        // Invalid arguments are kept as a string instead of failing the call.
        assert_eq!(resp.tool_calls()[0].input, json!("{\"ticker\": \"AA"));
    }

    #[test]
    fn refusal_and_content_filter_are_refused() {
        let refusal = parse(
            json!({ "model": "m", "status": "completed", "output": [{ "type": "message",
                "content": [{ "type": "refusal", "refusal": "I can't help with that." }] }] }),
            false,
        )
        .unwrap();
        assert_eq!(refusal.stop_reason, StopReason::Refused);
        assert_eq!(refusal.text(), "I can't help with that.");

        let filtered = parse(
            json!({ "model": "m", "status": "incomplete",
                "incomplete_details": { "reason": "content_filter" }, "output": [] }),
            false,
        )
        .unwrap();
        assert_eq!(filtered.stop_reason, StopReason::Refused);
    }

    #[test]
    fn reasoning_is_dropped_unless_replayable() {
        let output = json!([{ "type": "reasoning", "id": "rs_1", "summary": [] }]);
        let without_content = parse(
            json!({ "model": "m", "status": "completed", "output": output }),
            true,
        )
        .unwrap();
        assert!(
            without_content.content.is_empty(),
            "an id alone cannot be replayed"
        );

        let switched_off = parse(
            json!({ "model": "m", "status": "completed",
                "output": [{ "type": "reasoning", "id": "rs_1", "encrypted_content": "e" }] }),
            false,
        )
        .unwrap();
        assert!(switched_off.content.is_empty());
    }

    #[test]
    fn failed_response_is_an_error() {
        let err = parse(
            json!({ "model": "m", "status": "failed",
                "error": { "code": "server_error", "message": "boom" } }),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 500, .. }));
        assert!(err.is_retryable());
    }
}
