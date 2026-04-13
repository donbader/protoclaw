# Example 02: Real Agent Bot

A protoclaw bot with a real AI agent. OpenCode runs in an isolated Docker container using direct ACP mode (`opencode acp`) — no wrapper binary needed.

## Quick Start

```sh
docker compose up -d
```

Uses pre-built binaries from `ghcr.io/donbader/protoclaw` — only the Node.js + opencode layer is built locally (fast, no Rust compilation).

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
   │ opencode acp │  agent container
   │              │  (opencode direct ACP mode)
   └──────────────┘
```

Two Docker networks:
- `protoclaw-internal` — socket-proxy communication, no internet access
- `protoclaw-external` — protoclaw + agent containers, internet for API calls and Telegram

The [docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) restricts Docker API to containers and images only. Agent containers get `cap_drop: ALL` and `no-new-privileges`.

## Switching Agents

OpenCode is the default agent. To use a different agent:

### Claude Code

1. Build a claude-code agent image (replace the opencode-agent stage in Dockerfile):
   ```dockerfile
   FROM node:20-slim AS opencode-agent
   RUN npm install -g @anthropic-ai/claude-code --omit=dev
   USER node
   WORKDIR /home/node
   ENTRYPOINT ["claude", "--acp"]
   ```
2. Update `protoclaw.yaml`: change `entrypoint: ["opencode", "acp"]` to `entrypoint: ["claude", "--acp"]`
3. Rebuild: `docker compose up --build -d`

### Kiro

1. Build a kiro agent image (replace the opencode-agent stage in Dockerfile with Kiro CLI installation)
2. Update `protoclaw.yaml`: change `entrypoint: ["opencode", "acp"]` to `entrypoint: ["kiro", "--acp"]` (verify Kiro's ACP flag)
3. Rebuild: `docker compose up --build -d`

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage: pulls ghcr.io base + opencode target + agent image |
| `docker-compose.yml` | Socket-proxy + protoclaw + agent image build |
| `protoclaw.yaml` | Agent, channel, tool, and supervisor config |
| `.opencode/` | OpenCode config baked into agent image (gitignored — create your own or omit) |
| `.env.example` | Environment template |
| `test.sh` | E2E tests (Docker-only) |
