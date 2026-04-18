# Configuration Reference

Each agent variant contains an `anyclaw.yaml` that configures the entire stack. This document explains the shared structure — only the agent-specific block differs between variants.

## File Structure

```
<variant>/
├── anyclaw.yaml        # Main config (baked into Docker image)
├── .env                # Secrets and toggles (not committed)
└── .env.example        # Template for .env
```

`anyclaw.yaml` is copied into the Docker image at `/etc/anyclaw/anyclaw.yaml` at build time, outside the `/workspace` volume so it's never shadowed by persistent data. The entrypoint passes `--config /etc/anyclaw/anyclaw.yaml`. To change config, edit the file and rebuild: `docker compose up --build -d`.

## Environment Variable Substitution

Values in `anyclaw.yaml` support `${VAR}` and `${VAR:default}` syntax. Variables are resolved from the container's environment (passed via `env_file: .env` in docker-compose).

```yaml
enabled: "${TELEGRAM_ENABLED:false}"     # defaults to "false" if unset
TELEGRAM_BOT_TOKEN: "${TELEGRAM_BOT_TOKEN:}"  # empty string if unset
```

Missing variables without a default cause a hard error at startup.

## Config Sections

### `agents_manager`

```yaml
agents_manager:
  acp_timeout_secs: 120          # Max time to wait for ACP initialize handshake
  agents:
    <agent-name>:                # Name used by channels to route messages
      workspace:
        type: docker             # "docker" or "local"
        image: "..."             # Docker image for the agent container
        entrypoint: [...]        # ACP command (e.g., ["opencode", "acp"])
        docker_host: "tcp://socket-proxy:2375"  # Docker API endpoint
        network: "anyclaw-external"             # Docker network for agent container
        working_dir: "/home/<user>/workspace"   # CWD sent in ACP session/new
        pull_policy: "never"     # "always", "if_not_present", or "never"
        volumes:                 # Docker volume mounts
          - "<name>:<path>"
        env:                     # Environment variables passed to agent container
          KEY: "value"
      enabled: true
      tools:                     # MCP tools available to this agent
        - "system-info"
```

Per-agent overrides (optional):
- `acp_timeout_secs` — override the manager-level default
- `backoff: { base_delay_ms, max_delay_secs }` — crash recovery timing
- `crash_tracker: { max_crashes, window_secs }` — crash loop detection

### `channels_manager`

```yaml
channels_manager:
  channels:
    <channel-name>:
      binary: "@built-in/channels/debug-http"  # Channel binary path
      agent: "<agent-name>"      # Which agent receives messages from this channel
      enabled: true              # Can use env substitution: "${TELEGRAM_ENABLED:false}"
      options:                   # Passed to channel via initialize handshake
        HOST: "0.0.0.0"
        PORT: "8080"
      ack:                       # Message acknowledgment behavior
        reaction: false          # React to incoming messages
        reaction_emoji: "👀"     # Emoji for reaction (if enabled)
        typing: false            # Show typing indicator
        reaction_lifecycle: "remove"  # "remove" or "keep"
```

Per-channel overrides (optional):
- `init_timeout_secs` — override manager-level initialize timeout
- `exit_timeout_secs` — graceful shutdown wait
- `backoff` and `crash_tracker` — same as agents

### `tools_manager`

```yaml
tools_manager:
  tools_server_host: "anyclaw"   # Hostname for the aggregated MCP server
  tools:
    <tool-name>:
      binary: "@built-in/tools/system-info"  # MCP tool binary path
```

### `session_store`

```yaml
session_store:
  type: sqlite                   # "sqlite" or "noop"
  path: "/workspace/data/sessions.db"
  ttl_days: 7                    # Session expiry
```

### `supervisor`

```yaml
supervisor:
  shutdown_timeout_secs: 30      # Max time for graceful shutdown
  health_check_interval_secs: 5
  max_restarts: 5                # Max restarts within window before giving up
  restart_window_secs: 60
```

### Top-level

```yaml
log_level: "debug,hyper=warn,reqwest=warn,..."  # tracing filter directive
extensions_dir: "/usr/local/bin"                 # Base path for @built-in/ resolution
```

## Binary Path Resolution

`@built-in/` is a prefix that resolves against `extensions_dir`:
- `@built-in/channels/debug-http` → `/usr/local/bin/channels/debug-http`
- `@built-in/tools/system-info` → `/usr/local/bin/tools/system-info`

## What Differs Per Variant

Only the `agents_manager.agents` block changes between variants. Everything else (channels, tools, session store, supervisor) is identical. When creating a new variant, copy `anyclaw.yaml` from an existing one and modify only the agent section.

## Agent-Specific Config Directories

Each variant may include a config directory that gets baked into the agent Docker image at build time. Minimal configs are committed — customize with your provider settings and MCP server deps as needed.

| Variant | Directory | Baked into | Purpose |
|---------|-----------|------------|---------|
| OpenCode | `opencode-config/` | `/home/agent-opencode/.config/opencode/` | `opencode.json` config + optional `package.json` for MCP server deps |
| Kiro | `kiro-config/` | `/home/agent-kiro/.kiro/` | Kiro CLI settings (not auth — auth lives in a Docker volume) |

These directories are separate from runtime state. Auth tokens and session data are persisted via Docker volumes configured in `anyclaw.yaml`, not baked into images.

For the full config type definitions, see `crates/anyclaw-config/src/types.rs`.
