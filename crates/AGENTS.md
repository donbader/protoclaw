# crates/ — Workspace Crates

12 crates: 1 binary (`anyclaw`), 6 internal libraries, 4 SDK crates, 1 test utility.

## Crate Map

| Crate | Type | Files | Purpose |
|-------|------|-------|---------|
| `anyclaw` | binary | 7 | CLI entry point, Supervisor, init/status commands |
| `anyclaw-core` | lib | 7 | Manager trait, backoff, crash tracker, ChannelEvent, message types |
| `anyclaw-agents` | lib | 6 | ACP protocol, AgentsManager, agent subprocess lifecycle |
| `anyclaw-channels` | lib | 6 | ChannelsManager, channel subprocess routing, debug-http integration |
| `anyclaw-tools` | lib | 7 | ToolsManager, McpHost, WasmToolRunner, WasmTool |
| `anyclaw-config` | lib | 4 | Figment config loading, types, validation |
| `anyclaw-jsonrpc` | lib | 4 | JSON-RPC 2.0 codec (LinesCodec), types, error |
| `anyclaw-sdk-types` | lib | 3 | Shared wire types: capabilities, permissions, messages |
| `anyclaw-sdk-agent` | lib | 4 | AgentAdapter trait + GenericAcpAdapter |
| `anyclaw-sdk-channel` | lib | 7 | Channel trait + ChannelHarness + PermissionBroker + ChannelTester |
| `anyclaw-sdk-tool` | lib | 4 | Tool trait + ToolServer |
| `anyclaw-test-helpers` | lib | 1 | Shared test utilities (mock configs, port waiter, timeout helpers) |

## Internal vs SDK Crates

Internal crates (`anyclaw-core`, `anyclaw-agents`, `anyclaw-channels`, `anyclaw-tools`, `anyclaw-config`, `anyclaw-jsonrpc`) are used only by the `anyclaw` binary. They can have breaking changes freely.

SDK crates (`anyclaw-sdk-*`) are the public API for external implementors building channels, tools, or agent adapters. Changes here affect downstream consumers.

## Config Crate (`anyclaw-config`)

Config structs in `types.rs`: `AnyclawConfig` (with `log_level`, `extensions_dir`), `AgentConfig`, `ChannelConfig` (with `enabled`), `McpServerConfig` (with `enabled`), `WasmToolConfig`, `WasmSandboxConfig`, `SupervisorConfig`.

Files:
- `types.rs`: Config structs with serde defaults
- `lib.rs`: Figment loading + `AnyclawConfig::load()`
- `resolve.rs`: Binary path resolution (`@built-in/{agents,channels,tools}/<name>` → `extensions_dir`, with legacy alias support)
- `validate.rs`: Config validation rules
- `error.rs`: `ConfigError`

Loading in `lib.rs`: `Figment::from(defaults).merge(EnvYaml::file(path)).merge(Env::prefixed("ANYCLAW_").split("__"))`.

Env var override format: `ANYCLAW_AGENT__BINARY=claude-code` (double underscore = nesting).

## JSON-RPC Crate (`anyclaw-jsonrpc`)

Hand-rolled JSON-RPC 2.0 — no framework. Three files:
- `types.rs`: `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `JsonRpcMessage`
- `codec.rs`: `JsonRpcCodec` wrapping `LinesCodec` for line-delimited JSON over stdio
- `error.rs`: `JsonRpcCodecError`

## SDK Crates

All SDK crates follow the same pattern:
- Define a trait (`Channel`, `Tool`, `AgentAdapter`)
- Provide a harness/server that handles JSON-RPC framing over stdio
- External implementors only need to implement the trait

`anyclaw-sdk-types` exists separately to avoid circular deps between the three SDK impl crates.
