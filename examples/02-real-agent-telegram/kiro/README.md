# Kiro Agent Variant

An anyclaw bot using [Kiro CLI](https://kiro.dev/cli/) as the AI agent. Kiro runs in an isolated Docker container using ACP mode (`kiro-cli acp`).

## Prerequisites

Kiro supports two authentication methods. Pick one.

### Option A: API key (Pro+ subscription)

Simplest path. Set `KIRO_API_KEY` in `.env`:

```sh
cp .env.example .env
# Edit .env and set KIRO_API_KEY=ksk_xxxxxxxx
```

Generate an API key at [app.kiro.dev](https://app.kiro.dev) under the API Keys section. Requires Kiro Pro, Pro+, or Power subscription.

### Option B: Browser login (any tier)

For free tier or when you don't want to use an API key. Authenticate once interactively, storing credentials in a Docker volume:

```sh
docker compose build kiro-agent-image
docker run -it \
  -v kiro-auth-data:/home/kiro/.local/share/kiro-cli \
  --entrypoint kiro-cli \
  anyclaw-kiro-agent:latest \
  login --use-device-flow
```

Complete the device flow login when prompted (open the URL in your browser and enter the code). The auth tokens persist in the `kiro-auth-data` volume, which anyclaw mounts into each agent container via the `volumes` config in `anyclaw.yaml`.

## Quick Start

```sh
docker compose up -d
```

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

Tests require either `KIRO_API_KEY` in `.env` or a `kiro-auth-data` Docker volume.

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
   │ kiro-cli acp │  agent container
   │              │  (Kiro direct ACP mode)
   └──────────────┘
```

Two Docker networks:

- `anyclaw-internal` — socket-proxy communication, no internet access
- `anyclaw-external` — anyclaw + agent containers, internet for API calls and Telegram

## Token Refresh

If using browser login (Option B) and Kiro's auth tokens expire, the agent will fail to start and anyclaw's crash recovery loop will keep respawning it. When you see repeated agent failures in logs, re-run the login:

```sh
docker run -it \
  -v kiro-auth-data:/home/kiro/.local/share/kiro-cli \
  --entrypoint kiro-cli \
  anyclaw-kiro-agent:latest \
  login --use-device-flow
```

Then restart: `docker compose restart`

API key auth (Option A) does not have this problem — keys are long-lived and don't expire unless revoked.

## Configuration

The `anyclaw.yaml` file configures the agent, channels, tools, and supervisor. It's baked into the Docker image at build time — edit and rebuild to apply changes.

Key settings for this variant:

```yaml
agents_manager:
  agents:
    kiro:
      entrypoint: ["kiro-cli", "acp"]
      volumes:
        - "kiro-auth-data:/home/kiro/.local/share/kiro-cli"
        - "kiro-agent-workspace:/home/kiro/workspace"
        - "kiro-agent-packages:/usr/local"
      env:
        KIRO_API_KEY: "${KIRO_API_KEY:}"
```

The agent container runs as the `kiro` user with scoped sudo for `apt-get` only — the agent can install packages at runtime via `sudo apt-get install` without full root access. The `/usr/local` volume persists packages installed via `pip`, `npm install -g`, or `cargo install` across container restarts. Note that `apt-get` installs to system dirs (`/usr/bin`, `/usr/lib`) which are not on this volume — pre-install apt packages in the Dockerfile for persistence.

The `KIRO_API_KEY` env var is passed through from `.env` via `${KIRO_API_KEY:}` substitution. If using browser login instead, the `kiro-auth-data` volume provides credentials.

Kiro CLI settings (`.kiro/`) can optionally be placed in this directory before building. The Dockerfile creates `/home/kiro/.kiro` inside the agent image. However, runtime state (auth tokens, session data) lives in `/home/kiro/.local/share/kiro-cli/` and is persisted via the `kiro-auth-data` Docker volume — not baked into the image.

For the full config schema and all available options, see the [Configuration Reference](../CONFIGURATION.md).

## Files

| File                     | Purpose                                                             |
| ------------------------ | ------------------------------------------------------------------- |
| `Dockerfile`             | Multi-stage: pulls ghcr.io base + kiro-cli download + agent image   |
| `docker-compose.yml`     | Socket-proxy + anyclaw + agent image build                          |
| `anyclaw.yaml`           | Agent, channel, tool, and supervisor config                         |
| `.env.example`           | Environment template (KIRO_API_KEY, Telegram)                       |
| `test.sh`                | E2E tests (Docker-only, requires auth)                              |
| `docker-compose.dev.yml` | Contributor-only: dev build override (builds from workspace source) |
| `Dockerfile.dev-builder` | Contributor-only: agent stages for dev build (references shared base) |
| `Makefile`               | Contributor-only: `make dev` builds base + starts everything        |

## Development

> **Contributor-only** — these tools are for developing anyclaw itself, not for running the bot. See [Quick Start](#quick-start) for production usage.

### Local Source Builds

For iterating on anyclaw source code:

```sh
make dev    # Build base image + variant from source, start everything
make logs   # Follow anyclaw logs
make down   # Stop everything
```

The `Makefile` first builds `anyclaw-dev-base:latest` from the shared `../Dockerfile.dev-builder` (cargo-chef + mold + BuildKit cache mounts), then runs `docker compose` with the dev override to build the variant images and start services.
