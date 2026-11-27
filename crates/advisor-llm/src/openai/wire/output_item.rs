use serde::Deserialize;
use serde_json::Value;

use super::PROVIDER;
use crate::types::ContentBlock;

/// The output item types this adapter reads. Others (e.g. hosted tool calls)
/// are skipped.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<MessagePart>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MessagePart {
    OutputText {
        text: String,
    },
    Refusal {
        refusal: String,
    },
    #[serde(other)]
    Other,
}

/// What one output item contributes to the neutral response.
#[derive(Debug, Default)]
pub(crate) struct Converted {
    pub blocks: Vec<ContentBlock>,
    pub refused: bool,
}

/// Converts one raw output item. Reasoning items are kept as provider state
/// only when they can be replayed: with `store: false`, that needs their
/// encrypted content (an id alone refers to nothing stored at OpenAI).
pub(crate) fn convert(item: Value, replay_reasoning: bool) -> Converted {
    let kind = item
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if kind == "reasoning" {
        let replayable = item
            .get("encrypted_content")
            .is_some_and(|content| !content.is_null());
        let mut converted = Converted::default();
        if replay_reasoning && replayable {
            converted.blocks.push(ContentBlock::ProviderState {
                provider: PROVIDER.to_string(),
                data: item,
            });
        }
        return converted;
    }

    match serde_json::from_value::<OutputItem>(item) {
        Ok(OutputItem::Message { content }) => {
            let mut converted = Converted::default();
            for part in content {
                match part {
                    MessagePart::OutputText { text } => {
                        converted.blocks.push(ContentBlock::Text { text });
                    }
                    MessagePart::Refusal { refusal } => {
                        converted.refused = true;
                        converted.blocks.push(ContentBlock::Text { text: refusal });
                    }
                    MessagePart::Other => {}
                }
            }
            converted
        }
        Ok(OutputItem::FunctionCall {
            call_id,
            name,
            arguments,
        }) => {
            // The arguments are a JSON string written by the model and may be
            // invalid (or cut off). Keep them as a string then: the tool rejects
            // the input and the agent reports that back to the model.
            let input = serde_json::from_str(&arguments).unwrap_or_else(|e| {
                tracing::debug!(tool = %name, error = %e, "function call arguments are not valid JSON");
                Value::String(arguments)
            });
            Converted {
                // `call_id` (call_...) is what function_call_output must refer to,
                // not the item's `id` (fc_...).
                blocks: vec![ContentBlock::ToolUse {
                    id: call_id,
                    name,
                    input,
                }],
                refused: false,
            }
        }
        Err(_) => {
            tracing::debug!(item_type = %kind, "skipped an output item this code does not handle");
            Converted::default()
        }
    }
}
