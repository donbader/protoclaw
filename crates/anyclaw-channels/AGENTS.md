# anyclaw-channels — Channel Routing Layer

Manages channel subprocesses with per-channel crash isolation and session-keyed routing between channels and the agent.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ChannelsManager` — routing table, crash isolation, event-driven StreamMap loop |
| `connection.rs` | `ChannelConnection` — subprocess spawn, JSON-RPC framing, port discovery |
| `debug_http.rs` | `DebugHttpChannel` — in-process debug channel (not subprocess) |
| `error.rs` | `ChannelsError` |

## Channel Protocol (JSON-RPC 2.0)

| Method | Direction | Purpose |
|--------|-----------|---------|
| `initialize` | manager→channel | Handshake, get capabilities |
| `channel/sendMessage` | channel→manager | Inbound user message with `PeerInfo` |
| `channel/respondPermission` | channel→manager | User's permission response |
| `channel/deliverMessage` | manager→channel | Outbound agent update to channel |
| `channel/requestPermission` | manager→channel | Forward permission request to user (JSON-RPC **request** with id, not notification — since v0.3.1) |

## Routing Model

- `routing_table: HashMap<SessionKey, RoutingEntry>` — maps session key → (_channel_id, acp_session_id, slot_index, peer_info)
- Inbound: `channel/sendMessage` → lookup/create session via `AgentsCommand::CreateSession` → `AgentsCommand::EnqueueMessage`
  - If `CreateSession` returns an error, the channel receives an error `channel/deliverMessage` (not a silent drop)
- Outbound: `ChannelEvent::DeliverMessage` from agents → lookup routing table → `channel/deliverMessage` to correct channel

## Per-Channel Crash Isolation

Each channel gets its own `ChannelSlot` with independent:
- `connection: Option<ChannelConnection>` — None if crashed
- `backoff: ExponentialBackoff` — per-channel restart delay
- `crash_tracker: CrashTracker` — per-channel crash loop detection
- `disabled: bool` — set true on crash loop, channel stops restarting

A crash in one channel does NOT affect other channels or the sidecar.

## Crash Recovery Session Replay

When a channel subprocess crashes and respawns, `handle_channel_crash()` calls `replay_sessions_to_channel()` which:

1. Clears `acked_sessions` for all sessions routed to the crashed slot (so the next delivery re-sends `response_started`)
2. Sends `channel/sessionCreated` for each active session in the routing table belonging to that slot, using the stored `peer_info`

This ensures the new channel process has the session-to-peer mappings it needs to deliver in-flight messages. Without replay, the new process rejects deliveries with "unknown session" errors.

## Graceful Channel Shutdown

When the `ChannelsManager` receives a cancel signal, it shuts down each channel gracefully:

1. Send `channel/close` notification to the channel subprocess
2. Wait up to `exit_timeout_secs` (from `ChannelConfig` or `ChannelsManagerConfig` default) for the subprocess to exit
3. If the subprocess does not exit within the timeout, it is killed forcibly

The per-channel `exit_timeout_secs` in `ChannelConfig` overrides the manager-level default when set.

## Port Discovery

Channel subprocesses emit `PORT:{n}` to stderr when they bind a port. `ChannelConnection` watches stderr and exposes a `watch::Receiver<u16>` via `port_rx()`. Supervisor forwards this for debug-http.

## Inbound Message Routing (StreamMap)

Channel incoming messages are event-driven via `StreamMap<usize, ChannelStream>` — no polling. Each active channel connection contributes a stream built from its `mpsc::Receiver<IncomingChannelMessage>` wrapped to yield `Some(msg)` for each message and `None` on stream end (sender dropped = subprocess exited). When a channel subprocess exits, its stream ends and `handle_channel_crash()` is called inline in the `run()` select loop. On successful respawn, the new connection's stream is re-inserted into the map under the same slot index.

## Ack Flow

Ack notification (`channel/ackMessage`) and typing indicator (`channel/typingIndicator`) fire when `ChannelEvent::DispatchStarted` is received from agents — signaling that agents has dequeued a message and is dispatching it to the agent subprocess.

`messageId` is always `Null` — Telegram tracks the last message independently via `last_message_ids`.

`handle_session_complete()` receives `stop_reason: StopReason` from `ChannelEvent::SessionComplete` and includes it in the `channel/ackLifecycle` notification sent to the channel binary. This carries the canonical ACP completion reason (e.g., `end_turn`, `max_tokens`, `refusal`, `cancelled`) so channels can adapt rendering or messaging accordingly.

## Message Flow

Channels is a pure message forwarder — it does not queue or batch messages. Inbound messages are forwarded to agents via `AgentsCommand::EnqueueMessage`. Agents owns the session queue and handles batching, merging, and dispatch internally.

## Anti-Patterns (this crate)

- Do not reintroduce session queue logic in channels — the queue lives in agents (agent concurrency concern). Channels forwards messages immediately via `enqueue_to_agent()`.
- Bad channel binaries don't block startup — they log errors and continue with `connection: None`
- `cmd_rx.take().expect("cmd_rx must exist")` — same consumed-once pattern as agents
- `start()` skips channels with `enabled = false` — no slot is created for disabled channels
- Use `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` rather than bare `unwrap_or_default()` when falling back silently — makes silent fallbacks visible in logs
