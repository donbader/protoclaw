# ext/agents/ — Building Agent Extensions

Agent extensions are standalone binaries that implement the ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio. The `AgentsManager` spawns them as child processes with piped stdin/stdout/stderr.

There is no SDK harness for agents — they speak the wire protocol directly. The `anyclaw-sdk-agent` crate provides an `AgentAdapter` trait for intercepting/transforming ACP messages inside the supervisor, not for building agent binaries.

## Why ext/ and not crates/

These are standalone binaries, not libraries. They're spawned as child processes with piped stdio. Putting them in `ext/` makes the subprocess boundary explicit.

## Protocol: ACP over JSON-RPC 2.0

All communication is line-delimited JSON (NDJSON) over stdio. Each line is a complete JSON-RPC 2.0 message. The supervisor writes to the agent's stdin and reads from stdout. Stderr is captured for logging only.

### Wire Format

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}
{"jsonrpc":"2.0","id":1,"result":{...}}
{"jsonrpc":"2.0","method":"session/update","params":{...}}
```

- Requests have `id` (integer) + `method` + optional `params`
- Responses have `id` (matching request) + `result` or `error`
- Notifications have `method` + optional `params` but NO `id`

### Initialization Handshake

The supervisor sends `initialize` immediately after spawning:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocolVersion": 2,
  "capabilities": {"experimental": null},
  "options": {"key": "value"}
}}
```

The agent must respond with:

```json
{"jsonrpc":"2.0","id":1,"result":{
  "protocolVersion": 2,
  "capabilities": {
    "sessionCapabilities": {
      "resume": true,
      "load": true,
      "fork": false,
      "list": false
    }
  },
  "defaults": {
    "thinking": true,
    "echo_prefix": "Echo"
  }
}}
```

- `protocolVersion` must be `1` or `2`
- `options` contains arbitrary key-value pairs from the agent's config in `anyclaw.yaml`
- `capabilities.sessionCapabilities` declares what recovery methods the agent supports
- `defaults` (optional) — default option values the agent ships with. The manager merges these into the agent's `options` (user options win). Embed via `include_str!("../defaults.yaml")` and parse at init time.

### Session Lifecycle

After initialization, the supervisor creates a default session:

```json
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{
  "sessionId": null,
  "cwd": "/path/to/working/dir",
  "mcpServers": [
    {"name": "tools", "serverType": "http", "url": "http://127.0.0.1:12345/mcp"}
  ]
}}
```

The agent responds with a new session ID:

```json
{"jsonrpc":"2.0","id":2,"result":{"sessionId":"abc-123"}}
```

- `mcpServers` lists MCP tool endpoints the agent can connect to (provided by `ToolsManager`)
- `cwd` is the agent's working directory from config

User messages arrive as `session/prompt`:

```json
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{
  "sessionId": "abc-123",
  "prompt": [{"type": "text", "text": "Hello, agent"}]
}}
```

- `prompt` is an array of `ContentPart` (text or image)
- The agent streams updates via notifications, then sends a final `result` update
- Only after the `result` update should the agent respond to the JSON-RPC request:

```json
{"jsonrpc":"2.0","id":3,"result":{}}
```

Cancel an in-progress prompt (notification, no response expected):

```json
{"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":"abc-123"}}
```

### Streaming Updates

While processing a prompt, the agent sends `session/update` notifications to stream progress:

```json
{"jsonrpc":"2.0","method":"session/update","params":{
  "sessionId": "abc-123",
  "type": "agent_thought_chunk",
  "content": {"type": "text", "text": "Let me think about this..."}
}}
```

Update types (the `type` field):

| Type | Purpose | Content |
|------|---------|---------|
| `agent_thought_chunk` | Streaming thought/reasoning | `{ type: "text", text: "..." }` |
| `agent_message_chunk` | Streaming response text | `{ type: "text", text: "..." }` |
| `result` | Final result — signals prompt is complete. **Extension type — not part of core ACP. Agents MAY emit this as an early completion hint.** | `{ type: "text", text: "..." }` |
| `tool_call` | Agent started a tool call | `{ name, toolCallId, input }` |
| `tool_call_update` | Tool call status changed | `{ name, toolCallId, status, output }` |
| `plan` | Agent's execution plan | `{ content }` |
| `usage_update` | Token usage stats | `{ usage }` |
| `available_commands_update` | Agent-provided slash commands | `{ commands: [...] }` |
| `current_mode_update` | Agent mode changed | `{ mode }` |
| `session_info_update` | Session metadata changed | `{ sessionInfo }` |

The `result` update is critical — it signals the supervisor that the prompt is complete. The supervisor uses it to trigger finalization in channels (e.g., collapsing thought bubbles in Telegram).

### Prompt Response (StopReason)

The `session/prompt` JSON-RPC response carries a `stopReason` field indicating why the agent stopped:

```json
{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}
```

| StopReason | Meaning |
|---|---|
| `end_turn` | Agent finished normally |
| `max_tokens` | Output truncated by token limit |
| `max_turn_requests` | Turn limit reached |
| `refusal` | Agent refused the request |
| `cancelled` | Prompt was cancelled |

This is the canonical completion signal per the ACP spec. The streaming `result` update is an extension type that agents MAY emit as an early hint.

### Permission Flow

Agents can request user permission before performing sensitive actions. This is a JSON-RPC request (with `id`) sent from the agent to the supervisor:

```json
{"jsonrpc":"2.0","id":100,"method":"session/request_permission","params":{
  "sessionId": "abc-123",
  "description": "Allow writing to /etc/hosts?",
  "options": [
    {"optionId": "allow", "label": "Allow"},
    {"optionId": "deny", "label": "Deny"}
  ]
}}
```

The supervisor routes this to the channel, which displays a prompt to the user. When the user responds, the supervisor sends back:

```json
{"jsonrpc":"2.0","id":100,"result":{
  "outcome": {"outcome": "selected", "optionId": "allow"}
}}
```

The agent must wait for this response before proceeding. The supervisor may also auto-deny if a `permission_timeout_secs` is configured.

### Filesystem Access

Agents can request sandboxed filesystem access via JSON-RPC requests:

- `fs/read_text_file` — read a file (sandboxed to `working_dir`)
- `fs/write_text_file` — write a file (sandboxed to `working_dir`)

The supervisor validates paths via canonicalization + `starts_with` check against the configured working directory. Requests outside the sandbox are rejected with a JSON-RPC error.

### Crash Recovery

The supervisor automatically restarts crashed agent processes with exponential backoff (100ms → 30s). After respawning:

1. Re-sends `initialize` handshake
2. Attempts `session/resume` if the agent declared `resume` capability — preferred, no replay needed
3. Falls back to `session/load` if the agent declared `load` capability — replays conversation history
4. Falls back to `session/new` if neither works — fresh session, previous context lost

To support crash recovery, implement `session/resume` and/or `session/load`:

```json
// session/resume — restore session without replay
{"jsonrpc":"2.0","id":4,"method":"session/resume","params":{"sessionId":"abc-123"}}

// session/load — restore session with history replay
{"jsonrpc":"2.0","id":5,"method":"session/load","params":{"sessionId":"abc-123"}}
```

Both return `{"result":{}}` on success or a JSON-RPC error on failure.

If the agent crashes too frequently (configurable via `crash_tracker.max_crashes` within `crash_tracker.window_secs`), the supervisor disables it permanently for the session.

## Configuration

Agent extensions are configured in `anyclaw.yaml` under the `agents` key:

```yaml
agents:
  my-agent:
    workspace:
      type: local
      binary: /path/to/my-agent        # or @built-in/agents/my-agent
      working_dir: /workspace
      env:
        MY_API_KEY: !env "MY_API_KEY:"  # !env tag resolves from environment
    enabled: true
    tools: ["*"]                        # tool name filter, "*" = all
    acp_timeout_secs: 300
    backoff:
      base_delay_ms: 100
      max_delay_secs: 30
    crash_tracker:
      max_crashes: 5
      window_secs: 60
    options:                            # arbitrary key-value, passed in initialize
      model: "claude-sonnet-4-20250514"
```

- `@built-in/agents/<name>` resolves to `{extensions_dir}/agents/<name>`
- `options` is passed as-is in the `initialize` request's `params.options`
- `env` vars are set on the spawned subprocess
- Docker workspaces use `workspace.type: docker` with `image`, `pull_policy`, `volumes`, `network`, `resource_limits`

## Testing

Test your agent binary by running it directly and piping JSON-RPC messages to stdin:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":2,"capabilities":{"experimental":null},"options":{}}}' | ./my-agent
```

For automated testing, use the integration test harness in `tests/integration/` — it spawns a real supervisor with your agent binary and exercises the full pipeline.

Write unit tests for your protocol handling logic separately from the stdio layer. The `mock-agent` binary in this directory demonstrates how to structure testable protocol handlers.

## Anti-Patterns

- **Don't write to stderr for protocol messages** — stderr is for logging only. All JSON-RPC goes through stdout.
- **Don't emit non-JSON lines to stdout** — the supervisor skips non-JSON lines, but they waste cycles. Use stderr for debug output.
- **Don't block on permission requests indefinitely** — the supervisor may time out and auto-deny.
- **Don't forget the `result` update** — channels depend on it to finalize rendering (collapse thoughts, send final edit). Without it, the turn never completes.
- **Don't send `session/update` after `result`** — the supervisor may have already forwarded the completion signal to channels. Late updates may be dropped or cause rendering glitches.
- **Don't read env vars for config** — use `options` from the `initialize` handshake. The supervisor controls what config reaches the agent.
- **Don't rely solely on the streaming `result` update for completion** — `stopReason` in the RPC response is the canonical signal per the ACP spec.
