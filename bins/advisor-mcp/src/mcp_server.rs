use std::sync::Arc;
use std::time::Duration;

use advisor_core::{TimedError, timed};
use advisor_tools::{ToolError, ToolRegistry};
use serde_json::{Value, json};

use crate::json_rpc::{Incoming, error, success};
use crate::protocol::{
    LEGACY_VERSIONS, META_PROTOCOL_VERSION, META_SERVER_INFO, error_code, supported_versions,
};

/// What the server tells clients about itself, and its limits.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub version: String,
    pub instructions: String,
    pub tool_timeout: Duration,
    /// How long clients may cache `tools/list` and `server/discover`.
    pub cache_ttl: Duration,
}

/// Handles MCP messages, one at a time, independent of the transport.
pub struct McpServer {
    tools: Arc<ToolRegistry>,
    config: McpServerConfig,
}

impl McpServer {
    pub fn new(tools: Arc<ToolRegistry>, config: McpServerConfig) -> Self {
        Self { tools, config }
    }

    /// Handles one incoming JSON-RPC message. Returns the response to send,
    /// or `None` for notifications, which get no response.
    pub async fn handle(&self, message: Value) -> Option<Value> {
        let request = match Incoming::parse(message) {
            Ok(request) => request,
            Err(rejected) => {
                return Some(error(rejected.id, rejected.code, rejected.message, None));
            }
        };
        // Notifications (no id), e.g. the legacy `notifications/initialized`.
        let id = request.id.clone()?;

        Some(match self.dispatch(&request).await {
            Ok(result) => success(id, result),
            Err(Failure {
                code,
                message,
                data,
            }) => error(id, code, message, data),
        })
    }

    async fn dispatch(&self, request: &Incoming) -> Result<Value, Failure> {
        // Legacy handshake: no `_meta` version, the version is negotiated here.
        if request.method == "initialize" {
            return Ok(self.initialize(&request.params));
        }
        self.check_version(&request.params)?;

        match request.method.as_str() {
            "server/discover" => Ok(self.discover()),
            "tools/list" => Ok(self.list_tools()),
            "tools/call" => self.call_tool(&request.params).await,
            // Removed in 2026-07-28, still sent by legacy clients.
            "ping" => Ok(json!({})),
            other => Err(Failure::new(
                error_code::METHOD_NOT_FOUND,
                format!("method not found: {other}"),
            )),
        }
    }

    /// A modern request names its protocol version in `_meta`; reject versions
    /// this server does not speak, listing the ones it does. A request without
    /// one comes from a legacy client after `initialize` and is served as is.
    fn check_version(&self, params: &Value) -> Result<(), Failure> {
        let requested = params
            .get("_meta")
            .and_then(|meta| meta.get(META_PROTOCOL_VERSION))
            .and_then(Value::as_str);
        match requested {
            Some(version) if !supported_versions().contains(&version) => Err(Failure {
                code: error_code::UNSUPPORTED_PROTOCOL_VERSION,
                message: "Unsupported protocol version".to_string(),
                data: Some(json!({ "supported": supported_versions(), "requested": version })),
            }),
            _ => Ok(()),
        }
    }

    /// Legacy `initialize`: answer with the client's version if this server
    /// knows it, otherwise with the newest legacy version it supports.
    fn initialize(&self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = match requested {
            Some(version) if LEGACY_VERSIONS.contains(&version) => version,
            _ => LEGACY_VERSIONS[0],
        };
        tracing::info!(requested = ?requested, version, "legacy client initialized");
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": self.server_info(),
            "instructions": self.config.instructions,
        })
    }

    fn discover(&self) -> Value {
        self.complete(json!({
            "supportedVersions": supported_versions(),
            "capabilities": { "tools": {} },
            "instructions": self.config.instructions,
            "ttlMs": self.ttl_ms(),
            "cacheScope": "public",
        }))
    }

    /// Tools in name order (the registry is a `BTreeMap`), so clients can cache
    /// the list and the client model's prompt stays the same between runs.
    fn list_tools(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .definitions()
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                })
            })
            .collect();
        self.complete(json!({
            "tools": tools,
            "ttlMs": self.ttl_ms(),
            "cacheScope": "public",
        }))
    }

    /// Runs a tool. A failing tool is a *result* with `isError: true`, so the
    /// client's model sees the message and can correct itself; only an unknown
    /// tool or malformed params are protocol errors.
    async fn call_tool(&self, params: &Value) -> Result<Value, Failure> {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Err(Failure::new(
                error_code::INVALID_PARAMS,
                "tools/call needs a `name`",
            ));
        };
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let pending = timed(
            "mcp tool call",
            self.config.tool_timeout,
            self.tools.call(name, arguments),
        )
        .await;
        let elapsed_ms = pending.elapsed.as_millis() as u64;

        let (text, is_error) = match pending.result {
            Ok(output) => (output, false),
            Err(TimedError::Failed(ToolError::Unknown(_))) => {
                return Err(Failure::new(
                    error_code::INVALID_PARAMS,
                    format!("unknown tool: {name}"),
                ));
            }
            Err(TimedError::Failed(e)) => (e.to_string(), true),
            Err(e @ TimedError::TimedOut(_)) => (format!("tool `{name}` {e}"), true),
        };
        tracing::info!(tool = name, is_error, elapsed_ms, "mcp tool call");

        Ok(self.complete(json!({
            "content": [{ "type": "text", "text": text }],
            "isError": is_error,
        })))
    }

    /// Adds what every modern result carries. Legacy clients ignore these fields.
    fn complete(&self, mut result: Value) -> Value {
        result["resultType"] = json!("complete");
        result["_meta"] = json!({ META_SERVER_INFO: self.server_info() });
        result
    }

    fn server_info(&self) -> Value {
        json!({ "name": self.config.name, "version": self.config.version })
    }

    fn ttl_ms(&self) -> u64 {
        u64::try_from(self.config.cache_ttl.as_millis()).unwrap_or(u64::MAX)
    }
}

/// A JSON-RPC error to send back.
struct Failure {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl Failure {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}
