# anyclaw-channels — Channel Routing Layer

Manages channel subprocesses with per-channel crash isolation and session-keyed routing between channels and the agent.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `ChannelsManager` — routing table, crash isolation, event-driven StreamMap loop |
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
| `channel/requestPermission` | manager→channel | Forward permission request to user (JSON-RPC **request** with id, not notification — since v0.3.1) |

## Routing Model

- `routing_table: HashMap<SessionKey, RoutingEntry>` — maps session key → (_channel_id, acp_session_id, slot_index)
- Inbound: `channel/sendMessage` → lookup/create session via `AgentsCommand::CreateSession` → `AgentsCommand::PromptSession`
  - If `CreateSession` returns an error, the channel receives an error `channel/deliverMessage` (not a silent drop)
- Outbound: `ChannelEvent::DeliverMessage` from agents → lookup routing table → `channel/deliverMessage` to correct channel

## Per-Channel Crash Isolation

Each channel gets its own `ChannelSlot` with independent:
- `connection: Option<ChannelConnection>` — None if crashed
- `backoff: ExponentialBackoff` — per-channel restart delay
- `crash_tracker: CrashTracker` — per-channel crash loop detection
- `disabled: bool` — set true on crash loop, channel stops restarting

A crash in one channel does NOT affect other channels or the sidecar.

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

Ack notification (`channel/ackMessage`) fires only at dispatch time — when a message (or merged batch) is actually sent to the agent. Queued messages do NOT receive ack until they are flushed and dispatched.

`messageId` is always `Null` — Telegram tracks the last message independently via `last_message_ids`.

`handle_session_complete()` receives `stop_reason: StopReason` from `ChannelEvent::SessionComplete` and includes it in the `channel/ackLifecycle` notification sent to the channel binary. This carries the canonical ACP completion reason (e.g., `end_turn`, `max_tokens`, `refusal`) so channels can adapt rendering or messaging accordingly.

## Typing Indicator

`channel/typingIndicator` fires at dispatch time inside `dispatch_to_agent()`. This signals the channel that the agent is actively processing a message. Queued messages do not trigger typing — only the message being dispatched.

## Session Queue (FIFO) with Two-Phase Collect+Flush

Per-session FIFO queue (`SessionQueue`). Two-phase design ensures ALL buffered messages (including those arriving while session is idle) merge into a single prompt.

**Idle session (two-phase collect+flush):**
1. Messages arrive, session idle → `push_only()` queues without dispatching, returns session key for flush
2. After all polled messages processed → `flush_pending()` drains queue, joins with `\n`, marks active → dispatched as single merged prompt, ack sent**Busy session (queue+drain on completion):**
1. Message arrives, session busy → `push()` returns `Enqueued` — queued (no ack)
2. Agent finishes (Result event) → `mark_idle()` pops first queued + `drain_queued()` grabs rest → joined with `\n` → dispatched as single merged prompt, ack sent
3. No queued messages on result → session returns to idle

Key methods: `push_only()`, `flush_pending()`, `push()`, `mark_idle()`, `drain_queued()`
Key type: `SessionKey` (`"{channel}:{kind}:{peer_id}"`) is the queue key.

## Anti-Patterns (this crate)

- Bad channel binaries don't block startup — they log errors and continue with `connection: None`
- `cmd_rx.take().expect("cmd_rx must exist")` — same consumed-once pattern as agents
- `start()` skips channels with `enabled = false` — no slot is created for disabled channels
- Use `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` rather than bare `unwrap_or_default()` when falling back silently — makes silent fallbacks visible in logs
