# anyclaw

[![CI](https://github.com/donbader/anyclaw/actions/workflows/ci.yml/badge.svg)](https://github.com/donbader/anyclaw/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-types.svg)](https://crates.io/crates/anyclaw-sdk-types)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-types)](https://docs.rs/anyclaw-sdk-types)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue)](https://github.com/donbader/anyclaw/blob/main/Cargo.toml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE-MIT)

Infrastructure sidecar that connects AI agents to messaging channels and tools. You bring the agent; anyclaw handles the plumbing.

> ⚠️ **Unstable** — anyclaw is under active development. APIs, config format, and protocol details may change between releases.

## What is anyclaw?

Anyclaw is not an AI assistant — it's the infrastructure layer that any agent can plug into. It manages the lifecycle of agent subprocesses, routes messages between channels (Telegram, HTTP) and agents via the ACP protocol (JSON-RPC 2.0 over stdio), and provides tool access through MCP servers and WASM sandboxes.

It runs as a sidecar alongside your agent binary. You bring the AI logic; anyclaw handles crash recovery, message routing, config loading, and subprocess supervision.

## Architecture

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

Managers communicate exclusively through typed `mpsc` channels via `ManagerHandle<C>`. No shared mutable state crosses manager boundaries. Each subprocess has its own crash recovery loop with exponential backoff.

Boot order is `tools → agents → channels` because agents need tool URLs during initialization, and channels need agents ready to accept messages. Shutdown is reverse order.

## Quickstart

The fastest way to see anyclaw in action is the fake-agent example, which bundles a mock ACP agent, the debug-http channel, and a demo MCP tool:

```bash
git clone https://github.com/donbader/anyclaw.git
cd anyclaw/examples/01-fake-agent-telegram-bot
cp .env.example .env
# Edit .env with your Telegram bot token
docker compose up
```

See the [example README](examples/01-fake-agent-telegram-bot/README.md) for details.

## SDK Crates

Build your own agents, channels, and tools using the SDK crates:

| Crate                 | Description                          | docs.rs                                     |
| --------------------- | ------------------------------------ | ------------------------------------------- |
| `anyclaw-sdk-types`   | Shared types for the anyclaw SDK     | [docs](https://docs.rs/anyclaw-sdk-types)   |
| `anyclaw-sdk-agent`   | Build ACP-compatible agents          | [docs](https://docs.rs/anyclaw-sdk-agent)   |
| `anyclaw-sdk-channel` | Build messaging channel integrations | [docs](https://docs.rs/anyclaw-sdk-channel) |
| `anyclaw-sdk-tool`    | Build MCP-compatible tool servers    | [docs](https://docs.rs/anyclaw-sdk-tool)    |

The `ChannelHarness` and `ToolServer` in the channel and tool SDKs handle all JSON-RPC framing, the initialize handshake, and message routing. You only implement the `Channel` or `Tool` trait with your business logic.

## Building from Source

```bash
cargo build                                      # Build all workspace members
cargo test                                       # Unit tests (all crates)
cargo clippy --workspace                         # Lint all crates

# Integration tests require the mock binaries first:
cargo build --bin mock-agent --bin debug-http
cargo test -p integration
```

Rust stable toolchain required. Check `rust-toolchain.toml` for the pinned version.

## Documentation

- [Architecture overview](docs/architecture.md) — system design, crate deps, manager communication
- [Design principles](docs/design-principles.md) — core invariants, anti-patterns, failure modes
- [Project structure](docs/project-structure.md) — workspace layout, where to find things

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Inspiration

Anyclaw draws inspiration from these projects:

- [nanoclaw](https://github.com/qwibitai/nanoclaw) — a lightweight TypeScript personal AI assistant that bridges messaging channels to Claude agents in isolated containers. Established the core pattern of agents connected to channels with isolation as a first-class concern.
- [openclaw](https://github.com/openclaw/openclaw) — a feature-rich TypeScript personal AI assistant gateway with 20+ channel integrations and an ACP bridge. Demonstrated that a gateway/sidecar pattern cleanly separates channel routing from agent logic.
- [ironclaw](https://github.com/nearai/ironclaw) — a Rust personal AI assistant with WASM-sandboxed tools, MCP support, and PostgreSQL-backed memory. Proved out WASM tool sandboxing and channel abstraction in Rust.

Where these projects are complete AI assistants, anyclaw takes their architectural ideas — channel abstraction, tool sandboxing, protocol-driven communication — and applies them as a standalone infrastructure layer that any agent can plug into.

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
