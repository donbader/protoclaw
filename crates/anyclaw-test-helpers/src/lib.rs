#![warn(missing_docs)]

//! Shared test utilities used across all crate tests and integration tests.

// Grandfathered: typed replacement in Phase 2-4
#[allow(clippy::disallowed_types)]
/// Pre-built `AnyclawConfig` fixtures for tests (mock-agent, debug-http, etc.).
pub mod config;
/// Helper to create `ManagerHandle` + receiver pairs for tests.
pub mod handles;
/// Workspace-relative paths to built test binaries (mock-agent, debug-http, etc.).
pub mod paths;
/// Port discovery helper: poll a `watch::Receiver<u16>` until a non-zero port appears.
pub mod poll;
/// Port discovery helper: wait for a `watch::Receiver<u16>` to emit a non-zero port.
pub mod ports;
/// SSE (Server-Sent Events) collector for integration tests.
pub mod sse;
/// Supervisor boot helper that waits for all managers to start and returns the debug-http port.
pub mod supervisor;
/// Async timeout wrapper that panics with a clear message on expiry.
pub mod timeout;

pub use config::*;
pub use handles::*;
pub use paths::*;
pub use poll::*;
pub use ports::*;
pub use sse::*;
pub use supervisor::*;
/// Re-export of `test_log` for tracing-aware test logging.
pub use test_log;
pub use timeout::*;
