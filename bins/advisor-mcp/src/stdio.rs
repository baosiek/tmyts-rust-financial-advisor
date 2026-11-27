//! The stdio transport: one JSON-RPC message per line on stdin, one per line on
//! stdout. stdout carries nothing else (logs go to stderr).

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::McpServer;
use crate::json_rpc::error;
use crate::protocol::error_code;

/// Serves until the client closes its end (EOF on `reader`).
///
/// Each request runs in its own task, so a slow tool call does not hold up
/// other requests; responses are written by one task, whole lines at a time,
/// so they never interleave. Generic over reader and writer so tests can drive
/// it with in-memory pipes instead of the process's stdin/stdout.
pub async fn serve<R, W>(server: Arc<McpServer>, reader: R, mut writer: W) -> std::io::Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (responses, mut outgoing) = mpsc::unbounded_channel::<Value>();

    let write_task = tokio::spawn(async move {
        while let Some(response) = outgoing.recv().await {
            let mut line = response.to_string();
            line.push('\n');
            writer.write_all(line.as_bytes()).await?;
            writer.flush().await?;
        }
        Ok::<_, std::io::Error>(())
    });

    let mut requests = JoinSet::new();
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let responses = responses.clone();
        let server = server.clone();
        requests.spawn(async move {
            let response = match serde_json::from_str::<Value>(&line) {
                Ok(message) => server.handle(message).await,
                Err(e) => Some(error(
                    Value::Null,
                    error_code::PARSE_ERROR,
                    format!("parse error: {e}"),
                    None,
                )),
            };
            if let Some(response) = response {
                // Only fails if the writer stopped, i.e. the client is gone.
                let _ = responses.send(response);
            }
        });
    }

    // Input closed: let running requests finish, then flush their responses.
    while requests.join_next().await.is_some() {}
    drop(responses);
    write_task
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?
}
