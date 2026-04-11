//! Channel SDK for protoclaw.
//!
//! Provides the [`Channel`] trait for building messaging integrations and
//! [`ChannelHarness`] for JSON-RPC stdio framing, handshake, and message routing.
#![warn(missing_docs)]

/// Permission request/response oneshot management.
pub mod broker;
/// Extract displayable text from agent content values.
pub mod content;
/// Error types for channel SDK operations.
pub mod error;
/// JSON-RPC stdio harness that drives a [`Channel`] implementation.
pub mod harness;
/// Test wrapper for unit-testing [`Channel`] implementations without JSON-RPC framing.
pub mod testing;
/// The [`Channel`] trait that channel authors implement.
pub mod trait_def;

pub use broker::PermissionBroker;
pub use content::content_to_string;
pub use error::ChannelSdkError;
pub use harness::ChannelHarness;
pub use protoclaw_sdk_types::{
    ChannelAckConfig, ChannelCapabilities, ChannelSendMessage, DeliverMessage, PeerInfo,
};
pub use trait_def::Channel;
