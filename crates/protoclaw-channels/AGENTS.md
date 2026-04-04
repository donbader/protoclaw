# protoclaw-channels — Channel Routing Layer

Manages channel subprocesses with per-channel crash isolation and session-keyed routing between channels and the agent.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ChannelsManager` — routing table, crash isolation, poll loop |
| `connection.rs` | `ChannelConnection` — subprocess spawn, JSON-RPC framing, port discovery |
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

Ack notification (`channel/ackMessage`) is sent at dispatch time, not on inbound message receipt. This ensures batched messages only ack the last message in the batch.

Two dispatch sites call `send_ack_to_channel()`:
- **Immediate branch** — first message on idle session dispatched directly
- **Debounce flush** — post-response window expired, merged messages dispatched

`messageId` is always `Null` — Telegram tracks the last message independently via `last_message_ids`.

## Debounce Flow

Sliding window debounce with post-response re-debounce for queued messages.

1. Message arrives, session idle → `Buffered` (start debounce timer)
2. More messages during window → `Buffered` (reset timer, accumulate)
3. Timer expires → `drain()` merges and dispatches to agent
4. Message arrives, agent mid-response → `Queued`
5. Agent finishes (Result event) → `mark_session_idle()` moves queued messages into buffer with fresh timer
6. More messages during post-response window → `Buffered` (reset timer)
7. Timer expires → `drain()` merges and dispatches

The debounce window always applies — both on initial messages and after the agent responds. This ensures rapid typing always gets merged regardless of when it happens relative to agent processing.

## Anti-Patterns (this crate)

- Do not send ack in `handle_channel_message` — ack must fire at dispatch time so batched messages only ack the last message
- Do not remove the 50ms sleep in `poll_channels()` else branch
- Bad channel binaries don't block startup — they log errors and continue with `connection: None`
- `cmd_rx.take().expect("cmd_rx must exist")` — same consumed-once pattern as agents
- `start()` skips channels with `enabled = false` — no slot is created for disabled channels
