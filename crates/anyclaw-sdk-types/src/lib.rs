//! Shared types for the anyclaw agent-channel-tool SDK.
//!
//! This crate provides the wire types used by all three SDK implementation crates
//! (`anyclaw-sdk-agent`, `anyclaw-sdk-channel`, `anyclaw-sdk-tool`) and
//! the internal anyclaw supervisor.
//!
//! All serializable types use `camelCase` JSON field names.
//!
//! # Stability
//!
//! This crate is **unstable** — types, enums, and wire formats may change between releases.
//! Enums marked `#[non_exhaustive]` will have new variants added; match arms must include `_`.
#![warn(missing_docs)]

/// ACP (Agent Client Protocol) wire types for supervisor↔agent communication.
// Remaining Value usages: D-03 extensible fields (HashMap<String, Value>) and Vec<Value>
#[allow(clippy::disallowed_types)]
pub mod acp;
/// Channel protocol wire types (capabilities, initialize, deliver, send, ack, content).
// Remaining Value usages: DeliverMessage.content pass-through, D-03 extensible fields, ContentKind dispatch
#[allow(clippy::disallowed_types)]
pub mod channel;
/// Agent→channel bridge events routed through the supervisor.
// Remaining Value usages: DeliverMessage.content pass-through from agents manager
#[allow(clippy::disallowed_types)]
pub mod channel_event;
/// Permission prompt types (request, response, options).
pub mod permission;
/// Session routing key encoding channel + conversation identity.
pub mod session_key;

pub use acp::*;
pub use channel::*;
pub use channel_event::*;
pub use permission::*;
pub use session_key::*;

#[cfg(test)]
mod proptest_impls;
