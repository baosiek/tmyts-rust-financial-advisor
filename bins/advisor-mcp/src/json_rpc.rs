//! The JSON-RPC 2.0 envelope MCP messages travel in.

use serde_json::{Value, json};

/// A parsed incoming message. A message without `id` is a notification and
/// gets no response.
#[derive(Debug)]
pub(crate) struct Incoming {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

/// Why an incoming message could not be handled as a request at all.
pub(crate) struct Rejected {
    pub id: Value,
    pub code: i64,
    pub message: String,
}

impl Incoming {
    pub(crate) fn parse(message: Value) -> Result<Self, Rejected> {
        let id = message.get("id").cloned();
        let invalid = |reason: &str| Rejected {
            id: id.clone().unwrap_or(Value::Null),
            code: crate::protocol::error_code::INVALID_REQUEST,
            message: format!("invalid request: {reason}"),
        };
        if !message.is_object() {
            return Err(invalid(
                "expected a JSON object (batches are not supported)",
            ));
        }
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(invalid("`jsonrpc` must be \"2.0\""));
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return Err(invalid("missing `method`"));
        };
        Ok(Self {
            id,
            method: method.to_string(),
            params: message.get("params").cloned().unwrap_or_else(|| json!({})),
        })
    }
}

pub(crate) fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub(crate) fn error(
    id: Value,
    code: i64,
    message: impl Into<String>,
    data: Option<Value>,
) -> Value {
    let mut error = json!({ "code": code, "message": message.into() });
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": error })
}
