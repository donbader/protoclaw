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
- **Config layering**: Defaults → YAML file (with `!env` tag resolution). `@built-in/{agents,channels,tools}/<name>` binary prefix resolved against `extensions_dir`. No env var override layer — YAML is the single source of truth.
- **Tracing**: Use `tracing` spans/events, not `println!` or `log` crate. Exception: CLI entry points may use `println!`/`eprintln!` before tracing is initialized.
- **Test-driven development**: Write a failing test before implementation. Red → green → refactor. No code lands without a test that exercises it.
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
- **Entity config field placement**: Top-level fields on `AgentConfig`/`ChannelConfig`/`ToolConfig` are manager concerns (spawn, routing, restarts). Everything passed to the extension binary lives in `options: HashMap<String, Value>`. The manager extracts structured data from `options` when constructing init params (e.g., `ack` config for channels).
- **Extension defaults via init**: Extensions report their default option values in the `initialize` response (`defaults` field). The manager merges these into the entity's options (user options win). No sidecar files — extensions are self-describing.
- **No cross-manager crate imports**: Use trait abstractions (e.g., `AgentDispatch`) instead.
- **`ChannelEvent` lives in `anyclaw-sdk-types`**: `anyclaw-core` re-exports for backward compat.
- **ACP wire types live in `anyclaw-sdk-types`**: `anyclaw-agents/acp_types.rs` re-exports for backward compat.

## Design Documentation

For deeper context on design decisions, architecture rationale, and failure modes:
- `docs/design-principles.md` — Core invariants, why three managers, failure mode catalog

Load when making architectural changes, debugging crash recovery, or questioning why a pattern exists.

## Contribution Rules

All AI agents working on this codebase must follow the project's contribution and conduct standards:

- **Code of Conduct**: Follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Be respectful and inclusive in all generated code comments, documentation, commit messages, and PR descriptions. See `CODE_OF_CONDUCT.md`.
- **Commit messages**: Use [Conventional Commits](https://www.conventionalcommits.org/) — `<type>: <short description>`. Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`. Imperative mood, lowercase after type, no trailing period, max 72 chars. Body explains *why*, not *what*.
- **PR descriptions**: Include `## Motivation`, `## Solution`, and `## Testing` sections.
- **Before submitting**: Ensure `cargo test`, `cargo clippy --workspace`, and `cargo fmt --all -- --check` all pass.
- **Integration tests**: Build required binaries first — `cargo build --bin mock-agent --bin debug-http --bin sdk-test-tool --bin sdk-test-channel`, then `cargo test -p anyclaw-integration-tests`.
- **Test conventions**: rstest with BDD naming (`when_action_then_result`), no `test_` prefix, fixtures named `given_*`, parameterised cases use `#[case::label_name]`.
- **License**: All contributions are licensed under MIT OR Apache-2.0.

Full details in `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`.

## Commands

```bash
cargo build                                    # Build all workspace members
cargo test                                     # Unit tests (all crates)
cargo build --bin mock-agent --bin debug-http   # Required BEFORE integration tests
cargo test -p integration                      # E2E tests (needs binaries built first)
cargo clippy --workspace                       # Lint all crates
```
## AGENTS.md Hierarchy

This project uses hierarchical AGENTS.md files. Subdirectory files contain domain-specific detail — don't repeat root content.

| File | Scope |
|------|-------|
| `./AGENTS.md` | Root — project overview, structure, conventions, anti-patterns |
| `./crates/AGENTS.md` | Crate overview + SDK grouping |
| `./crates/anyclaw/AGENTS.md` | Binary, supervisor, CLI |
| `./crates/anyclaw-core/AGENTS.md` | Manager trait, backoff, ChannelEvent |
| `./crates/anyclaw-agents/AGENTS.md` | ACP protocol, agent lifecycle |
| `./crates/anyclaw-channels/AGENTS.md` | Channel routing, crash isolation |
| `./crates/anyclaw-tools/AGENTS.md` | MCP host, WASM sandbox |
| `./crates/anyclaw-config/AGENTS.md` | Config loading, types |
| `./crates/anyclaw-jsonrpc/AGENTS.md` | JSON-RPC codec |
| `./crates/anyclaw-sdk-types/AGENTS.md` | Shared wire types |
| `./crates/anyclaw-sdk-agent/AGENTS.md` | Agent SDK |
| `./crates/anyclaw-sdk-channel/AGENTS.md` | Channel SDK |
| `./crates/anyclaw-sdk-tool/AGENTS.md` | Tool SDK |
| `./ext/agents/AGENTS.md` | Guide: building agent extensions (ACP protocol, wire format) |
| `./ext/channels/AGENTS.md` | Guide: building channel extensions (Channel trait, harness) |
| `./ext/tools/AGENTS.md` | Guide: building tool extensions (Tool trait, MCP, WASM) |
| `./examples/02-real-agent-telegram/AGENTS.md` | Real agent example variants |

When code changes affect module structure, public APIs, conventions, or anti-patterns, update the relevant AGENTS.md file(s) in the same commit.

## Workflow Standards

All changes go through pull requests — no direct commits to `main`.

### Branch Naming
- `feat/`, `fix/`, `docs/`, `chore/`, `refactor/`, `ci/` prefixes required
- Example: `feat/wasm-tool-permissions`

### PR Titles
- Must follow [Conventional Commits](https://www.conventionalcommits.org/): `<type>: <description>`
- CI enforces this via `amannn/action-semantic-pull-request`
- The PR title becomes the merge commit message, which feeds into changelogs

### Issue References
- Reference related issues in PR body: `Closes #123` or `Relates to #456`
- For AI-generated PRs, include `[ai-assisted]` in the PR body (not the title)

### Release Process
- SDK crate releases are automated via release-plz on push to `main`
- Binary releases are triggered by pushing a `v*.*.*` tag
- See `docs/releasing.md` for the full process
