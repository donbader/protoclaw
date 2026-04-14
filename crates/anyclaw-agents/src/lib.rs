pub mod acp_error;
pub mod acp_types;
pub mod backend;
pub mod connection;
pub mod docker_backend;
pub mod error;
pub mod local_backend;
// D-03: manager.rs manipulates arbitrary agent content (timestamps, tool normalization, command injection)
#[allow(clippy::disallowed_types)]
pub mod manager;
// D-03: platform_commands_json() serializes typed structs to Value for agent content merging
#[allow(clippy::disallowed_types)]
pub mod platform_commands;
// D-03: last_available_commands stores arbitrary agent-reported availableCommands payload
#[allow(clippy::disallowed_types)]
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
