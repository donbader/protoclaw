# OpenCode Agent Variant

An anyclaw bot with OpenCode as the AI agent. OpenCode runs in an isolated Docker container using direct ACP mode (`opencode acp`).

## Quick Start

```sh
docker compose up -d
```

Uses pre-built binaries from `ghcr.io/donbader/anyclaw` — only the Node.js + opencode layer is built locally (fast, no Rust compilation).

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

## Add Telegram

1. Message [@BotFather](https://t.me/BotFather), send `/newbot`, copy the token
2. Set `TELEGRAM_BOT_TOKEN` in `.env`
3. Set `TELEGRAM_ENABLED=true` in `.env`
4. `docker compose restart`
5. Message your bot

## Architecture

```
┌─────────────────────────────────────────────────┐
│  anyclaw-internal network (no internet)         │
│                                                 │
│  ┌──────────┐    bollard     ┌──────────────┐   │
│  │ anyclaw  │──────────────→.│ socket-proxy │   │
│  │          │    tcp:2375    │ (haproxy)    │   │
│  └────┬─────┘                └──────┬───────┘   │
│       │                             │ :ro       │
│       │                             ▼           │
│       │                    /var/run/docker.sock │
└───────┼─────────────────────────────────────────┘
        │
        │ anyclaw-external network (internet)
        │
        │ spawns via bollard
        ▼
   ┌──────────────┐
   │ opencode acp │  agent container
   │              │  (opencode direct ACP mode)
   └──────────────┘
```

Two Docker networks:

- `anyclaw-internal` — socket-proxy communication, no internet access
- `anyclaw-external` — anyclaw + agent containers, internet for API calls and Telegram

The [docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) restricts Docker API to containers and images only. Agent containers get `cap_drop: ALL` and `no-new-privileges`.

See the [parent AGENTS.md](../AGENTS.md) for how to add a new agent variant (e.g., Claude Code).

## Files

| File                     | Purpose                                                                       |
| ------------------------ | ----------------------------------------------------------------------------- |
| `Dockerfile`             | Multi-stage: pulls ghcr.io base + opencode target + agent image               |
| `docker-compose.yml`     | Socket-proxy + anyclaw + agent image build                                    |
| `anyclaw.yaml`           | Agent, channel, tool, and supervisor config                                   |
| `.opencode/`             | OpenCode config baked into agent image (gitignored — create your own or omit) |
| `.env.example`           | Environment template                                                          |
| `test.sh`                | E2E tests (Docker-only)                                                       |
| `docker-compose.dev.yml` | Contributor-only: dev build override (builds from workspace source)           |
| `Dockerfile.dev-builder` | Contributor-only: local source build with cargo-chef caching                  |

## Development

> **Contributor-only** — these tools are for developing anyclaw itself, not for running the bot. See [Quick Start](#quick-start) for production usage.

### Local Source Builds

For iterating on anyclaw source code, use the dev override:

```sh
# Build from workspace source + start
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build

# Incremental rebuild (fast — BuildKit cache mounts preserve cargo target dir)
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build

# Follow logs
docker compose -f docker-compose.yml -f docker-compose.dev.yml logs -f anyclaw

# Stop
docker compose -f docker-compose.yml -f docker-compose.dev.yml down
```

This uses `docker-compose.dev.yml` (override) and `Dockerfile.dev-builder` (cargo-chef + mold + BuildKit cache mounts). Neither is loaded by the default `docker compose up`.
