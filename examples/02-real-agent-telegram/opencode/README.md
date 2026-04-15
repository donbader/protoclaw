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

## Configuration

The `anyclaw.yaml` file configures the agent, channels, tools, and supervisor. It's baked into the Docker image at build time — edit and rebuild to apply changes.

Key settings for this variant:

```yaml
agents_manager:
  agents:
    opencode:
      entrypoint: ["opencode", "acp"]
      volumes:
        - "opencode-agent-data:/home/node/.local/share"
        - "opencode-agent-workspace:/home/node/workspace"
        - "opencode-agent-packages:/usr/local"
      env:
        XDG_CONFIG_HOME: "/home/node/.config"
        XDG_DATA_HOME: "/home/node/.local/share"
```

The agent container runs as the `node` user with scoped sudo for `apt-get` only — the agent can install packages at runtime via `sudo apt-get install` without full root access. The `/usr/local` volume persists packages installed via `pip`, `npm install -g`, or `cargo install` across container restarts. Note that `apt-get` installs to system dirs (`/usr/bin`, `/usr/lib`) which are not on this volume — pre-install apt packages in the Dockerfile for persistence.

OpenCode config (`.opencode/`) can optionally be baked into the agent image. To use it:

1. Create `.opencode/opencode.json` with your OpenCode configuration
2. Optionally add `.opencode/package.json` for MCP server dependencies
3. Rebuild: `docker compose up --build -d`

The Dockerfile detects these files and copies them to `/home/node/.config/opencode/` inside the agent image. If a `package.json` is present, `npm install` runs automatically. This directory is gitignored — each user provides their own.

For the full config schema and all available options, see the [Configuration Reference](../CONFIGURATION.md).

## Files

| File                     | Purpose                                                                       |
| ------------------------ | ----------------------------------------------------------------------------- |
| `Dockerfile`             | Multi-stage: pulls ghcr.io base + opencode target + agent image               |
| `docker-compose.yml`     | Socket-proxy + anyclaw + agent image build                                    |
| `anyclaw.yaml`           | Agent, channel, tool, and supervisor config                                   |
| `.opencode/`             | OpenCode config baked into agent image (gitignored — create your own or omit) |
| `.env.example`           | Environment template                                                          |
| `test.sh`                | E2E tests (Docker-only)                                                       |
| `docker-compose.dev.yml` | Contributor-only: dev build override (passes `BUILDER_IMAGE` arg)    |
| `Makefile`               | Contributor-only: `make dev` builds base + starts everything                  |

## Development

> **Contributor-only** — these tools are for developing anyclaw itself, not for running the bot. See [Quick Start](#quick-start) for production usage.

### Local Source Builds

For iterating on anyclaw source code:

```sh
make dev    # Build base image + variant from source, start everything
make logs   # Follow anyclaw logs
make down   # Stop everything
```

The `Makefile` first builds `anyclaw-dev-base:latest` from the root `Dockerfile` (cargo-chef + mold + BuildKit cache mounts), then runs `docker compose` with the dev override which passes `BUILDER_IMAGE=anyclaw-dev-base:latest` to the same `Dockerfile` used in production.
