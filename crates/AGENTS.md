# crates/ — Workspace Crates

12 crates: 1 binary (`protoclaw`), 6 internal libraries, 4 SDK crates, 1 test utility.

## Crate Map

| Crate | Type | Files | Purpose |
|-------|------|-------|---------|
| `protoclaw` | binary | 7 | CLI entry point, Supervisor, init/status commands |
| `protoclaw-core` | lib | 7 | Manager trait, backoff, crash tracker, ChannelEvent, message types |
| `protoclaw-agents` | lib | 6 | ACP protocol, AgentsManager, agent subprocess lifecycle |
| `protoclaw-channels` | lib | 6 | ChannelsManager, channel subprocess routing, debug-http integration |
| `protoclaw-tools` | lib | 7 | ToolsManager, McpHost, WasmToolRunner, WasmTool |
| `protoclaw-config` | lib | 4 | Figment config loading, types, validation |
| `protoclaw-jsonrpc` | lib | 4 | JSON-RPC 2.0 codec (LinesCodec), types, error |
| `protoclaw-sdk-types` | lib | 3 | Shared wire types: capabilities, permissions, messages |
| `protoclaw-sdk-agent` | lib | 4 | AgentAdapter trait + GenericAcpAdapter |
| `protoclaw-sdk-channel` | lib | 4 | Channel trait + ChannelHarness |
| `protoclaw-sdk-tool` | lib | 4 | Tool trait + ToolServer |
| `protoclaw-test-helpers` | lib | 1 | Shared test utilities (mock configs, port waiter, timeout helpers) |

## Internal vs SDK Crates

Internal crates (`protoclaw-core`, `protoclaw-agents`, `protoclaw-channels`, `protoclaw-tools`, `protoclaw-config`, `protoclaw-jsonrpc`) are used only by the `protoclaw` binary. They can have breaking changes freely.

SDK crates (`protoclaw-sdk-*`) are the public API for external implementors building channels, tools, or agent adapters. Changes here affect downstream consumers.

## Config Crate (`protoclaw-config`)

Config structs in `types.rs`: `ProtoclawConfig` (with `log_level`, `extensions_dir`), `AgentConfig`, `ChannelConfig` (with `enabled`), `McpServerConfig` (with `enabled`), `WasmToolConfig`, `WasmSandboxConfig`, `SupervisorConfig`.

Files:
- `types.rs`: Config structs with serde defaults
- `lib.rs`: Figment loading + `ProtoclawConfig::load()`
- `resolve.rs`: Binary path resolution (`@built-in/` prefix → `extensions_dir`)
- `validate.rs`: Config validation rules
- `error.rs`: `ConfigError`

Loading in `lib.rs`: `Figment::from(defaults).merge(SubstYaml::file(path)).merge(Env::prefixed("PROTOCLAW_").split("__"))`.

Env var override format: `PROTOCLAW_AGENT__BINARY=claude-code` (double underscore = nesting).

## JSON-RPC Crate (`protoclaw-jsonrpc`)

Hand-rolled JSON-RPC 2.0 — no framework. Three files:
- `types.rs`: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcMessage`
- `codec.rs`: `JsonRpcCodec` wrapping `LinesCodec` for line-delimited JSON over stdio
- `error.rs`: `JsonRpcCodecError`

## SDK Crates

All SDK crates follow the same pattern:
- Define a trait (`Channel`, `Tool`, `AgentAdapter`)
- Provide a harness/server that handles JSON-RPC framing over stdio
- External implementors only need to implement the trait

`protoclaw-sdk-types` exists separately to avoid circular deps between the three SDK impl crates.
