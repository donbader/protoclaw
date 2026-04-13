# PROTOCLAW — Project Knowledge Base

Infrastructure sidecar connecting AI agents to channels (Telegram, Slack) and tools (MCP servers, WASM sandboxed). Rust workspace, ACP protocol (JSON-RPC 2.0 over stdio), three-manager architecture with Supervisor.

## Structure

```
protoclaw/
├── crates/                         # Core workspace crates (12 total)
│   ├── protoclaw/                  # Binary: CLI + Supervisor (entry point)
│   ├── protoclaw-core/             # Shared: Manager trait, backoff, crash tracker, message types
│   ├── protoclaw-agents/           # ACP protocol layer, agent subprocess management
│   ├── protoclaw-channels/         # Channel subprocess routing + lifecycle
│   ├── protoclaw-tools/            # MCP host, WASM sandbox, tools manager
│   ├── protoclaw-config/           # Figment-based config loading (protoclaw.yaml)
│   ├── protoclaw-jsonrpc/          # JSON-RPC 2.0 codec + types (LinesCodec-based)
│   ├── protoclaw-sdk-types/        # Shared SDK types (capabilities, permissions, ACP wire types)
│   ├── protoclaw-sdk-agent/        # SDK: AgentAdapter trait + GenericAcpAdapter
│   ├── protoclaw-sdk-channel/      # SDK: Channel trait + ChannelHarness
│   ├── protoclaw-sdk-tool/         # SDK: Tool trait + ToolServer
│   └── protoclaw-test-helpers/     # Shared test utilities (dev-dependency)
├── ext/                            # External binaries (spawned as subprocesses)
│   ├── agents/
│   │   ├── mock-agent/             # Mock ACP agent binary (echo + thinking simulation + commands)
│   │   └── acp-bridge/             # Generic ACP↔HTTP bridge (translates stdio JSON-RPC to REST+SSE)
│   └── channels/
│       ├── telegram/               # Telegram channel implementation
│       └── debug-http/             # Debug HTTP channel (minimal)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
├── examples/telegram-bot/          # Example config + docker-compose (no Rust source)
└── examples/01-fake-agent-telegram-bot/  # Fake agent example (Docker, mock-agent, debug-http)
    └── tools/system-info/          # Demo MCP tool binary (uses protoclaw-sdk-tool)
```

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `crates/protoclaw/src/cli.rs` | Clap derive, dispatched from `main.rs` |
| Change boot/shutdown order | `crates/protoclaw/src/supervisor.rs` | `MANAGER_ORDER` constant — read anti-patterns first |
| Add new manager | `crates/protoclaw-core/src/manager.rs` | Implement `Manager` trait, wire in supervisor |
| Modify ACP protocol | `crates/protoclaw-sdk-types/src/acp.rs` | Canonical location for ACP wire types; `protoclaw-agents/acp_types.rs` re-exports for backward compat |
| Add channel type | `crates/protoclaw-channels/` + `ext/channels/` | Manager routes, binary in ext/ |
| Add MCP tool | `crates/protoclaw-tools/src/mcp_host.rs` | McpHost manages external MCP server connections |
| Add WASM tool | `crates/protoclaw-tools/src/wasm_runner.rs` | WasmToolRunner + WasmTool for sandboxed execution |
| Build demo tool | `examples/01-fake-agent-telegram-bot/tools/system-info/` | Workspace member, uses protoclaw-sdk-tool |
| Change config schema | `crates/protoclaw-config/src/types.rs` | Serde structs (`WorkspaceConfig` enum, `AgentConfig`), update tests in `lib.rs` |
| Change session persistence | `crates/protoclaw-core/src/session_store.rs` | SessionStore trait, DynSessionStore, PersistedSession, NoopSessionStore |
| Change SQLite store impl | `crates/protoclaw-core/src/sqlite_session_store.rs` | SqliteSessionStore (rusqlite, bundled) |
| Modify JSON-RPC framing | `crates/protoclaw-jsonrpc/src/codec.rs` | LinesCodec-based, line-delimited JSON |
| Build channel SDK | `crates/protoclaw-sdk-channel/` | Channel trait + ChannelHarness |
| Build tool SDK | `crates/protoclaw-sdk-tool/` | Tool trait + ToolServer |
| Mock agent binary | `ext/agents/mock-agent/` | Mock ACP agent for testing (echo + thinking simulation + commands) |
| ACP↔HTTP bridge | `ext/agents/acp-bridge/` | Generic bridge: translates ACP stdio to HTTP REST+SSE (e.g. OpenCode serve API) |
| Add test helper | `crates/protoclaw-test-helpers/` | Shared across all crate tests |
| Integration tests | `tests/integration/tests/e2e.rs` | Requires `cargo build` first (needs mock-agent binary) |
| Dev iteration (contributor) | `examples/02-real-agents-telegram-bot/dev.sh` | Contributor-only helper — incremental rebuild + restart via persistent builder container; not needed to run the bot |
| Dev builder image (contributor) | `examples/02-real-agents-telegram-bot/Dockerfile.dev-builder` | Local source build with cargo-chef caching; contributor-only, not used by production `docker-compose.yml` |

## Crate Dependency Flow

```
protoclaw (binary)
├── protoclaw-config
├── protoclaw-core
├── protoclaw-agents ──→ protoclaw-core, protoclaw-jsonrpc
├── protoclaw-channels ─→ protoclaw-core, protoclaw-jsonrpc, protoclaw-sdk-types
└── protoclaw-tools ───→ protoclaw-core

SDK crates (for external implementors):
├── protoclaw-sdk-types (shared types: wire types, SessionKey, ChannelEvent, ACP wire types)
├── protoclaw-sdk-agent ──→ sdk-types, jsonrpc
├── protoclaw-sdk-channel ─→ sdk-types, jsonrpc
└── protoclaw-sdk-tool ───→ sdk-types

Example/ext binaries:
├── system-info (example) ──→ sdk-tool
├── mock-agent (ext) ──→ serde_json, tokio, uuid
└── acp-bridge (ext) ──→ sdk-types, jsonrpc, reqwest
```

## Conventions

- **Error handling boundary**: `thiserror` for library crates, `anyhow` only at application entry points (`main.rs`, `supervisor::run()`, `init.rs`, `status.rs`)
- **No `unsafe`**: Zero unsafe blocks exist. Do not introduce any.
- **unwrap() rule**: `.expect("reason")` for true invariants (piped stdio, consumed-once fields). Bare `.unwrap()` only in tests. Use `?` for fallible paths.
- **Module structure**: Flat `lib.rs` with `pub mod` + `pub use` re-exports. No `mod.rs` files.
- **Manager communication**: `tokio::sync::mpsc` channels via `ManagerHandle<C>`. No shared mutable state between managers.
- **Config layering**: Defaults → YAML file → env vars (`PROTOCLAW_` prefix, `__` separator). Top-level fields: `log_level` (default `"info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn,tower=warn"`, accepts full `EnvFilter` directive syntax), `extensions_dir` (default "/usr/local/bin"). `@built-in/{agents,channels,tools}/<name>` binary prefix resolved against `extensions_dir` in supervisor before manager construction. Legacy flat paths (e.g. `@built-in/mock-agent`) supported via built-in aliases with deprecation warnings.
- **Tracing**: Use `tracing` spans/events, not `println!` or `log` crate. Exception: CLI entry points (`main.rs`, `init.rs`, `status.rs`) may use `println!`/`eprintln!` for user-facing output before tracing is initialized.
- **Test framework**: `rstest = "0.23"` is a `[dev-dependency]` in every workspace crate. Use `#[rstest]` for all new and migrated tests.
- **BDD test naming**: Tests use `when_action_then_result` or `given_precondition_when_action_then_result` naming. No `test_` prefix. No `it_` prefix.
- **Fixtures**: rstest fixtures are free functions named `given_*` that return a precondition value. Example: `fn given_empty_buffer() -> BytesMut { BytesMut::new() }`.
- **Parameterised tests**: Use `#[case::label_name]` for named scenarios instead of anonymous `#[case]`. Example: `#[case::empty_input("")]`.
- **Async unit tests**: `#[rstest] #[tokio::test]` — two separate attributes, tokio drives execution.
- **Async integration tests**: `#[rstest] #[test_log::test(tokio::test)]` — three attributes; test_log captures tracing output on failure.

**Test examples:**

*Sync unit test with fixture:*
```rust
use rstest::{fixture, rstest};

#[fixture]
fn given_empty_buffer() -> BytesMut { BytesMut::new() }

#[rstest]
fn when_encoding_valid_json_then_output_ends_with_newline(
    given_empty_buffer: BytesMut,
) {
    let mut codec = NdJsonCodec::new();
    let mut buf = given_empty_buffer;
    codec.encode(serde_json::json!({"k": "v"}), &mut buf).unwrap();
    assert!(buf.ends_with(b"\n"));
}
```

*Async unit test:*
```rust
#[rstest]
#[tokio::test]
async fn when_tool_called_with_valid_input_then_returns_ok() {
    let tool = MyTool;
    let result = tool.execute(serde_json::json!({})).await;
    assert!(result.is_ok());
}
```

*Parameterised test:*
```rust
#[rstest]
#[case::empty_string("")]
#[case::whitespace("   ")]
#[case::newline_only("\n")]
fn when_decoding_non_json_input_then_returns_none(#[case] input: &str) {
    let mut codec = NdJsonCodec::new();
    let mut buf = BytesMut::from(input);
    assert!(codec.decode(&mut buf).unwrap().is_none());
}
```

- **AGENTS.md maintenance**: When making code changes that affect module structure, public APIs, conventions, or anti-patterns documented in any AGENTS.md file, update the relevant AGENTS.md file(s) in the same commit. See "AGENTS.md Auto-Update Rule" below.

## Anti-Patterns (DO NOT)

- **No `unsafe`**: Zero unsafe blocks exist. Do not introduce any.
- **No shared mutable state between managers**: All cross-manager communication is `tokio::sync::mpsc` via `ManagerHandle<C>`. No `Arc<Mutex<>>` across manager boundaries.
- **No `anyhow` in library crates**: Use `thiserror` typed enums at all library API boundaries. `anyhow` only in `main.rs`, `supervisor::run()`, `init.rs`, `status.rs`.
- **No bare `.unwrap()` in production code**: Use `.expect("reason")` for true invariants, `?` for fallible paths. Bare `.unwrap()` only in `#[cfg(test)]`.
- **No `mod.rs` files**: All modules use flat `lib.rs` with `pub mod` + `pub use` re-exports.
- **No `println!` or `log` crate**: Use `tracing` spans/events exclusively.
- **Do not change `MANAGER_ORDER`**: Boot order `tools → agents → channels` and reverse shutdown are load-bearing. Tests verify this explicitly.
- **Do not call `run()` without `start()`**: Manager lifecycle is `start().await?` then `run(cancel).await`. Both phases required.
- **Do not call `run()` twice**: `cmd_rx` is consumed via `.take()` on first `run()`. Second call panics.
- **`ChannelEvent` lives in `protoclaw-sdk-types`**: Relocated from `protoclaw-core` in v5.0. `protoclaw-core` re-exports it for backward compatibility. Both agents and channels import from `protoclaw-sdk-types` directly.
- **ACP wire types live in `protoclaw-sdk-types`**: Relocated from `protoclaw-agents` in v0.3.1. `protoclaw-agents/acp_types.rs` re-exports for backward compatibility. The bridge and SDK consumers import from `protoclaw-sdk-types::acp` directly.
- **Do not remove the 50ms sleep in `poll_channels()`**: It prevents busy-looping in the channel polling select.
- **Do not access `binary`/`env`/`working_dir` on `AgentConfig` directly**: These fields moved into `WorkspaceConfig::Local`. Match on `agent.workspace` to extract them.
- **No `std::env::var` in channel/tool binaries**: Runtime config flows through the initialize handshake (`ChannelInitializeParams.options`). CLI entry points (`main.rs`, `init.rs`, `status.rs`) are exempt.
- **No cross-manager crate imports**: Managers communicate only via `ManagerHandle<C>` commands. Use trait abstractions (e.g., `AgentDispatch`) instead of importing another manager's crate.
- **Config-driven channels**: Channel subprocesses receive configuration through `ChannelInitializeParams.options`, not environment variables. `ChannelConfig.options` in `protoclaw.yaml` is the single source.

## Design Documentation

For deeper context on design decisions, architecture rationale, and failure modes:
- `docs/design-principles.md` — Core invariants, why three managers, failure mode catalog, anti-pattern reasoning

Load `docs/design-principles.md` when:
- Making architectural changes (adding managers, changing boot order)
- Debugging crash recovery or lifecycle issues
- Questioning why a pattern exists (the "why" behind anti-patterns)

## Commands

```bash
cargo build                                    # Build all workspace members
cargo test                                     # Unit tests (all crates)
cargo build --bin mock-agent --bin debug-http   # Required BEFORE integration tests
cargo test -p integration                      # E2E tests (needs binaries built first)
cargo clippy --workspace                       # Lint all crates
```

## AGENTS.md Auto-Update Rule

When making code changes that affect any of the following, update the relevant AGENTS.md file(s) in the same commit:
- Module structure (adding/removing/renaming modules or crates)
- Public API changes (new traits, renamed types, changed signatures)
- Conventions (new patterns established, old patterns deprecated)
- Anti-patterns (new "do not" rules discovered)
- Build/test commands (new binaries, changed test requirements)
- Crate dependency changes (new edges in the dependency graph)

Check which AGENTS.md files exist in the affected directories and their parents. Update all that document the changed area. If unsure, update the root AGENTS.md at minimum.

## v5.1 Changes

- v5.1 milestone (Tech Debt & Hardening, phases 45-54) complete
- rstest/BDD test conventions enforced workspace-wide (all new and migrated tests use `#[rstest]`, `when_*`/`given_*` naming)

## v5.2 Changes

- `PreopenedDir.readonly` default flipped from `false` to `true` — WASM sandbox filesystem access is read-only by default; set `readonly: false` in config to grant write access
- `WasmToolRunner` now wires `memory_limit_bytes` into a `ResourceLimiter` on each `Store` and `preopened_dirs` into `WasiCtxBuilder` via `.preopened_dir()`; WASM tools enforce configured resource and filesystem boundaries
- `WasmState` wrapper struct introduced in `wasm_runner.rs` to hold both `WasiP1Ctx` and `WasmResourceLimiter` as `Store` data (required by wasmtime's `store.limiter()` API)

## v0.3.0 Changes

- `ToolConfig.options` wired as env vars to MCP server subprocesses (`external.rs`) and as WASI env vars to WASM tools (`wasm_runner.rs`, `wasm_tool.rs`)
- Supervisor circuit breaker tracing enhanced with structured `max_restarts` and `restart_window_secs` fields on both `CrashAction::Disabled` tracing events
- `AgentsCommand` extended with `ForkSession`, `ListSessions`, `CancelSession` — capability-gated dispatch checks `slot.agent_capabilities.session_capabilities` before sending fork/list requests; missing capability returns `AgentsError::CapabilityNotSupported`
- `ContentKind::AvailableCommandsUpdate { commands }` added to `protoclaw-sdk-types` — channels receive typed command-list updates from agents
- Telegram channel calls `bot.set_my_commands()` on `AvailableCommandsUpdate` — agent-provided commands appear in Telegram's `/` menu
- Per-session cancel via `CancelSession` command sends `session/cancel` to the agent for a specific session; the existing broadcast `CancelOperation` is preserved for shutdown scenarios
- `fs/read_text_file` and `fs/write_text_file` ACP tool handlers in `protoclaw-agents/src/manager.rs` enforce path sandboxing: all requested paths must resolve inside the agent's `working_dir`; traversals outside return JSON-RPC error code `-32000` with message `"path outside allowed directory"`; `resolve_agent_cwd()` and `validate_fs_path()` / `validate_fs_write_path()` helpers extracted for reuse
- `debug-http` channel supports optional bearer token auth: set `API_KEY` in `ChannelConfig.options` to require `Authorization: Bearer <token>` on all routes except `/health`; when `API_KEY` is absent, no auth is required (backward compatible); implemented via `axum::middleware::from_fn_with_state`
- `SupervisorConfig.admin_port` added (default `3000`) — configures the admin HTTP server port; set in `protoclaw.yaml` under `supervisor.admin_port`
- `HealthSnapshot`, `HealthStatus`, `AgentHealth` types added to `protoclaw-core` (`health.rs`) — point-in-time supervisor health used by both the admin endpoint and the CLI `status` command
- Admin HTTP server added to supervisor (`crates/protoclaw-supervisor/src/admin_server.rs`) — spawned as a background tokio task before the health loop; binds to `127.0.0.1:{admin_port}`
  - `GET /health` — JSON `HealthSnapshot`; 200 when healthy, 503 when degraded (tools degradation does NOT trigger 503)
  - `GET /metrics` — Prometheus text format via `metrics-exporter-prometheus`
- Metrics emitted by supervisor: `protoclaw_agents_connected` (gauge), `protoclaw_channels_running` (gauge), `protoclaw_manager_restarts_total` (counter, `manager` label)
- Metrics emitted by tools manager: `protoclaw_tool_invocations_total` (counter, `tool`+`status` labels), `protoclaw_tool_duration_seconds` (histogram, `tool` label)
- Audit logging added to `route_call()` in `protoclaw-tools/src/manager.rs` — `tracing::info!(target: "protoclaw::audit", tool_name, success, duration_ms, "tool_invoked")` emitted after every tool dispatch; no separate log file in v0.3.0, audit events flow through the tracing subscriber
- ACP protocol version negotiation added: supervisor sends `protocol_version: 2` in `initialize`; versions 1 and 2 are both accepted; version ≥3 or unknown returns `AcpError::ProtocolMismatch`; negotiated version stored in `AgentSlot.protocol_version: u32`; mock-agent updated to respond with `protocolVersion: 2`; `ChannelHarness` emits `tracing::warn!` if `protocol_version != 1` but accepts without failing the handshake
- Extension type naming in `session_update_type_name()` renamed: `"current_mode_update"` → `"extension:current_mode"`, `"config_option_update"` → `"extension:config_option"`, `"session_info_update"` → `"extension:session_info"`; `#[non_exhaustive]` added to `SessionUpdateType`; `CurrentModeUpdate`, `ConfigOptionUpdate`, `SessionInfoUpdate` variants documented as extension types
- `ContentKind` and `ChannelEvent` (in `protoclaw-sdk-types`) are now `#[non_exhaustive]` — external match arms must include a wildcard `_ =>` arm; wildcard arms added to `telegram/deliver.rs` and `protoclaw-channels/src/manager.rs`
- Stability disclaimer added to all four SDK crate doc roots (`protoclaw-sdk-types`, `protoclaw-sdk-agent`, `protoclaw-sdk-channel`, `protoclaw-sdk-tool`)

## v0.3.1 Changes

- ACP wire types relocated from `protoclaw-agents/src/acp_types.rs` to `protoclaw-sdk-types/src/acp.rs` — canonical import is `protoclaw_sdk_types::acp::*`; `protoclaw-agents/acp_types.rs` re-exports for backward compatibility
- `session_update_type_name()` in `protoclaw-agents/src/manager.rs` gained `_ => "unknown"` wildcard arm — required because `SessionUpdateType` is `#[non_exhaustive]` and now lives in an external crate
- Legacy aliases updated in `protoclaw-config/src/resolve.rs`: `agents/opencode` → `agents/acp-bridge`, `agents/opencode-wrapper` → `agents/acp-bridge`, `acp` → `agents/acp-bridge`
- `ext/agents/acp-bridge/` added — generic ACP↔HTTP bridge binary translating stdio JSON-RPC 2.0 to OpenCode's HTTP REST+SSE serve API; depends only on `protoclaw-jsonrpc` and `protoclaw-sdk-types` (zero core coupling)
- `ext/agents/opencode-wrapper/` removed — replaced by `acp-bridge`; legacy alias preserves backward compat for existing configs
- mock-agent sends `available_commands_update` notification after initialize — includes demo `help` and `status` commands; provides end-to-end test path for command registration
- `StringOrArray` config type added — `binary`/`entrypoint` fields accept string or array in YAML
- `AgentConfig.args` field removed — args merged into `binary`/`entrypoint` array (breaking change)
- `LocalWorkspaceConfig.binary`: `String` → `StringOrArray`; `DockerWorkspaceConfig.entrypoint`: `Option<String>` → `Option<StringOrArray>`
- Permission flow fixed: `request_id` falls back to JSON-RPC `id` field when `params.requestId` missing (OpenCode compat)
- `channel/requestPermission` changed from notification to request — harness now returns response via JSON-RPC id
- `_raw_response` sentinel replaced with `AgentConnection::send_raw()` — writes directly to agent stdin without method envelope
- Default `log_level` now suppresses hyper/reqwest/h2/tower noise: `"info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn,tower=warn"`
- `log_level` config field accepts full `EnvFilter` directive syntax (e.g. `"debug,hyper=warn"`)
- Permission flow tracing added at every handoff point (agents manager, channels manager, harness, telegram dispatcher, telegram permissions)
- `dev.sh` helper added to Example 02 for fast incremental rebuilds via persistent builder container
- `Dockerfile.dev-builder` added to Example 02 for local source builds with cargo-chef caching
- Example 02 simplified: 3-stage Dockerfile, single opencode agent with `entrypoint: ["opencode", "acp"]`, Docker-only test.sh

## v0.3.2 Changes

- **ACP permission wire format**: `channel/requestPermission` is a JSON-RPC **request** (not notification) since v0.3.1. The permission response the agent receives via `send_raw()` has this exact shape:
  ```json
  {
    "jsonrpc": "2.0",
    "id": "<request_id>",
    "result": {
      "outcome": {
        "outcome": "selected",
        "optionId": "<option_id>"
      }
    }
  }
  ```
  The `request_id` falls back to the JSON-RPC `id` field when `params.requestId` is missing or empty (OpenCode compat). Auto-deny sends `optionId: "denied"`. See `crates/protoclaw-agents/AGENTS.md` for `RespondPermission` command details.

- **`send_raw` bypass pattern**: `AgentConnection::send_raw(msg: serde_json::Value)` in `crates/protoclaw-agents/src/connection.rs` writes a raw `serde_json::Value` directly to agent stdin without wrapping it in a JSON-RPC method envelope. Used for permission responses that must match the agent's expected wire format exactly. Logs the exact JSON at DEBUG level (`send_raw to agent stdin`). The `_raw_response` sentinel was removed in v0.3.1; `send_raw()` replaced all four call sites.

- **Bollard stdin flush behavior**: The Docker backend's stdin bridge (`bollard::container::AttachContainerOptions`) may silently drop writes if the container's stdin pipe is broken. Write/flush failures on the agent stdin bridge emit `tracing::warn!` with the error and context, rather than panicking or silently discarding.

- **`protoclaw-test-helpers` `test_support` module**: `poll.rs` added with the `wait_for_condition` async helper. Polls an async condition closure at ~100ms intervals until it returns `Some(T)` or the timeout expires. Returns `Some(T)` on success, `None` on timeout. Used in E2E permission tests. Full module list: `config`, `handles`, `paths`, `poll`, `ports`, `sse`, `supervisor`, `timeout`.

- **PermissionBroker auto-deny gap (known limitation)**: When `permission_timeout_secs` fires, the channels manager sends auto-deny to the agent via `AgentsCommand::RespondPermission`, but does NOT resolve the channel's `PermissionBroker` oneshot sender. The channel harness remains blocked until its own internal timeout fires. This is a known limitation — unifying the auto-deny path to also resolve the broker oneshot is a future cleanup. See `crates/protoclaw-channels/AGENTS.md` for the full description.

## v0.3.3 Changes

- **Session persistence**: Pluggable session store so sessions survive agent crashes and protoclaw restarts. `SessionStore` trait (native async fn in trait) + `DynSessionStore` (object-safe `Pin<Box>` wrapper with blanket impl) in `protoclaw-core/src/session_store.rs`. `NoopSessionStore` is the default when no store is configured.
- **SQLite backend**: `SqliteSessionStore` in `protoclaw-core/src/sqlite_session_store.rs` — uses `rusqlite` with `bundled` feature (no system SQLite dependency). `Arc<Mutex<Connection>>` + `tokio::task::spawn_blocking` pattern because `rusqlite::Connection` is not `Send` across await points. WAL journal mode, auto-creates schema on open.
- **SessionStoreConfig**: Tagged enum in `protoclaw-config/src/types.rs` — `type: none` (default) or `type: sqlite` with optional `path` and `ttl_days` (default 7). Added to `ProtoclawConfig` as `session_store` field with `#[serde(default)]`.
- **Supervisor store wiring**: `build_session_store()` in `protoclaw-supervisor/src/lib.rs` matches on `SessionStoreConfig`, constructs the store, and passes it to `AgentsManager` via `.with_session_store()`. Falls back to `NoopSessionStore` on open failure (logged, not fatal).
- **Boot cleanup**: `AgentsManager::start()` calls `delete_expired(session_ttl_secs)` before `load_open_sessions()` — prunes stale sessions at boot. `session_ttl_secs` field (default 7 days) with `.with_session_ttl_secs()` builder.
- **Session loading at boot**: `start()` calls `load_open_sessions()` and populates `stale_sessions` on matching agent slots — enables `heal_session()` to attempt `session/load` on the next prompt.
- **Crash recovery**: `restore_or_start_session()` drains `session_map` into `stale_sessions` on crash. `try_restore_session()` reads from `stale_sessions` and moves them back to `session_map` on successful `session/load`. `heal_session()` in `prompt_session()` tries load from stale, falls back to `create_session`.
- **Lifecycle persistence**: `create_session()` calls `upsert_session(closed=false)` after success. `prompt_session()` spawns background `update_last_active()`. `shutdown_all()` calls `mark_closed()` for each session before `session/close`. All store failures are logged (`tracing::warn`) but never block runtime operations.
- **Error delivery (ERR-01)**: `dispatch_to_agent()` in channels manager delivers `channel/deliverMessage` to the originating channel when `PromptSession` fails — users see error messages instead of silence.
- **Structured audit tracing (ERR-02)**: `heal_session()` emits `tracing::info!` events with structured fields at each recovery step: `step="recovery_started"`, `step="load_attempted"` (with `success`), `step="create_attempted"` (with `success`), `step="recovery_outcome"` (with `outcome` = `"loaded"` / `"created"` / `"failed"`).
- **`stale_sessions` field on `AgentSlot`**: `HashMap<SessionKey, String>` — populated by draining `session_map` on crash, consumed by `heal_session()` and `try_restore_session()` for recovery.
