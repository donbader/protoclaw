# protoclaw-channels — Channel Routing Layer

Manages channel subprocesses with per-channel crash isolation and session-keyed routing between channels and the agent.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ChannelsManager` — routing table, crash isolation, poll loop |
| `connection.rs` | `ChannelConnection` — subprocess spawn, JSON-RPC framing, port discovery |
| `session_queue.rs` | `SessionQueue` — per-session FIFO message queue |
| `types.rs` | Wire types: `ChannelSendMessage`, `ChannelRespondPermission`, `ChannelCapabilities`, `PeerInfo` |
| `debug_http.rs` | `DebugHttpChannel` — in-process debug channel (not subprocess) |
| `error.rs` | `ChannelsError` |

## Channel Protocol (JSON-RPC 2.0)

| Method | Direction | Purpose |
|--------|-----------|---------|
| `initialize` | manager→channel | Handshake, get capabilities |
| `channel/sendMessage` | channel→manager | Inbound user message with `PeerInfo` |
| `channel/respondPermission` | channel→manager | User's permission response |
| `channel/deliverMessage` | manager→channel | Outbound agent update to channel |
| `channel/requestPermission` | manager→channel | Forward permission request to user |

## Routing Model

- `routing_table: HashMap<SessionKey, RoutingEntry>` — maps session key → (channel_id, acp_session_id, slot_index)
- Inbound: `channel/sendMessage` → lookup/create session via `AgentsCommand::CreateSession` → `AgentsCommand::PromptSession`
- Outbound: `ChannelEvent::DeliverMessage` from agents → lookup routing table → `channel/deliverMessage` to correct channel

## Per-Channel Crash Isolation

Each channel gets its own `ChannelSlot` with independent:
- `connection: Option<ChannelConnection>` — None if crashed
- `backoff: ExponentialBackoff` — per-channel restart delay
- `crash_tracker: CrashTracker` — per-channel crash loop detection
- `disabled: bool` — set true on crash loop, channel stops restarting

A crash in one channel does NOT affect other channels or the sidecar.

## Port Discovery

Channel subprocesses emit `PORT:{n}` to stderr when they bind a port. `ChannelConnection` watches stderr and exposes a `watch::Receiver<u16>` via `port_rx()`. Supervisor forwards this for debug-http.

## poll_channels() Pattern

The `poll_channels()` method uses 1ms timeout polling per connection — it's a workaround because `tokio::select!` can't dynamically branch over a variable number of futures. The 50ms sleep in the `else` branch prevents busy-looping when no messages are ready. Do not remove it.

## Ack Flow

Ack notification (`channel/ackMessage`) fires on every `push()` — both `Dispatch` (immediate) and `Enqueued` (queued). This gives users instant feedback that their message was received.

`messageId` is always `Null` — Telegram tracks the last message independently via `last_message_ids`.

## Typing Indicator

`channel/typingIndicator` fires at dispatch time inside `dispatch_to_agent()`. This signals the channel that the agent is actively processing a message. Queued messages do not trigger typing — only the message being dispatched.

## Session Queue (FIFO)

Per-session FIFO queue (`SessionQueue`) replaces the old debounce-merge model. Each message is processed individually in order, never merged.

1. Message arrives, session idle → `Dispatch(msg)` — dispatched immediately
2. Message arrives, session busy → `Enqueued` — queued behind active message
3. Agent finishes (Result event) → `mark_idle()` pops next queued message → `Dispatch`
4. No queued messages on result → session returns to idle

Key type: `SessionKey` (`"{channel}:{kind}:{peer_id}"`) is the queue key.

## Message Merge Window

Configurable merge window (`merge_window_ms`, default 1200ms) in `ChannelsManagerConfig` batches rapid user messages before dispatching to the agent.

Flow:
1. Message arrives, session idle → buffer message, start merge timer
2. More messages within window → append to buffer, reset timer
3. Timer expires → join buffered messages with `\n`, dispatch as single prompt
4. Agent finishes (result), queued messages exist → drain queue into merge buffer, start new merge window
5. `merge_window_ms: 0` disables batching (immediate dispatch, legacy behavior)

State lives in `ChannelsManager`: `merge_buffers`, `merge_timers`, `merge_tx`/`merge_rx`. Timer communicates back to `run()` select loop via mpsc channel.

## Anti-Patterns (this crate)

- Do not remove the 50ms sleep in `poll_channels()` else branch
- Bad channel binaries don't block startup — they log errors and continue with `connection: None`
- `cmd_rx.take().expect("cmd_rx must exist")` — same consumed-once pattern as agents
- `start()` skips channels with `enabled = false` — no slot is created for disabled channels
- Do not dispatch immediately when `merge_window_ms > 0` — always go through the merge buffer for idle sessions
