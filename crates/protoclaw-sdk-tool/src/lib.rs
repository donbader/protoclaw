//! Tool SDK for protoclaw.
//!
//! Provides the [`Tool`] trait for building MCP-compatible tools and
//! [`ToolServer`] for serving them over stdio.
#![warn(missing_docs)]

/// Error types for tool SDK operations.
pub mod error;
/// MCP tool server that dispatches to [`Tool`] implementations.
pub mod server;
/// The [`Tool`] trait that tool authors implement.
pub mod trait_def;

pub use error::ToolSdkError;
pub use server::ToolServer;
pub use trait_def::{DynTool, Tool};
