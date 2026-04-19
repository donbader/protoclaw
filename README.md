# anyclaw

[![CI](https://github.com/donbader/anyclaw/actions/workflows/ci.yml/badge.svg)](https://github.com/donbader/anyclaw/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/donbader/anyclaw/graph/badge.svg)](https://codecov.io/gh/donbader/anyclaw)
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

We're working toward a stable v1.0. Here's where things stand:

### Core

| Feature | Status | Notes |
|---------|--------|-------|
| Three-manager supervisor (tools → agents → channels) | ✅ | |
| Per-subprocess crash recovery with exponential backoff | ✅ | |
| Crash loop detection and escalation | ✅ | |
| Graceful shutdown with per-manager timeouts | ✅ | |
| Health check loop + admin HTTP server | ✅ | Includes Prometheus `/metrics` endpoint |
| YAML config with `!env` tag resolution and validation | ✅ | |
| JSON Schema for `anyclaw.yaml` (IDE autocomplete) | ✅ | `anyclaw schema` CLI command |
| Config validation CLI (`anyclaw validate`) | ✅ | Offline schema + semantic validation with `--strict` mode |
| Structured JSON logging | ✅ | `log_format: json` for production log aggregators |
| Extension defaults via initialize handshake | ✅ | |
| Agent-initiated messages | ✅ | Agents can push to channels without user input (custom extension) |
| Rich media delivery | ✅ | Images, files, audio between agents and channels (both directions) |
| Reply/thread context | ✅ | Agent knows which message the user is replying to |
| Rate limiting | planned | Per-session and per-channel depth caps with backpressure |
| Supervisor management API | planned | Authenticated HTTP API for session introspection, agent control, and runtime status |
| `anyclaw doctor` | planned | Config validation, binary probes, channel connectivity checks |

### Agents

| Feature | Status | Notes |
|---------|--------|-------|
| ACP protocol (JSON-RPC 2.0 over stdio) | ✅ | Uses official `agent-client-protocol-schema` for wire types |
| ACP↔HTTP bridge (connect any REST/SSE agent) | ✅ | |
| Docker workspace (run agents in containers) | ✅ | |
| Session persistence (SQLite-backed) | ✅ | |
| Session recovery after crash | ✅ | Resume preferred; falls back to history replay |
| Session fork and list | ✅ | `session/fork` and `session/list` ACP methods (capability-gated) |
| Filesystem sandboxing | ✅ | |
| Permission system (agent → user approval flow) | ✅ | |
| Platform commands (`/new`, `/cancel`) | ✅ | Built-in slash commands intercepted by the sidecar |
| Dynamic command menus | ✅ | Agents push `available_commands_update` to channels at runtime |
| Full ACP spec compliance | planned | Replace custom extensions (`session/push`, etc.) with upstream standard methods |
| Agent-to-agent communication | planned | Handoff, delegation, or direct IPC between agents |

### Channels

| Feature | Status | Notes |
|---------|--------|-------|
| Telegram | ✅ | |
| Debug HTTP (development + testing) | ✅ | |
| Telegram: reply/thread context | ✅ | Sender attribution, partial quotes, media placeholders, openclaw-compatible format |
| Telegram: external/cross-chat reply context | planned | Handle `external_reply` for replies to messages from other chats |
| Telegram: reply media download | ✅ | Photos from replies downloaded; other media types show placeholder |
| Telegram: reply context access control | ✅ | Suppress reply context in groups when original sender is not in allowlist |
| Telegram: group/user allowlists | ✅ | Control who can interact with the agent via `access_control` options |

### Tools

| Feature | Status | Notes |
|---------|--------|-------|
| MCP server hosting (external tool binaries) | ✅ | |
| WASM sandboxed tools | ✅ | Implemented, not yet battle-tested |

### SDK

| Feature | Status | Notes |
|---------|--------|-------|
| Channel, Tool, Types, Agent SDK crates on crates.io | ✅ | |
| Automated releases via release-plz | ✅ | |
| Stable API with semver guarantees | planned | |

### CI/CD & Release

| Feature | Status | Notes |
|---------|--------|-------|
| Multi-arch Docker images (amd64 + arm64) | ✅ | |
| PR-only workflow with conventional commit enforcement | ✅ | |
| Security audit + Trivy scanning | ✅ | |
| Separate ext/ image (`ghcr.io/donbader/anyclaw-ext`) | ✅ | Extensions built independently from core |
| Independent extension versioning | planned | Per-extension semver after SDK types reach 1.0 |

### Extension Ideas

Anyclaw is infrastructure — many features are best built as extensions rather than core. Here's what we'd love to see contributed:

| Extension | Type | Status | Notes |
|-----------|------|--------|-------|
| Slack | channel | planned | Same pattern as Telegram — use the [Channel SDK](https://docs.rs/anyclaw-sdk-channel) |
| Discord | channel | planned | |
| Task scheduler | tool | planned | Cron/interval/one-shot task CRUD via MCP (execution trigger depends on agent-initiated messages) |

Some features live entirely in the agent, not in anyclaw — skills, prompt extensions, vector memory, and knowledge graphs are configured in your agent (e.g., `CLAUDE.md`, `AGENTS.md`, MCP servers). Anyclaw doesn't need to know about them.

Have an idea? [Open a feature request](https://github.com/donbader/anyclaw/issues/new?template=feature_request.yml).

## Building from Source

```bash
cargo build                                                              # Build core workspace
cargo build --workspace --manifest-path ext/Cargo.toml                   # Build extension binaries
cargo test                                                               # Unit tests (core)
cargo test --workspace --manifest-path ext/Cargo.toml                    # Unit tests (extensions)
cargo clippy --workspace                                                 # Lint core
cargo clippy --workspace --manifest-path ext/Cargo.toml                  # Lint extensions

# Integration tests require ext/ binaries built first:
cargo build --workspace --manifest-path ext/Cargo.toml
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
- [Changelog](CHANGELOG.md) — release history

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
