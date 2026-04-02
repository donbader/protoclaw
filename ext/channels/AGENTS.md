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

## telegram/ (7 files)

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, `ChannelHarness::run_stdio()` |
| `channel.rs` | `TelegramChannel` impl of `Channel` trait |
| `dispatcher.rs` | Teloxide dispatcher setup, message/callback handlers |
| `deliver.rs` | Outbound: agent updates → Telegram messages |
| `permissions.rs` | Permission request → inline keyboard buttons |
| `peer.rs` | `PeerInfo` extraction from Telegram update context |
| `state.rs` | Shared state: bot instance, session tracking |

Requires `TELEGRAM_BOT_TOKEN` env var.

## debug-http/ (1 file)

Single `main.rs` — axum HTTP server. Emits `PORT:{n}` to stderr on bind for port discovery by supervisor. Used in integration tests and local development.

## Adding a New Channel

1. Create `ext/channels/{name}/` with `Cargo.toml` + `src/main.rs`
2. Add to workspace members in root `Cargo.toml`
3. Implement `Channel` trait, use `ChannelHarness::run_stdio()`
4. Add `ChannelConfig` entry in `protoclaw.toml`
5. Update `crates/protoclaw-channels/` if new routing logic needed
