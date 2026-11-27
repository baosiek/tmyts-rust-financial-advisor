#!/usr/bin/env sh
# Launcher for MCP clients (e.g. Claude Desktop). MCP clients start servers from
# an unknown working directory; this moves to the project root first, so `.env`
# and the relative paths in it (ADVISOR_CONFIG=config/advisor.toml) work, and
# secrets stay in .env instead of the client's configuration file.
# Build first: cargo build --release -p advisor-mcp
set -eu
cd "$(dirname "$0")/.."
exec ./target/release/advisor-mcp
