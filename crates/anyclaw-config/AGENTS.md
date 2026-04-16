# anyclaw-config — Configuration Loading

Figment-based layered configuration for anyclaw. Loads from embedded defaults → YAML file → environment variables. Provides typed config structs, binary path resolution, and semantic validation.

## Files

| File | Purpose |
|------|---------|
| `types.rs` | All config structs (`AnyclawConfig`, `AgentConfig`, `ChannelConfig`, `ToolConfig`, `SupervisorConfig`, etc.) with serde defaults |
| `lib.rs` | `AnyclawConfig::load()` — Figment layered loading |
| `resolve.rs` | `resolve_binary_path()` — `@built-in/{agents,channels,tools}/<name>` → `extensions_dir`, with legacy alias support |
| `validate.rs` | `validate_config()` — binary existence, working dir checks |
| `error.rs` | `ConfigError` enum (thiserror) |
| `parse.rs` | `parse_memory_limit()`, `parse_cpu_limit()` — K8s-style string to Docker-native units |
| `env_yaml.rs` | YAML provider with `!env` tag resolution for environment variables |
| `defaults.yaml` | Embedded defaults loaded as Figment base layer |

## Key Types

```rust
pub struct AnyclawConfig {
    pub log_level: String,           // default: "info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn,tower=warn"
    pub extensions_dir: String,      // default: "/usr/local/bin"
    pub agents_manager: AgentsManagerConfig,
    pub channels_manager: ChannelsManagerConfig,
    pub tools_manager: ToolsManagerConfig,
    pub supervisor: SupervisorConfig,
}
```

Manager configs use named `HashMap`s — entity names are map keys (no `name` field in structs):
- `AgentsManagerConfig { acp_timeout_secs: u64, shutdown_grace_ms: u64, agents: HashMap<String, AgentConfig> }`
- `ChannelsManagerConfig { init_timeout_secs: u64, exit_timeout_secs: u64, channels: HashMap<String, ChannelConfig> }`
- `ToolsManagerConfig { tools: HashMap<String, ToolConfig>, tools_server_host: String }`

Per-entity override types:
- `BackoffConfig { base_delay_ms: u64, max_delay_secs: u64 }` — optional on `AgentConfig` and `ChannelConfig`
- `CrashTrackerConfig { max_crashes: u32, window_secs: u64 }` — optional on `AgentConfig` and `ChannelConfig`
- `AgentConfig.acp_timeout_secs: Option<u64>` — overrides manager-level default when set
- `AgentConfig.workspace: WorkspaceConfig` — tagged enum, see below
- `ChannelConfig.init_timeout_secs: Option<u64>` — overrides manager-level default when set
- `ChannelConfig.exit_timeout_secs: Option<u64>` — graceful shutdown wait per channel; overrides manager-level `exit_timeout_secs` when set

`WorkspaceConfig` — tagged enum (`#[serde(tag = "type")]`):
- `WorkspaceConfig::Local(LocalWorkspaceConfig)` — `binary` (required), `working_dir` (optional), `env` (optional)
- `WorkspaceConfig::Docker(DockerWorkspaceConfig)` — `image` (required), `entrypoint`, `volumes`, `env`, `memory_limit`, `cpu_limit`, `docker_host`, `network`, `pull_policy`, `working_dir` (optional, path inside the container sent as `cwd` in ACP `session/new`)

`PullPolicy` — enum: `Always`, `IfNotPresent` (default), `Never`. Config-only; pull logic deferred to Docker runtime phase.

## EnvYaml — `!env` Tag Resolution

`EnvYaml` is a custom Figment provider that loads the YAML file and resolves `!env` tagged values from environment variables.

Two forms:
- `!env VAR_NAME` — hard error if env var is missing
- `!env "VAR_NAME:default"` — falls back to default if unset (colon separator)

Typed fields (booleans, numbers) should be YAML literals, not `!env` tags.

```yaml
# Secrets use !env tags
bot_token: !env TELEGRAM_BOT_TOKEN           # required, hard error if missing
api_key: !env "ANTHROPIC_API_KEY:"           # optional, empty default

# Typed fields are literals
enabled: false
```

## Loading Order

```rust
Figment::from(Yaml::string(DEFAULTS_YAML))    // 1. Embedded defaults
    .merge(EnvYaml::file(path))                // 2. User YAML file (with !env tag resolution)
    .extract()
```

The YAML file is the single source of truth. No env var override layer — all environment values flow through `!env` tags in the YAML.

## Binary Resolution

`resolve_binary_path()` expands `@built-in/` prefix against `extensions_dir`:
- Canonical: `@built-in/agents/mock-agent` + `/usr/local/bin` → `/usr/local/bin/agents/mock-agent`
- Canonical: `@built-in/channels/telegram` + `/usr/local/bin` → `/usr/local/bin/channels/telegram`
- Legacy flat paths (e.g. `@built-in/mock-agent`) are resolved via built-in aliases with a deprecation warning.
- Absolute paths and relative names pass through unchanged.

`resolve_all_binary_paths()` applies resolution to all binary paths in the config at once:
- Local agent `binary` fields
- Docker agent `entrypoint` fields
- Channel `binary` fields
- Tool `binary` fields

Called by Supervisor before manager construction — managers receive resolved paths.

## Validation

`validate_config()` checks:
- Local agents: binary exists (absolute path check or PATH lookup), `working_dir` exists
- Docker agents: `memory_limit` parses, `cpu_limit` parses, `docker_host` URI format, `volumes` syntax
- Channel binaries exist
- Tool binaries exist (if specified)

Returns `ValidationResult { errors, warnings }` — caller decides whether to abort.

## Entity Config Design Principle

Top-level fields on entity configs (`AgentConfig`, `ChannelConfig`, `ToolConfig`) are **manager concerns** — spawn, routing, restarts, timeouts. Everything passed to the extension binary lives in `options: HashMap<String, serde_json::Value>`.

| Consumer | Where it lives | Examples |
|----------|---------------|----------|
| Manager only | Top-level field | `binary`, `args`, `enabled`, `agent`, `backoff`, `crash_tracker`, `init_timeout_secs` |
| Extension binary | Inside `options` | `host`, `port`, `bot_token`, `ack` |

The manager extracts structured data from `options` when constructing init params. For example, `ack` config for channels is deserialized from `options["ack"]` into `AckConfig`, then converted to `ChannelAckConfig` for the wire format.

Extensions report their default option values in the `initialize` response (`defaults` field). The manager merges these into the entity's options (user options win). No sidecar files — extensions are self-describing.

Do NOT add new top-level fields for binary-facing config — put them in `options`.

## Anti-Patterns (this crate)

- **Don't use `anyhow`** — use `ConfigError` for all error paths
- **Don't skip validation** — `validate_config()` catches runtime failures at boot time
- **Don't hardcode defaults outside serde** — all defaults live in `defaults.yaml` or `#[serde(default = "...")]` functions in `types.rs`
- **Don't add `name` fields to entity structs** — names come from HashMap keys (manager-hierarchy pattern)
- **Don't use `!env` without a default for optional config** — missing env vars cause a hard error at load time; use `!env "VAR:default"` if the value is optional
