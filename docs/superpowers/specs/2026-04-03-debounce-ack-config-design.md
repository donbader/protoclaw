# Design: Message Debouncing, Ack Reactions & Config Defaults

**Date**: 2026-04-03
**Phase**: 12 (new phase in v2.1)
**Scope**: protoclaw-config, protoclaw-channels, telegram channel

## Overview

Three interrelated improvements:
1. **Message debouncing** at the channels manager level — batch rapid messages before forwarding to agent
2. **Ack reactions** in telegram — visual feedback (👀) when agent starts processing, removed on completion
3. **Config defaults file** — centralize all defaults in `defaults.toml`, remove direct env var overrides

## Feature 1: Message Debouncing

### Location

Channels manager (`crates/protoclaw-channels/src/manager.rs`). Not in the channel subprocess — all channels benefit automatically.

### Behavior

- Per-session debounce buffer in `ChannelsManager`
- Each channel config specifies `debounce_window_ms` (default: 1000)
- On incoming `channel/sendMessage`:
  1. If no active debounce for this session → start timer, buffer message
  2. If active debounce → reset timer to `now + window_ms`, append message to buffer with `\n` separator
  3. On timer expiry → flush: concatenate buffered messages, send single `AgentsCommand::PromptSession`
- While agent is processing (session has an outstanding prompt):
  - New messages queue in the next debounce batch
  - When agent completes (`result` event delivered), the next batch flushes if non-empty
- `debounce_window_ms = 0` disables debouncing (immediate forward, current behavior)

### Data Structures

```rust
// In ChannelsManager
struct DebounceEntry {
    messages: Vec<String>,       // buffered message texts
    deadline: tokio::time::Instant,
    session_key: SessionKey,
    channel_index: usize,
    agent_busy: bool,            // true while awaiting agent result
}
debounce_buffers: HashMap<SessionKey, DebounceEntry>
```

### Config

```toml
[[channels]]
name = "telegram"
binary = "@built-in/telegram-channel"
debounce_window_ms = 1000  # default
```

### Edge Cases

- Channel subprocess sends message metadata (sender info, message_id) alongside text. The debounce buffer stores the full `ChannelSendMessage` payloads, concatenating only the text content. Metadata from the last message in the batch is used for the flushed prompt.
- Session doesn't exist yet → create session first (existing flow), then start debounce timer
- Channel disconnects while debounce pending → drop the buffer, log warning

## Feature 2: Ack Reactions (Telegram)

### Location

Telegram channel subprocess (`ext/channels/telegram/`). This is channel-specific UX, not a protoclaw-level concern.

### Lifecycle

1. **On inbound message** (`dispatcher.rs`): store `chat_id → message_id` in `ack_pending: HashMap<i64, i32>` (overwrite — only last message in burst gets reaction)
2. **On first agent chunk arriving** (`deliver.rs`, first `agent_message_chunk` or `agent_thought_chunk` for a chat): call `setMessageReaction(chat_id, message_id, ack_emoji)` on the stored entry. Mark as "acked" so subsequent chunks don't re-fire.
3. **On `result` event** (`deliver.rs`): pop the entry from `ack_pending`. If `ack_done_emoji` is non-empty, call `setMessageReaction` with done emoji. If empty, call `setMessageReaction` with empty array (removes reaction).

### Telegram Bot API

```
POST /bot{token}/setMessageReaction
{
  "chat_id": 123456,
  "message_id": 789,
  "reaction": [{"type": "emoji", "emoji": "👀"}]  // or [] to remove
}
```

Fire-and-forget — errors logged as `tracing::warn`, never propagated.

### Config

Passed as opaque channel config in `protoclaw.toml`:

```toml
[[channels]]
name = "telegram"
binary = "@built-in/telegram-channel"

[channels.config]
ack_emoji = "👀"           # default
ack_done_emoji = ""        # default: "" = remove reaction on completion
```

### State Addition

```rust
// In SharedState (state.rs)
pub ack_pending: RwLock<HashMap<i64, i32>>,      // chat_id → message_id
pub ack_fired: RwLock<HashSet<i64>>,              // chat_ids where reaction already sent
```

### Edge Cases

- `ack_emoji = ""` → feature disabled entirely, no reactions set or removed
- Bot doesn't have reaction permission in group → `setMessageReaction` fails, logged and ignored
- Multiple rapid messages → only last message_id stored, only it gets the reaction

## Feature 3: Config Defaults File

### Current State

- Figment layering: `Serialized::defaults(SupervisorConfig::default())` → `SubstToml::file(path)` → `Env::prefixed("PROTOCLAW_").split("__")`
- Defaults scattered across `Default` impls in `types.rs`
- Direct env var overrides via `PROTOCLAW_AGENT__BINARY` pattern

### Target State

- `crates/protoclaw-config/defaults.toml` embedded via `include_str!`
- Layering: `defaults.toml` (embedded) → user `protoclaw.toml` (with `${VAR}` interpolation) → merged result
- Remove `Env::prefixed("PROTOCLAW_")` Figment layer — env vars only via `${VAR:default}` in TOML files
- Remove `Default` impls that duplicate the defaults file (keep only for Rust struct construction)

### `defaults.toml`

```toml
log_level = "info"
extensions_dir = "/usr/local/bin"

[agent]
binary = ""

[supervisor]
shutdown_timeout_secs = 30
health_check_interval_secs = 5
max_restarts = 5
restart_window_secs = 60
```

### Merge Semantics

Simple: defaults parsed to `toml::Value`, user config parsed to `toml::Value` (after `${VAR}` substitution), deep merge (user wins on conflict), then deserialize to `ProtoclawConfig`.

No null-to-remove pattern (TOML doesn't have null). To disable a channel, use `enabled = false`.

### Channel-Specific Config

Channels get an opaque `config: Option<toml::Value>` field in `ChannelConfig`. This is passed to the channel subprocess in the `initialize` call. The channel parses it into its own typed config struct.

```toml
[[channels]]
name = "telegram"
binary = "@built-in/telegram-channel"
debounce_window_ms = 1000

[channels.config]
ack_emoji = "👀"
ack_done_emoji = ""
```

`debounce_window_ms` is read by channels manager. `config.*` is forwarded opaquely to the channel subprocess.

### Migration

- Existing `protoclaw.toml` files continue to work — they just override defaults
- `${VAR:default}` syntax unchanged (already implemented via `subst` crate)
- `PROTOCLAW_*` env var overrides stop working (breaking change — document in changelog)

## Files Changed

| File | Change |
|------|--------|
| `crates/protoclaw-config/defaults.toml` | NEW — embedded defaults |
| `crates/protoclaw-config/src/lib.rs` | Replace Figment layering with defaults merge |
| `crates/protoclaw-config/src/subst_toml.rs` | Adapt for new merge flow |
| `crates/protoclaw-config/src/types.rs` | Add `debounce_window_ms` to `ChannelConfig`, add `config: Option<toml::Value>`, remove redundant `Default` impls |
| `crates/protoclaw-channels/src/manager.rs` | Add debounce buffer, timer logic, flush-on-expiry |
| `crates/protoclaw-core/src/channel_event.rs` | May need `AgentBusy`/`AgentDone` signals for debounce queue |
| `ext/channels/telegram/src/state.rs` | Add `ack_pending`, `ack_fired`, channel config struct |
| `ext/channels/telegram/src/deliver.rs` | Fire/remove reactions on chunk/result |
| `ext/channels/telegram/src/dispatcher.rs` | Store message_id in ack_pending on inbound |
| `ext/channels/telegram/src/channel.rs` | Parse opaque config into TelegramConfig |
| `ext/channels/telegram/src/http.rs` | NEW — `setMessageReaction` / `removeMessageReaction` helpers |
| `examples/01-fake-agent-telegram-bot/protoclaw.toml` | Add debounce + ack config |

## Non-Goals

- JSONC format switch — staying TOML
- Config discovery (`--config` flag, `$PROTOCLAW_CONFIG`) — future phase
- Debounce per-sender (Zig does `chat_id + sender_id`) — start with per-session, iterate later
- Group chat policies — future phase
