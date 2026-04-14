# Codebase Structure

**Analysis Date:** 2026-04-14

## Directory Layout

```
anyclaw/
├── crates/                           # Core workspace crates (14 members)
│   ├── anyclaw/                      # Binary: CLI entry point + supervisor re-export
│   ├── anyclaw-supervisor/           # Supervisor orchestration (extracted from anyclaw)
│   ├── anyclaw-core/                 # Shared primitives: Manager trait, backoff, types
│   ├── anyclaw-agents/               # AgentsManager: ACP protocol, subprocess lifecycle
│   ├── anyclaw-channels/             # ChannelsManager: channel routing, session queues
│   ├── anyclaw-tools/                # ToolsManager: MCP host, WASM sandbox
│   ├── anyclaw-config/               # Figment config loading + validation
│   ├── anyclaw-jsonrpc/              # JSON-RPC 2.0 codec + types
│   ├── anyclaw-sdk-types/            # Shared wire types (dependency-free leaf)
│   ├── anyclaw-sdk-agent/            # SDK: AgentAdapter trait + GenericAcpAdapter
│   ├── anyclaw-sdk-channel/          # SDK: Channel trait + ChannelHarness
│   ├── anyclaw-sdk-tool/             # SDK: Tool trait + ToolServer
│   └── anyclaw-test-helpers/         # Shared test utilities (dev-dependency)
├── ext/                              # External binaries (spawned as subprocesses)
│   ├── agents/
│   │   └── mock-agent/               # Mock ACP agent (echo + thinking simulation)
│   ├── channels/
│   │   ├── debug-http/               # Debug HTTP channel
│   │   ├── telegram/                 # Telegram channel
│   │   └── sdk-test-channel/         # SDK test channel
│   └── tools/
│       └── sdk-test-tool/            # SDK test tool
├── tests/
│   └── integration/                  # E2E tests (spawn real supervisor + mock-agent)
├── examples/
│   ├── 01-fake-agent-telegram-bot/   # Fake agent example (Docker, mock-agent)
│   │   └── tools/system-info/        # Demo MCP tool binary
│   └── 02-real-agents-telegram-bot/  # Real agent example (Docker, opencode)
├── docs/                             # Design documentation
│   └── design-principles.md          # Core invariants, failure modes
└── Cargo.toml                        # Workspace root
```

## Crate Dependency Graph

```
anyclaw (binary)
├── anyclaw-supervisor ──→ anyclaw-agents, anyclaw-channels, anyclaw-tools, anyclaw-core, anyclaw-config
├── anyclaw-config
└── anyclaw-core

anyclaw-agents ──→ anyclaw-core, anyclaw-jsonrpc, anyclaw-sdk-types
anyclaw-channels ─→ anyclaw-core, anyclaw-jsonrpc, anyclaw-sdk-types
anyclaw-tools ───→ anyclaw-core

SDK crates (for external implementors):
├── anyclaw-sdk-types (leaf — serde only, no internal deps)
├── anyclaw-sdk-agent ──→ sdk-types, jsonrpc
├── anyclaw-sdk-channel ─→ sdk-types, jsonrpc
└── anyclaw-sdk-tool ───→ sdk-types
```

## Module Organization Pattern

Every crate uses flat `lib.rs` with `pub mod` declarations and `pub use` re-exports. No `mod.rs` files anywhere.

```rust
// Example: crates/anyclaw-tools/src/lib.rs
pub mod error;
pub mod external;
pub mod manager;
pub mod mcp_host;
pub mod wasm_runner;
pub mod wasm_tool;

pub use error::*;
pub use external::ExternalMcpServer;
pub use manager::*;
pub use mcp_host::McpHost;
pub use wasm_runner::WasmToolRunner;
pub use wasm_tool::WasmTool;
```

Cross-crate re-exports for backward compatibility:
- `anyclaw-core` re-exports `ChannelEvent` and `SessionKey` from `anyclaw-sdk-types`
- `anyclaw-agents` re-exports `AgentsCommand`, `ToolsCommand`, `McpServerUrl` from `anyclaw-core`
- `anyclaw-tools` re-exports `McpServerUrl`, `ToolsCommand` from `anyclaw-core`

## Key Files by Purpose

**Entry Points:**
- `crates/anyclaw/src/main.rs`: Binary entry — tracing init, CLI dispatch
- `crates/anyclaw/src/cli.rs`: Clap derive — `run`, `init`, `validate`, `status` subcommands
- `crates/anyclaw-supervisor/src/lib.rs`: `Supervisor` struct, `boot_managers()`, `shutdown_ordered()`, `check_and_restart_managers()`, `create_manager()` factory

**Configuration:**
- `crates/anyclaw-config/src/types.rs`: All config structs — `AnyclawConfig`, `AgentConfig`, `ChannelConfig`, `McpServerConfig`, `WasmToolConfig`, `SupervisorConfig`, `WorkspaceConfig` enum
- `crates/anyclaw-config/src/lib.rs`: Figment loading — `AnyclawConfig::load()`
- `crates/anyclaw-config/src/resolve.rs`: Binary path resolution (`@built-in/` prefix → `extensions_dir`)
- `crates/anyclaw-config/src/validate.rs`: Config validation rules
- `crates/anyclaw-config/src/defaults.yaml`: Default config values

**Protocol Definitions:**
- `crates/anyclaw-sdk-types/src/acp.rs`: ACP wire types (`InitializeParams`, `SessionNewParams`, `SessionPromptParams`, etc.)
- `crates/anyclaw-sdk-types/src/channel.rs`: Channel protocol types (capabilities, initialize, deliver, send, ack, content)
- `crates/anyclaw-sdk-types/src/channel_event.rs`: `ChannelEvent` enum — agents→channels bridge
- `crates/anyclaw-sdk-types/src/session_key.rs`: `SessionKey` newtype — routing key
- `crates/anyclaw-sdk-types/src/permission.rs`: Permission request/response types
- `crates/anyclaw-jsonrpc/src/types.rs`: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcMessage`
- `crates/anyclaw-jsonrpc/src/codec.rs`: `JsonRpcCodec` — `LinesCodec`-based line-delimited JSON over stdio

**Manager Implementations:**
- `crates/anyclaw-tools/src/manager.rs`: `ToolsManager` + `AggregatedToolServer`
- `crates/anyclaw-tools/src/mcp_host.rs`: `McpHost` — native tool registry
- `crates/anyclaw-tools/src/wasm_runner.rs`: `WasmToolRunner` — shared wasmtime engine
- `crates/anyclaw-tools/src/external.rs`: `ExternalMcpServer` — subprocess MCP client
- `crates/anyclaw-agents/src/manager.rs`: `AgentsManager` — session lifecycle, command handling
- `crates/anyclaw-agents/src/connection.rs`: `AgentConnection` — subprocess spawn, JSON-RPC framing
- `crates/anyclaw-agents/src/slot.rs`: Agent slot management
- `crates/anyclaw-agents/src/backend.rs`: `ProcessBackend` trait
- `crates/anyclaw-agents/src/local_backend.rs`: `LocalBackend` — local process spawn
- `crates/anyclaw-agents/src/docker_backend.rs`: `DockerBackend` — Docker container spawn
- `crates/anyclaw-channels/src/manager.rs`: `ChannelsManager` — routing, poll loop
- `crates/anyclaw-channels/src/connection.rs`: `ChannelConnection` — subprocess spawn, port discovery
- `crates/anyclaw-channels/src/session_queue.rs`: `SessionQueue` — per-session FIFO

**Core Primitives:**
- `crates/anyclaw-core/src/manager.rs`: `Manager` trait + `ManagerHandle<C>`
- `crates/anyclaw-core/src/backoff.rs`: `ExponentialBackoff` (100ms→30s) + `CrashTracker`
- `crates/anyclaw-core/src/types.rs`: ID newtypes (`SessionId`, `ChannelId`, `ManagerId`, `MessageId`)
- `crates/anyclaw-core/src/agents_command.rs`: `AgentsCommand` enum
- `crates/anyclaw-core/src/tools_command.rs`: `ToolsCommand` enum
- `crates/anyclaw-core/src/session_store.rs`: `SessionStore` trait, `DynSessionStore`, `NoopSessionStore`
- `crates/anyclaw-core/src/sqlite_session_store.rs`: `SqliteSessionStore`
- `crates/anyclaw-core/src/constants.rs`: Named constants (`CMD_CHANNEL_CAPACITY`, `EVENT_CHANNEL_CAPACITY`, etc.)

**SDK Surface:**
- `crates/anyclaw-sdk-agent/src/adapter.rs`: `AgentAdapter` trait
- `crates/anyclaw-sdk-agent/src/generic.rs`: `GenericAcpAdapter`
- `crates/anyclaw-sdk-channel/src/trait_def.rs`: `Channel` trait
- `crates/anyclaw-sdk-channel/src/harness.rs`: `ChannelHarness` — JSON-RPC stdio harness
- `crates/anyclaw-sdk-channel/src/broker.rs`: `PermissionBroker`
- `crates/anyclaw-sdk-channel/src/content.rs`: `ContentKind` typed dispatch
- `crates/anyclaw-sdk-channel/src/testing.rs`: `ChannelTester`
- `crates/anyclaw-sdk-tool/src/trait_def.rs`: `Tool` trait
- `crates/anyclaw-sdk-tool/src/server.rs`: `ToolServer` — JSON-RPC stdio server

## Where to Add New Code

**New CLI command:**
- Add variant to `Commands` enum in `crates/anyclaw/src/cli.rs`
- Add handler file in `crates/anyclaw/src/` (e.g., `my_command.rs`)
- Dispatch from `crates/anyclaw/src/main.rs`

**New manager:** (rare — read `docs/design-principles.md` first)
- Implement `Manager` trait in a new crate under `crates/`
- Add to `MANAGER_ORDER`, `ManagerKind` enum, and `create_manager()` in `crates/anyclaw-supervisor/src/lib.rs`
- Wire mpsc channels in `Supervisor::boot_managers()`

**New channel type:**
- Create binary in `ext/channels/<name>/` implementing the channel JSON-RPC protocol
- Use `anyclaw-sdk-channel` crate: implement `Channel` trait, wrap with `ChannelHarness`
- Add channel config to `anyclaw.yaml` under `channels`

**New MCP tool (external):**
- Create binary implementing MCP server protocol
- Add to `tools` section in `anyclaw.yaml` with `type: mcp`

**New WASM tool:**
- Compile to `.wasm` targeting WASI
- Add to `tools` section in `anyclaw.yaml` with `type: wasm`

**New agent adapter:**
- Create binary in `ext/agents/<name>/` implementing ACP protocol over stdio
- Use `anyclaw-sdk-agent` crate: implement `AgentAdapter` trait, wrap with `GenericAcpAdapter`
- Add agent config to `anyclaw.yaml` under `agents`

**New cross-manager command:**
- Add variant to `AgentsCommand` (`crates/anyclaw-core/src/agents_command.rs`) or `ToolsCommand` (`crates/anyclaw-core/src/tools_command.rs`)
- Handle in the receiving manager's `run()` loop

**New shared type:**
- Internal types: `crates/anyclaw-core/src/types.rs`
- Wire types (SDK-facing): `crates/anyclaw-sdk-types/src/`

**New test helper:**
- `crates/anyclaw-test-helpers/src/` — shared across all crate tests

**New integration test:**
- `tests/integration/tests/` — requires `cargo build --bin mock-agent --bin debug-http` first

## Naming Conventions

**Files:** `snake_case.rs` — e.g., `session_queue.rs`, `mcp_host.rs`, `wasm_runner.rs`
**Directories:** `kebab-case` for crates and ext binaries — e.g., `anyclaw-sdk-channel`, `debug-http`, `mock-agent`
**Crate names:** `anyclaw-{domain}` for internal, `anyclaw-sdk-{role}` for SDK crates

## Special Directories

**`ext/`:**
- Purpose: External binaries spawned as subprocesses by managers
- Generated: No (source code, compiled via workspace)
- Committed: Yes
- Each subdirectory is a workspace member with its own `Cargo.toml`

**`examples/`:**
- Purpose: End-to-end deployment examples with Docker Compose
- Contains: `docker-compose.yml`, config files, example tool binaries
- `examples/01-fake-agent-telegram-bot/tools/system-info/` is a workspace member

**`tests/integration/`:**
- Purpose: E2E tests spawning real supervisor with mock-agent
- Prerequisite: `cargo build --bin mock-agent --bin debug-http` before running
- Run with: `cargo test -p integration`

**`docs/`:**
- Purpose: Design documentation and architecture rationale
- Key file: `docs/design-principles.md` — core invariants, failure mode catalog

**`.planning/`:**
- Purpose: GSD planning and codebase analysis documents
- Generated: Yes (by GSD commands)
- Committed: Yes

---

*Structure analysis: 2026-04-14*
