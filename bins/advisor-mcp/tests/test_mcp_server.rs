//! The MCP server with in-memory tools: every message type, both protocol eras.

use std::sync::Arc;
use std::time::Duration;

use advisor_mcp::{McpServer, McpServerConfig, serve};
use advisor_tools::{Tool, ToolError, ToolRegistry};
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct PriceTool;

#[derive(Deserialize, JsonSchema)]
struct PriceInput {
    /// The ticker, e.g. AAPL.
    ticker: String,
}

#[async_trait]
impl Tool for PriceTool {
    const NAME: &'static str = "price";
    const DESCRIPTION: &'static str = "Latest price of a ticker.";
    type Input = PriceInput;

    async fn run(&self, input: PriceInput) -> Result<String, ToolError> {
        if input.ticker == "SLOW" {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        Ok(format!("{}: 123.45", input.ticker))
    }
}

fn server() -> McpServer {
    let mut tools = ToolRegistry::new();
    tools.register(PriceTool);
    McpServer::new(
        Arc::new(tools),
        McpServerConfig {
            name: "advisor-mcp".into(),
            version: "0.1.0".into(),
            instructions: "Use price for prices.".into(),
            tool_timeout: Duration::from_millis(200),
            cache_ttl: Duration::from_secs(60),
        },
    )
}

/// A modern request: the protocol version travels in `_meta`.
fn modern(id: u64, method: &str, mut params: Value) -> Value {
    params["_meta"] = json!({ "io.modelcontextprotocol/protocolVersion": "2026-07-28" });
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

async fn result(message: Value) -> Value {
    let response = server().handle(message).await.expect("a response");
    assert!(
        response.get("error").is_none(),
        "unexpected error: {response}"
    );
    response["result"].clone()
}

async fn error_code(message: Value) -> i64 {
    let response = server().handle(message).await.expect("a response");
    response["error"]["code"].as_i64().expect("an error")
}

#[tokio::test]
async fn discover_lists_versions_capabilities_and_identity() {
    let result = result(modern(1, "server/discover", json!({}))).await;

    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"][0], "2026-07-28");
    assert!(
        result["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2025-11-25"))
    );
    assert_eq!(result["capabilities"], json!({ "tools": {} }));
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "advisor-mcp"
    );
    assert_eq!(result["ttlMs"], 60_000);
    assert_eq!(result["cacheScope"], "public");
}

#[tokio::test]
async fn tools_list_carries_the_generated_schema() {
    let result = result(modern(2, "tools/list", json!({}))).await;

    let tool = &result["tools"][0];
    assert_eq!(tool["name"], "price");
    assert_eq!(tool["description"], "Latest price of a ticker.");
    assert_eq!(
        tool["inputSchema"]["properties"]["ticker"]["description"],
        "The ticker, e.g. AAPL."
    );
}

#[tokio::test]
async fn tools_call_returns_text_content() {
    let result = result(modern(
        3,
        "tools/call",
        json!({ "name": "price", "arguments": { "ticker": "AAPL" } }),
    ))
    .await;

    assert_eq!(
        result["content"],
        json!([{ "type": "text", "text": "AAPL: 123.45" }])
    );
    assert_eq!(result["isError"], false);
}

#[tokio::test]
async fn bad_tool_input_is_a_tool_error_the_model_can_read() {
    let result = result(modern(
        4,
        "tools/call",
        json!({ "name": "price", "arguments": { "symbol": "AAPL" } }),
    ))
    .await;

    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("invalid input")
    );
}

#[tokio::test]
async fn slow_tool_is_cut_off_as_a_tool_error() {
    let result = result(modern(
        5,
        "tools/call",
        json!({ "name": "price", "arguments": { "ticker": "SLOW" } }),
    ))
    .await;

    assert_eq!(result["isError"], true);
    assert!(
        result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("timed out")
    );
}

#[tokio::test]
async fn unknown_tool_and_unknown_method_are_protocol_errors() {
    let unknown_tool = modern(6, "tools/call", json!({ "name": "nope", "arguments": {} }));
    assert_eq!(error_code(unknown_tool).await, -32602);
    assert_eq!(
        error_code(modern(7, "resources/list", json!({}))).await,
        -32601
    );
}

#[tokio::test]
async fn unsupported_version_lists_the_supported_ones() {
    let message = json!({
        "jsonrpc": "2.0", "id": 8, "method": "tools/list",
        "params": { "_meta": { "io.modelcontextprotocol/protocolVersion": "1900-01-01" } }
    });
    let response = server().handle(message).await.unwrap();

    assert_eq!(response["error"]["code"], -32022);
    assert_eq!(response["error"]["data"]["requested"], "1900-01-01");
    assert_eq!(response["error"]["data"]["supported"][0], "2026-07-28");
}

#[tokio::test]
async fn legacy_client_handshake_then_calls() {
    let server = server();
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "old-client", "version": "1" } }
    });
    let response = server.handle(init).await.unwrap();
    assert_eq!(
        response["result"]["protocolVersion"], "2025-06-18",
        "known version is echoed"
    );
    assert_eq!(response["result"]["serverInfo"]["name"], "advisor-mcp");
    assert!(response["result"]["capabilities"]["tools"].is_object());

    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert!(
        server.handle(initialized).await.is_none(),
        "notifications get no response"
    );

    // Legacy requests carry no `_meta` version and are served as is.
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
    assert_eq!(
        server.handle(list).await.unwrap()["result"]["tools"][0]["name"],
        "price"
    );
    let ping = json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" });
    assert_eq!(server.handle(ping).await.unwrap()["result"], json!({}));
}

#[tokio::test]
async fn unknown_legacy_version_gets_the_newest_legacy_one() {
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2099-01-01" }
    });
    assert_eq!(result(init).await["protocolVersion"], "2025-11-25");
}

#[tokio::test]
async fn malformed_requests_are_rejected() {
    let no_method = json!({ "jsonrpc": "2.0", "id": 1 });
    assert_eq!(error_code(no_method).await, -32600);
    let batch = json!([{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]);
    assert_eq!(error_code(batch).await, -32600);
}

// ---- stdio transport --------------------------------------------------------

#[tokio::test]
async fn stdio_answers_each_line_and_skips_notifications() {
    let (mut client, server_end) = tokio::io::duplex(64 * 1024);
    let (server_read, server_write) = tokio::io::split(server_end);
    let serving = tokio::spawn(serve(
        Arc::new(server()),
        BufReader::new(server_read),
        server_write,
    ));

    let messages = [
        modern(
            1,
            "tools/call",
            json!({ "name": "price", "arguments": { "ticker": "SLOW" } }),
        ),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        modern(2, "tools/list", json!({})),
    ];
    for message in &messages {
        client
            .write_all(format!("{message}\n").as_bytes())
            .await
            .unwrap();
    }
    client.write_all(b"this is not json\n").await.unwrap();

    let mut lines = BufReader::new(&mut client).lines();
    let mut responses = Vec::new();
    for _ in 0..3 {
        let line = lines.next_line().await.unwrap().unwrap();
        responses.push(serde_json::from_str::<Value>(&line).unwrap());
    }

    // The quick tools/list is not held up by the slow (timed-out) tool call.
    assert_eq!(responses[0]["id"], 2);
    let parse_error = responses.iter().find(|r| r["id"].is_null()).unwrap();
    assert_eq!(parse_error["error"]["code"], -32700);
    let slow = responses.iter().find(|r| r["id"] == 1).unwrap();
    assert_eq!(slow["result"]["isError"], true);

    drop(lines);
    drop(client);
    serving.await.unwrap().unwrap();
}
