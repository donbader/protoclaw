# Adding a New Agent Variant

This directory contains agent variants for the real-agent Telegram bot example. Each subdirectory is a self-contained Docker Compose setup that pairs anyclaw with a specific AI agent.

```
examples/02-real-agent-telegram/
├── dev/          # Shared Makefile and test runner
├── opencode/     # OpenCode (npm, opencode acp)
├── kiro/         # Kiro CLI (native binary, kiro-cli acp)
├── claude-code/  # Claude Code (npm, claude-agent-acp)
└── AGENTS.md     # This file
```



## Requirements

Any ACP-compatible agent can be added. The agent must:

1. Speak ACP (JSON-RPC 2.0 over stdio) — `initialize`, `session/new`, `session/prompt`, etc.
2. Be packageable as a Docker image
3. Run non-interactively (no TTY, no browser prompts during normal operation)

**Not all agents have native ACP support.** Some agents (e.g., OpenCode, Kiro) have a built-in ACP mode (`opencode acp`, `kiro-cli acp`). Others (e.g., Claude Code) do not — they require an ACP adapter package that bridges their SDK to the ACP protocol.

Before starting, check:
1. Does the agent CLI have an `--acp` flag or `acp` subcommand? → Use it directly.
2. If not, search npm/GitHub for `<agent-name> acp adapter` — community adapters often exist (e.g., `@agentclientprotocol/claude-agent-acp` for Claude Code).
3. If no adapter exists, you'll need to write one or wait for upstream ACP support.

## Step-by-Step: Add a New Variant

Use an existing variant as your starting point. Copy `opencode/` for npm-based agents with native ACP, `kiro/` for native binary agents, or `claude-code/` for agents that need an ACP adapter package.

```sh
cp -r opencode/ <your-agent>/
cd <your-agent>/
```

### 1. Dockerfile

Multi-stage build with four targets. Modify the deps and agent stages:

```
ARG BUILDER_IMAGE=ghcr.io/donbader/anyclaw-builder:latest
builder          ← ${BUILDER_IMAGE} (overridden to anyclaw-dev-base:latest for dev builds)
<agent>-deps     ← Install the agent CLI or ACP adapter (npm, curl, apt, etc.)
example-<name>   ← Anyclaw sidecar + ext/ binaries + anyclaw.yaml
<agent>-agent    ← Agent image (spawned by bollard at runtime)
```

Key decisions:

- **Base image**: `node:20-slim` for npm agents, `debian:bookworm-slim` for native binaries
- **Deps stage**: Install either the agent CLI (if it has native ACP) or the ACP adapter package (if it doesn't). The adapter pulls in the agent SDK as a dependency — you don't need to install both.
- **User**: Both the sidecar (`example-<name>`) and agent (`<agent>-agent`) stages must run as a non-root user. For the sidecar stage on `node:20-slim` bases, use the built-in `node` user. For `debian:bookworm-slim` bases, create a dedicated user (e.g., `useradd -m -s /bin/bash anyclaw`). In the agent stage, always create a dedicated user named `agent-<name>` (e.g., `useradd -m -s /bin/bash agent-opencode`). In the sidecar stage, `chown /workspace` to the user and add `USER <user>` before the entrypoint. In the agent stage, `chown` the home directory and add `USER agent-<name>`.
- **Scoped sudo**: Agent containers get passwordless `sudo apt-get` so the AI agent can install packages at runtime without full root. Add `sudo` to the apt install list and grant access via sudoers: `echo "agent-<name> ALL=(ALL) NOPASSWD: /usr/bin/apt-get" >> /etc/sudoers.d/agent-<name>`. Do not grant unrestricted sudo.
- **Entrypoint**: The ACP command. This varies by agent:
  - Native ACP: `["opencode", "acp"]`, `["kiro-cli", "acp"]`
  - ACP adapter: `["claude-agent-acp"]` (npm package that wraps the agent's SDK)
- **Home dirs**: Create `~/.local/share`, `~/.local/state`, and any agent-specific config dirs under `/home/agent-<name>/`. `chown` them to the agent user.
- **Package persistence**: Mount a named volume at `/usr/local` in the agent container so that packages installed via `pip`, `npm install -g`, `cargo install`, etc. survive container restarts. Note: `apt-get` installs to `/usr/bin` and `/usr/lib` which are not on this volume — use the Dockerfile for apt packages that must persist.

### 2. anyclaw.yaml

The agent config block. Key fields:

```yaml
agents_manager:
  agents:
    <agent-name>:
      workspace:
        type: docker
        image: "anyclaw-<agent-name>-agent:latest"
        entrypoint: ["<acp-binary-or-adapter>"]
        docker_host: "tcp://socket-proxy:2375"
        network: "anyclaw-external"
        working_dir: "/home/agent-<name>/workspace"
        pull_policy: "never"
        volumes:
          - "<agent-name>-agent-data:/home/agent-<name>/.local/share"
          - "<agent-name>-agent-workspace:/home/agent-<name>/workspace"
          - "<agent-name>-agent-packages:/usr/local"
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

Dev override that swaps the builder image via build arg. No separate Dockerfile needed:

```yaml
services:
  anyclaw:
    build:
      args:
        BUILDER_IMAGE: anyclaw-dev-base:latest
    volumes:
      - ./anyclaw.yaml:/workspace/anyclaw.yaml:ro

  <agent>-agent-image:
    build:
      args:
        BUILDER_IMAGE: anyclaw-dev-base:latest
```

Not invoked directly — use `make -f ../dev/Makefile dev` instead.

### 5. Shared dev tooling

The `dev/` directory contains shared tooling used by all variants:

- `dev/Makefile` — orchestrates the two-step dev build. Run from a variant directory:

```sh
make -f ../dev/Makefile dev       # Build base image + variant from source, start everything
make -f ../dev/Makefile dev-base  # Rebuild anyclaw binaries only
make -f ../dev/Makefile logs      # Follow anyclaw logs
make -f ../dev/Makefile down      # Stop everything
```

- `dev/test.sh` — shared E2E test runner. Run from a variant directory:

```sh
../dev/test.sh [base_url]
```

The test runner handles the full lifecycle (build, start, test, teardown). If `./test-auth.sh` exists in the variant directory, it is sourced before starting containers — use this hook to validate agent-specific credentials. OpenCode needs no auth hook. Kiro and Claude Code each have a `test-auth.sh` that checks for their respective credentials.

### 6. Supporting files

Copy from an existing variant and adjust:

- `.env.example` — agent-specific env vars (API keys, tokens)
- `.dockerignore` — ensure `anyclaw.yaml` is not excluded
- `.gitignore` — agent-specific credential dirs
- `test-auth.sh` — auth validation hook sourced by `../dev/test.sh` (optional — omit if no auth needed)
- `README.md` — document auth flow, quick start, architecture

## Auth Patterns

Agents handle authentication differently. Document the auth flow in your README.

| Pattern                      | Example                              | When to use                                                  |
| ---------------------------- | ------------------------------------ | ------------------------------------------------------------ |
| No auth needed               | OpenCode                             | Agent is free / uses ambient credentials                     |
| API key env var              | Kiro (`KIRO_API_KEY`)                | Agent supports headless API key auth                         |
| API key + custom base URL    | Claude Code (`ANTHROPIC_API_KEY` + `ANTHROPIC_BASE_URL`) | Agent supports API key auth with optional proxy/local server |
| Pre-authenticated volume     | Kiro (device flow)                   | Agent requires interactive login, credentials stored on disk |
| Config file baked into image | OpenCode (`opencode-config/`)        | Agent reads config from a known path                         |

For volume-based auth, the key is finding where the agent stores credentials. Run the agent interactively with the full home mounted, complete login, then `find` to locate the credential files:

```sh
docker run -it -v test-home:/home/agent-<name> --entrypoint <binary> <image> login
docker run --rm -v test-home:/home/agent-<name> --entrypoint find <image> /home/agent-<name> -type f
```

Then mount only the credential directory (not the entire home) as a named volume in `anyclaw.yaml`.

## Checklist

Before submitting a new variant:

- [ ] `docker compose build` succeeds
- [ ] `docker compose up -d` starts without errors
- [ ] `curl http://localhost:8080/health` returns `{"status":"ok"}`
- [ ] `curl -X POST http://localhost:8080/message -H 'Content-Type: application/json' -d '{"message":"hello"}'` gets a response via SSE
- [ ] `docker compose down` cleans up without orphans
- [ ] `../dev/test.sh` passes all assertions
- [ ] README documents auth flow, quick start, and architecture
- [ ] `.env.example` lists all required env vars (commented out)
- [ ] No secrets committed (check `.gitignore`)

## Troubleshooting

### Config changes not taking effect

`anyclaw.yaml` is baked into `/etc/anyclaw/` (outside the `/workspace` volume), so config changes take effect on rebuild without needing to remove volumes:

```sh
docker compose up -d --build
```

If you still see stale behavior from persisted session data, remove volumes:

```sh
docker compose down -v
docker compose up -d --build
```

### Agent binary not found

If you see `exec: "<binary>": executable file not found in $PATH`, the entrypoint in `anyclaw.yaml` doesn't match what's installed in the agent image. Run this to check what's available:

```sh
docker run --rm --entrypoint ls <image> /usr/local/bin/ | grep -i <agent>
```

### Agent doesn't speak ACP

If the agent CLI has no `--acp` flag or `acp` subcommand, you need an ACP adapter. Check:

```sh
# Does the agent have native ACP?
docker run --rm --entrypoint <binary> <image> --help 2>&1 | grep -i acp

# If not, search for an adapter
npm search acp <agent-name>
```

See the `claude-code/` variant for a working example of the adapter pattern.
