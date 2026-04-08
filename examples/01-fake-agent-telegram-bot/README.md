# Example 01: Fake Agent Bot

A working protoclaw bot with zero API keys. The mock agent echoes messages back with simulated thinking, showing the full flow: channel → agent → tool → channel.

## Quick Start

```sh
cp .env.example .env
docker compose up --build
```

First build takes a few minutes (Rust compilation). Subsequent starts use cached layers.

Send a message:

```sh
curl -X POST http://localhost:8080/message \
  -H "Content-Type: application/json" \
  -d '{"message": "hello"}'
```

Watch SSE events stream back:

```
data: Analyzing your message...
event: thought

data: Formulating response...
event: thought

data: Echo: hello
```

## Run Tests

```sh
./test.sh
```

Tests cover: health check, message acceptance, SSE streaming, thought events, and message merging (10 rapid messages → fewer agent turns).

Docker workspace mode (spawns mock-agent in a separate container via bollard):

```sh
./test.sh --docker
```

## Add Telegram

1. Message [@BotFather](https://t.me/BotFather), send `/newbot`, copy the token
2. Set `TELEGRAM_BOT_TOKEN` in `.env`
3. Set `TELEGRAM_ENABLED=true` in `.env`
4. `docker compose restart`
5. Message your bot

## Architecture

```
┌──────────────────────────────────────────────────┐
│  protoclaw container                             │
│                                                  │
│  ┌────────────┐  stdio  ┌──────────────┐        │
│  │ protoclaw  │────────→│ mock-agent   │        │
│  │ supervisor │         │ (echo+think) │        │
│  │            │────┐    └──────────────┘        │
│  └────────────┘    │                             │
│       │            │    ┌──────────────┐        │
│       │ stdio      └───→│ system-info  │        │
│       ▼                 │ (MCP tool)   │        │
│  ┌────────────┐         └──────────────┘        │
│  │ debug-http │ :8080                            │
│  │ channel    │                                  │
│  └────────────┘                                  │
└──────────────────────────────────────────────────┘
```

Protoclaw also connects to a [docker-socket-proxy](https://github.com/Tecnativa/docker-socket-proxy) for the optional Docker workspace mode (see `--docker` flag).

## Docker Workspace (Optional)

By default the mock agent runs as a local subprocess. To run it in an isolated container instead:

1. Build the agent image:
   ```sh
   docker compose --profile build-only build
   ```

2. Edit `protoclaw.yaml`:
   - Set `mock-docker.enabled: true`
   - Set `mock.enabled: false`
   - Change channel `agent` fields from `"mock"` to `"mock-docker"`

3. `docker compose up --build`

Security: socket proxy restricts Docker API to containers/images only. Agent containers get `cap_drop: ALL` and `no-new-privileges`.

## Files

| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage cargo-chef build, all binaries |
| `Dockerfile.agent` | Mock-agent Docker workspace image |
| `docker-compose.yml` | Protoclaw + socket-proxy + agent image build |
| `protoclaw.yaml` | Agent, channel, tool, and supervisor config |
| `.env.example` | Environment template |
| `test.sh` | E2E tests (`--docker` for Docker workspace) |
| `tools/system-info/` | Demo MCP tool binary |
