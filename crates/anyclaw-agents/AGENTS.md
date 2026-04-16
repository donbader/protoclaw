# anyclaw-agents — ACP Protocol Layer

Manages the agent subprocess lifecycle and implements the ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `AgentsManager` — struct, constructor, ACP handshake (`initialize_agent`, `start_session`), tool context, Manager trait impl, run loop |
| `commands.rs` | Command dispatch (`handle_command`), session CRUD (`create_session`, `prompt_session`, `fork_session`, `list_sessions`, `cancel_session`), platform commands |
| `fs_sandbox.rs` | Filesystem sandboxing: path validation (`validate_fs_path`, `validate_fs_write_path`), `handle_fs_read`, `handle_fs_write` |
| `session_recovery.rs` | Crash recovery: `handle_crash`, session restore (`try_restore_session`, `heal_session`), stale container cleanup |
| `incoming.rs` | Incoming message dispatch: `handle_incoming`, session update forwarding, tool event normalization, permission requests, `handle_prompt_completion` |
| `connection.rs` | `AgentConnection` — subprocess spawn, typed JSON-RPC framing over piped stdio, direct bridge to manager |
| `platform_commands.rs` | `PlatformCommand` — typed platform commands with `Serialize`, `platform_commands_json()` for D-03 merging |
| `slot.rs` | `AgentSlot` — per-agent state: session maps, capabilities, pending permissions |
| `acp_types.rs` | ACP wire types: re-exports from `anyclaw-sdk-types` (`InitializeParams`, `SessionNewParams`, etc.) |
| `acp_error.rs` | `AcpError` — protocol-level errors (version mismatch, etc.) |
| `error.rs` | `AgentsError` — manager-level errors (spawn, timeout, connection) |

## Typed Pipeline (Phase 3)

- **Connection layer:** `AgentConnection` reads/writes `JsonRpcMessage` directly via `NdJsonCodec` — no `from_value`/`to_value` shims
- **Pending requests:** `DashMap<u64, oneshot::Sender<JsonRpcResponse>>` — lock-free concurrent access replaces `Arc<Mutex<HashMap>>`
- **Incoming messages:** `IncomingMessage::AgentRequest(JsonRpcRequest)` / `AgentNotification(JsonRpcRequest)` — typed, not raw Value
- **Outbound responses:** `send_raw(JsonRpcResponse)` — typed response struct, not raw Value
- **Permission flow:** `PendingPermission.request` is `JsonRpcRequest` — typed throughout
- **Request handlers:** `handle_permission_request`, `handle_fs_read`, `handle_fs_write` extract params from `&JsonRpcRequest` internally

## D-03 Value Boundaries

Remaining `serde_json::Value` usages are documented D-03 extensible boundaries:

- **Agent content mutation** (`add_received_timestamp`, `normalize_tool_event_fields`, `forward_session_update` in `incoming.rs`): `DeliverMessage.content` stays as Value because the manager injects `_received_at_ms`, normalizes tool event fields, and merges platform commands — all raw JSON mutations
- **Permission params** (`handle_permission_request` in `incoming.rs`): agent-defined schemas where `requestId` location varies by agent implementation
- **FS request params** (`handle_fs_read`, `handle_fs_write` in `fs_sandbox.rs`): agent-defined path/content fields
- **`session/update` params** (`handle_session_update` in `incoming.rs`): deserialized into `SessionUpdateEvent` for typed dispatch, but raw Value forwarded as channel content
- **`last_available_commands`** (slot.rs): stores arbitrary agent-reported `availableCommands` payload
- **`platform_commands_json()`**: serialization boundary for merging typed `PlatformCommand` into agent content arrays
- **`send_request`/`send_notification` params** (connection.rs): method-specific schemas cannot be typed at the connection layer
- **Prompt completion error forwarding** (`prompt_session` in `commands.rs`): error content forwarded as raw JSON to channels

## ACP Methods (JSON-RPC 2.0)

| Method | Direction | Purpose |
|--------|-----------|---------|
| `initialize` | client→agent | Handshake, protocol version check |
| `session/new` | client→agent | Create new session, pass MCP server URLs |
| `session/prompt` | client→agent | Send user message to session |
| `session/cancel` | client→agent | Cancel in-progress operation |
| `session/load` | client→agent | Restore session after crash (if agent supports it, replays history) |
| `session/resume` | client→agent | Restore session without replay (preferred over load) |
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
5. If agent supports `session/resume` → attempt silent restore (no replay)
6. Else if agent supports `session/load` → attempt restore (replay suppressed via `awaiting_first_prompt`)
7. If restore fails → start fresh session
8. Reset backoff on success

## Session Persistence

- `shutdown_all()` leaves sessions open in the store — TTL-based expiry handles cleanup
- On restart, `load_open_sessions()` populates `stale_sessions` from the SQLite store
- `CreateSession` checks `stale_sessions` before creating new sessions
- `heal_session()` prefers `session/resume` (no replay) over `session/load` (replays history)
- For `session/load`, replay events are suppressed until the first `session/prompt` via `awaiting_first_prompt` set
- Agent data directories must be volume-mounted so the agent can restore from its own session history

## Completion Signal Flow

When an agent finishes processing a prompt, two signals arrive:

1. **Streaming Result** (`session/update` with `sessionUpdate: "result"`) — arrives via `incoming_rx` → `handle_incoming()`. Sends `DeliverMessage` (content) to channels and sets `streaming_completed` flag. Does NOT send `SessionComplete`.

2. **RPC Response** (JSON-RPC response to `session/prompt`) — arrives via `completion_rx` → `handle_prompt_completion()`. This is the **sole sender** of `SessionComplete`. Before sending, it drains `incoming_rx` to ensure all streaming events are forwarded first (the `select!` loop can pick `completion_rx` before `incoming_rx` is fully drained). If `streaming_completed` is set, skips the synthetic result `DeliverMessage`. If not set (agent didn't emit streaming Result), sends a synthetic result `DeliverMessage` before `SessionComplete`.

`handle_prompt_completion()` parses `PromptResponse { stop_reason }` from the RPC response body. The extracted `stop_reason: StopReason` is forwarded inside `ChannelEvent::SessionComplete`, carrying the canonical completion reason (per ACP spec) to channels. `PromptCompletion` carries `stop_reason: StopReason` as its primary completion field.

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
- Do not send `SessionComplete` from the streaming path (`handle_incoming` in `incoming.rs`) — it races with the RPC response and can cause duplicate completions that skip queued messages.
- Do not skip the `incoming_rx` drain in `handle_prompt_completion` (`incoming.rs`) — without it, `select!` can process the RPC response before all streaming events are forwarded, causing lost updates.
- `handle_crash` lives in `session_recovery.rs` — crash recovery, respawn, and session restore logic is co-located there.
- `handle_incoming` and `handle_prompt_completion` live in `incoming.rs` — all agent→manager message dispatch is co-located there.
- `handle_command`, `create_session`, `prompt_session`, `fork_session`, `list_sessions`, `cancel_session` live in `commands.rs` — all command dispatch and session CRUD is co-located there.
- `_raw_response` sentinel removed in v0.3.1 — replaced by `AgentConnection::send_raw()` which writes pre-built JSON-RPC directly to stdin without wrapping in a method envelope. Do not reintroduce `_raw_response`.
- Permission responses go through `send_raw()` because they're responses to agent-initiated requests, not client-initiated ones.
- `__jsonrpc_error` is a read-side sentinel — the connection reader task uses it to forward errors without losing the error context. Do not repurpose it.
- `cmd_rx.take().expect("cmd_rx must exist")` — consumed once at `run()` start. Never call `run()` twice.
- Use `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` rather than bare `unwrap_or_default()` when falling back silently — the tracing call makes the fallback visible in logs.
- Constructor uses `drain()` instead of `clone().into_iter()` when consuming maps to initialize session state — avoids unnecessary clones of large maps.
