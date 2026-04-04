# protoclaw-core — Shared Primitives

Foundation crate used by all internal crates. Defines the Manager contract, resilience primitives, and cross-manager message types.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `Manager` trait + `ManagerHandle<C>` typed command sender |
| `backoff.rs` | `ExponentialBackoff` (100ms→30s) + `CrashTracker` (N crashes in window) |
| `error.rs` | `SupervisorError` + `ManagerError` — all `thiserror` |
| `types.rs` | ID newtypes (`SessionKey`, `AgentId`, `ChannelId`), `SessionKey::new(channel, kind, peer)` |
| `message.rs` | Internal message envelope types |
| `channel_event.rs` | `ChannelEvent` enum — agents→channels bridge |
| `constants.rs` | Named constants: internal guards (`POLL_INTERVAL_MS`, `CMD_CHANNEL_CAPACITY`) and default values (`DEFAULT_BACKOFF_BASE_MS`, `DEFAULT_CRASH_MAX`) |

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

## ChannelEvent (WHY it's here)

`ChannelEvent` is the agents→channels message type. It lives in `protoclaw-core` (not agents or channels) because both crates need it — putting it in either would create a circular dependency. Two variants:
- `DeliverMessage { session_key, content }` — forward agent response to channel
- `RoutePermission { session_key, request_id, description, options }` — forward permission request

## Backoff Defaults (tested explicitly)

- `ExponentialBackoff::default()` = 100ms base, 30s cap, doubles each attempt
- `CrashTracker::default()` = 5 crashes within 60s = crash loop

Do not change defaults without updating tests in `backoff.rs`.

## SessionKey Format

`"{channel_name}:{kind}:{peer_id}"` — used as routing key in both agents and channels managers.
