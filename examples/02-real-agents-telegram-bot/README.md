# Protoclaw Real Agent Bot

Run a working protoclaw bot with real AI agents — opencode or claude-code. Mount your agent's config directory and API key, then `docker compose up`.

This example demonstrates protoclaw's multi-agent architecture: two agent definitions in config with an `enabled` toggle to switch between them. Each channel routes to a specific agent via the `agent` field.

## Prerequisites

- Docker and Docker Compose v2
- Anthropic API key ([console.anthropic.com](https://console.anthropic.com))
- Agent config directory (created automatically when you first run the agent locally)

## Quick Start (OpenCode)

```sh
cp .env.example .env
# Edit .env — set ANTHROPIC_API_KEY
docker compose up --build
```

The first build takes several minutes (Rust compilation + npm install). Subsequent starts use cached layers.

Test with curl:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

You'll see SSE events streaming back with the agent's response.

Check Docker logs for debug output showing the full routing flow.

## Switching Agents

This example ships with two agent definitions. OpenCode is enabled by default.

To switch to Claude Code:

1. In `docker-compose.yml`: change `target: opencode` to `target: claude-code`
2. In `docker-compose.yml`: comment out the opencode volume mount, uncomment the claude-code mount
3. In `protoclaw.toml`: set opencode `enabled = false`, claude-code `enabled = true`
4. In `protoclaw.toml`: update channel `agent` fields from `"opencode"` to `"claude-code"`
5. Rebuild: `docker compose up --build`

### Agent Comparison

| | OpenCode | Claude Code |
|---|---|---|
| Binary | `opencode` | `claude` |
| ACP flag | `acp` | `--acp` |
| Config dir | `~/.config/opencode` | `~/.claude` |
| Docker target | `opencode` | `claude-code` |
| Env vars | `XDG_CONFIG_HOME` | `CLAUDE_CONFIG_DIR` |
| npm package | `@anthropic-ai/opencode` | `@anthropic-ai/claude-code` |

Both agents require `ANTHROPIC_API_KEY` set in `.env`.

## Config Mount Details

Each agent stores its configuration in a different directory on the host:

| Agent | Host path | Container path | Env var |
|-------|-----------|----------------|---------|
| OpenCode | `~/.config/opencode` | `/home/protoclaw/.config/opencode` | `XDG_CONFIG_HOME=/home/protoclaw/.config` |
| Claude Code | `~/.claude` | `/home/protoclaw/.claude` | `CLAUDE_CONFIG_DIR=/home/protoclaw/.claude` |

The volume mount in `docker-compose.yml` maps the host directory into the container read-only (`:ro`). The agent's env table in `protoclaw.toml` tells the agent process where to find its config inside the container.

If you haven't run the agent locally yet, the config directory may not exist. Run the agent once on your host machine to create it, or create the directory manually.

## Enable Telegram

After verifying debug-http works:

1. Message [@BotFather](https://t.me/BotFather) on Telegram, send `/newbot`, copy the token
2. Set `TELEGRAM_BOT_TOKEN` in `.env`
3. Uncomment `TELEGRAM_ENABLED=true` in `.env`
4. Restart: `docker compose restart`
5. Message your bot on Telegram

## Configuration

| Section | Purpose |
|---------|---------|
| `log_level` | Logging verbosity (default: debug) |
| `extensions_dir` | Where `@built-in/` binaries live (default: /usr/local/bin) |
| `[agents-manager.agents.*]` | Agent definitions — opencode (enabled) and claude-code (disabled) |
| `[channels-manager.channels.*]` | debug-http (enabled) and telegram (disabled by default) |
| `[channels-manager.channels.*.ack]` | Per-channel ack reactions and typing indicators |
| `[tools-manager.tools.*]` | system-info tool — returns host/OS/arch info |
| `[supervisor]` | Restart policy, health checks, shutdown timeout |

## Troubleshooting

**"Invalid API key" or 401 errors** — Verify `ANTHROPIC_API_KEY` in `.env` is correct and has no trailing whitespace.

**Agent doesn't start / "command not found"** — The agent binary is installed via npm during Docker build. Rebuild with `docker compose build --no-cache` to retry the install.

**Config mount permission errors** — The volume is mounted read-only. Ensure the host config directory is readable by your user. On Linux, check that the container user's UID (1000) can read the files.

**Build fails with out of memory** — Ensure Docker has at least 4GB memory. Rust compilation is memory-intensive.

**Port 8080 already in use** — Change the port mapping in `docker-compose.yml`: `"9090:8080"`.

**Telegram bot doesn't respond** — Verify `TELEGRAM_BOT_TOKEN` is correct, `TELEGRAM_ENABLED=true` is uncommented in `.env`, and no other instance is using the same token.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage cargo-chef build with `--target opencode` and `--target claude-code` |
| `docker-compose.yml` | Single service with agent config mount, port 8080 |
| `protoclaw.toml` | Multi-agent config: opencode + claude-code with named agent maps |
| `.env.example` | Environment template — copy to `.env`, set API key |
| `.dockerignore` | Build context exclusions |
| `README.md` | This file |
