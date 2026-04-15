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
│   └── tools/
│       └── system-info/            # Demo MCP tool binary (uses anyclaw-sdk-tool)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
├── examples/01-fake-agent-telegram-bot/  # Fake agent example (Docker, mock-agent, debug-http)
└── examples/02-real-agent-telegram/      # Real agent examples (Docker, debug-http + telegram)
    ├── opencode/                           # OpenCode agent variant
    └── kiro/                               # Kiro CLI agent variant
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
| Build demo tool | `ext/tools/system-info/` | Demo MCP tool binary, uses anyclaw-sdk-tool |
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
| Add agent variant | `examples/02-real-agent-telegram/AGENTS.md` | Copy existing variant, follow the guide |
| Dev iteration (contributor) | `examples/02-real-agent-telegram/dev/Makefile` | From variant dir: `make -f ../dev/Makefile dev` |

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

<!-- GSD:project-start source:PROJECT.md -->
## Project

**Anyclaw — Code Quality Milestone**

A comprehensive code quality improvement pass across the entire anyclaw workspace — 12 core crates, external binaries, and examples. The goal is to make every crate feel intentional: typed JSON everywhere, consistent error handling, dead code removed, full test coverage, zero clippy warnings. Crate-by-crate, breaking changes allowed.

**Core Value:** Every line of code should be there for a reason, with typed data flowing through typed interfaces — no `serde_json::Value` soup, no bare unwraps, no inconsistent patterns across crates.

### Constraints

- **No unsafe**: Zero unsafe blocks exist. Do not introduce any.
- **No mod.rs**: Flat lib.rs with pub mod + pub use. Convention must be maintained.
- **Manager communication**: tokio::sync::mpsc via ManagerHandle only. No shared mutable state across managers.
- **Boot order**: tools → agents → channels. Do not change MANAGER_ORDER.
- **Test framework**: rstest 0.23 with BDD naming. All new tests must follow this.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->
## Technology Stack

## Languages
- Rust, Edition 2024, MSRV 1.94 — all workspace crates
- YAML — configuration (`anyclaw.yaml`, `defaults.yaml`)
- WAT/WASM — sandboxed tool modules
## Runtime
- Tokio async runtime (multi-threaded `rt-multi-thread`)
- Tokio version: `1.50`
- Feature sets vary per crate; heaviest in `anyclaw-agents`: `fs`, `io-util`, `macros`, `net`, `process`, `rt`, `rt-multi-thread`, `sync`, `time`
- Cargo with workspace resolver v2
- Lockfile: `Cargo.lock` present (workspace)
## Frameworks
- Axum `0.8` — HTTP server (debug-http channel, supervisor health/metrics endpoint, MCP aggregated tool server)
- Teloxide `0.17` — Telegram Bot API (in `ext/channels/telegram`, features: `macros`, `rustls`, `ctrlc_handler`)
- rmcp `1.4` — MCP protocol client/server (in `anyclaw-tools` and `anyclaw-sdk-tool`, features: `client`, `server`, `transport-io`, `transport-child-process`, `transport-streamable-http-server`)
- rstest `0.26` — parameterized/fixture-based test framework (all crates)
- tokio-test `0.4` — async test utilities
- test-log `0.2` — tracing-aware test logging (integration tests, test-helpers)
- temp-env `0.3` — scoped env var manipulation in tests
- Cargo workspace — 12 core crates + 5 ext binaries + 1 example tool + 1 integration test package
- mold linker via clang for musl targets (`.cargo/config.toml`)
- Release profile: `strip = true`, `lto = "thin"`
## Key Dependencies
- `tokio` `1.50` — async runtime, subprocess management, channels, timers
- `serde` `1` (with `derive`) — serialization for all config and wire types
- `serde_json` `1` — JSON parsing/generation throughout
- `agent-client-protocol-schema` `0.11` — ACP wire type definitions (features: `unstable_session_resume`, `unstable_session_fork`)
- `thiserror` `2` — typed error enums in all library crates
- `anyhow` `1` — entry points only (`main.rs`, `supervisor.rs`, `init.rs`, `status.rs`)
- `tracing` `0.1` — structured logging/spans throughout
- `tracing-subscriber` `0.3` (features: `env-filter`, `json`) — log output formatting
- `metrics` `0.24` — runtime metrics collection
- `metrics-exporter-prometheus` `0.18` — Prometheus metrics endpoint
- `tokio-util` `0.7` (features: `codec`) — `LinesCodec` for NDJSON framing
- `tokio-stream` `0.1` (features: `sync`) — stream adapters for channel broadcast
- `futures` `0.3` — future combinators
- `bytes` `1` — byte buffer primitives for codec
- `uuid` `1` (features: `v4`) — session and message IDs
- `clap` `4` (features: `derive`, `env`) — CLI argument parsing
- `reqwest` `0.12` (features: `json`, `rustls-tls`, `stream`) — HTTP client (CLI status command, integration tests)
- `bollard` `0.20` — Docker Engine API client (agent Docker workspace support)
- `figment` `0.10` (features: `toml`, `yaml`, `env`) — layered config loading
- `yaml_serde` `0.10` (aliased as `serde_yaml`) — YAML parsing
- `subst` `0.3` — `${VAR}` environment variable substitution in YAML
- `rusqlite` `0.31` (features: `bundled`) — SQLite session persistence (in `anyclaw-core`)
- `wasmtime` `43` — WASM runtime engine
- `wasmtime-wasi` `43` — WASI host implementation for sandboxed tools
- `schemars` `1` — JSON Schema generation for tool input schemas (in `anyclaw-sdk-tool`)
- `regex` `1` — pattern matching (in `ext/channels/telegram`)
## Configuration
- Layered: embedded `defaults.yaml` → user `anyclaw.yaml` (with `${VAR}` substitution) → env vars (`ANYCLAW_` prefix, `__` separator)
- Config file: `crates/anyclaw-config/src/defaults.yaml`
- Config types: `crates/anyclaw-config/src/types.rs`
- Config loading: `crates/anyclaw-config/src/lib.rs`
- Workspace root: `Cargo.toml` (all deps centralized via `[workspace.dependencies]`)
- Linker config: `.cargo/config.toml` (mold via clang for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`)
- Release profile: strip symbols, thin LTO
## Platform Requirements
- Rust 1.94+ (edition 2024)
- No `rust-toolchain.toml` — relies on MSRV in `Cargo.toml`
- Docker deployment (Alpine musl targets with mold linker)
- Dockerfiles: `Dockerfile` (root), `examples/01-fake-agent-telegram-bot/Dockerfile.mock-agent`, `examples/02-real-agent-telegram/opencode/Dockerfile`, `examples/02-real-agent-telegram/kiro/Dockerfile`, `tests/integration/Dockerfile.mock-agent`
- SQLite bundled (no external DB dependency)
## Notable Patterns
- All shared deps declared in `[workspace.dependencies]` in root `Cargo.toml`
- Crates reference via `{ workspace = true }` with per-crate feature overrides
- `anyclaw-sdk-types` `0.4.0`, `anyclaw-sdk-channel` `0.2.7`, `anyclaw-sdk-tool` `0.2.5`, `anyclaw-sdk-agent` `0.2.5`
- Licensed MIT OR Apache-2.0, with readme, keywords, categories
- Internal crates have `publish = false`
- `agent-client-protocol-schema`: `unstable_session_resume`, `unstable_session_fork`
- `rmcp`: split across consumer crates — `anyclaw-tools` uses `client`, `server`, `transport-io`, `transport-child-process`, `transport-streamable-http-server`; `anyclaw-sdk-tool` uses `server`, `transport-io`
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

## Error Handling
#[derive(Debug, Error)]
- Production code: use `.expect("reason")` for true invariants, `?` for fallible paths
- Prefer `unwrap_or_else(|| { tracing::warn!(...); Default::default() })` over bare `unwrap_or_default()` to make silent fallbacks visible in logs
- Bare `.unwrap()` is allowed only in tests
## Module Organization
## Naming Conventions
- `when_action_then_result` — most common
- `given_precondition_when_action_then_result` — when setup matters
- Fixture functions: `fn given_*()` (rstest fixtures)
- Parameterized cases: `#[case::label_name]`
#[test]
#[test]
## Async Patterns
## Trait Patterns
- `AgentAdapter` (`crates/anyclaw-sdk-agent/src/adapter.rs`) — per-method hooks with default passthrough impls
- `Channel` (`crates/anyclaw-sdk-channel/src/trait_def.rs`) — messaging integration lifecycle
- `Tool` (`crates/anyclaw-sdk-tool/src/trait_def.rs`) — MCP tool execution
## Logging & Observability
#[tracing::instrument(skip(slot), fields(agent = %slot.name()))]
- `tracing::error!` — crash loops, fatal failures, respawn failures
- `tracing::warn!` — recoverable errors, fallback paths, deserialization failures, missing routing entries
- `tracing::info!` — lifecycle events (session started, agent recovered, manager running/stopped), permission flow
- `tracing::debug!` — message routing, session updates, per-event tracing
## Configuration Patterns
## Code Quality Indicators
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

## Pattern Overview
- Supervisor orchestrates three independent managers (tools → agents → channels)
- All external processes (agents, channels, MCP tools) run as isolated subprocesses communicating via JSON-RPC 2.0 over stdio
- Inter-manager communication uses `tokio::sync::mpsc` channels via typed `ManagerHandle<C>` — no shared mutable state
- Crash isolation per-manager and per-channel with exponential backoff and crash loop detection
## Layers
- Purpose: CLI parsing, tracing init, supervisor bootstrap
- Location: `crates/anyclaw/src/`
- Contains: `main.rs` (entry), `cli.rs` (Clap derive), `supervisor.rs` (re-export), `init.rs`, `status.rs`, `banner.rs`
- Depends on: `anyclaw-supervisor`, `anyclaw-config`, `anyclaw-core`
- Purpose: Boot/shutdown orchestration, health monitoring, crash recovery
- Location: `crates/anyclaw-supervisor/src/lib.rs`
- Contains: `Supervisor` struct, `ManagerSlot`, `ManagerKind` enum, `create_manager()` factory, `admin_server` module
- Depends on: `anyclaw-agents`, `anyclaw-channels`, `anyclaw-tools`, `anyclaw-core`, `anyclaw-config`
- Key constant: `MANAGER_ORDER: [&str; 3] = ["tools", "agents", "channels"]` — boot order is load-bearing, DO NOT change
- Purpose: Domain-specific lifecycle management for tools, agents, and channels
- Location: `crates/anyclaw-tools/`, `crates/anyclaw-agents/`, `crates/anyclaw-channels/`
- All implement the `Manager` trait from `crates/anyclaw-core/src/manager.rs`
- Each manager owns its subprocess connections and internal state
- Purpose: Shared primitives — Manager trait, backoff, crash tracking, cross-manager command types, session persistence
- Location: `crates/anyclaw-core/src/`
- Contains: `Manager` trait, `ManagerHandle<C>`, `ExponentialBackoff`, `CrashTracker`, `SessionStore` trait, ID newtypes
- Used by: All internal crates
- Purpose: Traits and harnesses for external implementors building agents, channels, or tools
- Location: `crates/anyclaw-sdk-types/`, `crates/anyclaw-sdk-agent/`, `crates/anyclaw-sdk-channel/`, `crates/anyclaw-sdk-tool/`
- `anyclaw-sdk-types` is a dependency-free leaf crate with shared wire types
- Each SDK crate defines a trait + harness/server that handles JSON-RPC framing
- Purpose: Figment-based config loading with defaults → YAML → env var layering
- Location: `crates/anyclaw-config/src/`
- Contains: `AnyclawConfig`, `AgentConfig`, `ChannelConfig`, `McpServerConfig`, binary path resolution
## Manager Implementations
- Spawns external MCP server subprocesses via `ExternalMcpServer` (`crates/anyclaw-tools/src/external.rs`)
- Loads WASM-sandboxed tools via `WasmToolRunner` (`crates/anyclaw-tools/src/wasm_runner.rs`) with per-invocation fuel budgets and WASI isolation
- Hosts an `AggregatedToolServer` implementing rmcp's `ServerHandler` — merges native, WASM, and external tools into a single MCP endpoint over HTTP (StreamableHttpService, stateful mode)
- Advertises tool URLs to agents via `ToolsCommand::GetMcpUrls`
- Receives commands via `ManagerHandle<ToolsCommand>`
- Manages agent subprocess lifecycle via `AgentConnection` (`crates/anyclaw-agents/src/connection.rs`)
- Implements ACP (Agent Client Protocol) over JSON-RPC 2.0 stdio: `initialize`, `session/new`, `session/prompt`, `session/cancel`, `session/load`, `session/resume`
- Supports two spawn backends: `LocalBackend` (`local_backend.rs`) and `DockerBackend` (`docker_backend.rs`)
- Multi-session model: `session_map: HashMap<SessionKey, String>` maps channel identity → ACP session ID
- Crash recovery: respawn → re-initialize → attempt `session/resume` (preferred) or `session/load` (fallback) → fresh session if both fail
- Session persistence via `SessionStore` trait (`crates/anyclaw-core/src/session_store.rs`) with SQLite implementation (`sqlite_session_store.rs`)
- Bridge-collapsed architecture: `spawn_with_bridge()` pushes incoming messages directly to manager's shared channel — no intermediate forwarding task
- Receives commands via `ManagerHandle<AgentsCommand>`, sends events to channels via `mpsc::Sender<ChannelEvent>`
- Manages channel subprocesses via `ChannelConnection` (`crates/anyclaw-channels/src/connection.rs`)
- Per-channel crash isolation: each channel gets its own `ChannelSlot` with independent backoff and crash tracker
- Session-keyed routing: `routing_table: HashMap<SessionKey, RoutingEntry>` maps session key → (channel_id, acp_session_id, slot_index)
- Per-session FIFO message queue (`SessionQueue` in `crates/anyclaw-channels/src/session_queue.rs`) with two-phase collect+flush for message merging
- Port discovery: channel subprocesses emit `PORT:{n}` to stderr, forwarded via `watch::Receiver<u16>`
- Receives commands via `ManagerHandle<ChannelsCommand>`, receives events from agents via `mpsc::Receiver<ChannelEvent>`
## Data Flow
- Session state: `SessionStore` trait with `SqliteSessionStore` (rusqlite, bundled) or `NoopSessionStore`
- Per-session message queue: `SessionQueue` in `ChannelsManager` (in-memory FIFO)
- Routing table: `HashMap<SessionKey, RoutingEntry>` in `ChannelsManager` (in-memory)
## Error Handling
- Library crates define domain-specific error enums: `ManagerError` (`crates/anyclaw-core/src/error.rs`), `AgentsError` (`crates/anyclaw-agents/src/error.rs`), `ChannelsError` (`crates/anyclaw-channels/src/error.rs`), `ToolsError` (`crates/anyclaw-tools/src/error.rs`), `AcpError` (`crates/anyclaw-agents/src/acp_error.rs`)
- `SupervisorError` in `crates/anyclaw-supervisor/src/lib.rs` wraps `ManagerError` with manager name context
- `anyhow` permitted only in: `crates/anyclaw/src/main.rs`, `init.rs`, `status.rs`
- Failed external MCP servers and invalid WASM modules are logged and skipped — not fatal to startup
- Bad channel binaries log errors and continue with `connection: None` — don't block other channels
- Use `.expect("reason")` for true invariants, `?` for fallible paths, bare `.unwrap()` only in tests
## Cross-Cutting Concerns
## Key Design Decisions
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
