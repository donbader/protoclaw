# Project Structure

Workspace layout and navigation guide for the protoclaw codebase.

## Workspace Layout

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
│   ├── protoclaw-sdk-types/        # Shared SDK types (capabilities, permissions)
│   ├── protoclaw-sdk-agent/        # SDK: AgentAdapter trait + GenericAcpAdapter
│   ├── protoclaw-sdk-channel/      # SDK: Channel trait + ChannelHarness
│   ├── protoclaw-sdk-tool/         # SDK: Tool trait + ToolServer
│   └── protoclaw-test-helpers/     # Shared test utilities (dev-dependency)
├── ext/                            # External binaries (spawned as subprocesses)
│   ├── agents/
│   │   └── mock-agent/             # Mock ACP agent binary (echo + thinking simulation)
│   └── channels/
│       ├── telegram/               # Telegram channel implementation
│       └── debug-http/             # Debug HTTP channel (minimal)
├── tests/
│   └── integration/                # E2E tests (spawn real supervisor + mock-agent)
├── docs/                           # Developer documentation
│   ├── architecture.md             # System overview, crate deps, manager communication
│   ├── design-principles.md        # Core invariants, anti-patterns, failure modes
│   └── project-structure.md        # This file
├── examples/telegram-bot/          # Example config + docker-compose (no Rust source)
└── examples/01-fake-agent-telegram-bot/  # Runnable example (Docker, mock-agent, debug-http)
    └── tools/system-info/          # Demo MCP tool binary (uses protoclaw-sdk-tool)
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
| Build demo tool | `examples/01-fake-agent-telegram-bot/tools/system-info/` | Workspace member, uses protoclaw-sdk-tool |
| Change config schema | `crates/protoclaw-config/src/types.rs` | Serde structs (`WorkspaceConfig` enum, `AgentConfig`) |
| Modify JSON-RPC framing | `crates/protoclaw-jsonrpc/src/codec.rs` | LinesCodec-based, line-delimited JSON |
| Build a channel (SDK) | `crates/protoclaw-sdk-channel/` | Channel trait + ChannelHarness |
| Build a tool (SDK) | `crates/protoclaw-sdk-tool/` | Tool trait + ToolServer |
| Mock agent binary | `ext/agents/mock-agent/` | Mock ACP agent for testing |
| Add test helper | `crates/protoclaw-test-helpers/` | Shared across all crate tests |
| Integration tests | `tests/integration/tests/e2e.rs` | Requires `cargo build` first |

## Internal vs SDK Crates

The workspace has two categories of crates with different audiences.

**Internal crates** (in `crates/protoclaw*` without `sdk-` prefix) are the supervisor implementation. External implementors should not depend on these directly.

**SDK crates** (the four `protoclaw-sdk-*` crates) are public API for building integrations:

| Crate | Audience | Key types |
|-------|----------|-----------|
| `protoclaw-sdk-types` | All SDK users | `ChannelEvent`, `SessionKey`, wire types |
| `protoclaw-sdk-agent` | Agent implementors | `AgentAdapter`, `GenericAcpAdapter` |
| `protoclaw-sdk-channel` | Channel implementors | `Channel` trait, `ChannelHarness` |
| `protoclaw-sdk-tool` | Tool implementors | `Tool` trait, `ToolServer` |

`protoclaw-sdk-types` is the dependency-free leaf crate. If you're building a channel and a tool that need to share types, depend only on `protoclaw-sdk-types`.

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
