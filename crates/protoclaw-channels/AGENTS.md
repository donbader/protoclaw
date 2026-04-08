# protoclaw-channels — Channel Routing Layer

Manages channel subprocesses with per-channel crash isolation and session-keyed routing between channels and the agent.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ChannelsManager` — routing table, crash isolation, poll loop |
| `connection.rs` | `ChannelConnection` — subprocess spawn, JSON-RPC framing, port discovery |
| `session_queue.rs` | `SessionQueue` — per-session FIFO message queue |
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

- `routing_table: HashMap<SessionKey, RoutingEntry>` — maps session key → (_channel_id, acp_session_id, slot_index)
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

Ack notification (`channel/ackMessage`) fires only at dispatch time — when a message (or merged batch) is actually sent to the agent. Queued messages do NOT receive ack until they are flushed and dispatched.

`messageId` is always `Null` — Telegram tracks the last message independently via `last_message_ids`.

## Typing Indicator

`channel/typingIndicator` fires at dispatch time inside `dispatch_to_agent()`. This signals the channel that the agent is actively processing a message. Queued messages do not trigger typing — only the message being dispatched.

## Session Queue (FIFO) with Flush

Per-session FIFO queue (`SessionQueue`). Messages queue while the agent is busy, then flush as a single merged prompt when the agent finishes.

1. Message arrives, session idle → `Dispatch(msg)` — dispatched immediately, ack sent
2. Message arrives, session busy → `Enqueued` — queued (no ack)
3. Agent finishes (Result event) → `mark_idle()` pops first queued + `drain_queued()` grabs rest → joined with `\n` → dispatched as single merged prompt, ack sent
4. No queued messages on result → session returns to idle

Key type: `SessionKey` (`"{channel}:{kind}:{peer_id}"`) is the queue key.

## Anti-Patterns (this crate)

- Do not remove the 50ms sleep in `poll_channels()` else branch
- Bad channel binaries don't block startup — they log errors and continue with `connection: None`
- `cmd_rx.take().expect("cmd_rx must exist")` — same consumed-once pattern as agents
- `start()` skips channels with `enabled = false` — no slot is created for disabled channels
