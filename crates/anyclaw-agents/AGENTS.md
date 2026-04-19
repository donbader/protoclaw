# anyclaw-agents — ACP Protocol Layer

Manages the agent subprocess lifecycle and implements the ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `AgentsManager` — struct, constructor, ACP handshake (`initialize_agent`, `start_session`), tool context, Manager trait impl, run loop |
| `commands.rs` | Command dispatch (`handle_command`), session CRUD (`create_session`, `prompt_session`, `fork_session`, `list_sessions`, `cancel_session`), platform commands, session queue dispatch (`flush_and_dispatch`) |
| `fs_sandbox.rs` | Filesystem sandboxing: path validation (`validate_fs_path`, `validate_fs_write_path`), `handle_fs_read`, `handle_fs_write` |
| `session_recovery.rs` | Crash recovery: `handle_crash`, session restore (`try_restore_session`, `heal_session`), stale container cleanup |
| `incoming.rs` | Incoming message dispatch: `handle_incoming`, session update forwarding, tool event normalization, permission requests, `handle_prompt_completion`, completion-triggered queue drain and re-dispatch |
| `connection.rs` | `AgentConnection` — subprocess spawn, typed JSON-RPC framing over piped stdio (legacy, used in tests) |
| `sdk_runner.rs` | `AgentRunnerHandle` — SDK-based agent runner using `ClientSideConnection` on a dedicated `LocalSet` thread, event forwarding task |
| `platform_commands.rs` | `PlatformCommand` — typed platform commands with `Serialize`, `platform_commands_json()` for D-03 merging |
| `slot.rs` | `AgentSlot` — per-agent state: session maps, capabilities, pending permissions |
| `acp_types.rs` | ACP wire types: re-exports from `anyclaw-sdk-types` (`InitializeParams`, `SessionNewParams`, etc.) |
| `acp_error.rs` | `AcpError` — protocol-level errors (version mismatch, etc.) |
| `error.rs` | `AgentsError` — manager-level errors (spawn, timeout, connection) |
| `session_queue.rs` | `SessionQueue` — per-session FIFO message queue (agent concurrency concern) |

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
| `keepalive` | client→agent | Fire-and-forget notification to prevent idle Docker attach connection drops. Only sent to Docker-backed agents (local subprocess agents skip it). Agents silently ignore unknown notifications per JSON-RPC 2.0 spec §4.1. Non-compliant agents may log warnings but should not break. Interval configured via `keepalive_interval_secs` (default: 300s, 0 to disable). |
| `_raw_response` | internal | Removed in v0.3.1 — replaced by SDK-based permission flow via `PendingPermission.sdk_reply` oneshot. Legacy `send_raw()` no longer used in production paths. |
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
- When the agent reports "session not found" for a prompt, `handle_prompt_completion` moves the dead mapping into `stale_sessions` (not just drops it), so the next prompt triggers `heal_session` with the stale ACP ID available for `session/resume` or `session/load`
- Agent data directories must be volume-mounted so the agent can restore from its own session history

## Completion Signal Flow

When an agent finishes processing a prompt, two signals arrive:

1. **Streaming Result** (`session/update` with `sessionUpdate: "result"`) — arrives via `incoming_rx` → `handle_incoming()`. Sends `DeliverMessage` (content) to channels and sets `streaming_completed` flag. Does NOT send `SessionComplete`.

2. **RPC Response** (JSON-RPC response to `session/prompt`) — arrives via `completion_rx` → `handle_prompt_completion()`. This is the **sole sender** of `SessionComplete`. Before sending, it drains `incoming_rx` to ensure all streaming events are forwarded first (the `select!` loop can pick `completion_rx` before `incoming_rx` is fully drained). If `streaming_completed` is set, skips the synthetic result `DeliverMessage`. If not set (agent didn't emit streaming Result), sends a synthetic result `DeliverMessage` before `SessionComplete`. After sending `SessionComplete`, drains the session queue and re-dispatches any queued messages.

`handle_prompt_completion()` parses `PromptResponse { stop_reason }` from the RPC response body. The extracted `stop_reason: StopReason` is forwarded inside `ChannelEvent::SessionComplete`, carrying the canonical completion reason (per ACP spec) to channels. `PromptCompletion` carries `stop_reason: StopReason` as its primary completion field.

## Connection Architecture (SDK Runner)

The manager uses `AgentRunnerHandle` (from `sdk_runner.rs`) for all agent communication. Each agent gets a dedicated `std::thread` with a `new_current_thread` tokio runtime + `LocalSet`, bridging the `!Send` SDK (`agent-client-protocol` uses `Rc`, `LocalBoxFuture`) to the manager's multi-threaded runtime via `tokio::sync::mpsc` channels.

- **`spawn_agent_runner(config, name, log_level)`** — spawns the subprocess, creates the SDK `ClientSideConnection`, returns `AgentRunnerHandle { cmd_tx, event_rx, backend }`.
- **`spawn_event_forwarder(slot_idx, event_rx, incoming_tx)`** — tokio task that converts `AgentRunnerEvent` variants to `IncomingMessage` variants and pushes `SlotIncoming` to the manager's shared `incoming_tx`. `ConnectionClosed` maps to `SlotIncoming { msg: None }`.
- **`AgentRunnerCommand`** — typed enum sent via `cmd_tx`: `Initialize`, `NewSession`, `Prompt`, `Cancel`, `LoadSession`, `ResumeSession`, `ForkSession`, `ListSessions`, `Kill`. Each request variant carries a `oneshot::Sender` for the reply.
- **`AgentRunnerEvent`** — typed enum received via `event_rx`: `SessionNotification`, `PermissionRequest { args, reply }`, `FsRead { args, reply }`, `FsWrite { args, reply }`, `ConnectionClosed`. Permission/FS events carry reply oneshots that block the SDK until answered.
- **`IncomingMessage`** — extended with SDK variants: `SdkSessionNotification`, `SdkPermissionRequest`, `SdkFsRead`, `SdkFsWrite`. Legacy `AgentRequest`/`AgentNotification` variants kept for `AgentConnection` test infrastructure.

The `AgentConnection` type is retained for backward-compatible tests but is no longer used in production paths. All production spawn/initialize/session flows go through `spawn_agent_runner`.

### Legacy Connection Architecture (Bridge Collapse)

`AgentConnection` supported two spawn modes (now legacy, kept for tests):

- **`spawn(config, name)`** — standalone mode with internal incoming channel.
- **`spawn_with_bridge(config, name, slot_idx, bridge_tx)`** — bridge mode pushing directly to manager's `incoming_tx`.

## Tool Event Normalization

`normalize_tool_event_fields()` runs on `tool_call` and `tool_call_update` events before forwarding to channels. It translates agent-specific wire quirks into the canonical format that `ContentKind` expects:

- `title` → `name` (if `name` is absent)
- `rawOutput.output` → `output` (if `output` is absent, `tool_call_update` only)

This keeps `ContentKind` in `anyclaw-sdk-types` agent-agnostic — it only reads `name` and `output`. Agent-specific field mappings stay in the agents crate.

## Anti-Patterns (this crate)

- Do not reintroduce raw JSON-RPC `send_request`/`send_notification` calls for ACP methods — all agent communication goes through typed `AgentRunnerCommand` variants via `cmd_tx`. The SDK enforces spec compliance at the transport level.
- Do not send `SessionComplete` from the streaming path (`handle_incoming` in `incoming.rs`) — it races with the RPC response and can cause duplicate completions that skip queued messages.
- Do not skip the `incoming_rx` drain in `handle_prompt_completion` (`incoming.rs`) — without it, `select!` can process the RPC response before all streaming events are forwarded, causing lost updates.
- Permission responses go through `PendingPermission.sdk_reply` oneshot — the SDK blocks until the reply is sent. Do not drop the oneshot without sending a response (the SDK side will get `RecvError`).
- `handle_crash` lives in `session_recovery.rs` — crash recovery, respawn, and session restore logic is co-located there.
- `handle_incoming` and `handle_prompt_completion` live in `incoming.rs` — all agent→manager message dispatch is co-located there.
- `handle_command`, `create_session`, `prompt_session`, `fork_session`, `list_sessions`, `cancel_session`, `flush_and_dispatch` live in `commands.rs` — all command dispatch, session CRUD, and queue dispatch is co-located there.
- Platform commands (`/new`, `/cancel`) bypass the session queue entirely — they are intercepted in the `EnqueueMessage` handler before any queue interaction. Do not route them through the queue.
- Do not move the session queue back to channels — it enforces an agent constraint (one-prompt-at-a-time) and must stay in agents for `/cancel` to bypass it.
- `_raw_response` sentinel removed in v0.3.1 — replaced by SDK-based permission flow via `PendingPermission.sdk_reply` oneshot. Do not reintroduce `_raw_response`.
- Permission responses go through `sdk_reply` oneshot, not `send_raw()` — the SDK blocks until the reply is sent.
- `__jsonrpc_error` is a read-side sentinel — the connection reader task uses it to forward errors without losing the error context. Do not repurpose it.
- `cmd_rx.take().expect("cmd_rx must exist")` — consumed once at `run()` start. Never call `run()` twice.
- Use `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` rather than bare `unwrap_or_default()` when falling back silently — the tracing call makes the fallback visible in logs.
- Constructor uses `drain()` instead of `clone().into_iter()` when consuming maps to initialize session state — avoids unnecessary clones of large maps.
