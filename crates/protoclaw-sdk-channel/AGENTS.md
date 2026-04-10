# protoclaw-sdk-channel — Channel SDK

SDK for building channel extensions. Provides the `Channel` trait for business logic and `ChannelHarness` that handles all JSON-RPC stdio framing, initialize handshake, and bidirectional message routing.

## Files

| File | Purpose |
|------|---------|
| `trait_def.rs` | `Channel` trait — implement this to build a channel |
| `harness.rs` | `ChannelHarness<C>` — JSON-RPC stdio loop, dispatches to `Channel` methods |
| `broker.rs` | `PermissionBroker` — register/resolve helper for permission oneshot management |
| `testing.rs` | `ChannelTester<C>` — typed test wrapper that bypasses JSON-RPC framing |
| `content.rs` | `content_to_string` — extract displayable text from agent content |
| `error.rs` | `ChannelSdkError` enum (thiserror) |
| `lib.rs` | Re-exports `Channel`, `ChannelHarness`, `ChannelSdkError`, `PermissionBroker`, and sdk-types |

## Key Types

```rust
#[async_trait]
pub trait Channel: Send + 'static {
    fn capabilities(&self) -> ChannelCapabilities;
    async fn on_initialize(&mut self, params: ChannelInitializeParams) -> Result<(), ChannelSdkError>;
    async fn on_ready(&mut self, outbound: mpsc::Sender<ChannelSendMessage>) -> Result<(), ChannelSdkError>;
    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError>;
    async fn request_permission(&mut self, req: ChannelRequestPermission) -> Result<PermissionResponse, ChannelSdkError>;
    async fn handle_unknown(&mut self, method: &str, params: Value) -> Result<Value, ChannelSdkError>;
    async fn on_session_created(&mut self, msg: SessionCreated) -> Result<(), ChannelSdkError>;
}

pub struct ChannelHarness<C: Channel> { channel: C }
```

## How to Implement

1. Create a struct implementing `Channel`
2. Return capabilities in `capabilities()` (streaming, rich_text)
3. In `on_ready()`, store the `outbound` sender — use it to send user messages to protoclaw
4. Implement `deliver_message()` to render agent responses to your platform
5. Implement `request_permission()` to show permission prompts and return user's choice
6. Wrap in `ChannelHarness::new(my_channel).run_stdio().await`

**Default implementations:** `on_initialize()`, `handle_unknown()`, and `on_session_created()` have defaults — override only if needed.

## Harness Lifecycle

1. **Startup** — `ChannelHarness::run_stdio()` or `run(reader, writer)` begins the event loop
2. **Initialize** — Harness receives `initialize` request, calls `capabilities()` + `on_initialize()` + `on_ready()`, responds with protocol version and capabilities
3. **Run loop** — `tokio::select!` on stdin lines and outbound channel:
   - Inbound JSON-RPC → dispatched to `Channel` methods
   - Outbound `ChannelSendMessage` → serialized as `channel/sendMessage` notification
4. **Shutdown** — stdin EOF breaks the loop, harness returns `Ok(())`

## Anti-Patterns (this crate)

- **Don't depend on internal crates** — this is external-facing SDK
- **Don't handle JSON-RPC manually** — the harness does all framing; implement `Channel` trait only
- **Don't block in trait methods** — all methods are async; use `.await` for I/O, don't block the tokio runtime

## v5.1 Changes

- Unwrap audit in harness serialization paths: `unwrap_or_default()` → `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` so silent fallbacks are visible in logs
