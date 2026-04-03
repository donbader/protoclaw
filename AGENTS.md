# PROTOCLAW — Project Knowledge Base

Infrastructure sidecar connecting AI agents to channels (Telegram, Slack) and tools (MCP servers, WASM sandboxed). Rust workspace, ACP protocol (JSON-RPC 2.0 over stdio), three-manager architecture with Supervisor.

## Structure

```
protoclaw-rust/
├── crates/                         # Core workspace crates (11 total)
│   ├── protoclaw/                  # Binary: CLI + Supervisor (entry point)
│   ├── protoclaw-core/             # Shared: Manager trait, backoff, crash tracker, message types
│   ├── protoclaw-agents/           # ACP protocol layer, agent subprocess management
│   ├── protoclaw-channels/         # Channel subprocess routing + lifecycle
│   ├── protoclaw-tools/            # MCP host, WASM sandbox, tools manager
│   ├── protoclaw-config/           # Figment-based config loading (protoclaw.toml)
│   ├── protoclaw-jsonrpc/          # JSON-RPC 2.0 codec + types (LinesCodec-based)
│   ├── protoclaw-sdk-types/        # Shared SDK types (capabilities, permissions)
│   ├── protoclaw-sdk-agent/        # SDK: AgentAdapter trait + GenericAcpAdapter
│   ├── protoclaw-sdk-channel/      # SDK: Channel trait + ChannelHarness
│   └── protoclaw-sdk-tool/         # SDK: Tool trait + ToolServer
├── ext/                            # External binaries (spawned as subprocesses)
│   ├── agents/
│   │   └── mock-agent/             # Mock ACP agent binary (echo + thinking simulation)
│   └── channels/
│       ├── telegram/               # Telegram channel implementation
│       └── debug-http/             # Debug HTTP channel (minimal)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
└── examples/telegram-bot/          # Example config + docker-compose (no Rust source)
```

## Where to Look

| Task | Location | Notes |
|------|----------|-------|
| Add CLI command | `crates/protoclaw/src/cli.rs` | Clap derive, dispatched from `main.rs` |
| Change boot/shutdown order | `crates/protoclaw/src/supervisor.rs` | `MANAGER_ORDER` constant — read anti-patterns first |
| Add new manager | `crates/protoclaw-core/src/manager.rs` | Implement `Manager` trait, wire in supervisor |
| Modify ACP protocol | `crates/protoclaw-agents/src/acp_types.rs` | JSON-RPC method types for agent communication |
| Add channel type | `crates/protoclaw-channels/` + `ext/channels/` | Manager routes, binary in ext/ |
| Add MCP tool | `crates/protoclaw-tools/src/mcp_host.rs` | McpHost manages external MCP server connections |
| Add WASM tool | `crates/protoclaw-tools/src/wasm_runner.rs` | WasmToolRunner + WasmTool for sandboxed execution |
| Change config schema | `crates/protoclaw-config/src/types.rs` | Serde structs, update tests in `lib.rs` |
| Modify JSON-RPC framing | `crates/protoclaw-jsonrpc/src/codec.rs` | LinesCodec-based, line-delimited JSON |
| Build channel SDK | `crates/protoclaw-sdk-channel/` | Channel trait + ChannelHarness |
| Build tool SDK | `crates/protoclaw-sdk-tool/` | Tool trait + ToolServer |
| Mock agent binary | `ext/agents/mock-agent/` | Mock ACP agent for testing (echo + thinking simulation) |
| Integration tests | `tests/integration/tests/e2e.rs` | Requires `cargo build` first (needs mock-agent binary) |

## Crate Dependency Flow

```
protoclaw (binary)
├── protoclaw-config
├── protoclaw-core
├── protoclaw-agents ──→ protoclaw-core, protoclaw-jsonrpc
├── protoclaw-channels ─→ protoclaw-core, protoclaw-jsonrpc, protoclaw-sdk-types
└── protoclaw-tools ───→ protoclaw-core

SDK crates (for external implementors):
├── protoclaw-sdk-types (shared types, no deps on internal crates)
├── protoclaw-sdk-agent ──→ sdk-types, jsonrpc
├── protoclaw-sdk-channel ─→ sdk-types, jsonrpc
└── protoclaw-sdk-tool ───→ sdk-types
```

## Conventions

- **Error handling boundary**: `thiserror` for library crates, `anyhow` only at application entry points (`main.rs`, `supervisor::run()`, `init.rs`, `status.rs`)
- **No `unsafe`**: Zero unsafe blocks exist. Do not introduce any.
- **unwrap() rule**: `.expect("reason")` for true invariants (piped stdio, consumed-once fields). Bare `.unwrap()` only in tests. Use `?` for fallible paths.
- **Module structure**: Flat `lib.rs` with `pub mod` + `pub use` re-exports. No `mod.rs` files.
- **Manager communication**: `tokio::sync::mpsc` channels via `ManagerHandle<C>`. No shared mutable state between managers.
- **Config layering**: Defaults → TOML file → env vars (`PROTOCLAW_` prefix, `__` separator)
- **Tracing**: Use `tracing` spans/events, not `println!` or `log` crate
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
- **Do not move `ChannelEvent` out of `protoclaw-core`**: It lives there to break the circular dependency between agents and channels crates.
- **Do not remove the 50ms sleep in `poll_channels()`**: It prevents busy-looping in the channel polling select.

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
