# DX Improvements: Fast Iteration, Log Filtering, Permission Tracing

**Date:** 2026-04-13
**Status:** Approved

## Problem

Debugging the permission flow bug took hours due to three compounding DX issues:
1. Every code change required a full Docker rebuild (5-10 min via cargo-chef)
2. 95% of log output was hyper/reqwest connection pool noise, drowning signal
3. No structured tracing at permission flow handoff points — couldn't tell where the chain broke

## Design

### 1. Fast Iteration: Persistent Builder Container

**Goal:** Incremental rebuilds in ~10-15s instead of 5-10 min.

Keep `Dockerfile.dev-builder` for cold builds. Add a `dev.sh` helper script in `examples/02-real-agents-telegram-bot/` that:

1. On first run: `docker compose build` (uses Dockerfile.dev-builder via override)
2. On subsequent runs with `--rebuild` flag:
   - Starts a persistent builder container with workspace source mounted + cargo registry/target cache volumes
   - Runs `cargo build --release --bin anyclaw --bin telegram-channel --bin debug-http --bin system-info` inside it
   - `docker cp` binaries into the running anyclaw container
   - Restarts the anyclaw container

Cache volumes:
- `anyclaw-cargo-registry` → `/usr/local/cargo/registry`
- `anyclaw-cargo-git` → `/usr/local/cargo/git`
- `anyclaw-target` → `/build/target`

The builder container image is `lukemathwalker/cargo-chef:latest-rust-1.94-alpine` (same as Dockerfile.dev-builder).

### 2. Config-Driven Log Filtering

**Goal:** Suppress noisy crates without recompiling.

Add `log_filter` field to `SupervisorConfig` in `anyclaw-config/src/types.rs`:

```yaml
supervisor:
  log_filter: "info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn"
```

- Default: `"info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn,tower=warn"`
- Applied as `tracing_subscriber::EnvFilter` in `main.rs`
- `ANYCLAW_SUPERVISOR__LOG_FILTER` env var override works via Figment
- Channel subprocesses: pass `log_filter` in `ChannelInitializeParams.options` so channels can apply the same filter to their own tracing subscriber
- `RUST_LOG` env var takes precedence if set (standard convention)

### 3. Permission Flow Tracing

**Goal:** Every handoff in the permission chain emits a structured `tracing::info!` event.

Events to add (all at INFO level with structured fields):

**agents/manager.rs:**
- `handle_permission_request`: already has `permission requested` ✓
- `RespondPermission` command handler: `tracing::info!(agent, request_id, option_id, "permission response received from channel")`
- After `send_raw`: `tracing::info!(agent, request_id, "permission response sent to agent")`

**channels/manager.rs:**
- `RoutePermission` command: `tracing::info!(channel, session_key, request_id, "permission routed to channel")`
- Spawned response task: `tracing::info!(request_id, option_id, "permission response received from channel, forwarding to agents")`

**harness.rs (anyclaw-sdk-channel):**
- `channel/requestPermission` dispatch: `tracing::debug!(request_id, "permission request dispatched to channel")`
- After response: `tracing::debug!(request_id, "permission response ready, returning to harness")`

**telegram/dispatcher.rs:**
- `handle_callback_query`: `tracing::info!(request_id, option_id, chat_id, "callback query received")`

**telegram/permissions.rs:**
- `process_callback`: `tracing::info!(request_id, option_id, "permission broker resolved")`

## Files Modified

| File | Change |
|------|--------|
| `examples/02-real-agents-telegram-bot/dev.sh` | New: helper script for fast rebuilds |
| `crates/anyclaw-config/src/types.rs` | Add `log_filter` to `SupervisorConfig` |
| `crates/anyclaw/src/main.rs` | Apply `log_filter` to tracing subscriber |
| `crates/anyclaw-agents/src/manager.rs` | Add permission response tracing |
| `crates/anyclaw-channels/src/manager.rs` | Add permission routing tracing |
| `crates/anyclaw-sdk-channel/src/harness.rs` | Add permission dispatch tracing |
| `ext/channels/telegram/src/dispatcher.rs` | Add callback query tracing |
| `ext/channels/telegram/src/permissions.rs` | Add broker resolution tracing |
| `examples/02-real-agents-telegram-bot/anyclaw.yaml` | Add `log_filter` config |

## Success Criteria

1. `dev.sh --rebuild` completes incremental rebuild + restart in <30s
2. `docker compose logs` shows zero hyper/reqwest pool messages at default config
3. Permission flow is traceable end-to-end: request → route → channel dispatch → callback → response → agent
4. All existing tests pass
