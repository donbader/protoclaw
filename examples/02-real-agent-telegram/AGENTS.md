# Adding a New Agent Variant

This directory contains agent variants for the real-agent Telegram bot example. Each subdirectory is a self-contained Docker Compose setup that pairs anyclaw with a specific AI agent.

```
examples/02-real-agent-telegram/
├── opencode/     # OpenCode (npm, opencode acp)
├── kiro/         # Kiro CLI (native binary, kiro-cli acp)
└── AGENTS.md     # This file
```

## Requirements

Any ACP-compatible agent can be added. The agent must:

1. Speak ACP (JSON-RPC 2.0 over stdio) — `initialize`, `session/new`, `session/prompt`, etc.
2. Be packageable as a Docker image
3. Run non-interactively (no TTY, no browser prompts during normal operation)

## Step-by-Step: Add a New Variant

Use an existing variant as your starting point. Copy `opencode/` for npm-based agents, or `kiro/` for native binary agents.

```sh
cp -r opencode/ claude-code/
cd claude-code/
```

### 1. Dockerfile

Multi-stage build with four targets. Modify the deps and agent stages:

```
builder          ← ghcr.io/donbader/anyclaw-builder (unchanged)
<agent>-deps     ← Install the agent CLI (npm, curl, apt, etc.)
example-<name>   ← Anyclaw sidecar + ext/ binaries + anyclaw.yaml
<agent>-agent    ← Agent image (spawned by bollard at runtime)
```

Key decisions:

- **Base image**: `node:20-slim` for npm agents, `debian:bookworm-slim` for native binaries
- **User**: Create a non-root user (e.g., `node`, `claude`, `kiro`). The agent container runs as this user.
- **Entrypoint**: The ACP command (e.g., `["claude", "--acp"]`, `["opencode", "acp"]`, `["kiro-cli", "acp"]`)
- **Home dirs**: Create `~/.local/share`, `~/.local/state`, and any agent-specific config dirs. `chown` them to the agent user.

### 2. anyclaw.yaml

The agent config block. Key fields:

```yaml
agents_manager:
  agents:
    <agent-name>:
      workspace:
        type: docker
        image: "anyclaw-<agent-name>-agent:latest"
        entrypoint: ["<binary>", "<acp-flag>"]
        docker_host: "tcp://socket-proxy:2375"
        network: "anyclaw-external"
        working_dir: "/home/<user>/workspace"
        pull_policy: "never"
        volumes:
          - "<agent-name>-agent-data:/home/<user>/.local/share"
          - "<agent-name>-agent-workspace:/home/<user>/workspace"
        env:
          # Agent-specific env vars (API keys, XDG paths, etc.)
      enabled: true
      tools:
        - "system-info"
```

Update `channels_manager.channels.*.agent` to match your agent name.

For the full config schema (all sections, all options), see [CONFIGURATION.md](./CONFIGURATION.md).

### 3. docker-compose.yml

Three services (same pattern for every variant):

```yaml
services:
  socket-proxy: # Docker socket proxy (unchanged)
  anyclaw: # Anyclaw sidecar (target: example-<name>)
  <agent>-agent-image: # Build-only service to tag the agent image
    build:
      target: <agent>-agent
    image: anyclaw-<agent-name>-agent:latest
    entrypoint: ["true"]
    restart: "no"
```

### 4. docker-compose.dev.yml

Dev override for building from workspace source. Three services:

```yaml
services:
  dev-base:              # Builds shared Dockerfile.dev-builder → anyclaw-dev-base:latest
  anyclaw:               # Builds variant Dockerfile.dev-builder → sidecar target
  <agent>-agent-image:   # Builds variant Dockerfile.dev-builder → agent target
```

Both `anyclaw` and `<agent>-agent-image` depend on `dev-base` completing first.

### 5. Dockerfile.dev-builder (shared base)

The parent-level `Dockerfile.dev-builder` compiles all anyclaw + ext binaries from workspace source using cargo-chef. It produces `anyclaw-dev-base:latest` — a minimal image with just the compiled binaries.

Do not modify this file when adding a variant.

### 6. Dockerfile.dev-builder (per-variant)

Each variant has its own `Dockerfile.dev-builder` that starts with `FROM anyclaw-dev-base:latest AS builder` and adds the agent-specific stages (deps, sidecar, agent image). These are identical to the production `Dockerfile` stages but reference the dev base instead of `ghcr.io/donbader/anyclaw-builder`.

### 7. Supporting files

Copy from an existing variant and adjust:

- `.env.example` — agent-specific env vars (API keys, tokens)
- `.dockerignore` — ensure `anyclaw.yaml` is not excluded
- `.gitignore` — agent-specific credential dirs
- `test.sh` — adjust auth checks and agent name in test output
- `README.md` — document auth flow, quick start, architecture

## Auth Patterns

Agents handle authentication differently. Document the auth flow in your README.

| Pattern                      | Example                 | When to use                                                  |
| ---------------------------- | ----------------------- | ------------------------------------------------------------ |
| No auth needed               | OpenCode                | Agent is free / uses ambient credentials                     |
| API key env var              | Kiro (`KIRO_API_KEY`)   | Agent supports headless API key auth                         |
| Pre-authenticated volume     | Kiro (device flow)      | Agent requires interactive login, credentials stored on disk |
| Config file baked into image | OpenCode (`.opencode/`) | Agent reads config from a known path                         |

For volume-based auth, the key is finding where the agent stores credentials. Run the agent interactively with the full home mounted, complete login, then `find` to locate the credential files:

```sh
docker run -it -v test-home:/home/<user> --entrypoint <binary> <image> login
docker run --rm -v test-home:/home/<user> --entrypoint find <image> /home/<user> -type f
```

Then mount only the credential directory (not the entire home) as a named volume in `anyclaw.yaml`.

## Checklist

Before submitting a new variant:

- [ ] `docker compose build` succeeds
- [ ] `docker compose up -d` starts without errors
- [ ] `curl http://localhost:8080/health` returns `{"status":"ok"}`
- [ ] `curl -X POST http://localhost:8080/message -H 'Content-Type: application/json' -d '{"message":"hello"}'` gets a response via SSE
- [ ] `docker compose down` cleans up without orphans
- [ ] README documents auth flow, quick start, and architecture
- [ ] `.env.example` lists all required env vars (commented out)
- [ ] No secrets committed (check `.gitignore`)
