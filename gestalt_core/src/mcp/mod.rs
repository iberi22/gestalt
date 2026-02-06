//! MCP (Model Context Protocol) Module
//!
//! Provides MCP client implementation and tool execution.

pub mod client_impl;
pub mod registry;

pub use client_impl::{
    McpClient,
    McpError,
    ToolCall,
    ToolInfo,
    ToolResult,
    GestaltMcpClient,
};

pub use registry::{
    McpRegistry,
    ToolContext,
    DefaultToolContext,
};
