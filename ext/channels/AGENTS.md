# ext/channels/ — External Channel Binaries

Channel implementations that run as subprocesses spawned by `ChannelsManager`. They communicate with anyclaw over JSON-RPC 2.0 stdio using the channel SDK (`anyclaw-sdk-channel`).

## Binaries

| Binary | Files | Purpose |
|--------|-------|---------|
| `telegram` | 8 | Full Telegram bot channel via `teloxide` |
| `debug-http` | 1 | Minimal HTTP endpoint for development/testing |

## Why ext/ and not crates/

These are standalone binaries, not libraries. They depend on SDK crates but are architecturally separate — they're spawned as child processes with piped stdio. Putting them in `ext/` makes the boundary explicit.

## Shared Pattern

Both channels follow the same structure:
1. Implement `Channel` trait from `anyclaw-sdk-channel`
2. Call `ChannelHarness::run_stdio(channel).await` in `main()`
3. The harness handles all JSON-RPC framing, initialization handshake, and message routing

## Thought Rendering

Both channels use `ContentKind::from_content(&msg.content)` from `anyclaw-sdk-types` for typed dispatch over content types:

- `ContentKind::Thought(thought)` — thought content from the agent's reasoning process
- `ContentKind::UserMessageChunk { text }` — user's message echoed back (includes merged prompt text)
- `ContentKind::MessageChunk { text }` / `ContentKind::Result { text }` — agent response chunks and final result
- `ContentKind::ToolCall { name, tool_call_id, input }` — agent started a tool call
- `ContentKind::ToolCallUpdate { name, tool_call_id, status, output }` — tool call status change (in_progress, completed, failed)
- `ContentKind::AvailableCommandsUpdate { commands }` — agent-provided command list; Telegram calls `bot.set_my_commands()` on this
- `ContentKind::UsageUpdate` / `ContentKind::Unknown` — silently ignored

**debug-http:** Emits thoughts as named SSE event `"thought"`, tool calls as `"tool_call"`, tool call updates as `"tool_call_update"`, and user message chunks as `"user_message_chunk"` via `SsePayload` struct. Tool call SSE data is JSON with `toolCallId`, `name`, `input`/`status`/`output` fields. Regular messages use default SSE data events. SSE clients filter by event type.

**telegram:** Streams thoughts as 🧠-prefixed messages with debounced edits (400ms internal timer). On `"result"`, collapses to "🧠 Thought for Xs" (timing only, no content). All tool calls in a turn are combined into a single Telegram message — the first tool call sends a new message, subsequent tool calls edit it to append a new line. Each line shows a status emoji (🔧 started, ⏳ in_progress, ✅ completed, ❌ failed) and the tool name. Tool call updates edit the combined message in-place, updating only the affected line's emoji. Failed tools include error output in a `<pre>` block. Tool call state tracked in `ChatTurn.tool_calls: HashMap<String, ToolCallTrack>` with insertion order preserved in `ChatTurn.tool_call_order: Vec<String>`. The single combined message ID is stored in `ChatTurn.tools_msg_id`. Emoji prefix configurable via `thought_emoji` option in `ChannelInitializeParams.options` (default: 🧠). Thinking state tracked inside `ChatTurn.thought` (see below).

## ChatTurn State Machine (telegram)

All per-chat streaming state is encapsulated in a single `ChatTurn` struct per chat, stored in `SharedState.turns: RwLock<HashMap<i64, ChatTurn>>`. This replaced 12 scattered `HashMap`/`HashSet` fields that were prone to race conditions.

### Types (`turn.rs`)

| Type | Purpose |
|------|---------|
| `ChatTurn` | One agent turn per chat. Owns `message_id`, `phase`, `thought`, `response`, `tool_calls`, `tools_msg_id`, `tool_call_order`. |
| `ThoughtTrack` | Telegram message ID, buffer, debounce handle, timing, suppression flag. |
| `ResponseTrack` | Telegram message ID, buffer, last-edit timestamp (rate limiting). |
| `ToolCallTrack` | Tool name + `ToolCallStatus` enum for combined message rendering. |
| `ToolCallStatus` | `Started`, `InProgress`, `Completed`, `Failed(Option<String>)` — drives emoji selection in combined tool message. |
| `TurnPhase` | `Active` (streaming) or `Finalizing(JoinHandle<()>)` (waiting for late chunks). |

### Lifecycle

1. **First event arrives** → `ChatTurn::new(message_id)` inserted into `turns` map.
2. **Thought/response chunks** → `append_thought()` / `append_response()` accumulate into track buffers. `can_edit_response()` enforces 1-second rate limit.
3. **`result` received** → `collapse_thought()` returns elapsed time for "🧠 Thought for Xs". `begin_finalizing(handle)` transitions phase to `Finalizing` with a 200ms timer.
4. **Late chunks after result** → `append_response()` still works during `Finalizing`. Timer is cancelled and restarted (new `begin_finalizing()` aborts old handle).
5. **Finalization timer fires** → `take_response_for_finalize()` reads buffer atomically. Final edit sent. Turn removed from map.
6. **New turn detected** → `is_different_turn(message_id)` returns true. `cleanup()` aborts all handles and clears state. Old turn removed, new `ChatTurn` inserted.

### Truncation Fix

The old architecture had a race: `finalize_previous_turn()` cleared `message_buffers` before the finalization timer could read them, causing truncated final edits. The ChatTurn fix: the buffer lives inside the struct, and `take_response_for_finalize()` reads it atomically before any state transitions — no race possible.

### Lock Discipline

`state.turns.write().await` must NEVER be held across `.await` points. Pattern: extract data → drop lock → do async work (Telegram API calls) → re-acquire if needed.

### Anti-patterns

- Do NOT hold `turns` write lock across Telegram API calls — causes deadlocks when concurrent events arrive.
- Do NOT clean up response buffer in the `result` handler — late chunks still need to append. Cleanup happens only when the finalization timer fires or a new turn is detected.
- Do NOT skip `is_different_turn()` check — without it, new prompt events corrupt the previous turn's state.
- Do NOT bypass `can_edit_response()` for normal streaming — only late chunks (during `Finalizing` phase) skip the rate limit.
- Do NOT call `cleanup()` without removing the turn from the `turns` map — the struct clears internal state but doesn't remove itself.
- Do NOT call Telegram API methods directly in retry-sensitive paths — wrap with `retry_telegram_op()` which applies exponential backoff on rate-limit (429) and server-error (5xx) responses.
- Do NOT use bare `.unwrap()` on Telegram API call results — use `.expect("descriptive reason")` so failures are identifiable in crash logs.

## telegram/ (9 files)

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, `ChannelHarness::run_stdio()` |
| `channel.rs` | `TelegramChannel` impl of `Channel` trait (`rich_text: true`) |
| `dispatcher.rs` | Teloxide dispatcher setup, message/callback handlers |
| `deliver.rs` | Outbound: agent updates → Telegram messages, thought rendering + collapse |
| `formatting.rs` | Markdown→HTML conversion, `escape_html`, `close_open_tags` for streaming |
| `turn.rs` | `ChatTurn` state machine — per-chat turn lifecycle, thought/response tracks |
| `permissions.rs` | Permission request → inline keyboard buttons |
| `peer.rs` | `PeerInfo` extraction from Telegram update context |
| `state.rs` | Shared state: bot instance, session tracking, `turns` map |

Bot token and thought emoji are received via `ChannelInitializeParams.options` in `on_initialize()`, not from environment variables. Additional configurable options (received via the same `options` map):

| Option key | Default | Purpose |
|------------|---------|---------|
| `cooldown_ms` | 800 | Minimum ms between sending new Telegram messages in a turn |
| `debounce_ms` | 400 | Debounce window for thought edits |
| `finalization_delay_ms` | 200 | Wait after `result` before sending final edit (collects late chunks) |

## Telegram Retry Helper

`retry_telegram_op()` in `deliver.rs` wraps Telegram API calls with exponential backoff. It retries on:
- HTTP 429 (Too Many Requests) — uses `retry_after` seconds from the Telegram response when available
- HTTP 5xx (server errors) — fixed backoff

Non-retryable errors (4xx other than 429, message not found, etc.) are returned immediately. All retry-worthy paths in `deliver.rs` use this helper — do not call Telegram API methods directly in the deliver path.

### HTML Parse Mode

All outbound messages use `ParseMode::Html`. Agent responses go through `format_telegram_html()` which converts markdown to Telegram-safe HTML using a placeholder-extraction pattern (code blocks extracted first to prevent double-escaping). Streaming edits additionally use `close_open_tags()` to ensure partial HTML is valid. Thought messages use `escape_html()` only (no markdown conversion). The `split_message()` function respects `<pre>` block boundaries — never splits inside them.

## debug-http/ (1 file)

Single `main.rs` — axum HTTP server with `SsePayload` typed broadcast for named SSE events. Emits `PORT:{n}` to stderr on bind for port discovery by supervisor. Used in integration tests and local development. Host and port are received via `ChannelInitializeParams.options` in `on_initialize()`.

## Adding a New Channel

1. Create `ext/channels/{name}/` with `Cargo.toml` + `src/main.rs`
2. Add to workspace members in root `Cargo.toml`
3. Implement `Channel` trait, use `ChannelHarness::run_stdio()`
4. Add `ChannelConfig` entry in `anyclaw.yaml`
5. Update `crates/anyclaw-channels/` if new routing logic needed

### Channel trait skeleton

```rust
use anyclaw_sdk_channel::{Channel, ChannelCapabilities, ChannelSdkError, ChannelSendMessage, PermissionBroker};
use anyclaw_sdk_types::{ChannelRequestPermission, ContentKind, DeliverMessage, PermissionResponse};

#[async_trait]
impl Channel for MyChannel {
    fn capabilities(&self) -> ChannelCapabilities { /* ... */ }
    async fn on_ready(&mut self, outbound: mpsc::Sender<ChannelSendMessage>, permission_tx: mpsc::Sender<PermissionResponse>) -> Result<(), ChannelSdkError> { /* ... */ }
    async fn deliver_message(&mut self, msg: DeliverMessage) -> Result<(), ChannelSdkError> { /* ... */ }
    async fn show_permission_prompt(&mut self, req: ChannelRequestPermission) -> Result<(), ChannelSdkError> { /* ... */ }
}
```

### ContentKind matching in deliver_message

```rust
let kind = ContentKind::from_content(&msg.content);
match kind {
    ContentKind::Thought(thought) => { /* thought.content has the text */ }
    ContentKind::MessageChunk { text } => { /* streaming response chunk */ }
    ContentKind::Result { text } => { /* final result */ }
    ContentKind::UserMessageChunk { .. } | ContentKind::UsageUpdate | ContentKind::Unknown => Ok(()),
}
```

### content_to_string for displayable text

```rust
use anyclaw_sdk_channel::content_to_string;
let text = content_to_string(&msg.content); // handles OpenCode wrapper + plain strings
```

### PermissionBroker for non-blocking permission handling

```rust
// In state struct:
pub permission_broker: Mutex<PermissionBroker>,
pub permission_tx: Mutex<Option<mpsc::Sender<PermissionResponse>>>,

// In show_permission_prompt():
self.state.permission_broker.lock().await.register(&req.request_id);
// ... send UI prompt (inline keyboard, etc.) ...
// Return immediately — do NOT await the user's response here.

// In callback/resolution handler (e.g. Telegram callback button):
self.state.permission_broker.lock().await.resolve(&request_id, &option_id);
let tx = self.state.permission_tx.lock().await.clone();
if let Some(tx) = tx {
    tx.send(PermissionResponse { request_id, option_id }).await.ok();
}
```

### ChannelTester for unit tests

```rust
use anyclaw_sdk_channel::testing::ChannelTester;
let mut tester = ChannelTester::new(my_channel);
tester.initialize(None).await.unwrap();
tester.deliver(DeliverMessage { session_id: "s1".into(), content: json!("hi") }).await.unwrap();
let outbound_msg = tester.outbound_rx.try_recv();
```

See `ext/channels/telegram/` and `ext/channels/debug-http/` for full working examples.
