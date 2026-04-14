# External Integrations

**Analysis Date:** 2026-04-14

## Subprocess Management

**Binary spawning:**
- All agents, channels, and tools run as child processes with piped stdio (stdin/stdout for JSON-RPC, stderr for diagnostics)
- Subprocess lifecycle managed per-manager: `AgentsManager` (`crates/anyclaw-agents/src/connection.rs`), `ChannelsManager` (`crates/anyclaw-channels/src/connection.rs`), `ExternalMcpServer` (`crates/anyclaw-tools/src/external.rs`)
- Docker container spawning via `bollard` `0.20` for `WorkspaceConfig::Docker` agents (`crates/anyclaw-agents/src/connection.rs`)

**Binary resolution (`@built-in` prefix):**
- `@built-in/agents/<name>` → `{extensions_dir}/agents/<name>`
- `@built-in/channels/<name>` → `{extensions_dir}/channels/<name>`
- `@built-in/tools/<name>` → `{extensions_dir}/tools/<name>`
- Resolution logic: `crates/anyclaw-config/src/resolve.rs`
- Legacy flat aliases (e.g. `@built-in/mock-agent`) supported with deprecation warning

**Crash recovery:**
- Exponential backoff (100ms→30s) per subprocess: `crates/anyclaw-core/src/backoff.rs`
- Crash loop detection (N crashes in window): `CrashTracker` in same file
- Per-channel isolation — one channel crash does not affect others

## Protocol Integrations

**ACP (Agent Client Protocol):**
- JSON-RPC 2.0 over stdio (NDJSON framing, one JSON object per `\n`)
- Codec: `crates/anyclaw-jsonrpc/src/codec.rs` — `NdJsonCodec` with 32MB max line size
- Wire types: `crates/anyclaw-sdk-types/` + `agent-client-protocol-schema` `0.11`
- Methods: `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/load`, `session/resume`, `session/update`, `session/request_permission`
- Agent-side SDK: `crates/anyclaw-sdk-agent/` — `AgentAdapter` trait for intercepting/transforming ACP messages

**MCP (Model Context Protocol):**
- Client: `rmcp` `1.4` with `transport-child-process` — connects to external MCP server subprocesses (`crates/anyclaw-tools/src/external.rs`)
- Server: `rmcp` `1.4` with `transport-streamable-http-server` — aggregated tool endpoint served over HTTP (`crates/anyclaw-tools/src/manager.rs`)
- Stateful mode required for multi-turn tool conversations
- Tool SDK: `crates/anyclaw-sdk-tool/` — `Tool` trait + `ToolServer` wrapping rmcp `ServerHandler`

**Channel Protocol (JSON-RPC 2.0 over stdio):**
- Manager→channel: `initialize`, `channel/deliverMessage`, `channel/requestPermission`, `channel/close`
- Channel→manager: `channel/sendMessage`, `channel/respondPermission`
- Port discovery: channel emits `PORT:{n}` to stderr on bind
- Channel SDK: `crates/anyclaw-sdk-channel/` — `Channel` trait + `ChannelHarness` handles all framing
- Wire types: `crates/anyclaw-sdk-types/src/channel.rs`

## External Services

**Telegram Bot API:**
- Implementation: `ext/channels/telegram/` (9 source files)
- SDK: `teloxide` `0.17` (features: `macros`, `rustls`, `ctrlc_handler`)
- Auth: bot token received via `ChannelInitializeParams.options` (not env vars)
- Features: streaming thought rendering, tool call status, inline keyboard permissions, message rate limiting, retry with backoff on 429/5xx

**HTTP Debug Channel:**
- Implementation: `ext/channels/debug-http/src/main.rs` (single file)
- Framework: Axum `0.8` with SSE broadcast
- Named SSE events: `thought`, `tool_call`, `tool_call_update`, `user_message_chunk`
- Host/port received via `ChannelInitializeParams.options`

**Docker Engine API:**
- Client: `bollard` `0.20` (in `anyclaw-agents`)
- Used for: spawning agent containers (`WorkspaceConfig::Docker`), container lifecycle management
- Configurable per-agent: `docker_host`, `network`, `pull_policy`, `memory_limit`, `cpu_limit`, `volumes`

**SQLite (Session Persistence):**
- Client: `rusqlite` `0.31` (bundled, no external SQLite dependency)
- Implementation: `crates/anyclaw-core/src/sqlite_session_store.rs`
- Trait: `SessionStore` in `crates/anyclaw-core/src/session_store.rs`
- Default: `NoopSessionStore` (configurable via `session_store.type` in config)

**Prometheus Metrics:**
- Exporter: `metrics-exporter-prometheus` `0.18` (in `anyclaw-supervisor`)
- Collection: `metrics` `0.24` (in `anyclaw-supervisor`, `anyclaw-tools`)

## WASM Sandbox

- Runtime: `wasmtime` `43` with WASI P1 support (`wasmtime-wasi` `43`)
- Runner: `crates/anyclaw-tools/src/wasm_runner.rs`
- Tool wrapper: `crates/anyclaw-tools/src/wasm_tool.rs`
- Sandbox controls: fuel budget (CPU), epoch interruption (wall-clock), memory limiter, filesystem preopens
- Each invocation gets a fresh `Store` — no shared state between calls

## SDK Surface (for external implementors)

**Agent SDK (`crates/anyclaw-sdk-agent/`, published `0.2.5`):**
- `AgentAdapter` trait — per-method hooks for ACP message interception/transformation
- `GenericAcpAdapter` — stateless passthrough default
- Depends only on `anyclaw-sdk-types`, `serde`, `serde_json`, `thiserror`

**Channel SDK (`crates/anyclaw-sdk-channel/`, published `0.2.7`):**
- `Channel` trait — implement `capabilities()`, `on_ready()`, `deliver_message()`, `request_permission()`
- `ChannelHarness<C>` — handles all JSON-RPC stdio framing, initialize handshake, bidirectional routing
- `PermissionBroker` — oneshot management for permission request/response pairing
- `ChannelTester<C>` — typed test wrapper bypassing JSON-RPC framing
- `content_to_string()` — extract displayable text from agent content
- Depends on `anyclaw-sdk-types`, `serde`, `serde_json`, `tokio`, `thiserror`, `tracing`

**Tool SDK (`crates/anyclaw-sdk-tool/`, published `0.2.5`):**
- `Tool` trait — implement `name()`, `description()`, `input_schema()`, `execute()`
- `ToolServer` — wraps rmcp `ServerHandler`, serves tools over MCP stdio
- `schemars` `1` re-exported for JSON Schema generation
- Depends on `anyclaw-sdk-types`, `serde`, `serde_json`, `tokio`, `thiserror`, `schemars`, `rmcp`, `tracing`

**Shared Types (`crates/anyclaw-sdk-types/`, published `0.4.0`):**
- Leaf crate — depends only on `serde`, `serde_json`, `agent-client-protocol-schema`
- `ChannelEvent`, `SessionKey`, `ContentKind`, `PeerInfo`, `DeliverMessage`, `PermissionRequest`, `PermissionResponse`
- All types use `#[serde(rename_all = "camelCase")]` for JSON wire format

## Configuration Integration

**Config loading (Figment):**
- Entry: `AnyclawConfig::load(path)` in `crates/anyclaw-config/src/lib.rs`
- Layer order: embedded `defaults.yaml` → user YAML (with `${VAR:-default}` substitution) → `ANYCLAW_*` env vars
- Env var nesting: double underscore (`ANYCLAW_SUPERVISOR__SHUTDOWN_TIMEOUT_SECS=60`)
- Missing `${VAR}` without default = hard error at startup

**YAML config structure (`anyclaw.yaml`):**
- `log_level`, `log_format`, `extensions_dir` — top-level
- `agents_manager.agents.<name>` — per-agent config (workspace type, binary/image, env, backoff, crash tracker)
- `channels_manager.channels.<name>` — per-channel config (binary, enabled, env, init/exit timeouts)
- `tools_manager.tools.<name>` — per-tool config (binary, enabled, type)
- `supervisor` — shutdown timeout, health check interval, max restarts
- `session_store` — type (`none` or `sqlite`), path

**Environment variables (`.env` files):**
- `.env` files present in `examples/01-fake-agent-telegram-bot/` and `examples/02-real-agents-telegram-bot/`
- `.env.example` templates provided for both examples
- Channel-specific config (bot tokens, etc.) flows through `ChannelInitializeParams.options`, not env vars

**Validation:**
- `validate_config()` in `crates/anyclaw-config/src/validate.rs`
- Checks: binary existence, working dir existence, Docker config parsing (memory/cpu limits, volume syntax)
- Returns `ValidationResult { errors, warnings }` — caller decides abort threshold

---

*Integration audit: 2026-04-14*
