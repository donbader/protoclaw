#![warn(missing_docs)]

//! Shared primitives for the anyclaw workspace.
//!
//! Defines the [`Manager`] trait contract, resilience primitives ([`ExponentialBackoff`],
//! [`CrashTracker`]), cross-manager command types, session persistence, and ID newtypes.
//! Every internal crate depends on `anyclaw-core`.

/// Cross-manager commands sent to the agents manager via [`ManagerHandle`].
pub mod agents_command;
/// Exponential backoff and crash-loop detection for manager restart resilience.
pub mod backoff;
/// Named constants for internal guards and default values.
pub mod constants;
/// Error types shared across the supervisor and manager layers.
pub mod error;
/// Runtime health snapshot types used by the admin `/health` endpoint.
pub mod health;
/// The [`Manager`] trait and [`ManagerHandle`] typed command sender.
pub mod manager;
/// Session persistence trait and no-op implementation.
pub mod session_store;
/// Per-manager slot lifecycle: cancel token, backoff, crash tracking, disabled flag.
pub mod slot_lifecycle;
/// SQLite-backed [`SessionStore`] implementation using rusqlite (bundled).
pub mod sqlite_session_store;
/// Cross-manager commands sent to the tools manager via [`ManagerHandle`].
pub mod tools_command;
/// ID newtypes for sessions, channels, managers, and messages.
pub mod types;

pub use agents_command::*;
pub use backoff::*;
pub use constants::*;
pub use error::*;
pub use health::*;
pub use manager::*;
pub use session_store::*;
pub use slot_lifecycle::*;
pub use sqlite_session_store::SqliteSessionStore;
pub use tools_command::*;
pub use types::*;

// LIMITATION: ChannelEvent canonical location
// ChannelEvent and SessionKey were relocated to anyclaw-sdk-types so external
// channel implementors can use them without depending on anyclaw-core. These
// re-exports exist solely for backward compatibility — internal crates that
// already depend on anyclaw-sdk-types should import directly from there.
// See also: AGENTS.md §Anti-Patterns
pub use anyclaw_sdk_types::ChannelEvent;
/// Re-exported from `anyclaw-sdk-types` for backward compatibility.
pub use anyclaw_sdk_types::SessionKey;
