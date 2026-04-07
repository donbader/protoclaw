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

## Docker Workspace

By default, the mock agent runs as a local subprocess inside the protoclaw container. You can optionally run it in an isolated Docker container instead, managed by protoclaw via [bollard](https://github.com/fussybeaver/bollard).

### Architecture

```
┌─────────────────────────────────────────────┐
│  protoclaw-internal network (internal: true) │
│                                              │
│  ┌────────────┐    ┌───────────────────┐     │
│  │ protoclaw  │───▶│  socket-proxy     │     │
│  │            │    │  (Docker API gate) │     │
│  └────────────┘    └───────┬───────────┘     │
│        │                   │                 │
│        │           /var/run/docker.sock (ro)  │
│        ▼                                     │
│  ┌────────────────┐                          │
│  │ mock-agent     │  (spawned by protoclaw   │
│  │ container      │   via bollard at runtime) │
│  └────────────────┘                          │
└──────────────────────────────────────────────┘
```

The [Tecnativa docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) restricts Docker API access to containers and images only — no exec, build, secrets, networks, or volumes.

### Setup

1. Build the agent image:
   ```sh
   docker compose --profile build-only build
   ```

2. Edit `protoclaw.yaml`:
   - Set `mock-docker.enabled: true`
   - Set `mock.enabled: false` (or remove it)
   - Change channel `agent` fields from `"mock"` to `"mock-docker"`

3. Start:
   ```sh
   docker compose up --build
   ```

### Security

The Docker workspace applies defense-in-depth:

- **Socket proxy** — Only `CONTAINERS` and `IMAGES` API endpoints are exposed. `EXEC`, `BUILD`, `SECRETS`, `NETWORKS`, `VOLUMES` are all denied.
- **Read-only socket** — Docker socket is mounted `:ro` on the proxy.
- **Internal network** — `protoclaw-internal` is marked `internal: true`, preventing external access.
- **Container hardening** — protoclaw's DockerBackend applies `cap_drop: ALL` and `no-new-privileges` to spawned agent containers at runtime via bollard's HostConfig.

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

## Testing

Run the full test suite:

```sh
./test.sh
```

Run with Docker workspace mode (builds agent image, patches config, tests Docker path):

```sh
./test.sh --docker
```

## Troubleshooting

**Build fails with out of memory** — Ensure Docker has at least 4GB memory. Rust compilation is memory-intensive.

**Port 8080 already in use** — Change the port mapping in `docker-compose.yml`: `"9090:8080"`.

**Telegram bot doesn't respond** — Verify `TELEGRAM_BOT_TOKEN` is correct, `TELEGRAM_ENABLED=true` is uncommented in `.env`, and no other instance is using the same token.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage cargo-chef build, all binaries in one image |
| `Dockerfile.agent` | Multi-stage build for mock-agent Docker workspace image |
| `docker-compose.yml` | Protoclaw + socket-proxy + build-only agent image service |
| `protoclaw.yaml` | Config: mock-agent (local default), mock-docker (opt-in), channels, tools |
| `.env.example` | Environment template — copy to `.env` |
| `.dockerignore` | Build context exclusions |
| `test.sh` | E2E test script (`--docker` flag for Docker workspace path) |
| `README.md` | This file |
| `tools/system-info/` | Demo MCP tool binary (workspace member) |
