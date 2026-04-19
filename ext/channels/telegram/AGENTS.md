# ext/channels/telegram/ — Telegram Channel Extension

Telegram bot channel using [teloxide](https://docs.rs/teloxide). Handles DMs, groups, supergroups with streaming response editing, thought bubbles, tool call rendering, and permission prompts via inline keyboards.

## Module Map

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point, module registration |
| `channel.rs` | `Channel` trait impl — init, ready, deliver, permissions |
| `dispatcher.rs` | Inbound message routing — text, media, callbacks, mention detection |
| `deliver.rs` | Outbound rendering — streaming edits, thought bubbles, result finalization |
| `state.rs` | `SharedState` — shared across dispatcher and delivery via `Arc<SharedState>` |
| `turn.rs` | Per-chat turn state machine — thought → response → finalize lifecycle |
| `formatting.rs` | Markdown → Telegram HTML conversion |
| `peer.rs` | `PeerInfo` construction from chat metadata |
| `permissions.rs` | Inline keyboard permission prompts + callback resolution |

## Access Control

Access control evaluation has been removed from this extension. The `ChannelsManager` (in `crates/anyclaw-channels/`) is responsible for allowlist and policy enforcement. The Telegram extension forwards **all** inbound messages to the manager unconditionally.

The `defaults.yaml` still includes an `access_control` section — the manager reads these defaults from the extension's `initialize` response and merges them with user config.

## Mention Detection (`dispatcher.rs`)

Three layers, used to populate `was_mentioned` on every outbound message:
1. Entity-based: `msg.entities` with `type=mention`, UTF-16 → byte offset conversion via `slice_utf16`
2. String matching: case-insensitive `@botusername` fallback
3. Implicit: reply-to-bot-message counts as mention

## Message Flow

```
Telegram Update
  → run_dispatcher (teloxide Dispatcher)
    → handle_text_message / handle_callback_query
      → process_text_message / process_media_message
        → reply_metadata_from_message
        → outbound.send(ChannelSendMessage)
```

All messages are forwarded. Access policy enforcement happens in the manager.

## State

`SharedState` fields relevant to mention detection:
- `bot_username: RwLock<Option<String>>` — from `getMe`, for mention matching
- `bot_id: RwLock<Option<u64>>` — from `getMe`, for reply-to-bot detection
