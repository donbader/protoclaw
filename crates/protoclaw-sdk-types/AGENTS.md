# protoclaw-sdk-types — Shared SDK Wire Types

Shared serde types used by all three SDK crates (agent, channel, tool) and by internal crates (agents, channels, core). Exists as a separate leaf crate to avoid circular dependencies.

## Files

| File | Purpose |
|------|---------|
| `channel.rs` | Channel protocol types: capabilities, initialize, deliver, send, ack, thought, session |
| `channel_event.rs` | `ChannelEvent` enum — agents→channels bridge type (relocated from protoclaw-core in v5.0) |
| `session_key.rs` | `SessionKey` newtype — routing key encoding channel + conversation identity (relocated from protoclaw-core in v5.0) |
| `permission.rs` | Permission types: request, response, options |
| `lib.rs` | Re-exports from all modules |

## Key Types

**Cross-manager types (relocated from protoclaw-core in v5.0):**
- `ChannelEvent` — agents→channels message enum: `DeliverMessage`, `SessionComplete`, `RoutePermission`, `AckMessage`
- `SessionKey` — routing key newtype: `"{channel_name}:{kind}:{peer_id}"`, with `new()`, `channel_name()`, `Display`, `FromStr`

**Channel protocol:**
- `ChannelCapabilities { streaming, rich_text }` — advertised during initialize
- `ChannelInitializeParams { agent_name, ack_config, options }` / `ChannelInitializeResult` — handshake types. `options: HashMap<String, Value>` forwards channel-specific config from `protoclaw.yaml`
- `DeliverMessage { session_id, content }` — protoclaw → channel
- `ChannelSendMessage { peer_info, content }` — channel → protoclaw
- `PeerInfo { channel_name, peer_id, kind }` — inbound message identity
- `ThoughtContent` — helper to extract `agent_thought_chunk` from `DeliverMessage.content`
- `ContentKind` — typed dispatch enum over `DeliverMessage.content`: `Thought`, `MessageChunk`, `Result`, `UserMessageChunk`, `UsageUpdate`, `ToolCall`, `ToolCallUpdate`, `AvailableCommandsUpdate`, `Unknown`
- `AckNotification` / `AckLifecycleNotification` — ack reaction lifecycle
- `ChannelAckConfig` — ack settings passed via initialize
- `SessionCreated` — session-to-peer mapping notification

**Permission protocol:**
- `PermissionOption { option_id, label }` — single choice in a permission prompt
- `PermissionRequest { request_id, description, options }` — agent asks permission
- `PermissionResponse { request_id, option_id }` — user's choice
- `ChannelRequestPermission` — protoclaw → channel permission forwarding

## Why Separate

All three SDK crates (`sdk-agent`, `sdk-channel`, `sdk-tool`) need shared wire types. Putting them in any one SDK crate would force the others to depend on it, creating coupling. `sdk-types` is a dependency-free leaf — it depends only on `serde` and `serde_json`.

## Serde Convention

All types use `#[serde(rename_all = "camelCase")]` for JSON wire format. Rust fields use `snake_case`, JSON uses `camelCase`. Tests verify round-trip serialization for every type.

## Anti-Patterns (this crate)

- **Don't add dependencies on internal crates** — this is external-facing; it must stay a leaf
- **Don't break serde compatibility** — downstream channels/tools depend on the wire format
- **Don't add protocol logic** — this crate is pure data types; behavior belongs in SDK impl crates
