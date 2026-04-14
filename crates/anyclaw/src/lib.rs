#![warn(missing_docs)]

//! Anyclaw binary crate: CLI entry point, config loading, and supervisor bootstrap.

/// ASCII startup banner showing configured agents, channels, and tools.
pub mod banner;
/// Clap-derived CLI argument parsing.
pub mod cli;
/// `anyclaw init` — scaffold a default `anyclaw.yaml` config file.
pub mod init;
// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
/// `anyclaw status` — runtime health check via the admin HTTP endpoint.
pub mod status;
/// Re-export of the supervisor for test access.
pub mod supervisor;
