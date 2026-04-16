# Project Structure

Workspace layout and navigation guide for the anyclaw codebase.

## Workspace Layout

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
│   ├── channels/
│   │   ├── telegram/               # Telegram channel implementation
│   │   └── debug-http/             # Debug HTTP channel (minimal)
│   └── tools/
│       └── system-info/            # Demo MCP tool binary (uses anyclaw-sdk-tool)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
├── docs/                           # Developer documentation
│   ├── architecture.md             # System overview, crate deps, manager communication
│   ├── design-principles.md        # Core invariants, anti-patterns, failure modes
│   ├── project-structure.md        # This file
│   ├── releasing.md                # Release process for SDK crates and binary
│   ├── container-images.md         # Docker image publishing and usage
│   ├── getting-started.md          # User guide: copy an example, customize, deploy
│   └── building-extensions.md      # Extension builder guide: SDK crates and patterns
├── examples/01-fake-agent-telegram-bot/  # Fake agent example (Docker, mock-agent, debug-http)
└── examples/02-real-agent-telegram/      # Real agent examples (Docker, debug-http + telegram)
    ├── opencode/                           # OpenCode agent variant
    ├── kiro/                               # Kiro CLI agent variant
    └── claude-code/                        # Claude Code agent variant
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
| Modify JSON-RPC framing | `crates/anyclaw-jsonrpc/src/codec.rs` | LinesCodec-based, line-delimited JSON |
| Build a channel (SDK) | `crates/anyclaw-sdk-channel/` | Channel trait + ChannelHarness |
| Build a tool (SDK) | `crates/anyclaw-sdk-tool/` | Tool trait + ToolServer |
| Mock agent binary | `ext/agents/mock-agent/` | Mock ACP agent for testing |
| ACP↔HTTP bridge | `ext/agents/acp-bridge/` | Translates ACP stdio to HTTP REST+SSE |
| Add test helper | `crates/anyclaw-test-helpers/` | Shared across all crate tests |
| Integration tests | `tests/integration/tests/e2e.rs` | Requires `cargo build` first (needs mock-agent binary) |

## Internal vs SDK Crates

The workspace has two categories of crates with different audiences.

**Internal crates** (in `crates/anyclaw*` without `sdk-` prefix) are the supervisor implementation. External implementors should not depend on these directly.

**SDK crates** (the four `anyclaw-sdk-*` crates) are public API for building integrations:

| Crate | Audience | Key types |
|-------|----------|-----------|
| `anyclaw-sdk-types` | All SDK users | `ChannelEvent`, `SessionKey`, wire types |
| `anyclaw-sdk-agent` | Agent implementors | `AgentAdapter`, `GenericAcpAdapter` |
| `anyclaw-sdk-channel` | Channel implementors | `Channel` trait, `ChannelHarness` |
| `anyclaw-sdk-tool` | Tool implementors | `Tool` trait, `ToolServer` |

`anyclaw-sdk-types` is the dependency-free leaf crate. If you're building a channel and a tool that need to share types, depend only on `anyclaw-sdk-types`.

## Test Conventions

All tests use `rstest` with BDD-style naming.

**Naming pattern:**
- `when_action_then_result` for unit tests
- `given_precondition_when_action_then_result` for tests with complex setup

**Fixtures** are free functions named `given_*`:

```rust
#[fixture]
fn given_empty_buffer() -> BytesMut { BytesMut::new() }
```

**Parameterised tests** use `#[case::label_name]` — named cases only:

```rust
#[rstest]
#[case::empty_string("")]
#[case::whitespace("   ")]
fn when_decoding_non_json_input_then_returns_none(#[case] input: &str) { ... }
```

**Async tests:**
- Unit: `#[rstest] #[tokio::test]` (two attributes)
- Integration: `#[rstest] #[test_log::test(tokio::test)]` (three attributes; captures tracing on failure)

**No `test_` prefix.** No `it_` prefix. No bare `.unwrap()` in production code.

## Build and Test Commands

```bash
cargo build                                    # Build all workspace members
cargo test                                     # Unit tests (all crates)
cargo build --bin mock-agent --bin debug-http  # Required before integration tests
cargo test -p integration                      # E2E tests (needs binaries above)
cargo clippy --workspace                       # Lint all crates
```
