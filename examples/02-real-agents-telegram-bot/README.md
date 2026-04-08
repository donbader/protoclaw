# Example 02: Real Agent Bot

A protoclaw bot with a real AI agent (OpenCode + Claude). The agent runs in an isolated Docker container — config baked in at build time, API keys passed via environment variables.

## Quick Start

```sh
cp .env.example .env
# Optionally set ANTHROPIC_API_KEY in .env (agent works without it via baked-in config)
docker compose --profile build-only build   # Build agent image (required first time)
docker compose up --build -d                # Start in background
```

First build takes several minutes (Rust compilation + npm install). Subsequent starts use cached layers.

Send a message:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

SSE events stream back with the agent's response.

## Run Tests

```sh
./test.sh
```

Tests cover: health check, message acceptance, SSE streaming, result delivery, and message merging (5 rapid messages → fewer agent turns). Takes ~2 minutes due to real AI response times.

Local workspace mode (agent runs as subprocess, no Docker container):

```sh
./test.sh --local
```

## Add Telegram

1. Message [@BotFather](https://t.me/BotFather), send `/newbot`, copy the token
2. Set `TELEGRAM_BOT_TOKEN` in `.env`
3. Set `TELEGRAM_ENABLED=true` in `.env`
4. `docker compose restart`
5. Message your bot

## Architecture

```
┌─────────────────────────────────────────────────┐
│  protoclaw-internal network (no internet)       │
│                                                  │
│  ┌──────────┐    bollard     ┌──────────────┐   │
│  │ protoclaw │──────────────→│ socket-proxy │   │
│  │          │    tcp:2375    │ (haproxy)    │   │
│  └────┬─────┘               └──────┬───────┘   │
│       │                            │ :ro        │
│       │                            ▼            │
│       │                    /var/run/docker.sock  │
└───────┼─────────────────────────────────────────┘
        │
        │ protoclaw-external network (internet)
        │
        │ spawns via bollard
        ▼
   ┌──────────────┐
   │ opencode     │  agent container
   │ agent        │  (opencode + wrapper)
   └──────────────┘
```

Two Docker networks:
- `protoclaw-internal` — socket-proxy communication, no internet access
- `protoclaw-external` — protoclaw + agent containers, internet for API calls and Telegram

The [docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) restricts Docker API to containers and images only. Agent containers get `cap_drop: ALL` and `no-new-privileges`.

## Switching Agents

Three agents ship in `protoclaw.yaml`. OpenCode Docker workspace is enabled by default.

| | OpenCode (Docker) | OpenCode (Local) | Claude Code |
|---|---|---|---|
| Workspace | `docker` | `local` | `local` |
| Binary | `opencode-wrapper` | `@built-in/agents/opencode` | `claude` |
| Config | Baked into image | Volume-mounted | Volume-mounted |

To switch, edit `protoclaw.yaml`: disable the current agent, enable the new one, update channel `agent` fields. See comments in the YAML for details.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage build: `opencode` and `claude-code` targets |
| `Dockerfile.agent` | Agent image: opencode + wrapper, baked config |
| `docker-compose.yml` | Socket-proxy + protoclaw + agent image build |
| `protoclaw.yaml` | Agent, channel, tool, and supervisor config |
| `.opencode/` | OpenCode config baked into agent image |
| `.env.example` | Environment template |
| `test.sh` | E2E tests (`--local` for local workspace) |
