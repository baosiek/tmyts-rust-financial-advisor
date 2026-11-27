//! A Model Context Protocol (MCP) server for the advisor's tools.
//!
//! MCP lets any MCP client (Claude Desktop, IDEs, other agents) discover and
//! call tools. This server exposes the same `ToolRegistry` the agent uses, built
//! by `advisor-app`, so the tools, their configuration, health checks and the
//! prompt-injection screening of news are identical. It is written directly on
//! JSON-RPC, without an MCP SDK: the protocol part is small and every message
//! stays visible and testable.
//!
//! It is *dual-era*: it serves modern clients (protocol `2026-07-28`, stateless,
//! version in each request's `_meta`, `server/discover`) and legacy clients
//! (`2025-11-25` and earlier, `initialize` handshake).

mod json_rpc;
mod mcp_server;
mod protocol;
mod stdio;

pub use mcp_server::{McpServer, McpServerConfig};
pub use protocol::{LEGACY_VERSIONS, MODERN_VERSIONS};
pub use stdio::serve;
