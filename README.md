# anyclaw

[![CI](https://github.com/donbader/anyclaw/actions/workflows/ci.yml/badge.svg)](https://github.com/donbader/anyclaw/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-types.svg)](https://crates.io/crates/anyclaw-sdk-types)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-types)](https://docs.rs/anyclaw-sdk-types)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue)](https://github.com/donbader/anyclaw/blob/main/Cargo.toml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)

Build any bot you want. Connect any AI agent — Claude, GPT, a custom LLM, anything — to Telegram, Slack, HTTP, and more. You write the AI logic in any language; anyclaw handles message routing, crash recovery, tool access, and subprocess supervision.

> ⚠️ **Unstable** — anyclaw is under active development. APIs, config format, and protocol details may change between releases.

## What is anyclaw?

Anyclaw is infrastructure, not an AI assistant. It's a sidecar process that sits between your agent and the outside world:

- **Channels** deliver messages to and from users (Telegram, HTTP, and more coming)
- **Agents** are your AI backends — any binary that speaks [ACP](ext/agents/AGENTS.md) (JSON-RPC 2.0 over stdio)
- **Tools** give agents capabilities via [MCP](https://modelcontextprotocol.io/) servers or WASM sandboxes

All three are standalone binaries spawned as child processes. Write them in Rust, Python, Go, TypeScript — whatever you prefer. Anyclaw manages their lifecycle, restarts them on crash, and routes messages between them.

## Quickstart

See anyclaw running in under a minute — no API keys needed:

```bash
git clone https://github.com/donbader/anyclaw.git
cd anyclaw/examples/01-fake-agent-telegram-bot
cp .env.example .env
docker compose up
```

In another terminal, send a message:

```bash
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

You'll see the mock agent "think" and respond with `Echo: hello`. That's the full pipeline — channel receives message, routes to agent, agent streams response back.

Want to connect Telegram? Add your bot token to `.env` and set `TELEGRAM_ENABLED=true`. See the [Getting Started guide](docs/getting-started.md) for deploying with a real agent.

## Built-in Extensions

Anyclaw ships with these extensions in [`ext/`](ext/), ready to use:

| Type | Name | Description |
|------|------|-------------|
| Agent | [mock-agent](ext/agents/mock-agent/) | Echo agent with simulated thinking (for testing) |
| Agent | [acp-bridge](ext/agents/acp-bridge/) | ACP↔HTTP bridge — connect REST/SSE agents to anyclaw |
| Channel | [telegram](ext/channels/telegram/) | Telegram bot integration |
| Channel | [debug-http](ext/channels/debug-http/) | HTTP + SSE endpoint for development and testing |
| Tool | [system-info](ext/tools/system-info/) | Demo MCP tool returning system information |

We're actively growing this collection. If you build a channel, tool, or agent adapter that others would find useful, consider [contributing it](CONTRIBUTING.md).

## Build Your Own

Extensions are standalone binaries communicating over stdio — no SDK dependency required. The Rust SDK crates handle protocol framing for you, but you can also speak the wire protocol directly from any language.

| Crate | What it does | docs.rs |
|-------|-------------|---------|
| `anyclaw-sdk-channel` | Build channel integrations (Telegram, Slack, etc.) | [docs](https://docs.rs/anyclaw-sdk-channel) |
| `anyclaw-sdk-tool` | Build MCP-compatible tool servers | [docs](https://docs.rs/anyclaw-sdk-tool) |
| `anyclaw-sdk-types` | Shared wire types used across all SDK crates | [docs](https://docs.rs/anyclaw-sdk-types) |
| `anyclaw-sdk-agent` | Supervisor-side hooks for intercepting agent messages | [docs](https://docs.rs/anyclaw-sdk-agent) |

For channels and tools, implement a trait and hand it to the SDK harness — it handles all JSON-RPC/MCP framing. Agents speak the [ACP wire protocol](ext/agents/AGENTS.md) directly and don't need an SDK crate.

See [Building Extensions](docs/building-extensions.md) for the full guide, including how to build extensions in non-Rust languages.

## Roadmap

<!-- TODO: Fill in after brainstorming session -->

We're working toward a stable v1.0. Here's what's on the horizon:

- 🔌 More built-in channels (Slack, Discord, and more)
- 🧰 More built-in tools and WASM sandbox improvements
- 🔄 Session persistence and conversation history
- 📡 Multi-agent routing (multiple agents per channel)
- 📦 Stable SDK with semver guarantees

Have an idea? [Open a feature request](https://github.com/donbader/anyclaw/issues/new?template=feature_request.yml).

## Building from Source

```bash
cargo build                                                              # Build all workspace members
cargo test                                                               # Unit tests (all crates)
cargo clippy --workspace                                                 # Lint all crates

# Integration tests require the mock binaries first:
cargo build --bin mock-agent --bin debug-http --bin sdk-test-tool --bin sdk-test-channel
cargo test -p anyclaw-integration-tests
```

Rust stable toolchain required. Check `rust-toolchain.toml` for the pinned version.

## Documentation

### For Users

Deploy anyclaw with your own AI agent:

- [Getting started](docs/getting-started.md) — copy an example, customize, deploy
- [Configuration reference](examples/02-real-agent-telegram/CONFIGURATION.md) — full `anyclaw.yaml` schema
- [Container images](docs/container-images.md) — Docker image tags, platforms, usage
- [Examples](examples/) — ready-to-run setups (fake agent, OpenCode, Kiro, Claude Code)
- [Changelog](CHANGELOG.md) — binary release history

### For Extension Builders

Build a custom channel (Slack, Discord, etc.), tool, or agent in any language:

- [Building extensions](docs/building-extensions.md) — start here: pattern overview, SDK vs wire protocol, testing
- [ext/agents/AGENTS.md](ext/agents/AGENTS.md) — ACP wire format (for building agent binaries in any language)
- [ext/channels/AGENTS.md](ext/channels/AGENTS.md) — Channel trait, harness, testing utilities
- [ext/tools/AGENTS.md](ext/tools/AGENTS.md) — Tool trait, MCP server, WASM tools
- [Architecture overview](docs/architecture.md) — system design, protocol details

### For Contributors

- [Contributing guide](CONTRIBUTING.md) — workflow, tests, PR process
- [Project structure](docs/project-structure.md) — workspace layout, where to find things
- [Design principles](docs/design-principles.md) — core invariants, anti-patterns
- [Releasing](docs/releasing.md) — how releases work
- [Support](SUPPORT.md) — how to get help

## Contributing

We welcome contributions — especially new channel integrations, tools, and agent variants. See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow, and check [`E-help-wanted`](https://github.com/donbader/anyclaw/labels/E-help-wanted) issues for a starting point.

<details>
<summary>Architecture overview</summary>

```
                        ┌─────────────────────────────┐
                        │         Supervisor          │
                        │  (boot: tools→agents→chans) │
                        └──────────────┬──────────────┘
                                       │
          ┌────────────────────────────┼────────────────────────────┐
          │                            │                            │
 ┌────────▼────────┐        ┌──────────▼───────────┐     ┌──────────▼──────────┐
 │  ToolsManager   │        │   AgentsManager      │     │  ChannelsManager    │
 │                 │        │                      │     │                     │
 │  MCP servers    │        │  ACP subprocess      │     │  Telegram           │
 │  WASM sandbox   │◄───────│  (JSON-RPC/stdio)    │◄────│  debug-http         │
 └─────────────────┘        └──────────────────────┘     └─────────────────────┘
        ▲                           ▲                            │
        │  tool URLs                │  route messages            │ user messages
        └───────────────────────────┘◄───────────────────────────┘
```

Three managers communicate exclusively through typed `mpsc` channels via `ManagerHandle<C>`. No shared mutable state crosses manager boundaries. Each subprocess has its own crash recovery loop with exponential backoff.

Boot order is `tools → agents → channels` because agents need tool URLs during initialization, and channels need agents ready to accept messages. Shutdown is reverse order.

</details>

## Inspiration

Anyclaw draws inspiration from these projects:

- [nanoclaw](https://github.com/qwibitai/nanoclaw) — lightweight TypeScript personal AI assistant bridging messaging channels to Claude agents in isolated containers
- [openclaw](https://github.com/openclaw/openclaw) — feature-rich TypeScript AI assistant gateway with 20+ channel integrations and an ACP bridge
- [ironclaw](https://github.com/nearai/ironclaw) — Rust personal AI assistant with WASM-sandboxed tools, MCP support, and PostgreSQL-backed memory

Where these projects are complete AI assistants, anyclaw takes their architectural ideas — channel abstraction, tool sandboxing, protocol-driven communication — and applies them as a standalone infrastructure layer that any agent can plug into.

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
