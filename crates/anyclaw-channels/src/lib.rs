// D-03: ChannelEvent content and JSON-RPC method params are arbitrary agent/protocol JSON
#[allow(clippy::disallowed_types)]
pub mod connection;
pub mod debug_http;
pub mod error;
// D-03: ChannelEvent content, channel protocol params, and permission payloads are arbitrary JSON
#[allow(clippy::disallowed_types)]
pub mod manager;
pub mod session_queue;

pub use connection::*;
pub use debug_http::DebugHttpChannel;
pub use error::*;
pub use manager::{ChannelsCommand, ChannelsManager};
