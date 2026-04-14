#![warn(missing_docs)]

//! MCP host, WASM sandbox, and tools manager.
//!
//! Manages tool availability: spawns external MCP servers, loads WASM-sandboxed
//! tools, and serves an aggregated MCP endpoint over HTTP.

/// [`ToolsError`] — tool-level errors.
pub mod error;
// D-03: Config options are arbitrary user-defined Values (HashMap<String, Value>)
#[allow(clippy::disallowed_types)]
/// [`ExternalMcpServer`] — spawns and communicates with external MCP server subprocesses.
pub mod external;
// D-03: Tool trait boundary (input_schema/execute use Value) + ToolsCommand args
#[allow(clippy::disallowed_types)]
/// [`ToolsManager`] + [`AggregatedToolServer`] (rmcp `ServerHandler` impl).
pub mod manager;
// D-03: Tool dispatch args are arbitrary JSON (serde_json::Map<String, Value>)
#[allow(clippy::disallowed_types)]
/// [`McpHost`] — registry for native `Tool` impls, dispatches calls.
pub mod mcp_host;
// D-03: WASM sandbox config options are arbitrary user-defined Values
#[allow(clippy::disallowed_types)]
/// [`WasmToolRunner`] — shared wasmtime `Engine`, per-invocation `Store` with fuel/WASI.
pub mod wasm_runner;
// D-03: Tool trait boundary — input_schema/execute use Value for arbitrary tool schemas
#[allow(clippy::disallowed_types)]
/// [`WasmTool`] — wraps a WASM module as a `Tool` impl.
pub mod wasm_tool;

pub use anyclaw_core::{McpServerUrl, ToolsCommand};
pub use error::*;
pub use external::ExternalMcpServer;
pub use manager::*;
pub use mcp_host::McpHost;
pub use wasm_runner::WasmToolRunner;
pub use wasm_tool::WasmTool;
