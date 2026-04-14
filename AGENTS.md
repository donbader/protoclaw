# ANYCLAW — Project Knowledge Base

Infrastructure sidecar connecting AI agents to channels (Telegram, Slack) and tools (MCP servers, WASM sandboxed). Rust workspace, ACP protocol (JSON-RPC 2.0 over stdio), three-manager architecture with Supervisor.

## Structure

```
anyclaw/
├── crates/                         # Core workspace crates (12 total)
│   ├── anyclaw/                  # Binary: CLI + Supervisor (entry point)
│   ├── anyclaw-core/             # Shared: Manager trait, backoff, crash tracker, message types
│   ├── anyclaw-agents/           # ACP protocol layer, agent subprocess management
│   ├── anyclaw-channels/         # Channel subprocess routing + lifecycle
│   ├── anyclaw-tools/            # MCP host, WASM sandbox, tools manager
│   ├── anyclaw-config/           # Figment-based config loading (anyclaw.yaml)
│   ├── anyclaw-jsonrpc/          # JSON-RPC 2.0 codec + types (LinesCodec-based)
│   ├── anyclaw-sdk-types/        # Shared SDK types (capabilities, permissions, ACP wire types)
│   ├── anyclaw-sdk-agent/        # SDK: AgentAdapter trait + GenericAcpAdapter
│   ├── anyclaw-sdk-channel/      # SDK: Channel trait + ChannelHarness
│   ├── anyclaw-sdk-tool/         # SDK: Tool trait + ToolServer
│   └── anyclaw-test-helpers/     # Shared test utilities (dev-dependency)
├── ext/                            # External binaries (spawned as subprocesses)
│   ├── agents/
│   │   ├── mock-agent/             # Mock ACP agent binary (echo + thinking simulation + commands)
│   │   └── acp-bridge/             # Generic ACP↔HTTP bridge (translates stdio JSON-RPC to REST+SSE)
│   └── channels/
│       ├── telegram/               # Telegram channel implementation
│       └── debug-http/             # Debug HTTP channel (minimal)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
├── examples/01-fake-agent-telegram-bot/  # Fake agent example (Docker, mock-agent, debug-http)
│   └── tools/system-info/          # Demo MCP tool binary (uses anyclaw-sdk-tool)
└── examples/02-real-agents-telegram-bot/ # Real agent example (Docker, opencode, debug-http + telegram)
```

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `crates/anyclaw/src/cli.rs` | Clap derive, dispatched from `main.rs` |
| Change boot/shutdown order | `crates/anyclaw/src/supervisor.rs` | `MANAGER_ORDER` constant — read anti-patterns first |
| Add new manager | `crates/anyclaw-core/src/manager.rs` | Implement `Manager` trait, wire in supervisor |
| Modify ACP protocol | `crates/anyclaw-sdk-types/src/acp.rs` | Canonical location; `anyclaw-agents/acp_types.rs` re-exports for backward compat |
| Add channel type | `crates/anyclaw-channels/` + `ext/channels/` | Manager routes, binary in ext/ |
| Add MCP tool | `crates/anyclaw-tools/src/mcp_host.rs` | McpHost manages external MCP server connections |
| Add WASM tool | `crates/anyclaw-tools/src/wasm_runner.rs` | WasmToolRunner + WasmTool for sandboxed execution |
| Build demo tool | `examples/01-fake-agent-telegram-bot/tools/system-info/` | Workspace member, uses anyclaw-sdk-tool |
| Change config schema | `crates/anyclaw-config/src/types.rs` | Serde structs (`WorkspaceConfig` enum, `AgentConfig`) |
| Change session persistence | `crates/anyclaw-core/src/session_store.rs` | SessionStore trait, DynSessionStore, NoopSessionStore |
| Change SQLite store impl | `crates/anyclaw-core/src/sqlite_session_store.rs` | SqliteSessionStore (rusqlite, bundled) |
| Modify JSON-RPC framing | `crates/anyclaw-jsonrpc/src/codec.rs` | LinesCodec-based, line-delimited JSON |
| Build channel SDK | `crates/anyclaw-sdk-channel/` | Channel trait + ChannelHarness |
| Build tool SDK | `crates/anyclaw-sdk-tool/` | Tool trait + ToolServer |
| Mock agent binary | `ext/agents/mock-agent/` | Mock ACP agent for testing |
| ACP↔HTTP bridge | `ext/agents/acp-bridge/` | Translates ACP stdio to HTTP REST+SSE |
| Add test helper | `crates/anyclaw-test-helpers/` | Shared across all crate tests |
| Integration tests | `tests/integration/tests/e2e.rs` | Requires `cargo build` first (needs mock-agent binary) |
| Dev iteration (contributor) | `examples/02-real-agents-telegram-bot/docker-compose.dev.yml` | Override: `docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build` |

## Crate Dependency Flow

```
anyclaw (binary)
├── anyclaw-config
├── anyclaw-core
├── anyclaw-agents ──→ anyclaw-core, anyclaw-jsonrpc
├── anyclaw-channels ─→ anyclaw-core, anyclaw-jsonrpc, anyclaw-sdk-types
└── anyclaw-tools ───→ anyclaw-core

SDK crates (for external implementors):
├── anyclaw-sdk-types (shared types: wire types, SessionKey, ChannelEvent, ACP wire types)
├── anyclaw-sdk-agent ──→ sdk-types, jsonrpc
├── anyclaw-sdk-channel ─→ sdk-types, jsonrpc
└── anyclaw-sdk-tool ───→ sdk-types

Example/ext binaries:
├── system-info (example) ──→ sdk-tool
├── mock-agent (ext) ──→ serde_json, tokio, uuid
└── acp-bridge (ext) ──→ sdk-types, jsonrpc, reqwest
```

## Conventions

- **Error handling boundary**: `thiserror` for library crates, `anyhow` only at application entry points (`main.rs`, `supervisor.rs`, `init.rs`, `status.rs`)
- **No `unsafe`**: Zero unsafe blocks exist. Do not introduce any.
- **unwrap() rule**: `.expect("reason")` for true invariants. Bare `.unwrap()` only in tests. Use `?` for fallible paths.
- **Module structure**: Flat `lib.rs` with `pub mod` + `pub use` re-exports. No `mod.rs` files.
- **Manager communication**: `tokio::sync::mpsc` channels via `ManagerHandle<C>`. No shared mutable state between managers.
- **Config layering**: Defaults → YAML file → env vars (`ANYCLAW_` prefix, `__` separator). `@built-in/{agents,channels,tools}/<name>` binary prefix resolved against `extensions_dir`.
- **Tracing**: Use `tracing` spans/events, not `println!` or `log` crate. Exception: CLI entry points may use `println!`/`eprintln!` before tracing is initialized.
- **Test framework**: `rstest = "0.23"` with `#[rstest]` for all tests. BDD naming: `when_action_then_result` or `given_precondition_when_action_then_result`. Fixtures: `fn given_*()`. Parameterised: `#[case::label_name]`. Async: `#[rstest] #[tokio::test]`.
- **AGENTS.md maintenance**: When code changes affect module structure, public APIs, conventions, or anti-patterns, update the relevant AGENTS.md file(s) in the same commit.

## Anti-Patterns (DO NOT)

- **No shared mutable state between managers**: All cross-manager communication is `tokio::sync::mpsc` via `ManagerHandle<C>`. No `Arc<Mutex<>>` across manager boundaries.
- **No `anyhow` in library crates**: Use `thiserror` typed enums. `anyhow` only in entry points.
- **No bare `.unwrap()` in production code**: Use `.expect("reason")` or `?`.
- **No `mod.rs` files**: Flat `lib.rs` with `pub mod` + `pub use`.
- **No `println!` or `log` crate**: Use `tracing` exclusively.
- **Do not change `MANAGER_ORDER`**: Boot order `tools → agents → channels` and reverse shutdown are load-bearing.
- **Do not call `run()` without `start()`**: Manager lifecycle is `start().await?` then `run(cancel).await`. Both required.
- **Do not call `run()` twice**: `cmd_rx` is consumed via `.take()` on first `run()`.
- **Do not access `binary`/`env`/`working_dir` on `AgentConfig` directly**: Match on `agent.workspace` (`WorkspaceConfig::Local` or `WorkspaceConfig::Docker`).
- **No `std::env::var` in channel/tool binaries**: Config flows through the initialize handshake (`ChannelInitializeParams.options`).
- **No cross-manager crate imports**: Use trait abstractions (e.g., `AgentDispatch`) instead.
- **`ChannelEvent` lives in `anyclaw-sdk-types`**: `anyclaw-core` re-exports for backward compat.
- **ACP wire types live in `anyclaw-sdk-types`**: `anyclaw-agents/acp_types.rs` re-exports for backward compat.

## Design Documentation

For deeper context on design decisions, architecture rationale, and failure modes:
- `docs/design-principles.md` — Core invariants, why three managers, failure mode catalog

Load when making architectural changes, debugging crash recovery, or questioning why a pattern exists.

## Commands

```bash
cargo build                                    # Build all workspace members
cargo test                                     # Unit tests (all crates)
cargo build --bin mock-agent --bin debug-http   # Required BEFORE integration tests
cargo test -p integration                      # E2E tests (needs binaries built first)
cargo clippy --workspace                       # Lint all crates
```
