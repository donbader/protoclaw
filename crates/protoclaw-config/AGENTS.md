# protoclaw-config — Configuration Loading

Figment-based layered configuration for protoclaw. Loads from embedded defaults → YAML file → environment variables. Provides typed config structs, binary path resolution, and semantic validation.

## Files

| File | Purpose |
|------|---------|
| `types.rs` | All config structs (`ProtoclawConfig`, `AgentConfig`, `ChannelConfig`, `ToolConfig`, `SupervisorConfig`, etc.) with serde defaults |
| `lib.rs` | `ProtoclawConfig::load()` — Figment layered loading |
| `resolve.rs` | `resolve_binary_path()` — `@built-in/` prefix → `extensions_dir` |
| `validate.rs` | `validate_config()` — binary existence, working dir checks |
| `error.rs` | `ConfigError` enum (thiserror) |
| `subst_yaml.rs` | YAML provider with environment variable substitution |
| `defaults.yaml` | Embedded defaults loaded as Figment base layer |

## Key Types

```rust
pub struct ProtoclawConfig {
    pub log_level: String,           // default: "info"
    pub extensions_dir: String,      // default: "/usr/local/bin"
    pub agents_manager: AgentsManagerConfig,
    pub channels_manager: ChannelsManagerConfig,
    pub tools_manager: ToolsManagerConfig,
    pub supervisor: SupervisorConfig,
}
```

Manager configs use named `HashMap`s — entity names are map keys (no `name` field in structs):
- `AgentsManagerConfig { acp_timeout_secs: u64, shutdown_grace_ms: u64, agents: HashMap<String, AgentConfig> }`
- `ChannelsManagerConfig { init_timeout_secs: u64, channels: HashMap<String, ChannelConfig>, debounce: DebounceConfig }`
- `ToolsManagerConfig { tools: HashMap<String, ToolConfig> }`

Per-entity override types:
- `BackoffConfig { base_delay_ms: u64, max_delay_secs: u64 }` — optional on `AgentConfig` and `ChannelConfig`
- `CrashTrackerConfig { max_crashes: u32, window_secs: u64 }` — optional on `AgentConfig` and `ChannelConfig`
- `AgentConfig.acp_timeout_secs: Option<u64>` — overrides manager-level default when set
- `ChannelConfig.init_timeout_secs: Option<u64>` — overrides manager-level default when set

## Loading Order

```rust
Figment::from(Yaml::string(DEFAULTS_YAML))    // 1. Embedded defaults
    .merge(SubstYaml::file(path))               // 2. User YAML file (with env substitution)
    .merge(Env::prefixed("PROTOCLAW_").split("__"))  // 3. Environment variables
    .extract()
```

Env var override format: `PROTOCLAW_SUPERVISOR__SHUTDOWN_TIMEOUT_SECS=60` (double underscore = nesting).

## Binary Resolution

`resolve_binary_path()` expands `@built-in/` prefix against `extensions_dir`:
- `@built-in/mock-agent` + `/usr/local/bin` → `/usr/local/bin/mock-agent`
- Absolute paths and relative names pass through unchanged.

Called by Supervisor before manager construction — managers receive resolved paths.

## Validation

`validate_config()` checks:
- Agent binaries exist (absolute path check or PATH lookup)
- Agent `working_dir` directories exist
- Channel binaries exist
- Tool binaries exist (if specified)

Returns `ValidationResult { errors, warnings }` — caller decides whether to abort.

## Anti-Patterns (this crate)

- **Don't use `anyhow`** — use `ConfigError` for all error paths
- **Don't skip validation** — `validate_config()` catches runtime failures at boot time
- **Don't hardcode defaults outside serde** — all defaults live in `defaults.yaml` or `#[serde(default = "...")]` functions in `types.rs`
- **Don't add `name` fields to entity structs** — names come from HashMap keys (manager-hierarchy pattern)
