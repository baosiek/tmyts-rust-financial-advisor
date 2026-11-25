use super::{ContentBlock, WireUsage};
use crate::types::{ChatResponse, StopReason, Usage};
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
        let received = r.content.len();
        let content: Vec<_> = r
            .content
            .into_iter()
            .filter_map(ContentBlock::into_neutral)
            .collect();
        if content.len() < received {
            // e.g. "thinking" blocks: not an error, but worth seeing when an answer looks empty.
            tracing::debug!(
                dropped = received - content.len(),
                "dropped content blocks of types this code does not handle"
            );
        }
        ChatResponse {
            content,
            model: r.model,
            stop_reason: stop_reason_from_api(r.stop_reason.as_deref()),
            usage: Usage {
                input_tokens: r.usage.input_tokens,
                output_tokens: r.usage.output_tokens,
                // Absent (or null) when the request used no prompt caching.
                cache_creation_input_tokens: r.usage.cache_creation_input_tokens.unwrap_or(0),
                cache_read_input_tokens: r.usage.cache_read_input_tokens.unwrap_or(0),
            },
        }
    }
}

fn stop_reason_from_api(reason: Option<&str>) -> StopReason {
    match reason {
        Some("end_turn") => StopReason::EndTurn,
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        Some(other) => StopReason::Other(other.to_string()),
        None => StopReason::Other("missing".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentBlock as Neutral;
    use serde_json::json;

    #[test]
    fn tool_use_response_becomes_tool_call() {
        let body = json!({
            "model": "m",
            "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "Let me check." },
                { "type": "tool_use", "id": "toolu_1", "name": "current_date", "input": {} },
                { "type": "thinking", "thinking": "..." }
            ],
            "usage": { "input_tokens": 10, "output_tokens": 5 }
        });
        let resp: ChatResponse = serde_json::from_value::<MessageResponse>(body)
            .unwrap()
            .into();

        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.text(), "Let me check.");
        assert_eq!(resp.content.len(), 2, "unknown block types are dropped");
        assert!(
            matches!(&resp.content[1], Neutral::ToolUse { name, .. } if name == "current_date")
        );
        assert_eq!(resp.tool_calls()[0].id, "toolu_1");
    }

    #[test]
    fn cache_tokens_are_read_and_null_means_zero() {
        let body = json!({
            "model": "m",
            "stop_reason": "end_turn",
            "content": [],
            "usage": {
                "input_tokens": 10,
                "output_tokens": 5,
                "cache_creation_input_tokens": null,
                "cache_read_input_tokens": 900
            }
        });
        let resp: ChatResponse = serde_json::from_value::<MessageResponse>(body)
            .unwrap()
            .into();

        assert_eq!(resp.usage.cache_creation_input_tokens, 0);
        assert_eq!(resp.usage.cache_read_input_tokens, 900);
        assert_eq!(resp.usage.total_input_tokens(), 910);
    }
}
