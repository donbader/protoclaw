pub mod acp_error;
pub mod acp_types;
pub mod backend;
pub mod connection;
pub mod docker_backend;
pub mod error;
pub mod local_backend;
pub mod manager;
pub mod slot;

pub use backend::ProcessBackend;
pub use docker_backend::DockerBackend;
pub use error::*;
pub use local_backend::LocalBackend;
pub use manager::*;
pub use protoclaw_core::{
    AgentStatusInfo, AgentsCommand, McpServerUrl, PendingPermissionInfo, ToolsCommand,
};
pub use slot::*;
