//! MCP (Model Context Protocol) module
//!
//! Provides MCP server functionality for external AI agent control of Burd.
//! The MCP server communicates via stdio (JSON-RPC) and calls the Burd HTTP API.

pub mod client;
pub mod protocol;
pub mod server;
pub mod tools;
