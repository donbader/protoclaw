#![warn(missing_docs)]

//! JSON-RPC 2.0 codec and types for line-delimited (NDJSON) framing over stdio.
//!
//! This crate is pure framing — it knows how to read and write JSON-RPC messages
//! but has no knowledge of ACP methods or protocol semantics. Method-specific
//! handling belongs in `anyclaw-agents`.

/// NDJSON codec: one JSON-RPC message per line, newline-delimited.
pub mod codec;
/// Framing-level error types (oversized frames, invalid JSON, I/O errors).
pub mod error;
// Extensible Value fields: params/result/data schemas are method-defined (D-03).
// clippy::disallowed_types fires on inner type expressions, not suppressible at struct/field level.
#[allow(clippy::disallowed_types)]
/// JSON-RPC 2.0 wire types: [`JsonRpcRequest`], [`JsonRpcResponse`], [`JsonRpcError`], [`JsonRpcMessage`].
pub mod types;

pub use codec::*;
pub use error::*;
pub use types::*;
