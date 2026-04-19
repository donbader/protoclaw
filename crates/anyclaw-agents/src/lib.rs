#![warn(missing_docs)]

//! ACP protocol layer and agent subprocess management.
//!
//! Manages agent subprocess lifecycles, implements the Agent Client Protocol (ACP)
//! over JSON-RPC 2.0 stdio, and handles session mapping, crash recovery, and
//! filesystem sandboxing.

/// ACP protocol-level errors (version mismatch, session not found, transport failures).
pub mod acp_error;
/// ACP wire type re-exports from `anyclaw-sdk-types` for backward compatibility.
pub mod acp_types;
/// [`ProcessBackend`] trait abstracting over local and Docker subprocess management.
pub mod backend;
/// [`connection::AgentConnection`] — subprocess spawn, typed JSON-RPC framing, direct bridge to manager.
pub mod connection;
// D-03: commands.rs handles session content via prompt_session (completion_tx, channels_sender forwarding)
#[allow(clippy::disallowed_types)]
/// Command dispatch, session CRUD, and prompt handling.
pub(crate) mod commands;
/// [`DockerBackend`] — Docker container lifecycle via bollard.
pub mod docker_backend;
/// [`AgentsError`] — manager-level errors (spawn, timeout, connection).
pub mod error;
/// Filesystem sandboxing: path validation and agent FS request handlers.
pub(crate) mod fs_sandbox;
// D-03: incoming.rs handles session/update content mutation (timestamps, tool normalization, command injection)
#[allow(clippy::disallowed_types)]
/// Incoming message dispatch, session update forwarding, and tool event normalization.
pub(crate) mod incoming;
/// [`LocalBackend`] — native subprocess lifecycle via `tokio::process::Child`.
pub mod local_backend;
// D-03: manager.rs manipulates arbitrary agent content (timestamps, tool normalization, command injection)
#[allow(clippy::disallowed_types)]
/// [`AgentsManager`] — session lifecycle, command handling, crash recovery.
pub mod manager;
// D-03: platform_commands_json() serializes typed structs to Value for agent content merging
#[allow(clippy::disallowed_types)]
/// Typed platform commands with serialization boundary for agent content merging.
pub mod platform_commands;
/// SDK-based agent runner using `ClientSideConnection` on a dedicated `LocalSet` thread.
#[allow(dead_code)] // Not wired into AgentsManager yet — follow-up PR
pub(crate) mod sdk_runner;
/// Per-session FIFO message queue (migrated from channels — agent concurrency concern).
pub(crate) mod session_queue;
/// Crash recovery, session restore, and stale container cleanup.
pub(crate) mod session_recovery;
// D-03: last_available_commands stores arbitrary agent-reported availableCommands payload
#[allow(clippy::disallowed_types)]
/// [`AgentSlot`] — per-agent state: session maps, capabilities, pending permissions.
pub mod slot;

pub use anyclaw_core::{
    AgentStatusInfo, AgentsCommand, McpServerUrl, PendingPermissionInfo, ToolsCommand,
};
pub use backend::ProcessBackend;
pub use docker_backend::DockerBackend;
pub use error::*;
pub use local_backend::LocalBackend;
pub use manager::*;
pub use slot::*;
