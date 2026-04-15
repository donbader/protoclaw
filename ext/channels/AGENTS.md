# ext/channels/ — Building Channel Extensions

Channel extensions are standalone binaries that connect anyclaw to messaging platforms (Telegram, Slack, HTTP, etc.). The `ChannelsManager` spawns them as child processes communicating over JSON-RPC 2.0 stdio.

The `anyclaw-sdk-channel` crate provides the `Channel` trait and `ChannelHarness` — implement the trait, wrap it in the harness, and the SDK handles all JSON-RPC framing, initialization, and bidirectional message routing.

## Why ext/ and not crates/

These are standalone binaries, not libraries. They depend on SDK crates but are architecturally separate — they're spawned as child processes with piped stdio. Putting them in `ext/` makes the subprocess boundary explicit.

## How It Works

```
User ──platform──▶ Your Channel Binary ──stdio──▶ ChannelsManager ──▶ AgentsManager
                   (ChannelHarness)                                       ▼
User ◀──platform── Your Channel Binary ◀──stdio── ChannelsManager ◀── Agent
```

1. `ChannelsManager` spawns your binary and sends `initialize`
2. `ChannelHarness` handles the handshake, calls your `on_initialize()` + `on_ready()`
3. User messages: your channel sends `channel/sendMessage` notifications via the `outbound` sender
4. Agent responses: the harness calls your `deliver_message()` with each update
5. Permissions: the harness calls `show_permission_prompt()`, you send `PermissionResponse` via `permission_tx`

## Implementing a Channel

### 1. Create the binary

```
ext/channels/my-channel/
├── Cargo.toml
└── src/
    └── main.rs
```

### Cargo.toml

```toml
[package]
name = "my-channel"
edition.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
anyclaw-sdk-channel = { workspace = true }
anyclaw-sdk-types = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

Add to workspace members in root `Cargo.toml`.

### main.rs — Channel trait implementation

```rust
use anyclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelHarness, ChannelSdkError, PermissionBroker};
use anyclaw_sdk_types::{
    ChannelInitializeParams, ChannelRequestPermission, ChannelSendMessage,
    ContentKind, DeliverMessage, PermissionResponse, SessionCreated,
};
use tokio::sync::mpsc;

struct MyChannel {
    outbound: Option<mpsc::Sender<ChannelSendMessage>>,
    permission_tx: Option<mpsc::Sender<PermissionResponse>>,
}

impl Channel for MyChannel {
    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            streaming: true,
            rich_text: false,
        }
    }

    async fn on_initialize(&mut self, params: ChannelInitializeParams) -> Result<(), ChannelSdkError> {
        // Extract config from params.options (e.g., API keys, host, port)
        Ok(())
    }

    async fn on_ready(
        &mut self,
        outbound: mpsc::Sender<ChannelSendMessage>,
        permission_tx: mpsc::Sender<PermissionResponse>,
    ) -> Result<(), ChannelSdkError> {
        self.outbound = Some(outbound);
        self.permission_tx = Some(permission_tx);
        // Start your platform listener (HTTP server, websocket, polling loop, etc.)
        Ok(())
    }

    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
        // Render agent response to your platform — see ContentKind dispatch below
        Ok(())
    }

    async fn show_permission_prompt(&mut self, req: ChannelRequestPermission) -> Result<(), ChannelSdkError> {
        // Display permission UI to user — return immediately, do NOT await user response
        // When user responds, send PermissionResponse through permission_tx
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ChannelHarness::new(MyChannel { outbound: None, permission_tx: None })
        .run_stdio()
        .await?;
    Ok(())
}
```

### Channel trait contract

| Method | Required | Purpose |
|--------|----------|---------|
| `capabilities()` | yes | Declare streaming + rich_text support |
| `on_initialize(params)` | default no-op | Extract config from `params.options` |
| `on_ready(outbound, permission_tx)` | yes | Store senders, start platform listener |
| `deliver_message(msg)` | yes | Render agent updates to your platform |
| `show_permission_prompt(req)` | yes | Display permission UI — must return immediately |
| `handle_unknown(method, params)` | default error | Handle custom JSON-RPC methods |
| `on_session_created(msg)` | default no-op | React to new session creation |

### Sending user messages to the agent

When a user sends a message on your platform, forward it via the `outbound` sender:

```rust
use anyclaw_sdk_types::{ChannelSendMessage, PeerInfo};

let msg = ChannelSendMessage {
    peer_info: PeerInfo {
        channel_name: "my-channel".into(),
        kind: "user".into(),
        peer_id: user_id.to_string(),
    },
    content: user_text.into(),
};
outbound.send(msg).await.ok();
```

The `PeerInfo` identifies the user — `channel_name` + `kind` + `peer_id` form a `SessionKey` that the supervisor uses for session routing.

### ContentKind dispatch in deliver_message

Use `ContentKind::from_content()` for typed dispatch over agent update types:

```rust
use anyclaw_sdk_types::ContentKind;

async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> {
    let kind = ContentKind::from_content(&msg.content);
    match kind {
        ContentKind::Thought(thought) => {
            // Agent's reasoning — thought.content has the text
        }
        ContentKind::MessageChunk { text } => {
            // Streaming response chunk
        }
        ContentKind::Result { text } => {
            // Final result — signals the turn is complete
        }
        ContentKind::UserMessageChunk { text } => {
            // User's message echoed back (includes merged prompt text)
        }
        ContentKind::ToolCall { name, tool_call_id, input } => {
            // Agent started a tool call
        }
        ContentKind::ToolCallUpdate { name, tool_call_id, status, output } => {
            // Tool call status changed (in_progress, completed, failed)
        }
        ContentKind::AvailableCommandsUpdate { commands } => {
            // Agent-provided slash command list
        }
        ContentKind::UsageUpdate | ContentKind::Unknown => {
            // Silently ignore
        }
    }
    Ok(())
}
```

For simple text extraction without typed dispatch:

```rust
use anyclaw_sdk_channel::content_to_string;
let text = content_to_string(&msg.content); // handles OpenCode wrapper + plain strings
```

### Permission handling

`show_permission_prompt()` must return immediately — do NOT block waiting for the user's response. Display the UI (inline keyboard, button, modal, etc.), then send the `PermissionResponse` asynchronously through `permission_tx` when the user responds.

For channels with async callback patterns (webhooks, button callbacks), use `PermissionBroker`:

```rust
use anyclaw_sdk_channel::PermissionBroker;
use std::sync::Arc;
use tokio::sync::Mutex;

// In your channel struct:
struct MyChannel {
    permission_broker: Arc<Mutex<PermissionBroker>>,
    permission_tx: Arc<Mutex<Option<mpsc::Sender<PermissionResponse>>>>,
    // ...
}

// In show_permission_prompt():
async fn show_permission_prompt(&mut self, req: ChannelRequestPermission) -> Result<(), ChannelSdkError> {
    self.permission_broker.lock().await.register(&req.request_id);
    // Display UI prompt to user (e.g., inline keyboard buttons)
    // Return immediately — do NOT await the user's response
    Ok(())
}

// In your callback handler (e.g., button click):
async fn handle_user_permission_response(state: &MyChannel, request_id: &str, option_id: &str) {
    state.permission_broker.lock().await.resolve(request_id, option_id);
    if let Some(tx) = state.permission_tx.lock().await.as_ref() {
        tx.send(PermissionResponse {
            request_id: request_id.to_string(),
            option_id: option_id.to_string(),
        }).await.ok();
    }
}
```

### Port discovery (for HTTP-based channels)

If your channel binds an HTTP server, emit `PORT:{n}` to stderr so the supervisor can discover the port:

```rust
eprintln!("PORT:{}", listener.local_addr()?.port());
```

The `ChannelsManager` watches stderr for this pattern and exposes the port via a `watch::Receiver<u16>`. This is used by integration tests and the debug-http status endpoint.

### Protocol: Channel Protocol over JSON-RPC 2.0

You don't need to handle this directly — the `ChannelHarness` manages all framing. This section documents what happens on the wire for debugging and understanding.

**Supervisor → Channel (via harness → your trait methods):**

| Method | Type | Dispatches to |
|--------|------|---------------|
| `initialize` | request | `capabilities()` + `on_initialize()` + `on_ready()` |
| `channel/deliverMessage` | notification | `deliver_message()` |
| `channel/requestPermission` | request | `show_permission_prompt()` |
| `channel/sessionCreated` | notification | `on_session_created()` |
| `channel/ackMessage` | notification | (handled by harness) |
| `channel/ackLifecycle` | notification | (handled by harness) |
| `channel/typingIndicator` | notification | (handled by harness) |

**Channel → Supervisor (via outbound sender):**

| Method | Type | Triggered by |
|--------|------|-------------|
| `channel/sendMessage` | notification | `outbound.send(ChannelSendMessage)` |
| `channel/respondPermission` | response | `permission_tx.send(PermissionResponse)` |

## Configuration

Channel extensions are configured in `anyclaw.yaml` under the `channels` key:

```yaml
channels:
  my-channel:
    binary: /path/to/my-channel         # or @built-in/channels/my-channel
    args: []
    enabled: true
    agent: my-agent                     # which agent to route messages to
    init_timeout_secs: 10
    exit_timeout_secs: 5
    permission_timeout_secs: 120        # auto-deny after timeout (optional)
    ack:
      on_dispatch: true                 # send ack when message dispatched to agent
      on_response_started: true         # send ack when agent starts responding
      on_response_completed: true       # send ack when agent finishes
    backoff:
      base_delay_ms: 100
      max_delay_secs: 30
    crash_tracker:
      max_crashes: 5
      window_secs: 60
    options:                            # arbitrary key-value, passed in initialize + as env vars
      BOT_TOKEN: "${TELEGRAM_BOT_TOKEN}"
      HOST: "0.0.0.0"
      PORT: "8080"
```

- `@built-in/channels/<name>` resolves to `{extensions_dir}/channels/<name>`
- `options` are both set as env vars on the subprocess AND passed in `ChannelInitializeParams.options`
- `agent` determines which agent receives messages from this channel
- `ack` controls which acknowledgment notifications the supervisor sends

## Testing

Use `ChannelTester` to unit test your channel without JSON-RPC framing:

```rust
use anyclaw_sdk_channel::testing::ChannelTester;
use anyclaw_sdk_types::DeliverMessage;

#[tokio::test]
async fn when_message_delivered_then_channel_renders_it() {
    let mut tester = ChannelTester::new(MyChannel { outbound: None, permission_tx: None });
    tester.initialize(None).await.unwrap();

    tester.deliver(DeliverMessage {
        session_id: "s1".into(),
        content: serde_json::json!("hello"),
    }).await.unwrap();

    // Assert your channel's side effects (HTTP responses, stored messages, etc.)
}

#[tokio::test]
async fn when_permission_prompt_shown_then_channel_displays_ui() {
    let mut tester = ChannelTester::new(MyChannel { outbound: None, permission_tx: None });
    tester.initialize(None).await.unwrap();

    tester.show_permission_prompt(ChannelRequestPermission {
        request_id: "perm-1".into(),
        session_id: "s1".into(),
        description: "Allow?".into(),
        options: vec![PermissionOption { option_id: "allow".into(), label: "Allow".into() }],
    }).await.unwrap();
}
```

`ChannelTester` provides:
- `initialize(options)` — runs the full init handshake
- `deliver(msg)` — calls `deliver_message()` directly
- `show_permission_prompt(req)` — calls `show_permission_prompt()` directly
- `outbound_rx` — receive `ChannelSendMessage` sent by your channel
- `channel_mut()` — mutable access to your channel for assertions

Test naming follows BDD convention: `when_action_then_result`. Use `rstest` for parameterized tests.

## Adding a New Channel

1. Create `ext/channels/{name}/` with `Cargo.toml` + `src/main.rs`
2. Add to workspace members in root `Cargo.toml`
3. Implement `Channel` trait, use `ChannelHarness::new(channel).run_stdio().await`
4. Add `ChannelConfig` entry in `anyclaw.yaml`
5. Update `crates/anyclaw-channels/` if new routing logic is needed

## Anti-Patterns

- **Don't block in `show_permission_prompt()`** — return immediately after displaying UI. Send `PermissionResponse` asynchronously through `permission_tx`. Blocking stalls delivery of subsequent agent messages.
- **Don't handle JSON-RPC manually** — the harness does all framing. Implement `Channel` trait only.
- **Don't depend on internal crates** — only use `anyclaw-sdk-channel` and `anyclaw-sdk-types`. SDK crates are the public API boundary.
- **Don't read env vars directly for config** — use `params.options` from `on_initialize()`. The supervisor sets env vars AND passes them in the handshake; prefer the handshake params for consistency.
- **Don't use `println!` for protocol messages** — stdout is owned by the harness for JSON-RPC. Use stderr for debug output, `tracing` for structured logging.
- **Don't hold async locks across `.await` points** — especially if your channel uses shared state with `RwLock`/`Mutex`. Extract data → drop lock → do async work → re-acquire if needed.
- **Don't skip the `Result` content kind** — it signals turn completion. Channels that ignore it will have incomplete rendering (e.g., thoughts never collapse, final edits never sent).
