# protoclaw-agents — ACP Protocol Layer

Manages the agent subprocess lifecycle and implements the ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio.

## Files

| File | Purpose |
|------|---------|
| `manager.rs` | `AgentsManager` — session lifecycle, command handling, crash recovery |
| `connection.rs` | `AgentConnection` — subprocess spawn, JSON-RPC framing over piped stdio |
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
| `session/close` | client→agent | Graceful session teardown |
| `session/update` | agent→client | Streaming agent response updates |
| `session/request_permission` | agent→client | Agent requests user permission |
| `fs/read_text_file` | agent→client | Agent requests file read |
| `fs/write_text_file` | agent→client | Agent requests file write |
| `_raw_response` | internal | Sentinel method to bypass framing — sends pre-built JSON-RPC response directly to agent stdin |

## Multi-Session Model

- `session_map: HashMap<SessionKey, String>` — maps channel identity → ACP session ID
- `reverse_map: HashMap<String, SessionKey>` — reverse lookup for routing agent updates back
- `channels_sender: mpsc::Sender<ChannelEvent>` — outbound pipe to ChannelsManager

## Crash Recovery Flow

1. Agent process exits → `handle_crash()` called
2. Backoff delay (exponential, 100ms→30s)
3. Respawn subprocess + re-initialize
4. If agent supports `session/load` → attempt session restore
5. If restore fails → start fresh session
6. Reset backoff on success

## Anti-Patterns (this crate)

- `_raw_response` is a hack — it sends pre-built JSON-RPC directly to stdin, bypassing normal request/response framing. Do not use it for new methods.
- `cmd_rx.take().expect("cmd_rx must exist")` — consumed once at `run()` start. Never call `run()` twice.
- Permission responses go through `_raw_response` because they're responses to agent-initiated requests, not client-initiated ones.
