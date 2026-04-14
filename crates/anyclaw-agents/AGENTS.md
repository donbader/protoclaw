# anyclaw-agents — ACP Protocol Layer

Manages the agent subprocess lifecycle and implements the ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `AgentsManager` — session lifecycle, command handling, crash recovery |
| `connection.rs` | `AgentConnection` — subprocess spawn, JSON-RPC framing over piped stdio, direct bridge to manager |
| `acp_types.rs` | ACP wire types: `InitializeParams`, `SessionNewParams`, `SessionPromptParams`, etc. |
| `acp_error.rs` | `AcpError` — protocol-level errors (version mismatch, etc.) |
| `error.rs` | `AgentsError` — manager-level errors (spawn, timeout, connection) |

## ACP Methods (JSON-RPC 2.0)

| Method | Direction | Purpose |
|--------|-----------|---------|
| `initialize` | client→agent | Handshake, protocol version check |
| `session/new` | client→agent | Create new session, pass MCP server URLs |
| `session/prompt` | client→agent | Send user message to session |
| `session/cancel` | client→agent | Cancel in-progress operation |
| `session/load` | client→agent | Restore session after crash (if agent supports it) |
| `session/update` | agent→client | Streaming agent response updates |
| `session/request_permission` | agent→client | Agent requests user permission |
| `fs/read_text_file` | agent→client | Agent requests file read |
| `fs/write_text_file` | agent→client | Agent requests file write |
| `_raw_response` | internal | Removed in v0.3.1 — replaced by `AgentConnection::send_raw()` which writes pre-built JSON-RPC directly to stdin without method envelope |
| `__jsonrpc_error` | internal | Sentinel method used in `AgentConnection` reader task to forward ACP-level JSON-RPC errors from the agent back to the manager as typed `AcpError` variants |

## Tracing Instrumentation

`initialize_agent()` and `create_session()` are annotated with `#[tracing::instrument]`. This automatically creates spans for each call with the function arguments as fields, making it easy to trace individual agent handshakes and session creation in distributed traces.

## Multi-Session Model

- `session_map: HashMap<SessionKey, String>` — maps channel identity → ACP session ID
- `reverse_map: HashMap<String, SessionKey>` — reverse lookup for routing agent updates back
- `channels_sender: mpsc::Sender<ChannelEvent>` — outbound pipe to ChannelsManager

## Crash Recovery Flow

1. Agent process exits → `handle_crash()` called
2. Old connection cleaned up via `kill()` (stops + removes Docker container if applicable)
3. Backoff delay (exponential, 100ms→30s)
4. Respawn subprocess + re-initialize
5. If agent supports `session/load` → attempt session restore
6. If restore fails → start fresh session
7. Reset backoff on success

## Completion Signal Flow

When an agent finishes processing a prompt, two signals arrive:

1. **Streaming Result** (`session/update` with `sessionUpdate: "result"`) — arrives via `incoming_rx` → `handle_incoming()`. Sends `DeliverMessage` (content) to channels and sets `streaming_completed` flag. Does NOT send `SessionComplete`.

2. **RPC Response** (JSON-RPC response to `session/prompt`) — arrives via `completion_rx` → `handle_prompt_completion()`. This is the **sole sender** of `SessionComplete`. Before sending, it drains `incoming_rx` to ensure all streaming events are forwarded first (the `select!` loop can pick `completion_rx` before `incoming_rx` is fully drained). If `streaming_completed` is set, skips the synthetic result `DeliverMessage`. If not set (agent didn't emit streaming Result), sends a synthetic result `DeliverMessage` before `SessionComplete`.

## Connection Architecture (Bridge Collapse)

`AgentConnection` supports two spawn modes:

- **`spawn(config, name)`** — standalone mode. Creates its own internal `(incoming_tx, incoming_rx)` channel. Used in tests and when the caller manages its own receive loop.
- **`spawn_with_bridge(config, name, slot_idx, bridge_tx)`** — bridge mode. The reader task pushes `SlotIncoming { slot_idx, msg }` directly to the manager's shared `incoming_tx`. No intermediate channel, no bridge task.

The manager always uses `spawn_with_bridge()` in both `start()` and `handle_crash()`. This eliminates the two-hop latency that previously caused premature `SessionComplete` — the old design had a `spawn_incoming_bridge()` task forwarding from the connection's internal channel to the manager's channel, and events could be stuck in the bridge queue when `try_recv()` drained `incoming_rx`.

## Tool Event Normalization

`normalize_tool_event_fields()` runs on `tool_call` and `tool_call_update` events before forwarding to channels. It translates agent-specific wire quirks into the canonical format that `ContentKind` expects:

- `title` → `name` (if `name` is absent)
- `rawOutput.output` → `output` (if `output` is absent, `tool_call_update` only)

This keeps `ContentKind` in `anyclaw-sdk-types` agent-agnostic — it only reads `name` and `output`. Agent-specific field mappings stay in the agents crate.

## Anti-Patterns (this crate)

- Do not reintroduce `spawn_incoming_bridge()` or any intermediate forwarding channel between `AgentConnection` and the manager's `incoming_rx` — the two-hop latency causes premature `SessionComplete` when `try_recv()` sees an empty channel while events are still in the bridge queue.
- Do not send `SessionComplete` from the streaming path (`handle_incoming`) — it races with the RPC response and can cause duplicate completions that skip queued messages.
- Do not skip the `incoming_rx` drain in `handle_prompt_completion` — without it, `select!` can process the RPC response before all streaming events are forwarded, causing lost updates.
- `_raw_response` sentinel removed in v0.3.1 — replaced by `AgentConnection::send_raw()` which writes pre-built JSON-RPC directly to stdin without wrapping in a method envelope. Do not reintroduce `_raw_response`.
- Permission responses go through `send_raw()` because they're responses to agent-initiated requests, not client-initiated ones.
- `__jsonrpc_error` is a read-side sentinel — the connection reader task uses it to forward errors without losing the error context. Do not repurpose it.
- `cmd_rx.take().expect("cmd_rx must exist")` — consumed once at `run()` start. Never call `run()` twice.
- Use `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` rather than bare `unwrap_or_default()` when falling back silently — the tracing call makes the fallback visible in logs.
- Constructor uses `drain()` instead of `clone().into_iter()` when consuming maps to initialize session state — avoids unnecessary clones of large maps.
