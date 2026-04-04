# Protoclaw Fake Agent Bot

Run a working protoclaw bot with zero AI API keys — just Docker.

The mock agent echoes your messages back with simulated thinking delays, showing the full message flow through protoclaw's infrastructure: channel → agent → tool → channel.

## Prerequisites

- Docker and Docker Compose v2
- Telegram bot token (optional — only needed for Telegram channel)

## Quick Start

```sh
cp .env.example .env
docker compose up --build
```

The first build takes a few minutes (Rust compilation). Subsequent starts use cached layers.

Test with curl:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

You'll see SSE events streaming back — thought chunks first, then the echo response:

```
data: {"type":"agent_thought_chunk","content":"thinking..."}
...
data: {"type":"result","content":"Echo: hello"}
```

Check the Docker logs for debug output showing the full routing flow.

## Enable Telegram

After verifying debug-http works:

1. Message [@BotFather](https://t.me/BotFather) on Telegram, send `/newbot`, copy the token
2. Set `TELEGRAM_BOT_TOKEN` in `.env`
3. Uncomment `TELEGRAM_ENABLED=true` in `.env`
4. Restart: `docker compose restart`
5. Message your bot on Telegram

## What You'll See

With `LOG_LEVEL=debug` (the default), Docker logs show:

- Agent receiving messages and generating thought chunks
- Channel routing decisions (session creation, message delivery)
- MCP tool calls (system-info) and responses
- Supervisor health checks and manager status

## Configuration

| Section | Purpose |
|---------|---------|
| `log_level` | Logging verbosity (default: debug) |
| `extensions_dir` | Where `@built-in/` binaries live (default: /usr/local/bin) |
| `agents-manager.agents.*` | Mock agent binary — echoes messages with simulated thinking |
| `channels-manager.channels.*` | debug-http (enabled) and telegram (disabled by default) |
| `channels-manager.debounce` | Message debounce settings (window, enabled) |
| `tools-manager.tools.*` | system-info tool — returns host/OS/arch info |
| `supervisor` | Restart policy, health checks, shutdown timeout |

**Tip:** Edit `protoclaw.yaml` and restart the container — no rebuild needed. The config file is volume-mounted, so changes take effect on next `docker compose restart`.

## Troubleshooting

**Build fails with out of memory** — Ensure Docker has at least 4GB memory. Rust compilation is memory-intensive.

**Port 8080 already in use** — Change the port mapping in `docker-compose.yml`: `"9090:8080"`.

**Telegram bot doesn't respond** — Verify `TELEGRAM_BOT_TOKEN` is correct, `TELEGRAM_ENABLED=true` is uncommented in `.env`, and no other instance is using the same token.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage cargo-chef build, all binaries in one image |
| `docker-compose.yml` | Single service with port 8080, debug logging |
| `protoclaw.yaml` | Config: mock-agent, debug-http, telegram, system-info |
| `.env.example` | Environment template — copy to `.env` |
| `.dockerignore` | Build context exclusions |
| `README.md` | This file |
| `tools/system-info/` | Demo MCP tool binary (workspace member) |
