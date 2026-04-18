# anyclaw-core — Shared Primitives

Foundation crate used by all internal crates. Defines the Manager contract, resilience primitives, and cross-manager message types.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `Manager` trait + `ManagerHandle<C>` typed command sender |
| `backoff.rs` | `ExponentialBackoff` (100ms→30s) + `CrashTracker` (N crashes in window) |
| `error.rs` | `SupervisorError` + `ManagerError` — all `thiserror` |
| `types.rs` | ID newtypes (`SessionId`, `ChannelId`, `ManagerId`, `MessageId`) |
| `constants.rs` | Named constants: internal guards (`CMD_CHANNEL_CAPACITY`, `EVENT_CHANNEL_CAPACITY`) and default values (`DEFAULT_BACKOFF_BASE_MS`, `DEFAULT_CRASH_MAX`) |
| `agents_command.rs` | `AgentsCommand` enum for cross-manager dispatch (`EnqueueMessage`, `CancelSession`, `CreateSession`, etc.) |
| `tools_command.rs` | `ToolsCommand` enum for cross-manager dispatch |

## Manager Trait

```rust
pub trait Manager: Send + 'static {
    type Command: Send + 'static;
    fn name(&self) -> &str;
    fn start(&mut self) -> impl Future<Output = Result<(), ManagerError>> + Send;
    fn run(self, cancel: CancellationToken) -> impl Future<Output = Result<(), ManagerError>> + Send;
    fn health_check(&self) -> impl Future<Output = bool> + Send;
}
```

`start()` = sync setup (spawn subprocess, bind ports). `run()` = async event loop (consumes `self`). Both required, in order.

## ManagerHandle<C>

Typed wrapper around `mpsc::Sender<C>`. Only way to send commands across manager boundaries. Cloneable. `send()` returns `ManagerError::SendFailed` if channel closed.

## Re-exports from anyclaw-sdk-types

`ChannelEvent` and `SessionKey` were relocated to `anyclaw-sdk-types` in v5.0. `anyclaw-core` re-exports both for backward compatibility:
- `pub use anyclaw_sdk_types::ChannelEvent;`
- `pub use anyclaw_sdk_types::SessionKey;`

Crates that already depend on `anyclaw-sdk-types` should import directly from there.

## Backoff Defaults (tested explicitly)

- `ExponentialBackoff::default()` = 100ms base, 30s cap, doubles each attempt
- `CrashTracker::default()` = 5 crashes within 60s (short window) OR 10 crashes within 1 hour (long horizon) = crash loop
- Long horizon is configurable via `with_long_horizon(max_crashes, window)`

Do not change defaults without updating tests in `backoff.rs`.
