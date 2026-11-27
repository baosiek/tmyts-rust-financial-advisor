//! MCP server over stdio. An MCP client starts this program and talks to it
//! through stdin/stdout:
//!
//!   cargo build --release -p advisor-mcp
//!   scripts/advisor-mcp.sh        # what an MCP client should launch
//!
//! It needs the settings file and the services of the enabled tools (Postgres,
//! embeddings, Qdrant), but no LLM API key: the client brings its own model.

use std::process::ExitCode;
use std::sync::Arc;

use advisor_app::build_checked_tools;
use advisor_app::config::Settings;
use advisor_mcp::{McpServer, McpServerConfig, serve};
use tokio::io::BufReader;

fn main() -> ExitCode {
    let settings = match Settings::load() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Logs go to stderr; MCP clients usually show a server's stderr in their logs.
    if let Err(e) = advisor_observability::init_tracing(&settings.logging.filter) {
        eprintln!("error: {e}");
        return ExitCode::FAILURE;
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(settings.runtime.worker_threads)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::error!("cannot start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(settings)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(settings: Settings) -> Result<(), Box<dyn std::error::Error>> {
    let mcp = settings
        .mcp
        .clone()
        .ok_or("the settings file has no [mcp] section")?;

    let get_key = |key: &str| std::env::var(key).ok();
    let tools = build_checked_tools(&settings, get_key).await?;
    tracing::info!(tools = tools.len(), "mcp server ready on stdio");

    let server = Arc::new(McpServer::new(
        Arc::new(tools),
        McpServerConfig {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            instructions: mcp.instructions.clone(),
            tool_timeout: mcp.tool_timeout(),
            cache_ttl: mcp.cache_ttl(),
        },
    ));

    serve(
        server,
        BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
    )
    .await?;
    tracing::info!("client closed the connection, shutting down");
    Ok(())
}
