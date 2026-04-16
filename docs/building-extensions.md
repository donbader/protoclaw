# Building Extensions

Anyclaw is designed to be extended. You bring the logic; anyclaw handles subprocess lifecycle, crash recovery, JSON-RPC framing, and message routing.

There are three extension points:

- **Agents** — AI backends that receive user messages and stream responses via the ACP protocol (JSON-RPC 2.0 over stdio)
- **Channels** — platform connectors (Telegram, Slack, HTTP, etc.) that route messages between users and agents
- **Tools** — capability servers that agents call via MCP (Model Context Protocol)

All three are standalone binaries spawned as child processes with piped stdio. The subprocess boundary is intentional — it provides crash isolation and language independence.

## Quick Reference

| Extension | SDK crate | Detailed guide | Reference implementation |
|-----------|-----------|----------------|--------------------------|
| Agent | none (speak wire protocol directly) | [ext/agents/AGENTS.md](../ext/agents/AGENTS.md) | [ext/agents/mock-agent/](../ext/agents/mock-agent/) |
| Channel | `anyclaw-sdk-channel` | [ext/channels/AGENTS.md](../ext/channels/AGENTS.md) | [ext/channels/debug-http/](../ext/channels/debug-http/) |
| Tool | `anyclaw-sdk-tool` | [ext/tools/AGENTS.md](../ext/tools/AGENTS.md) | [ext/tools/system-info/](../ext/tools/system-info/) |

## The Pattern

Channels and tools follow the same structure: implement a trait, hand it to the SDK harness, call `.run_stdio()` or `.serve_stdio()`. The harness owns stdout for JSON-RPC/MCP framing — you only implement business logic.

```
Channel:  implement Channel trait  →  ChannelHarness::new(channel).run_stdio().await
Tool:     implement Tool trait     →  ToolServer::new(vec![Box::new(tool)]).serve_stdio().await
```

The harness handles:
- The `initialize` handshake (including merging your `defaults()` with user config)
- All JSON-RPC or MCP framing over stdio
- Message dispatch to your trait methods

Agents are different. There's no harness — agents speak the ACP wire protocol directly. The `anyclaw-sdk-agent` crate provides an `AgentAdapter` for intercepting messages *inside* the supervisor, not for building agent binaries. See [ext/agents/AGENTS.md](../ext/agents/AGENTS.md) for the full wire format.

## Detailed Guides

Each guide covers the full implementation: trait contract, configuration in `anyclaw.yaml`, testing utilities, and anti-patterns.

- **[ext/agents/AGENTS.md](../ext/agents/AGENTS.md)** — ACP wire format, initialization handshake, session lifecycle, streaming updates, crash recovery, permission flow, filesystem access
- **[ext/channels/AGENTS.md](../ext/channels/AGENTS.md)** — `Channel` trait, `ChannelHarness`, `ContentKind` dispatch, `PermissionBroker`, `ChannelTester` for unit tests
- **[ext/tools/AGENTS.md](../ext/tools/AGENTS.md)** — `Tool` trait, `ToolServer`, WASM tools, input schema, error handling

## Testing Your Extension

**Channels** — use `ChannelTester` from `anyclaw-sdk-channel::testing` to test your `Channel` impl without any JSON-RPC framing. See the testing section in [ext/channels/AGENTS.md](../ext/channels/AGENTS.md).

**Tools** — call `Tool::execute()` directly in unit tests. No MCP framing needed. See the testing section in [ext/tools/AGENTS.md](../ext/tools/AGENTS.md).

**Agents** — pipe JSON-RPC messages to your binary's stdin to test the wire protocol manually. For full pipeline testing, configure your binary in a test `anyclaw.yaml` and use the integration test harness in [tests/integration/](../tests/integration/).

The reference implementations (`mock-agent`, `debug-http`, `system-info`) are also useful as integration test fixtures — the CI pipeline uses them.

## SDK Crates on docs.rs

| Crate | docs.rs |
|-------|---------|
| `anyclaw-sdk-types` | [docs.rs/anyclaw-sdk-types](https://docs.rs/anyclaw-sdk-types) |
| `anyclaw-sdk-channel` | [docs.rs/anyclaw-sdk-channel](https://docs.rs/anyclaw-sdk-channel) |
| `anyclaw-sdk-tool` | [docs.rs/anyclaw-sdk-tool](https://docs.rs/anyclaw-sdk-tool) |
| `anyclaw-sdk-agent` | [docs.rs/anyclaw-sdk-agent](https://docs.rs/anyclaw-sdk-agent) |

`anyclaw-sdk-types` contains the shared wire types (`ChannelEvent`, `SessionKey`, ACP types) used across all SDK crates.
