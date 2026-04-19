# Claude Code Agent Variant

An anyclaw bot using [Claude Code](https://docs.anthropic.com/en/docs/claude-code) as the AI agent. Uses [`@agentclientprotocol/claude-agent-acp`](https://github.com/agentclientprotocol/claude-agent-acp) to bridge Claude's Agent SDK to ACP. The agent runs in an isolated Docker container.

## Prerequisites

Set your Anthropic API key in `.env`:

```sh
cp .env.example .env
# Edit .env and set ANTHROPIC_API_KEY=sk-ant-xxxxxxxx
```

Get an API key at [console.anthropic.com](https://console.anthropic.com/).

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

Tests require `ANTHROPIC_API_KEY` in `.env`.

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
│  │ anyclaw  │──────────────→ │ socket-proxy │   │
│  │          │    tcp:2375    │ (haproxy)    │   │
│  └────┬─────┘                └──────┬───────┘   │
│       │                             │ :ro       │
│       │                             ▼           │
│       │                    /var/run/docker.sock  │
└───────┼─────────────────────────────────────────┘
        │
        │ anyclaw-external network (internet)
        │
        │ spawns via bollard
        ▼
   ┌──────────────────┐
   │ claude-agent-acp │  agent container
   │                  │  (ACP adapter for Claude Agent SDK)
   └──────────────────┘
```

Two Docker networks:

- `anyclaw-internal` — socket-proxy communication, no internet access
- `anyclaw-external` — anyclaw + agent containers, internet for Anthropic API calls and Telegram

## Configuration

The `anyclaw.yaml` file configures the agent, channels, tools, and supervisor. It's baked into the Docker image at build time — edit and rebuild to apply changes.

Key settings for this variant:

```yaml
agents_manager:
  agents:
    claude-code:
      entrypoint: ["claude-agent-acp"]
      volumes:
        - "claude-code-agent-data:/home/agent-claude-code/.claude"
        - "claude-code-agent-workspace:/home/agent-claude-code/workspace"
        - "claude-code-agent-packages:/usr/local"
      env:
        ANTHROPIC_API_KEY: "${ANTHROPIC_API_KEY:}"
```

The agent container runs as the `agent-claude-code` user with scoped sudo for `apt-get` only — the agent can install packages at runtime via `sudo apt-get install` without full root access. The `/usr/local` volume persists packages installed via `pip`, `npm install -g`, or `cargo install` across container restarts. Note that `apt-get` installs to system dirs (`/usr/bin`, `/usr/lib`) which are not on this volume — pre-install apt packages in the Dockerfile for persistence.

The `ANTHROPIC_API_KEY` env var is passed through from `.env` via `${ANTHROPIC_API_KEY:}` substitution.

For the full config schema and all available options, see the [Configuration Reference](../CONFIGURATION.md).

### Claude Code Settings

Claude Code's own settings are baked into the agent image and can be customized before building:

- `.claude/settings.json` — permissions, hooks, env vars, model config. Copied to `~/.claude/settings.json` in the agent container. By default, all Bash/Read/Write permissions are allowed (headless mode).
- `.claude.json` — preferences, MCP server configs, per-project state. Copied to `~/.claude.json` in the agent container.

Edit these files, then rebuild:

```sh
docker compose build
docker compose up -d
```

The `claude-code-agent-data` volume persists `~/.claude/` across restarts, so baked-in defaults only apply on first run (or after `docker compose down -v`).

For local overrides that shouldn't be committed, create `.claude/settings.local.json` (gitignored).

See [Claude Code settings docs](https://docs.anthropic.com/en/docs/claude-code/settings) for all available options.

## Files

| File                     | Purpose                                                                |
| ------------------------ | ---------------------------------------------------------------------- |
| `Dockerfile`             | Multi-stage: pulls ghcr.io base + claude-agent-acp npm install + agent image |
| `docker-compose.yml`     | Socket-proxy + anyclaw + agent image build                             |
| `anyclaw.yaml`           | Agent, channel, tool, and supervisor config                            |
| `.claude/settings.json`  | Claude Code permissions and settings (baked into agent image)          |
| `.claude.json`           | Claude Code preferences and MCP config (baked into agent image)        |
| `.env.example`           | Environment template (ANTHROPIC_API_KEY, Telegram)                     |
| `test-auth.sh`           | Auth validation hook (sourced by `../dev/test.sh`)                     |
| `docker-compose.dev.yml` | Contributor-only: dev build override (passes `CORE_IMAGE`/`EXT_IMAGE` args) |

## Development

> **Contributor-only** — these tools are for developing anyclaw itself, not for running the bot. See [Quick Start](#quick-start) for production usage.

### Local Source Builds

For iterating on anyclaw source code:

```sh
make -f ../dev/Makefile dev    # Build base image + variant from source, start everything
make -f ../dev/Makefile logs   # Follow anyclaw logs
make -f ../dev/Makefile down   # Stop everything
```
