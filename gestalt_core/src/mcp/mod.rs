//! MCP (Model Context Protocol) Module
//!
//! Provides MCP client implementation and tool execution.

pub mod client_impl;
pub mod registry;

pub use client_impl::{GestaltMcpClient, McpClient, McpError, ToolCall, ToolInfo, ToolResult};

pub use registry::{DefaultToolContext, McpRegistry, ToolContext};
