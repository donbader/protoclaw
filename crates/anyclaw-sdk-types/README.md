# anyclaw-sdk-types

[![crates.io](https://img.shields.io/crates/v/anyclaw-sdk-types.svg)](https://crates.io/crates/anyclaw-sdk-types)
[![docs.rs](https://img.shields.io/docsrs/anyclaw-sdk-types)](https://docs.rs/anyclaw-sdk-types)

Shared wire types for the anyclaw SDK ecosystem.

Part of [anyclaw](https://github.com/donbader/anyclaw) — an infrastructure sidecar connecting AI agents to channels and tools.

> ⚠️ **Unstable** — types and wire formats may change between releases.

## Key Types

### Routing

| Type | Description |
|------|-------------|
| `SessionKey` | Routing key encoding channel + conversation identity (`"{channel}:{kind}:{peer_id}"`). |
| `ChannelEvent` | Agent-to-channel bridge enum: `DeliverMessage`, `SessionComplete`, `RoutePermission`, `AckMessage`, `DispatchStarted`. |

### Channel Protocol

| Type | Description |
|------|-------------|
| `ChannelCapabilities` | Capabilities advertised during the initialize handshake (`streaming`, `rich_text`). |
| `ChannelInitializeParams` | Handshake params sent to a channel subprocess, including `agent_name`, `ack_config`, and forwarded `options`. |
| `DeliverMessage` | anyclaw → channel: deliver agent output to a peer session. |
| `ChannelSendMessage` | channel → anyclaw: inbound user message with `PeerInfo`. |
| `ContentKind` | Typed dispatch enum over `DeliverMessage.content`: `Thought`, `MessageChunk`, `Result`, `ToolCall`, and more. |
| `AckNotification` | Ack reaction lifecycle notification. |

### ACP Protocol (supervisor ↔ agent)

| Type | Description |
|------|-------------|
| `InitializeParams` / `InitializeResult` | ACP handshake types. |
| `SessionNewParams` / `SessionPromptParams` | Session lifecycle request types. |
| `SessionUpdateEvent` / `SessionUpdateType` | Streaming update events from the agent. |
| `StopReason` | Why the agent stopped: `EndTurn`, `MaxTokens`, `Refusal`, `Cancelled`, etc. |
| `PromptResponse` | Parsed from `session/prompt` RPC response — canonical ACP completion signal. |

### Permissions

| Type | Description |
|------|-------------|
| `PermissionRequest` | Agent asks the user for a permission choice. |
| `PermissionResponse` | User's selected option, returned to the agent. |
| `PermissionOption` | A single labeled choice in a permission prompt. |

## Leaf Crate

`anyclaw-sdk-types` depends only on `serde` and `serde_json`. It's the dependency-free base of the SDK. If you're building both a channel and a tool that need to share types, depend on this crate alone — no need to pull in `anyclaw-sdk-channel` or `anyclaw-sdk-tool`.

All JSON field names use `camelCase` (Rust fields are `snake_case`).

## Documentation

- [API reference on docs.rs](https://docs.rs/anyclaw-sdk-types)
- [Building extensions guide](https://github.com/donbader/anyclaw/blob/main/docs/building-extensions.md)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
