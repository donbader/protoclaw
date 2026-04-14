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
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
pub mod server;
/// The [`Tool`] trait that tool authors implement.
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
pub mod trait_def;

pub use error::ToolSdkError;
pub use server::ToolServer;
pub use trait_def::{DynTool, Tool};
