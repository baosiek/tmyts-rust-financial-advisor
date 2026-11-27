//! Protocol constants: versions, `_meta` keys and error codes.

/// Stateless revisions: version, identity and capabilities in each request's `_meta`.
pub const MODERN_VERSIONS: [&str; 1] = ["2026-07-28"];

/// Handshake revisions (`initialize`), newest first. The newest is offered to
/// a legacy client that asks for a version this server does not know.
pub const LEGACY_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// JSON-RPC 2.0 error codes.
pub mod error_code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    /// MCP 2026-07-28: the requested protocol version is not supported.
    pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
}

/// Every version this server speaks, modern first.
pub fn supported_versions() -> Vec<&'static str> {
    MODERN_VERSIONS
        .iter()
        .chain(LEGACY_VERSIONS.iter())
        .copied()
        .collect()
}
