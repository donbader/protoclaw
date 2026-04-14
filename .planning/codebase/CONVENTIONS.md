# Coding Conventions

**Analysis Date:** 2026-04-14

## Error Handling

**Boundary rule:** `thiserror` for all library crates, `anyhow` only at application entry points.

**Library crate pattern** (every crate under `crates/` except `anyclaw`):
Use `#[derive(Debug, thiserror::Error)]` enums with `#[error("...")]` format strings.

```rust
// crates/anyclaw-agents/src/error.rs
#[derive(Debug, Error)]
pub enum AgentsError {
    #[error("Failed to spawn agent process: {0}")]
    SpawnFailed(String),
    #[error("ACP protocol error: {0}")]
    Protocol(#[from] crate::acp_error::AcpError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

Each manager crate has its own error enum: `AgentsError` (`crates/anyclaw-agents/src/error.rs`), `ChannelsError` (`crates/anyclaw-channels/src/error.rs`), `ToolsError` (`crates/anyclaw-tools/src/error.rs`), `ConfigError` (`crates/anyclaw-config/src/error.rs`), `FramingError` (`crates/anyclaw-jsonrpc/src/error.rs`). Core defines `SupervisorError` and `ManagerError` (`crates/anyclaw-core/src/error.rs`). SDK crates: `AgentSdkError`, `ChannelSdkError`, `ToolSdkError`.

**anyhow usage:** Only in `crates/anyclaw/src/main.rs`, `crates/anyclaw-supervisor/src/lib.rs`, `crates/anyclaw/src/init.rs`, `crates/anyclaw/src/status.rs`, and test helper `crates/anyclaw-test-helpers/src/supervisor.rs`.

**unwrap rules:**
- Production code: use `.expect("reason")` for true invariants, `?` for fallible paths
- Prefer `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` over bare `unwrap_or_default()` to make silent fallbacks visible in logs
- Bare `.unwrap()` is allowed only in tests

## Module Organization

**Flat `lib.rs` pattern** — every crate uses `pub mod` + `pub use` re-exports in `lib.rs`. No `mod.rs` files anywhere.

```rust
// crates/anyclaw-core/src/lib.rs
pub mod agents_command;
pub mod backoff;
pub mod constants;
pub mod error;
pub mod manager;
// ...
pub use agents_command::*;
pub use backoff::*;
pub use error::*;
pub use manager::*;
```

**Re-export pattern for relocated types:** When types move between crates, the old crate re-exports for backward compat:
```rust
// crates/anyclaw-core/src/lib.rs
pub use anyclaw_sdk_types::ChannelEvent;
pub use anyclaw_sdk_types::SessionKey;
```

## Naming Conventions

**Types:** PascalCase. Enum variants are PascalCase. Newtype wrappers for IDs: `SessionId`, `ChannelId`, `ManagerId`, `MessageId` (`crates/anyclaw-core/src/types.rs`). Use `impl_id_type!` macro for shared `Display`, `From<String>`, `From<&str>`, `AsRef<str>` impls.

**Modules:** snake_case, one concept per file. File names match the primary type: `backoff.rs` → `ExponentialBackoff`, `manager.rs` → `*Manager`, `error.rs` → `*Error`.

**Functions:** snake_case. Constructors use `new()`. Builder-style: `with_*()` (e.g., `with_native_tools()`, `with_wasm_configs()`).

**Constants:** SCREAMING_SNAKE_CASE. All named constants live in `crates/anyclaw-core/src/constants.rs`. Internal guards (not user-configurable) vs default values (mirrored in config serde defaults) are separated by comments.

**Config struct fields:** snake_case in Rust, camelCase in JSON wire format via `#[serde(rename_all = "camelCase")]` on SDK types. Config YAML uses snake_case.

**Test naming (BDD):**
- `when_action_then_result` — most common
- `given_precondition_when_action_then_result` — when setup matters
- Fixture functions: `fn given_*()` (rstest fixtures)
- Parameterized cases: `#[case::label_name]`

```rust
// Example from crates/anyclaw-core/src/backoff.rs
#[test]
fn when_new_backoff_created_then_initial_delay_is_100ms() { ... }

#[test]
fn given_rapid_crashes_when_threshold_exceeded_then_crash_loop_detected() { ... }
```

## Async Patterns

**Runtime:** tokio (multi-threaded). All managers are async. Entry point uses `#[tokio::main]`.

**Channel communication:** `tokio::sync::mpsc` exclusively for cross-manager messaging. No `Arc<Mutex<>>` across manager boundaries. `ManagerHandle<C>` (`crates/anyclaw-core/src/manager.rs`) wraps `mpsc::Sender<C>` as the typed command sender.

```rust
// crates/anyclaw-core/src/manager.rs
pub struct ManagerHandle<C: Send + 'static> {
    sender: mpsc::Sender<C>,
}
```

**Cancellation:** `tokio_util::sync::CancellationToken` for graceful shutdown. Passed to `Manager::run(cancel)`. Managers use `tokio::select!` to race cancel signal against work.

**Manager lifecycle:** Always `start().await?` then `run(cancel).await`. Both required, in order. `cmd_rx` is consumed via `.take()` on first `run()` — never call `run()` twice.

**Watch channels:** `tokio::sync::watch` for port discovery (`crates/anyclaw-test-helpers/src/ports.rs`). Channel subprocesses emit `PORT:{n}` to stderr.

## Trait Patterns

**Manager trait** (`crates/anyclaw-core/src/manager.rs`):
```rust
pub trait Manager: Send + 'static {
    type Command: Send + 'static;
    fn name(&self) -> &str;
    fn start(&mut self) -> impl Future<Output = Result<(), ManagerError>> + Send;
    fn run(self, cancel: CancellationToken) -> impl Future<Output = Result<(), ManagerError>> + Send;
    fn health_check(&self) -> impl std::future::Future<Output = bool> + Send;
}
```

**SDK traits** — each SDK crate defines one primary trait for external implementors:
- `AgentAdapter` (`crates/anyclaw-sdk-agent/src/adapter.rs`) — per-method hooks with default passthrough impls
- `Channel` (`crates/anyclaw-sdk-channel/src/trait_def.rs`) — messaging integration lifecycle
- `Tool` (`crates/anyclaw-sdk-tool/src/trait_def.rs`) — MCP tool execution

**Pattern:** SDK traits use `async_trait` or RPITIT. Each has a harness/server that handles JSON-RPC framing — implementors only write business logic. Default method implementations provide passthrough behavior (e.g., `GenericAcpAdapter`).

**Dyn-compatible wrappers:** `DynAgentAdapter`, `DynTool` for trait object usage where needed.

## Logging & Observability

**Framework:** `tracing` exclusively. No `println!`, `eprintln!`, or `log` crate in library code. Exception: CLI entry points may use `println!`/`eprintln!` before tracing is initialized.

**Instrumentation:** Key functions annotated with `#[tracing::instrument]` for automatic span creation:
```rust
// crates/anyclaw-agents/src/manager.rs
#[tracing::instrument(skip(slot), fields(agent = %slot.name()))]
async fn initialize_agent(slot: &mut AgentSlot) -> Result<...> { ... }
```

**Log levels used:**
- `tracing::error!` — crash loops, fatal failures, respawn failures
- `tracing::warn!` — recoverable errors, fallback paths, deserialization failures, missing routing entries
- `tracing::info!` — lifecycle events (session started, agent recovered, manager running/stopped), permission flow
- `tracing::debug!` — message routing, session updates, per-event tracing

**Structured fields:** Use `field = %value` syntax for structured logging. Common fields: `agent`, `session_key`, `session_id`, `channel`, `request_id`, `error`, `manager`.

## Configuration Patterns

**Figment layering** (`crates/anyclaw-config/src/lib.rs`):
```rust
Figment::from(Yaml::string(DEFAULTS_YAML))    // 1. Embedded defaults
    .merge(SubstYaml::file(path))               // 2. User YAML (with ${VAR} substitution)
    .merge(Env::prefixed("ANYCLAW_").split("__"))  // 3. Env vars
    .extract()
```

**Serde derive pattern:** All config structs use `#[derive(Debug, Clone, Serialize, Deserialize)]` with `#[serde(default)]` for optional sections. Defaults live in `defaults.yaml` or `#[serde(default = "fn_name")]` functions in `crates/anyclaw-config/src/types.rs`.

**Entity naming:** Config uses named `HashMap`s — entity names are map keys, not fields in structs. No `name` field on `AgentConfig`, `ChannelConfig`, etc.

**Tagged enums:** `WorkspaceConfig` uses `#[serde(tag = "type")]` for `Local` vs `Docker` variants.

**Env var format:** `ANYCLAW_SUPERVISOR__SHUTDOWN_TIMEOUT_SECS=60` (double underscore = nesting).

## Code Quality Indicators

**No `unsafe`:** Zero unsafe blocks exist. Do not introduce any.

**Clippy:** `cargo clippy --workspace` enforced. All crates pass without warnings.

**Documentation:** SDK crates use `#![warn(missing_docs)]` (`crates/anyclaw-sdk-types/src/lib.rs`, `crates/anyclaw-sdk-agent/src/lib.rs`, `crates/anyclaw-sdk-channel/src/lib.rs`, `crates/anyclaw-sdk-tool/src/lib.rs`). Internal crates have doc comments on public items but don't enforce `missing_docs`.

**Serde convention:** All SDK wire types use `#[serde(rename_all = "camelCase")]` for JSON. Rust fields are `snake_case`, JSON is `camelCase`. Tests verify round-trip serialization.

**`#[non_exhaustive]`:** SDK enums are marked `#[non_exhaustive]` — match arms must include `_` wildcard.

---

*Convention analysis: 2026-04-14*
