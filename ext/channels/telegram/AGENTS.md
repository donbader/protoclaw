# ext/channels/telegram/ — Telegram Channel Extension

Telegram bot channel using [teloxide](https://docs.rs/teloxide). Handles DMs, groups, supergroups with streaming response editing, thought bubbles, tool call rendering, and permission prompts via inline keyboards.

## Module Map

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point, module registration |
| `channel.rs` | `Channel` trait impl — init, ready, deliver, permissions |
| `dispatcher.rs` | Inbound message routing — text, media, callbacks, access checks, mention detection |
| `deliver.rs` | Outbound rendering — streaming edits, thought bubbles, result finalization |
| `access_control.rs` | Pure access control logic — allowlists, group policies, mention gating, reply context suppression |
| `state.rs` | `SharedState` — shared across dispatcher and delivery via `Arc<SharedState>` |
| `turn.rs` | Per-chat turn state machine — thought → response → finalize lifecycle |
| `formatting.rs` | Markdown → Telegram HTML conversion |
| `peer.rs` | `PeerInfo` construction from chat metadata |
| `permissions.rs` | Inline keyboard permission prompts + callback resolution |

## Access Control (`access_control.rs`)

Pure synchronous logic, no async, no Telegram types — fully unit-testable.

### Config shape (in `options.access_control`)

```yaml
access_control:
  group_policy: "open"              # open | allowlist | disabled
  allowed_users: ["*"]              # DM allowlist: "*", "@username", or numeric ID
  group_allowed_users: ["*"]        # group sender allowlist (when policy=allowlist)
  require_mention: false            # bot must be @mentioned in groups
  groups:                           # per-group overrides keyed by chat_id
    "-100123":
      enabled: true
      group_policy: "allowlist"
      allowed_users: ["@alice"]
      require_mention: true
```

### Key semantics

- `["*"]` = allow everyone (default). `[]` = block all. Explicit intent required.
- `AllowlistEntry`: `Wildcard` / `UserId(i64)` / `Username(String)` — supports mixed lists
- Username matching is case-insensitive, `@` prefix optional in config
- Omitting `allowed_users` from config defaults to `["*"]` (not empty)
- Per-group config overrides global; empty per-group `allowed_users` falls back to global
- Evaluation order: group enabled → group policy → allowlist → mention check
- Denied users cannot bypass allowlist by mentioning the bot

### Mention detection (in `dispatcher.rs`)

Three layers:
1. Entity-based: `msg.entities` with `type=mention`, UTF-16 → byte offset conversion via `slice_utf16`
2. String matching: case-insensitive `@botusername` fallback
3. Implicit: reply-to-bot-message counts as mention

### Reply context suppression

When `group_policy=allowlist`, reply metadata is stripped if the original sender is not in the active allowlist. Prevents leaking non-allowlisted user content to the agent.

## Message Flow

```
Telegram Update
  → run_dispatcher (teloxide Dispatcher)
    → handle_text_message / handle_callback_query
      → check_access (allowlist + mention gate)
        → Deny/SkipNoMention → silent drop
        → Allow → process_text_message / process_media_message
          → maybe_suppress_reply_context
          → outbound.send(ChannelSendMessage)
```

## State

`SharedState` fields relevant to access control:
- `access_config: RwLock<AccessConfig>` — parsed from options on init
- `bot_username: RwLock<Option<String>>` — from `getMe`, for mention matching
- `bot_id: RwLock<Option<u64>>` — from `getMe`, for reply-to-bot detection
