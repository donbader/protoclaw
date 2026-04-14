pub mod acp_error;
pub mod acp_types;
pub mod backend;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
pub mod connection;
pub mod docker_backend;
pub mod error;
pub mod local_backend;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
pub mod manager;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
pub mod platform_commands;
// Grandfathered: typed replacement in Phase 2-4
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
