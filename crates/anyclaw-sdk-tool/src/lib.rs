//! Tool SDK for anyclaw.
//!
//! Provides the [`Tool`] trait for building MCP-compatible tools and
//! [`ToolServer`] for serving them over stdio.
//!
//! # Stability
//!
//! This crate is **unstable** — APIs may change between releases.
//! Enums marked `#[non_exhaustive]` will have new variants added; match arms must include `_`.
#![warn(missing_docs)]

/// Error types for tool SDK operations.
pub mod error;
/// MCP tool server that dispatches to [`Tool`] implementations.
// D-03 boundary: Tool I/O is inherently dynamic — JSON Schema input, arbitrary JSON output.
// All Value usages in server.rs flow from the Tool trait contract being Value-based.
#[allow(clippy::disallowed_types)]
pub mod server;
/// The [`Tool`] trait that tool authors implement.
// D-03 boundary: Tool I/O uses Value because tool input is defined by a JSON Schema
// (no fixed Rust type) and tool output is arbitrary JSON. See trait_def.rs doc comments.
#[allow(clippy::disallowed_types)]
pub mod trait_def;

pub use error::ToolSdkError;
pub use server::ToolServer;
pub use trait_def::{DynTool, Tool};
