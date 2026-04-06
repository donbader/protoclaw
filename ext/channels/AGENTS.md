# ext/channels/ — External Channel Binaries

Channel implementations that run as subprocesses spawned by `ChannelsManager`. They communicate with protoclaw over JSON-RPC 2.0 stdio using the channel SDK (`protoclaw-sdk-channel`).

## Binaries

| Binary | Files | Purpose |
|--------|-------|---------|
| `telegram` | 7 | Full Telegram bot channel via `teloxide` |
| `debug-http` | 1 | Minimal HTTP endpoint for development/testing |

## Why ext/ and not crates/

These are standalone binaries, not libraries. They depend on SDK crates but are architecturally separate — they're spawned as child processes with piped stdio. Putting them in `ext/` makes the boundary explicit.

## Shared Pattern

Both channels follow the same structure:
1. Implement `Channel` trait from `protoclaw-sdk-channel`
2. Call `ChannelHarness::run_stdio(channel).await` in `main()`
3. The harness handles all JSON-RPC framing, initialization handshake, and message routing

## Thought Rendering

Both channels inspect `content["type"]` in `DeliverMessage` to render thoughts differently:

- `"agent_thought_chunk"` — thought content from the agent's reasoning process
- All other types — existing behavior (message chunks, results, etc.)

**debug-http:** Emits thoughts as named SSE event `"thought"` via `SsePayload` struct. Regular messages use default SSE data events. SSE clients filter by event type.

**telegram:** Streams thoughts as 🧠-prefixed messages with debounced edits (400ms internal timer). On `"result"`, collapses to "🧠 Thought for Xs" (timing only, no content). Emoji prefix configurable via `TELEGRAM_THOUGHT_EMOJI` env var (default: 🧠). Thinking state tracked in `SharedState.thinking_messages` and `SharedState.thought_debounce_handles`.

## Late-chunk Race Condition Fix (telegram)

The agents manager sends `result` events via a separate channel from `agent_message_chunk` events, so chunks can arrive after the result. This is handled with a timer-based finalization pattern using `SharedState.result_received: RwLock<HashSet<i64>>` and `SharedState.finalize_handles: RwLock<HashMap<i64, JoinHandle<()>>>`:

- **`result` handler**: Collapses thinking message. Sets `result_received` and `thought_suppressed`. Does NOT immediately clean up `message_buffers` or `active_messages`. Spawns a 200ms debounced finalization task.
- **`agent_message_chunk` handler (late chunk)**: If `result_received` is set, this is a late chunk. Accumulate into buffer, force an immediate edit (bypass rate-limit), cancel the old finalization timer, and spawn a new 200ms timer. This ensures all late chunks are captured before final state cleanup.
- **Finalization timer**: After 200ms with no more late chunks, reads the complete buffer, does the final edit with overflow splitting, then cleans up all state (`message_buffers`, `active_messages`, `last_edit_time`, `thought_suppressed`, `result_received`, `finalize_handles`).
- **`agent_thought_chunk` handler (new turn)**: If `result_received` is set, cancel the finalization timer immediately and do synchronous cleanup of all previous-turn state before processing the new thought.

## messageId-based Turn Detection (telegram)

Handles the race where a new prompt's streaming events arrive at the telegram channel before the previous prompt's `result` event. State is keyed by `chat_id`, but events from different prompts carry different `messageId` values.

- **`current_message_id: RwLock<HashMap<i64, String>>`** — tracks the `messageId` of the current turn per chat.
- **`check_message_id_turn_change()`** — called at the top of both `agent_thought_chunk` and `agent_message_chunk` handlers. Compares incoming `messageId` against `current_message_id`. If different, finalizes the previous turn (cancels timers, clears all per-chat state) before processing the new event.
- **`finalize_previous_turn()`** — shared cleanup function used by both messageId turn detection and the finalization timer. Clears all per-chat state including `current_message_id`.
- First event for a chat (no `current_message_id` entry) is not treated as a turn change.
- `messageId` is extracted from `content["update"]["messageId"]` in the ACP `session/update` payload.

Anti-patterns:
- Do NOT skip the messageId check — without it, new prompt events corrupt the previous turn's state when the result event is delayed.
- Do NOT remove `current_message_id` cleanup from finalization timers — stale entries would prevent turn detection on the next prompt cycle.

Anti-patterns:
- Do NOT clean up `message_buffers`/`active_messages` in the `result` handler — doing so breaks late-chunk delivery.
- Do NOT call `result_received.remove()` in the `agent_message_chunk` handler — it must remain set until the timer fires.
- The `can_edit` rate-limiter is bypassed only for late chunks (when `result_received` is set); normal streaming still respects the 1-second limit.

## telegram/ (7 files)

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, `ChannelHarness::run_stdio()` |
| `channel.rs` | `TelegramChannel` impl of `Channel` trait |
| `dispatcher.rs` | Teloxide dispatcher setup, message/callback handlers |
| `deliver.rs` | Outbound: agent updates → Telegram messages, thought rendering + collapse |
| `permissions.rs` | Permission request → inline keyboard buttons |
| `peer.rs` | `PeerInfo` extraction from Telegram update context |
| `state.rs` | Shared state: bot instance, session tracking |

Requires `TELEGRAM_BOT_TOKEN` env var.

## debug-http/ (1 file)

Single `main.rs` — axum HTTP server with `SsePayload` typed broadcast for named SSE events. Emits `PORT:{n}` to stderr on bind for port discovery by supervisor. Used in integration tests and local development.

## Adding a New Channel

1. Create `ext/channels/{name}/` with `Cargo.toml` + `src/main.rs`
2. Add to workspace members in root `Cargo.toml`
3. Implement `Channel` trait, use `ChannelHarness::run_stdio()`
4. Add `ChannelConfig` entry in `protoclaw.yaml`
5. Update `crates/protoclaw-channels/` if new routing logic needed
