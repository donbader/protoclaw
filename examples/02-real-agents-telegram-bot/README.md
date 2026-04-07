# Protoclaw Real Agent Bot — Docker Workspace

Opencode runs in an isolated Docker container managed by protoclaw via bollard. No agent config volume mounts — config is baked into the agent image at build time. API keys pass through environment variables.

## Prerequisites

- Docker and Docker Compose v2
- Anthropic API key

## Quick Start

```sh
cp .env.example .env
# Set ANTHROPIC_API_KEY in .env
docker compose --profile build-only build   # Build agent image
docker compose up --build                   # Start protoclaw + socket proxy
```

The first build takes several minutes (Rust compilation + npm install). Subsequent starts use cached layers.

Test with curl:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

SSE events stream back with the agent's response.

## Architecture

```
┌─────────────────────────────────────────────────┐
│              protoclaw-internal network          │
│                                                  │
│  ┌──────────┐    bollard     ┌──────────────┐   │
│  │ protoclaw │──────────────→│ socket-proxy │   │
│  │          │    tcp:2375    │ (haproxy)    │   │
│  └────┬─────┘               └──────┬───────┘   │
│       │                            │            │
│       │ spawns via bollard         │ :ro        │
│       ▼                            ▼            │
│  ┌──────────────┐         /var/run/docker.sock  │
│  │ opencode     │                               │
│  │ agent        │                               │
│  │ container    │                               │
│  └──────────────┘                               │
└─────────────────────────────────────────────────┘
```

Protoclaw connects to Docker through [Tecnativa/docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy), an haproxy-based filter that restricts which Docker API endpoints are accessible:

| API Category | Allowed | Why |
|---|---|---|
| CONTAINERS | ✓ | Create, start, stop, remove agent containers |
| IMAGES | ✓ | List/inspect images for pull policy checks |
| POST | ✓ | Required for container lifecycle operations |
| SECRETS | ✗ | No access to Docker secrets |
| BUILD | ✗ | No image building from inside protoclaw |
| EXEC | ✗ | No exec into running containers |
| NETWORKS | ✗ | No network manipulation |
| VOLUMES | ✗ | No volume management |

The socket proxy and protoclaw run on an `internal: true` Docker network. Agent containers join the same network when spawned.

## Security

- **Socket proxy API filtering** — only CONTAINERS, IMAGES, and POST enabled; all other Docker API categories blocked
- **Agent container hardening** — `cap_drop: ALL`, `security_opt: no-new-privileges` applied by DockerBackend
- **Non-root agent user** — agent container runs as UID 1000
- **No Docker socket in agent** — agent containers have no access to the Docker socket
- **API keys via env only** — keys pass through container environment variables at runtime, never baked into images

## Switching Agents

Three agent definitions ship in `protoclaw.yaml`. Opencode (Docker workspace) is enabled by default.

**Docker → Local workspace:**

1. In `protoclaw.yaml`: set `opencode` `enabled: false`, `opencode-local` `enabled: true`
2. In `protoclaw.yaml`: update channel `agent` fields to `"opencode-local"`
3. In `docker-compose.yml`: add volume mounts for `.opencode` config directory
4. Rebuild: `docker compose up --build`

**Docker → Claude Code:**

1. In `docker-compose.yml`: change `target: opencode` to `target: claude-code`
2. In `protoclaw.yaml`: set `opencode` `enabled: false`, `claude-code` `enabled: true`
3. In `protoclaw.yaml`: update channel `agent` fields to `"claude-code"`
4. In `docker-compose.yml`: add volume mount for `~/.claude`
5. Rebuild: `docker compose up --build`

### Agent Comparison

| | OpenCode (Docker) | OpenCode (Local) | Claude Code |
|---|---|---|---|
| Workspace | `docker` | `local` | `local` |
| Binary | `opencode-wrapper` (in container) | `@built-in/agents/opencode` | `claude` |
| Config | Baked into agent image | Volume-mounted | Volume-mounted |
| Auth | `ANTHROPIC_API_KEY` env var | `ANTHROPIC_API_KEY` env var | `ANTHROPIC_API_KEY` env var |
| npm package | `opencode-ai` | `opencode-ai` | `@anthropic-ai/claude-code` |

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
| `agents-manager.agents.*` | Agent definitions — opencode (Docker), opencode-local, claude-code |
| `channels-manager.channels.*` | debug-http (enabled) and telegram (disabled by default) |
| `channels-manager.channels.*.ack` | Per-channel ack reactions and typing indicators |
| `tools-manager.tools.*` | system-info tool — returns host/OS/arch info |
| `supervisor` | Restart policy, health checks, shutdown timeout |

**Tip:** Edit `protoclaw.yaml` and restart the container — no rebuild needed. The config file is volume-mounted, so changes take effect on next `docker compose restart`.

## Troubleshooting

**Agent container doesn't start** — Ensure `docker compose --profile build-only build` ran first. The agent image must exist locally (`protoclaw-opencode-agent:latest`). Check with `docker images | grep protoclaw-opencode`.

**Socket proxy connection refused** — Verify the socket-proxy service is running: `docker compose logs socket-proxy`. Ensure `/var/run/docker.sock` exists on the host.

**"Invalid API key" or 401 errors** — Verify `ANTHROPIC_API_KEY` in `.env` is correct and has no trailing whitespace.

**Build fails with out of memory** — Ensure Docker has at least 4GB memory. Rust compilation is memory-intensive.

**Port 8080 already in use** — Change the port mapping in `docker-compose.yml`: `"9090:8080"`.

**Telegram bot doesn't respond** — Verify `TELEGRAM_BOT_TOKEN` is correct, `TELEGRAM_ENABLED=true` is uncommented in `.env`, and no other instance is using the same token.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage cargo-chef build with `--target opencode` and `--target claude-code` for protoclaw |
| `Dockerfile.agent` | Opencode agent image — opencode + wrapper binary, baked config |
| `docker-compose.yml` | Socket proxy, protoclaw, agent image build (build-only profile) |
| `protoclaw.yaml` | Docker workspace default, local and claude-code fallbacks |
| `.env.example` | Environment template — copy to `.env`, set API key |
| `.dockerignore` | Build context exclusions |
| `test.sh` | E2E test script — `./test.sh` (Docker) or `./test.sh --local` (local workspace) |
| `README.md` | This file |
