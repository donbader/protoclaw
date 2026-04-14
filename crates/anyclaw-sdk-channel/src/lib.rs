//! Channel SDK for anyclaw.
//!
//! Provides the [`Channel`] trait for building messaging integrations and
//! [`ChannelHarness`] for JSON-RPC stdio framing, handshake, and message routing.
//!
//! # Stability
//!
//! This crate is **unstable** — APIs may change between releases.
//! Enums marked `#[non_exhaustive]` will have new variants added; match arms must include `_`.
#![warn(missing_docs)]

/// Permission request/response oneshot management.
pub mod broker;
/// Extract displayable text from agent content values.
// D-03 boundary: DeliverMessage.content is Value — agent content shape is agent-defined
#[allow(clippy::disallowed_types)]
pub mod content;
/// Error types for channel SDK operations.
pub mod error;
/// JSON-RPC stdio harness that drives a [`Channel`] implementation.
pub mod harness;
/// Test wrapper for unit-testing [`Channel`] implementations without JSON-RPC framing.
pub mod testing;
/// The [`Channel`] trait that channel authors implement.
// D-03 boundary: handle_unknown params/return are Value — unknown methods have no schema
#[allow(clippy::disallowed_types)]
pub mod trait_def;

pub use anyclaw_sdk_types::{
    ChannelAckConfig, ChannelCapabilities, ChannelSendMessage, DeliverMessage, PeerInfo,
};
pub use broker::PermissionBroker;
pub use content::content_to_string;
pub use error::ChannelSdkError;
pub use harness::ChannelHarness;
pub use trait_def::Channel;
