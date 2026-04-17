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
  -v kiro-auth-data:/home/agent-kiro/.local/share/kiro-cli \
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
../dev/test.sh
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
  -v kiro-auth-data:/home/agent-kiro/.local/share/kiro-cli \
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
        - "kiro-auth-data:/home/agent-kiro/.local/share/kiro-cli"
        - "kiro-session-data:/home/agent-kiro/.kiro"
        - "kiro-agent-workspace:/home/agent-kiro/workspace"
        - "kiro-agent-packages:/usr/local"
      env:
        KIRO_API_KEY: "${KIRO_API_KEY:}"
```

The agent container runs as the `agent-kiro` user with scoped sudo for `apt-get` only — the agent can install packages at runtime via `sudo apt-get install` without full root access. The `/usr/local` volume persists packages installed via `pip`, `npm install -g`, or `cargo install` across container restarts. Note that `apt-get` installs to system dirs (`/usr/bin`, `/usr/lib`) which are not on this volume — pre-install apt packages in the Dockerfile for persistence.

The `KIRO_API_KEY` env var is passed through from `.env` via `${KIRO_API_KEY:}` substitution. If using browser login instead, the `kiro-auth-data` volume provides credentials.

Kiro CLI stores data in two locations: auth tokens and chat history in `~/.local/share/kiro-cli/` (persisted via `kiro-auth-data` volume), and ACP session files (`<session-id>.json` + `<session-id>.jsonl`) in `~/.kiro/sessions/cli/` (persisted via `kiro-session-data` volume). Both volumes are required for session recovery after container restarts.

For the full config schema and all available options, see the [Configuration Reference](../CONFIGURATION.md).

## Files

| File                     | Purpose                                                             |
| ------------------------ | ------------------------------------------------------------------- |
| `Dockerfile`             | Multi-stage: pulls ghcr.io base + kiro-cli download + agent image   |
| `docker-compose.yml`     | Socket-proxy + anyclaw + agent image build                          |
| `anyclaw.yaml`           | Agent, channel, tool, and supervisor config                         |
| `.env.example`           | Environment template (KIRO_API_KEY, Telegram)                       |
| `test-auth.sh`           | Auth validation hook (sourced by `../dev/test.sh`)                  |
| `docker-compose.dev.yml` | Contributor-only: dev build override (passes `BUILDER_IMAGE` arg)   |

## Development

> **Contributor-only** — these tools are for developing anyclaw itself, not for running the bot. See [Quick Start](#quick-start) for production usage.

### Local Source Builds

For iterating on anyclaw source code:

```sh
make -f ../dev/Makefile dev    # Build base image + variant from source, start everything
make -f ../dev/Makefile logs   # Follow anyclaw logs
make -f ../dev/Makefile down   # Stop everything
```

The `Makefile` first builds `anyclaw-dev-base:latest` from the root `Dockerfile` (cargo-chef + mold + BuildKit cache mounts), then runs `docker compose` with the dev override which passes `BUILDER_IMAGE=anyclaw-dev-base:latest` to the same `Dockerfile` used in production.
