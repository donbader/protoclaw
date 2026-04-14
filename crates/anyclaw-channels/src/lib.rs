#![warn(missing_docs)]

//! Channel subprocess routing and lifecycle management.
//!
//! Manages channel subprocesses (Telegram, debug-http, etc.) with per-channel
//! crash isolation and session-keyed routing between channels and the agent.

// D-03: ChannelEvent content and JSON-RPC method params are arbitrary agent/protocol JSON
#[allow(clippy::disallowed_types)]
/// [`ChannelConnection`] — subprocess spawn, JSON-RPC framing, port discovery.
pub mod connection;
/// In-process debug HTTP channel (not a subprocess).
pub mod debug_http;
/// [`ChannelsError`] — channel-level errors.
pub mod error;
// D-03: ChannelEvent content, channel protocol params, and permission payloads are arbitrary JSON
#[allow(clippy::disallowed_types)]
/// [`ChannelsManager`] — routing table, crash isolation, poll loop.
pub mod manager;
/// Per-session FIFO message queue with two-phase collect+flush.
pub mod session_queue;

pub use connection::*;
pub use debug_http::DebugHttpChannel;
pub use error::*;
pub use manager::{ChannelsCommand, ChannelsManager};
