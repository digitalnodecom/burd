//! MCP CLI command
//!
//! Runs the MCP (Model Context Protocol) server for AI agent integration.

use crate::mcp::server;

/// Run the MCP server
///
/// This starts an MCP server that communicates via stdio (JSON-RPC).
/// It requires the Burd desktop application to be running.
pub fn run_mcp() -> Result<(), String> {
    server::run_server()
}
