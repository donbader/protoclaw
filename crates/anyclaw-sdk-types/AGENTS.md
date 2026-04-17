# anyclaw-sdk-types — Shared SDK Wire Types

Shared serde types used by all three SDK crates (agent, channel, tool) and by internal crates (agents, channels, core). Exists as a separate leaf crate to avoid circular dependencies.

## Files

| File | Purpose |
|------|---------|
| `channel.rs` | Channel protocol types: capabilities, initialize, deliver, send, ack, thought, session |
| `channel_event.rs` | `ChannelEvent` enum — agents→channels bridge type (relocated from anyclaw-core in v5.0) |
| `session_key.rs` | `SessionKey` newtype — routing key encoding channel + conversation identity (relocated from anyclaw-core in v5.0) |
| `permission.rs` | Permission types: request, response, options |
| `lib.rs` | Re-exports from all modules |

## Key Types

**Cross-manager types (relocated from anyclaw-core in v5.0):**
- `ChannelEvent` — agents→channels message enum: `DeliverMessage`, `SessionComplete`, `RoutePermission`, `AckMessage`, `DispatchStarted`
- `SessionKey` — routing key newtype: `"{channel_name}:{kind}:{peer_id}"`, with `new()`, `channel_name()`, `Display`, `FromStr`

**Channel protocol:**
- `ChannelCapabilities { streaming, rich_text, media }` — advertised during initialize
- `ChannelInitializeParams { agent_name, ack_config, options }` / `ChannelInitializeResult` — handshake types. `options: HashMap<String, Value>` forwards channel-specific config from `anyclaw.yaml`
- `DeliverMessage { session_id, content }` — anyclaw → channel (also used for agent-initiated push via per-part delivery)
- `ChannelSendMessage { peer_info, content, metadata }` — channel → anyclaw. `content` is `Vec<ContentPart>`, `metadata` is `Option<MessageMetadata>` for reply/thread context
- `PeerInfo { channel_name, peer_id, kind }` — inbound message identity
- `MessageMetadata { reply_to_message_id, reply_to_text, thread_id }` — optional reply/thread context on inbound messages
- `ThoughtContent` — helper to extract `agent_thought_chunk` from `DeliverMessage.content`
- `ContentKind` — typed dispatch enum over `DeliverMessage.content`: `Thought`, `MessageChunk`, `Result`, `UserMessageChunk`, `UsageUpdate`, `ToolCall`, `ToolCallUpdate`, `AvailableCommandsUpdate`, `Image`, `File`, `Audio`, `Unknown`
- `AckNotification` / `AckLifecycleNotification` — ack reaction lifecycle
- `ChannelAckConfig` — ack settings passed via initialize
- `SessionCreated` — session-to-peer mapping notification

**ACP protocol (via `agent-client-protocol-schema` re-exports):**
- `ContentBlock` — official ACP content enum: `Text`, `Image`, `Audio`, `Resource`, `ResourceLink` (from `agent-client-protocol-schema`)
- `ContentPart` — internal tagged enum for channel routing: `Text { text }`, `Image { url }`, `File { url, filename, mime_type }`, `Audio { url, mime_type }`
- Conversion functions: `content_part_to_block`, `content_block_to_part`, `content_parts_to_blocks`, `content_blocks_to_parts` — convert between internal `ContentPart` (URL-based) and ACP wire `ContentBlock` (base64-based) at the agent boundary
- `StopReason` — re-exported from `agent-client-protocol-schema`: `EndTurn`, `MaxTokens`, `MaxTurnRequests`, `Refusal`, `Cancelled` (`#[non_exhaustive]`)
- `SessionPushParams { session_id, content }` / `SessionPushResult` — anyclaw extension for agent-initiated push via `_session/push` method
- `PromptResponse { stop_reason }` — parsed from `session/prompt` RPC response body; canonical completion signal per the ACP spec
- `SessionUpdateType::Result` — extension type not in the official ACP spec; anyclaw-specific early completion hint for UX purposes
- All ACP wire types include `_meta: Option<Value>` for protocol extensibility per ACP spec

**Permission protocol:**
- `PermissionOption { option_id, label }` — single choice in a permission prompt
- `PermissionRequest { request_id, description, options }` — agent asks permission
- `PermissionResponse { request_id, option_id }` — user's choice
- `ChannelRequestPermission` — anyclaw → channel permission forwarding

## Why Separate

All three SDK crates (`sdk-agent`, `sdk-channel`, `sdk-tool`) need shared wire types. Putting them in any one SDK crate would force the others to depend on it, creating coupling. `sdk-types` is a leaf crate — it depends only on `serde`, `serde_json`, and `agent-client-protocol-schema` (for official ACP wire types).

## Serde Convention

All types use `#[serde(rename_all = "camelCase")]` for JSON wire format. Rust fields use `snake_case`, JSON uses `camelCase`. Tests verify round-trip serialization for every type.

## Anti-Patterns (this crate)

- **Don't add dependencies on internal crates** — this is external-facing; it must stay a leaf
- **Don't break serde compatibility** — downstream channels/tools depend on the wire format
- **Don't add protocol logic** — this crate is pure data types; behavior belongs in SDK impl crates
