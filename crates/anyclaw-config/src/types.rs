use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::path::PathBuf;

/// A config value that can be either a plain string or an array of strings.
///
/// In YAML:
/// ```yaml
/// binary: "opencode"          # string form → ["opencode"]
/// binary: ["opencode", "acp"] # array form → ["opencode", "acp"]
/// ```
///
/// The first element is the executable; remaining elements are prepended arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct StringOrArray(pub Vec<String>);

impl StringOrArray {
    /// Return all elements as a slice.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Return the first element (the command), if any.
    pub fn first(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }

    /// Split into (command, args). Panics if the vec is empty.
    pub fn command_and_args(&self) -> (&str, &[String]) {
        let (cmd, rest) = self
            .0
            .split_first()
            .expect("StringOrArray must contain at least one element");
        (cmd.as_str(), rest)
    }
}

impl From<String> for StringOrArray {
    fn from(s: String) -> Self {
        StringOrArray(vec![s])
    }
}

impl From<&str> for StringOrArray {
    fn from(s: &str) -> Self {
        StringOrArray(vec![s.to_string()])
    }
}

impl std::fmt::Display for StringOrArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for s in &self.0 {
            if !first {
                write!(f, " ")?;
            }
            write!(f, "{s}")?;
            first = false;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for StringOrArray {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawStringOrArray {
            Single(String),
            Array(Vec<String>),
        }

        match RawStringOrArray::deserialize(deserializer)? {
            RawStringOrArray::Single(s) => Ok(StringOrArray(vec![s])),
            RawStringOrArray::Array(v) => Ok(StringOrArray(v)),
        }
    }
}

impl Serialize for StringOrArray {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.0.len() == 1 {
            serializer.serialize_str(&self.0[0])
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl schemars::JsonSchema for StringOrArray {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "StringOrArray".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "oneOf": [
                { "type": "string" },
                { "type": "array", "items": { "type": "string" } }
            ]
        })
    }
}

/// Embedded defaults YAML — loaded as base layer in Figment.
pub const DEFAULTS_YAML: &str = include_str!("defaults.yaml");

/// Output format for tracing/log output.
///
/// `pretty` is the default and suitable for development. `json` emits
/// structured JSON lines, which is preferable in production environments
/// where log aggregators (e.g., Datadog, CloudWatch) ingest structured output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Human-readable colored output (default, suitable for development).
    #[default]
    Pretty,
    /// Structured JSON lines (suitable for production log aggregators).
    Json,
}

/// SQLite-backed session store configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct SqliteStoreConfig {
    /// Path to the SQLite database file. Defaults to an in-memory database when absent.
    #[serde(default)]
    pub path: Option<String>,
    /// How many days of inactivity before a session is eligible for expiry cleanup.
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u32,
}

impl Default for SqliteStoreConfig {
    fn default() -> Self {
        Self {
            path: None,
            ttl_days: default_ttl_days(),
        }
    }
}

fn default_ttl_days() -> u32 {
    7
}

/// Selects the session persistence backend.
///
/// Configured under `session_store.type` in `anyclaw.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionStoreConfig {
    /// No session persistence (default). Sessions are not saved across restarts.
    #[default]
    None,
    /// Persist sessions to a SQLite database.
    Sqlite(SqliteStoreConfig),
}

/// Top-level anyclaw configuration.
///
/// Loaded from layered providers: defaults → YAML file → environment variables.
/// Manager-hierarchy: each manager owns its children as named maps.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AnyclawConfig {
    /// Tracing filter directive (e.g. `"info,hyper=warn"`).
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Output format for tracing logs.
    #[serde(default)]
    pub log_format: LogFormat,
    /// Base directory for resolving `@built-in/` binary paths.
    #[serde(default = "default_extensions_dir")]
    pub extensions_dir: String,
    /// Agents manager configuration and named agent map.
    #[serde(default)]
    pub agents_manager: AgentsManagerConfig,
    /// Channels manager configuration and named channel map.
    #[serde(default)]
    pub channels_manager: ChannelsManagerConfig,
    /// Tools manager configuration and named tool map.
    #[serde(default)]
    pub tools_manager: ToolsManagerConfig,
    /// Supervisor-level settings (shutdown timeout, health interval, restart limits).
    #[serde(default)]
    pub supervisor: SupervisorConfig,
    /// Session persistence backend selection.
    #[serde(default)]
    pub session_store: SessionStoreConfig,
}

/// Per-subprocess backoff configuration for restart delays.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct BackoffConfig {
    /// Initial delay before the first restart attempt (milliseconds).
    #[serde(default = "default_backoff_base_ms")]
    pub base_delay_ms: u64,
    /// Maximum delay cap — the backoff will never exceed this (seconds).
    #[serde(default = "default_backoff_max_secs")]
    pub max_delay_secs: u64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_delay_ms: default_backoff_base_ms(),
            max_delay_secs: default_backoff_max_secs(),
        }
    }
}

/// Per-subprocess crash recovery limits.
///
/// Controls how many times an individual agent or channel subprocess may crash
/// within a rolling time window before the crash recovery loop gives up and
/// lets the manager propagate the failure upward to the Supervisor.
///
/// This is distinct from `SupervisorConfig.max_restarts` / `restart_window_secs`,
/// which govern manager-level restart attempts by the Supervisor itself.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CrashTrackerConfig {
    /// Maximum number of subprocess crashes allowed within `window_secs`.
    #[serde(default = "default_crash_max")]
    pub max_crashes: u32,
    /// Rolling window in seconds over which crashes are counted.
    #[serde(default = "default_crash_window_secs")]
    pub window_secs: u64,
}

impl Default for CrashTrackerConfig {
    fn default() -> Self {
        Self {
            max_crashes: default_crash_max(),
            window_secs: default_crash_window_secs(),
        }
    }
}

/// Configuration for the agents manager and its named agent map.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AgentsManagerConfig {
    /// Default ACP request timeout in seconds (overridable per-agent).
    #[serde(default = "default_acp_timeout_secs")]
    pub acp_timeout_secs: u64,
    /// Idle timeout for session/prompt in seconds. The timer resets on every
    /// session/update from the agent. Only fires when the agent goes completely
    /// silent. Default: 120s. Set to 0 to disable.
    #[serde(default = "default_prompt_idle_timeout_secs")]
    pub prompt_idle_timeout_secs: u64,
    /// Grace period after sending shutdown before force-killing agent subprocesses (ms).
    #[serde(default = "default_shutdown_grace_ms")]
    pub shutdown_grace_ms: u64,
    /// Named agent configurations (keys are agent names used in routing).
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
}

impl Default for AgentsManagerConfig {
    fn default() -> Self {
        Self {
            acp_timeout_secs: default_acp_timeout_secs(),
            prompt_idle_timeout_secs: default_prompt_idle_timeout_secs(),
            shutdown_grace_ms: default_shutdown_grace_ms(),
            agents: HashMap::new(),
        }
    }
}

/// Configuration for the channels manager and its named channel map.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ChannelsManagerConfig {
    /// Timeout for the `initialize` handshake with channel subprocesses (seconds).
    #[serde(default = "default_init_timeout_secs")]
    pub init_timeout_secs: u64,
    /// Graceful shutdown wait before force-killing channel subprocesses (seconds).
    #[serde(default = "default_exit_timeout_secs")]
    pub exit_timeout_secs: u64,
    /// Named channel configurations (keys are channel names used in routing).
    #[serde(default)]
    pub channels: HashMap<String, ChannelConfig>,
}

impl Default for ChannelsManagerConfig {
    fn default() -> Self {
        Self {
            init_timeout_secs: default_init_timeout_secs(),
            exit_timeout_secs: default_exit_timeout_secs(),
            channels: HashMap::new(),
        }
    }
}

/// Configuration for the tools manager and its named tool map.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ToolsManagerConfig {
    /// Named tool configurations (keys are tool names advertised to agents).
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
    /// Host/IP for the aggregated MCP HTTP endpoint. Default `127.0.0.1`.
    /// Set to the container hostname in Docker deployments so agent containers can reach it.
    #[serde(default = "default_tools_server_host")]
    pub tools_server_host: String,
}

impl Default for ToolsManagerConfig {
    fn default() -> Self {
        Self {
            tools: HashMap::new(),
            tools_server_host: default_tools_server_host(),
        }
    }
}

/// Docker image pull policy for agent containers.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub enum PullPolicy {
    /// Always pull the image before starting the container.
    Always,
    /// Only pull if the image is not already present locally (default).
    #[default]
    IfNotPresent,
    /// Never pull — fail if the image is not present locally.
    Never,
}

impl<'de> serde::Deserialize<'de> for PullPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt.as_deref() {
            None | Some("") => Ok(PullPolicy::IfNotPresent),
            Some("always") => Ok(PullPolicy::Always),
            Some("never") => Ok(PullPolicy::Never),
            Some("if_not_present") => Ok(PullPolicy::IfNotPresent),
            Some(other) => Err(serde::de::Error::unknown_variant(
                other,
                &["always", "never", "if_not_present"],
            )),
        }
    }
}

impl schemars::JsonSchema for PullPolicy {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PullPolicy".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "enum": ["always", "never", "if_not_present"],
            "default": "if_not_present"
        })
    }
}

/// Local workspace: agent runs as a native subprocess on the host.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct LocalWorkspaceConfig {
    /// Executable path (or `@built-in/` prefix resolved at boot).
    pub binary: StringOrArray,
    /// Working directory for the subprocess. `None` = inherit supervisor's cwd.
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    /// Extra environment variables passed to the subprocess.
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub env: HashMap<String, String>,
}

/// Docker workspace: agent runs inside a Docker container.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
pub struct DockerWorkspaceConfig {
    /// Docker image name (e.g. `"anyclaw/opencode:latest"`).
    pub image: String,
    /// Override the container entrypoint.
    #[serde(default)]
    pub entrypoint: Option<StringOrArray>,
    /// Volume mounts in `host:container[:ro]` format.
    #[serde(default)]
    pub volumes: Vec<String>,
    /// Extra environment variables passed to the container.
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub env: HashMap<String, String>,
    /// Docker memory limit (K8s-style, e.g. `"512m"`, `"2g"`).
    #[serde(default)]
    pub memory_limit: Option<String>,
    /// Docker CPU limit (e.g. `"1.5"` = 1.5 cores).
    #[serde(default)]
    pub cpu_limit: Option<String>,
    /// Docker daemon socket URI (e.g. `"unix:///var/run/docker.sock"`).
    #[serde(default)]
    pub docker_host: Option<String>,
    /// Docker network to attach the container to.
    #[serde(default)]
    pub network: Option<String>,
    /// Image pull policy before starting the container.
    #[serde(default)]
    pub pull_policy: PullPolicy,
    /// Working directory to pass as `cwd` in the ACP `session/new` handshake.
    /// Refers to a path *inside* the agent container, not on the host.
    /// When unset, the supervisor's own working directory is used (which is
    /// usually wrong for Docker workspaces — set this to the container's WORKDIR).
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    /// Extra `/etc/hosts` entries for the container, in `hostname:ip` format.
    #[serde(default)]
    pub extra_hosts: Vec<String>,
}

/// Tagged enum selecting the agent's execution environment.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceConfig {
    /// Run the agent as a local subprocess.
    Local(LocalWorkspaceConfig),
    /// Run the agent inside a Docker container.
    Docker(DockerWorkspaceConfig),
}

// LIMITATION: Do not access binary/env/working_dir on AgentConfig directly
// AgentConfig.workspace is a tagged enum (WorkspaceConfig::Local or WorkspaceConfig::Docker).
// Binary, env, and working_dir live on the variant structs, not on AgentConfig itself.
// Always match on agent.workspace to access these fields. Accessing them directly would
// require adding redundant fields that drift out of sync with the workspace variant.
// See also: AGENTS.md §Anti-Patterns

/// JSON Schema helper: allows `enabled` to be a boolean or a string (for `!env` tag compatibility).
///
/// yaml-language-server sees `!env "VAR:false"` as a string; the Rust type stays `bool`
/// because the `!env` tag is resolved before serde sees the value.
fn bool_or_string_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "oneOf": [
            { "type": "boolean" },
            { "type": "string" }
        ]
    })
}

/// Per-agent configuration. Names come from the HashMap key in [`AgentsManagerConfig`].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AgentConfig {
    /// Execution environment (local subprocess or Docker container).
    pub workspace: WorkspaceConfig,
    /// Whether this agent is active. Disabled agents are not spawned.
    #[serde(default = "default_true")]
    #[schemars(schema_with = "bool_or_string_schema")]
    pub enabled: bool,
    /// Tool names this agent is allowed to use (matched against tool config keys).
    #[serde(default)]
    pub tools: Vec<String>,
    /// Per-agent ACP timeout override (seconds). `None` = use manager default.
    #[serde(default)]
    pub acp_timeout_secs: Option<u64>,
    /// Per-agent backoff override. `None` = use default backoff.
    #[serde(default)]
    pub backoff: Option<BackoffConfig>,
    /// Per-agent crash tracker override. `None` = use default crash tracker.
    #[serde(default)]
    pub crash_tracker: Option<CrashTrackerConfig>,
    /// Arbitrary key-value options passed to the agent during initialization.
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

/// Per-channel configuration. Names come from the HashMap key in [`ChannelsManagerConfig`].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ChannelConfig {
    /// Path to the channel binary (resolved via `@built-in/` at boot).
    pub binary: String,
    /// Extra CLI arguments passed to the channel subprocess.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether this channel is active. Disabled channels are not spawned.
    #[serde(default = "default_true")]
    #[schemars(schema_with = "bool_or_string_schema")]
    pub enabled: bool,
    /// Which agent to route messages to (matches an agent config key).
    #[serde(default = "default_agent")]
    pub agent: String,
    /// Per-channel init timeout override (seconds). `None` = use manager default.
    #[serde(default)]
    pub init_timeout_secs: Option<u64>,
    /// Per-channel graceful shutdown timeout override (seconds). `None` = use manager default.
    #[serde(default)]
    pub exit_timeout_secs: Option<u64>,
    /// Per-channel backoff override. `None` = use default backoff.
    #[serde(default)]
    pub backoff: Option<BackoffConfig>,
    /// Per-channel crash tracker override. `None` = use default crash tracker.
    #[serde(default)]
    pub crash_tracker: Option<CrashTrackerConfig>,
    /// Arbitrary key-value options passed to the channel during initialization.
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

/// What happens to the reaction emoji after the agent finishes responding.
///
/// - `remove`: the reaction is deleted once the response is sent
/// - `replace_done`: the in-progress reaction is swapped for a "done" checkmark emoji
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReactionLifecycle {
    /// Remove the reaction emoji after the agent finishes (default).
    #[default]
    Remove,
    /// Replace the in-progress emoji with a "done" checkmark.
    ReplaceDone,
}

impl std::fmt::Display for ReactionLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReactionLifecycle::Remove => write!(f, "remove"),
            ReactionLifecycle::ReplaceDone => write!(f, "replace_done"),
        }
    }
}

/// Message acknowledgement configuration for a channel.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct AckConfig {
    /// Whether to add a reaction emoji when a message is received.
    #[serde(default)]
    pub reaction: bool,
    /// Whether to send a typing indicator while the agent is processing.
    #[serde(default)]
    pub typing: bool,
    /// Emoji used for the "in progress" reaction.
    #[serde(default = "default_reaction_emoji")]
    pub reaction_emoji: String,
    /// What to do with the reaction after the agent finishes responding.
    #[serde(default)]
    pub reaction_lifecycle: ReactionLifecycle,
}

impl Default for AckConfig {
    fn default() -> Self {
        Self {
            reaction: false,
            typing: false,
            reaction_emoji: default_reaction_emoji(),
            reaction_lifecycle: ReactionLifecycle::default(),
        }
    }
}

impl From<AckConfig> for anyclaw_sdk_types::ChannelAckConfig {
    fn from(ack: AckConfig) -> Self {
        Self {
            reaction: ack.reaction,
            typing: ack.typing,
            reaction_emoji: ack.reaction_emoji,
            reaction_lifecycle: ack.reaction_lifecycle.to_string(),
        }
    }
}

/// Whether a tool is served by an external MCP server process or a local WASM module.
///
/// - `mcp`: spawn an external binary and communicate over JSON-RPC/stdio
/// - `wasm`: load a `.wasm` module and execute in the built-in sandboxed runner
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    /// External MCP server subprocess communicating over JSON-RPC/stdio (default).
    #[default]
    Mcp,
    /// WASM module executed in the built-in sandboxed runner.
    Wasm,
}

/// Per-tool configuration. Names come from the HashMap key in [`ToolsManagerConfig`].
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ToolConfig {
    /// Whether this tool is an external MCP server or a WASM module.
    #[serde(default)]
    pub tool_type: ToolType,
    /// Path to the MCP server binary (for `Mcp` type tools).
    #[serde(default)]
    pub binary: Option<String>,
    /// Extra CLI arguments passed to the tool binary.
    #[serde(default)]
    pub args: Vec<String>,
    /// Whether this tool is active. Disabled tools are not started.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Path to the `.wasm` module file (for `Wasm` type tools).
    #[serde(default)]
    pub module: Option<PathBuf>,
    /// Human-readable description of what the tool does.
    #[serde(default)]
    pub description: String,
    /// JSON Schema string describing the tool's input parameters.
    #[serde(default)]
    pub input_schema: Option<String>,
    /// WASM sandbox limits (fuel, timeout, memory, filesystem preopens).
    #[serde(default)]
    pub sandbox: WasmSandboxConfig,
    /// Arbitrary key-value options passed to the tool during initialization.
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

/// WASM sandbox resource limits for sandboxed tool execution.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct WasmSandboxConfig {
    /// Maximum fuel (instruction count) per tool invocation.
    #[serde(default = "default_fuel_limit")]
    pub fuel_limit: u64,
    /// Wall-clock timeout per invocation (seconds), enforced via epoch interruption.
    #[serde(default = "default_epoch_timeout")]
    pub epoch_timeout_secs: u64,
    /// Maximum memory the WASM module may allocate (bytes).
    #[serde(default = "default_memory_limit")]
    pub memory_limit_bytes: u64,
    /// Host directories pre-opened for WASI filesystem access.
    #[serde(default)]
    pub preopened_dirs: Vec<PreopenedDir>,
}

/// A host directory pre-opened for WASI filesystem access inside the WASM sandbox.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PreopenedDir {
    /// Path on the host filesystem.
    pub host: PathBuf,
    /// Path visible to the WASM module.
    pub guest: String,
    /// Whether the directory is mounted read-only (default: `true`).
    #[serde(default = "default_readonly_true")]
    pub readonly: bool,
}

/// Supervisor-level manager restart limits.
///
/// Controls how many times the Supervisor will restart a *manager* (ToolsManager,
/// AgentsManager, or ChannelsManager) if it exits unexpectedly, within a rolling
/// time window.
///
/// This is distinct from `CrashTrackerConfig`, which limits restarts of individual
/// agent or channel *subprocesses* within a manager's crash recovery loop.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct SupervisorConfig {
    /// Total shutdown timeout divided equally among managers (seconds).
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
    /// Interval between health check sweeps (seconds).
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    /// Maximum number of manager restart attempts within `restart_window_secs`.
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    /// Rolling window in seconds over which manager crash attempts are counted.
    #[serde(default = "default_restart_window")]
    pub restart_window_secs: u64,
    /// TCP port for the admin HTTP server (`/health`, `/metrics`). Default: 3000.
    #[serde(default = "default_admin_port")]
    pub admin_port: u16,
    /// Optional permission response timeout. None = block indefinitely (default).
    #[serde(default)]
    pub permission_timeout_secs: Option<u64>,
}

/// Deserialize a map where values may have been coerced from strings to integers/bools
/// by the env-substitution layer. Coerces all scalar values back to strings.
fn deserialize_string_map<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, serde_json::Value> = HashMap::deserialize(deserializer)?;
    Ok(map
        .into_iter()
        .map(|(k, v)| {
            let s = match v {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                other => other.to_string(),
            };
            (k, s)
        })
        .collect())
}

fn default_log_level() -> String {
    "info,hyper=warn,reqwest=warn,h2=warn,hyper_util=warn,tower=warn".into()
}
fn default_extensions_dir() -> String {
    "/usr/local/bin".into()
}
fn default_true() -> bool {
    true
}
fn default_readonly_true() -> bool {
    true
}
fn default_agent() -> String {
    "default".into()
}
fn default_reaction_emoji() -> String {
    "👀".into()
}
fn default_tools_server_host() -> String {
    "127.0.0.1".into()
}
fn default_fuel_limit() -> u64 {
    1_000_000
}
fn default_epoch_timeout() -> u64 {
    30
}
fn default_memory_limit() -> u64 {
    67_108_864
}
fn default_shutdown_timeout() -> u64 {
    30
}
fn default_health_interval() -> u64 {
    5
}
fn default_max_restarts() -> u32 {
    5
}
fn default_restart_window() -> u64 {
    60
}
fn default_admin_port() -> u16 {
    3000
}
fn default_acp_timeout_secs() -> u64 {
    30
}
fn default_prompt_idle_timeout_secs() -> u64 {
    120
}
fn default_init_timeout_secs() -> u64 {
    10
}
fn default_exit_timeout_secs() -> u64 {
    5
}
fn default_shutdown_grace_ms() -> u64 {
    100
}
fn default_backoff_base_ms() -> u64 {
    100
}
fn default_backoff_max_secs() -> u64 {
    30
}
fn default_crash_max() -> u32 {
    5
}
fn default_crash_window_secs() -> u64 {
    60
}

impl Default for WasmSandboxConfig {
    fn default() -> Self {
        Self {
            fuel_limit: default_fuel_limit(),
            epoch_timeout_secs: default_epoch_timeout(),
            memory_limit_bytes: default_memory_limit(),
            preopened_dirs: Vec::new(),
        }
    }
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            shutdown_timeout_secs: default_shutdown_timeout(),
            health_check_interval_secs: default_health_interval(),
            max_restarts: default_max_restarts(),
            restart_window_secs: default_restart_window(),
            admin_port: default_admin_port(),
            permission_timeout_secs: None,
        }
    }
}

impl AnyclawConfig {
    /// Return the name of the first enabled agent, or `None` if no agents are configured.
    pub fn default_agent_name(&self) -> Option<&str> {
        self.agents_manager
            .agents
            .iter()
            .find(|(_, a)| a.enabled)
            .map(|(name, _)| name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_no_log_level_set_then_defaults_to_info_with_noise_filters() {
        let yaml = "";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.log_level.starts_with("info,"));
        assert!(config.log_level.contains("hyper=warn"));
    }

    #[test]
    fn when_log_level_in_yaml_then_uses_provided_value() {
        let yaml = "log_level: \"debug\"";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn when_no_extensions_dir_set_then_defaults_to_usr_local_bin() {
        let yaml = "";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.extensions_dir, "/usr/local/bin");
    }

    #[test]
    fn when_extensions_dir_in_yaml_then_uses_provided_path() {
        let yaml = "extensions_dir: \"/custom/path\"";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.extensions_dir, "/custom/path");
    }

    #[test]
    fn when_agents_manager_has_agents_then_parses_named_map_with_workspace() {
        let yaml = r#"
agents_manager:
  agents:
    opencode:
      workspace:
        type: local
        binary: "opencode"
        env:
          ANTHROPIC_API_KEY: "sk-test"
      tools:
        - "system-info"
        - "filesystem"
    claude-code:
      workspace:
        type: local
        binary: "claude"
      enabled: false
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 2);
        let oc = &config.agents_manager.agents["opencode"];
        if let WorkspaceConfig::Local(ref local) = oc.workspace {
            assert_eq!(local.binary, StringOrArray(vec!["opencode".into()]));
            assert_eq!(local.env["ANTHROPIC_API_KEY"], "sk-test");
        } else {
            panic!("expected Local workspace");
        }
        assert!(oc.enabled);
        assert_eq!(oc.tools, vec!["system-info", "filesystem"]);
        let cc = &config.agents_manager.agents["claude-code"];
        assert!(!cc.enabled);
    }

    #[test]
    fn when_channel_config_parsed_then_name_comes_from_map_key() {
        let yaml = r#"
channels_manager:
  channels:
    debug-http:
      binary: "debug-http"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert!(config.channels_manager.channels.contains_key("debug-http"));
    }

    #[test]
    fn when_channel_has_no_enabled_field_then_defaults_to_true() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn when_channel_enabled_is_false_then_disabled() {
        let yaml = "binary: \"telegram-channel\"\nenabled: false";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn when_channel_has_no_agent_field_then_defaults_to_default() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agent, "default");
    }

    #[test]
    fn when_channel_agent_in_yaml_then_uses_provided_name() {
        let yaml = "binary: \"telegram-channel\"\nagent: \"opencode\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agent, "opencode");
    }

    #[test]
    fn when_ack_config_defaulted_then_reaction_and_typing_are_false() {
        let ack = AckConfig::default();
        assert!(!ack.reaction);
        assert!(!ack.typing);
        assert_eq!(ack.reaction_emoji, "👀");
        assert_eq!(ack.reaction_lifecycle, ReactionLifecycle::Remove);
    }

    #[test]
    fn when_ack_config_in_yaml_then_all_fields_parsed() {
        let yaml = "reaction: true\ntyping: true\nreaction_emoji: \"⏳\"\nreaction_lifecycle: \"replace_done\"";
        let config: AckConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.reaction);
        assert!(config.typing);
        assert_eq!(config.reaction_emoji, "⏳");
        assert_eq!(config.reaction_lifecycle, ReactionLifecycle::ReplaceDone);
    }

    #[test]
    fn when_channel_has_ack_config_in_options_then_parses_correctly() {
        let yaml = r#"
channels_manager:
  channels:
    telegram:
      binary: "telegram-channel"
      agent: "opencode"
      options:
        ack:
          reaction: true
          typing: true
          reaction_emoji: "👀"
          reaction_lifecycle: "remove"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        let tg = &config.channels_manager.channels["telegram"];
        let ack: AckConfig =
            serde_json::from_value(tg.options["ack"].clone()).expect("ack in options");
        assert!(ack.reaction);
        assert!(ack.typing);
        assert_eq!(ack.reaction_emoji, "👀");
    }

    #[test]
    fn when_tool_has_no_type_field_then_defaults_to_mcp() {
        let yaml = r#"
tools_manager:
  tools:
    filesystem:
      binary: "mcp-server-filesystem"
      args:
        - "--root"
        - "/workspace"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        let fs = &config.tools_manager.tools["filesystem"];
        assert_eq!(fs.tool_type, ToolType::Mcp);
        assert_eq!(fs.binary, Some("mcp-server-filesystem".into()));
        assert!(fs.enabled);
    }

    #[test]
    fn when_tool_type_is_wasm_then_module_and_sandbox_parsed() {
        let yaml = r#"
tools_manager:
  tools:
    my-tool:
      tool_type: "wasm"
      module: "/path/to/tool.wasm"
      description: "A test tool"
      input_schema: '{"type": "object"}'
      sandbox:
        fuel_limit: 500000
        epoch_timeout_secs: 10
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        let t = &config.tools_manager.tools["my-tool"];
        assert_eq!(t.tool_type, ToolType::Wasm);
        assert_eq!(t.module, Some(PathBuf::from("/path/to/tool.wasm")));
        assert_eq!(t.description, "A test tool");
        assert_eq!(t.sandbox.fuel_limit, 500_000);
        assert_eq!(t.sandbox.epoch_timeout_secs, 10);
    }

    #[test]
    fn when_tool_config_parsed_then_name_comes_from_map_key() {
        let yaml = "binary: \"mcp-server-filesystem\"";
        let config: ToolConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.binary, Some("mcp-server-filesystem".into()));
    }

    #[test]
    fn when_no_log_format_set_then_defaults_to_pretty() {
        let yaml = "";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn when_log_format_in_yaml_then_uses_provided_value() {
        let yaml = "log_format: \"json\"";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, LogFormat::Json);
    }

    #[test]
    fn when_parsing_defaults_yaml_then_all_expected_values_present() {
        let config: AnyclawConfig = serde_yaml::from_str(DEFAULTS_YAML).unwrap();
        assert!(config.log_level.starts_with("info,"));
        assert!(config.log_level.contains("hyper=warn"));
        assert_eq!(config.log_format, LogFormat::Pretty);
        assert_eq!(config.extensions_dir, "/usr/local/bin");
        assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
        assert_eq!(config.agents_manager.acp_timeout_secs, 30);
        assert_eq!(config.agents_manager.shutdown_grace_ms, 100);
        assert_eq!(config.channels_manager.init_timeout_secs, 10);
        assert_eq!(config.channels_manager.exit_timeout_secs, 5);
        assert_eq!(config.tools_manager.tools_server_host, "127.0.0.1");
        assert_eq!(config.supervisor.admin_port, 3000);
    }

    #[test]
    fn when_defaults_yaml_values_match_serde_default_fns_then_no_drift() {
        let config: AnyclawConfig = serde_yaml::from_str(DEFAULTS_YAML).unwrap();

        assert_eq!(config.log_level, default_log_level(), "log_level drift");
        assert_eq!(
            config.extensions_dir,
            default_extensions_dir(),
            "extensions_dir drift"
        );
        assert_eq!(config.log_format, LogFormat::Pretty, "log_format drift");

        assert_eq!(
            config.agents_manager.acp_timeout_secs,
            default_acp_timeout_secs(),
            "acp_timeout_secs drift"
        );
        assert_eq!(
            config.agents_manager.shutdown_grace_ms,
            default_shutdown_grace_ms(),
            "shutdown_grace_ms drift"
        );

        assert_eq!(
            config.channels_manager.init_timeout_secs,
            default_init_timeout_secs(),
            "init_timeout_secs drift"
        );
        assert_eq!(
            config.channels_manager.exit_timeout_secs,
            default_exit_timeout_secs(),
            "exit_timeout_secs drift"
        );

        assert_eq!(
            config.tools_manager.tools_server_host,
            default_tools_server_host(),
            "tools_server_host drift"
        );

        assert_eq!(
            config.supervisor.shutdown_timeout_secs,
            default_shutdown_timeout(),
            "shutdown_timeout_secs drift"
        );
        assert_eq!(
            config.supervisor.health_check_interval_secs,
            default_health_interval(),
            "health_check_interval_secs drift"
        );
        assert_eq!(
            config.supervisor.max_restarts,
            default_max_restarts(),
            "max_restarts drift"
        );
        assert_eq!(
            config.supervisor.restart_window_secs,
            default_restart_window(),
            "restart_window_secs drift"
        );
        assert_eq!(
            config.supervisor.admin_port,
            default_admin_port(),
            "admin_port drift"
        );
    }

    #[test]
    fn when_defaults_yaml_deserialized_alone_then_all_fixed_path_fields_populated() {
        let config: AnyclawConfig = serde_yaml::from_str(DEFAULTS_YAML).unwrap();

        assert!(
            !config.log_level.is_empty(),
            "log_level should not be empty"
        );
        assert!(
            !config.extensions_dir.is_empty(),
            "extensions_dir should not be empty"
        );

        assert!(
            config.agents_manager.acp_timeout_secs > 0,
            "acp_timeout_secs should be > 0"
        );
        assert!(
            config.agents_manager.shutdown_grace_ms > 0,
            "shutdown_grace_ms should be > 0"
        );

        assert!(
            config.channels_manager.init_timeout_secs > 0,
            "init_timeout_secs should be > 0"
        );
        assert!(
            config.channels_manager.exit_timeout_secs > 0,
            "exit_timeout_secs should be > 0"
        );

        assert!(
            !config.tools_manager.tools_server_host.is_empty(),
            "tools_server_host should not be empty"
        );

        assert!(
            config.supervisor.shutdown_timeout_secs > 0,
            "shutdown_timeout_secs should be > 0"
        );
        assert!(
            config.supervisor.health_check_interval_secs > 0,
            "health_check_interval_secs should be > 0"
        );
        assert!(
            config.supervisor.max_restarts > 0,
            "max_restarts should be > 0"
        );
        assert!(
            config.supervisor.restart_window_secs > 0,
            "restart_window_secs should be > 0"
        );
        assert!(config.supervisor.admin_port > 0, "admin_port should be > 0");
    }

    #[test]
    fn when_wasm_sandbox_config_defaulted_then_has_expected_limits() {
        let config = WasmSandboxConfig::default();
        assert_eq!(config.fuel_limit, 1_000_000);
        assert_eq!(config.epoch_timeout_secs, 30);
        assert_eq!(config.memory_limit_bytes, 67_108_864);
        assert!(config.preopened_dirs.is_empty());
    }

    #[test]
    fn when_agent_config_has_only_workspace_then_all_optionals_absent() {
        let yaml = r#"
workspace:
  type: local
  binary: "test-agent"
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert!(config.tools.is_empty());
        assert!(config.acp_timeout_secs.is_none());
        assert!(config.backoff.is_none());
        assert!(config.crash_tracker.is_none());
        assert!(config.options.is_empty());
    }

    #[test]
    fn when_agent_config_has_options_map_then_parses_to_json_values() {
        let yaml = r#"
workspace:
  type: local
  binary: "test-agent"
options:
  thinking: true
  verbose: false
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.options["thinking"], serde_json::json!(true));
        assert_eq!(config.options["verbose"], serde_json::json!(false));
    }

    #[test]
    fn when_multiple_agents_present_then_default_agent_name_returns_first_enabled() {
        let yaml = r#"
agents_manager:
  agents:
    disabled-one:
      workspace:
        type: local
        binary: "x"
      enabled: false
    enabled-one:
      workspace:
        type: local
        binary: "y"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.default_agent_name(), Some("enabled-one"));
    }

    #[test]
    fn when_no_agents_configured_then_default_agent_name_returns_none() {
        let yaml = "";
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.default_agent_name(), None);
    }

    #[test]
    fn when_manager_keys_are_snake_case_then_parses_correctly() {
        let yaml = r#"
agents_manager:
  agents:
    default:
      workspace:
        type: local
        binary: "opencode"
channels_manager:
  channels:
    debug-http:
      binary: "debug-http"
tools_manager:
  tools:
    filesystem:
      binary: "mcp-server-filesystem"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert_eq!(config.tools_manager.tools.len(), 1);
    }

    #[test]
    fn when_manager_keys_are_hyphenated_then_fields_are_silently_ignored() {
        let yaml = r#"
agents-manager:
  agents:
    default:
      workspace:
        type: local
        binary: "opencode"
channels-manager:
  channels:
    debug-http:
      binary: "debug-http"
tools-manager:
  tools:
    filesystem:
      binary: "mcp-server-filesystem"
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        // Hyphenated keys are unknown to serde — silently ignored, fields get defaults
        assert_eq!(config.agents_manager.agents.len(), 0);
        assert_eq!(config.channels_manager.channels.len(), 0);
        assert_eq!(config.tools_manager.tools.len(), 0);
    }

    #[test]
    fn when_pull_policy_empty_then_defaults_to_if_not_present() {
        let policy: PullPolicy = serde_yaml::from_str("").unwrap();
        assert_eq!(policy, PullPolicy::IfNotPresent);
    }

    #[test]
    fn when_pull_policy_in_yaml_then_parses_all_variants() {
        let always: PullPolicy = serde_yaml::from_str("always").unwrap();
        assert_eq!(always, PullPolicy::Always);
        let never: PullPolicy = serde_yaml::from_str("never").unwrap();
        assert_eq!(never, PullPolicy::Never);
        let ifnp: PullPolicy = serde_yaml::from_str("if_not_present").unwrap();
        assert_eq!(ifnp, PullPolicy::IfNotPresent);
    }

    #[test]
    fn when_workspace_type_is_local_then_parses_binary_working_dir_env() {
        let yaml = r#"
type: local
binary: "opencode"
working_dir: "/tmp"
env:
  MY_KEY: "val"
"#;
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Local(local) => {
                assert_eq!(local.binary, StringOrArray(vec!["opencode".into()]));
                assert_eq!(local.working_dir, Some(PathBuf::from("/tmp")));
                assert_eq!(local.env["MY_KEY"], "val");
            }
            _ => panic!("expected Local variant"),
        }
    }

    #[test]
    fn when_workspace_type_is_local_minimal_then_optional_fields_absent() {
        let yaml = "type: local\nbinary: \"agent\"";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Local(local) => {
                assert_eq!(local.binary, StringOrArray(vec!["agent".into()]));
                assert!(local.working_dir.is_none());
                assert!(local.env.is_empty());
            }
            _ => panic!("expected Local variant"),
        }
    }

    #[test]
    fn when_full_config_yaml_parsed_then_all_sections_populated() {
        let yaml = r#"
log_level: "info"
extensions_dir: "/usr/local/bin"

agents_manager:
  agents:
    opencode:
      workspace:
        type: local
        binary: "@built-in/agents/acp-bridge"
        env:
          OPENCODE_API_KEY: "test"
      tools:
        - "system-info"

channels_manager:
  debounce:
    enabled: true
    window_ms: 1000
  channels:
    telegram:
      binary: "@built-in/channels/telegram"
      agent: "opencode"
      options:
        ack:
          reaction: true
          typing: true
    debug-http:
      binary: "@built-in/channels/debug-http"

tools_manager:
  tools:
    system-info:
      binary: "@built-in/tools/system-info"

supervisor:
  shutdown_timeout_secs: 15
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 2);
        assert_eq!(config.tools_manager.tools.len(), 1);
        assert_eq!(config.supervisor.shutdown_timeout_secs, 15);
        let tg = &config.channels_manager.channels["telegram"];
        let ack: AckConfig =
            serde_json::from_value(tg.options["ack"].clone()).expect("ack in options");
        assert!(ack.reaction);
        assert_eq!(tg.agent, "opencode");
    }

    #[test]
    fn when_backoff_config_defaulted_then_has_expected_delays() {
        let config = BackoffConfig::default();
        assert_eq!(config.base_delay_ms, 100);
        assert_eq!(config.max_delay_secs, 30);
    }

    #[test]
    fn when_backoff_config_in_yaml_then_uses_provided_values() {
        let yaml = "base_delay_ms: 200\nmax_delay_secs: 60";
        let config: BackoffConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.base_delay_ms, 200);
        assert_eq!(config.max_delay_secs, 60);
    }

    #[test]
    fn when_backoff_config_partially_set_then_unset_fields_use_defaults() {
        let yaml = "base_delay_ms: 500";
        let config: BackoffConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_secs, 30);
    }

    #[test]
    fn when_crash_tracker_config_defaulted_then_has_expected_limits() {
        let config = CrashTrackerConfig::default();
        assert_eq!(config.max_crashes, 5);
        assert_eq!(config.window_secs, 60);
    }

    #[test]
    fn when_crash_tracker_config_in_yaml_then_uses_provided_values() {
        let yaml = "max_crashes: 10\nwindow_secs: 120";
        let config: CrashTrackerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.max_crashes, 10);
        assert_eq!(config.window_secs, 120);
    }

    #[test]
    fn when_agents_manager_config_defaulted_then_has_expected_timeouts() {
        let config = AgentsManagerConfig::default();
        assert_eq!(config.acp_timeout_secs, 30);
        assert_eq!(config.shutdown_grace_ms, 100);
        assert!(config.agents.is_empty());
    }

    #[test]
    fn when_channels_manager_config_defaulted_then_has_expected_timeout() {
        let config = ChannelsManagerConfig::default();
        assert_eq!(config.init_timeout_secs, 10);
        assert_eq!(config.exit_timeout_secs, 5);
        assert!(config.channels.is_empty());
    }

    #[test]
    fn when_agent_has_per_agent_timeout_and_backoff_then_all_parsed() {
        let yaml = r#"
agents_manager:
  agents:
    slow-agent:
      workspace:
        type: local
        binary: "slow"
      acp_timeout_secs: 60
      backoff:
        base_delay_ms: 500
        max_delay_secs: 120
      crash_tracker:
        max_crashes: 10
        window_secs: 300
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        let agent = &config.agents_manager.agents["slow-agent"];
        assert_eq!(agent.acp_timeout_secs, Some(60));
        let backoff = agent.backoff.as_ref().unwrap();
        assert_eq!(backoff.base_delay_ms, 500);
        assert_eq!(backoff.max_delay_secs, 120);
        let ct = agent.crash_tracker.as_ref().unwrap();
        assert_eq!(ct.max_crashes, 10);
        assert_eq!(ct.window_secs, 300);
    }

    #[test]
    fn when_channel_has_per_channel_timeout_and_backoff_then_all_parsed() {
        let yaml = r#"
channels_manager:
  channels:
    flaky-channel:
      binary: "flaky"
      init_timeout_secs: 30
      backoff:
        base_delay_ms: 200
      crash_tracker:
        max_crashes: 3
"#;
        let config: AnyclawConfig = serde_yaml::from_str(yaml).unwrap();
        let ch = &config.channels_manager.channels["flaky-channel"];
        assert_eq!(ch.init_timeout_secs, Some(30));
        let backoff = ch.backoff.as_ref().unwrap();
        assert_eq!(backoff.base_delay_ms, 200);
        assert_eq!(backoff.max_delay_secs, 30);
        let ct = ch.crash_tracker.as_ref().unwrap();
        assert_eq!(ct.max_crashes, 3);
        assert_eq!(ct.window_secs, 60);
    }

    #[test]
    fn when_channel_config_minimal_then_optional_override_fields_are_none() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.init_timeout_secs.is_none());
        assert!(config.exit_timeout_secs.is_none());
        assert!(config.backoff.is_none());
        assert!(config.crash_tracker.is_none());
    }

    #[test]
    fn when_workspace_type_is_docker_full_then_all_fields_parsed() {
        let yaml = r#"
type: docker
image: "anyclaw/opencode:latest"
entrypoint: "/usr/bin/opencode"
volumes:
  - "/workspace:/workspace"
  - "/tmp:/tmp:ro"
env:
  MODEL: "claude"
memory_limit: "512m"
cpu_limit: "1.5"
docker_host: "unix:///var/run/docker.sock"
network: "my-net"
pull_policy: always
extra_hosts:
  - "myhost:192.168.1.100"
  - "otherhost:10.0.0.1"
"#;
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(d.image, "anyclaw/opencode:latest");
                assert_eq!(d.entrypoint, Some("/usr/bin/opencode".into()));
                assert_eq!(d.volumes, vec!["/workspace:/workspace", "/tmp:/tmp:ro"]);
                assert_eq!(d.env["MODEL"], "claude");
                assert_eq!(d.memory_limit, Some("512m".into()));
                assert_eq!(d.cpu_limit, Some("1.5".into()));
                assert_eq!(d.docker_host, Some("unix:///var/run/docker.sock".into()));
                assert_eq!(d.network, Some("my-net".into()));
                assert_eq!(d.pull_policy, PullPolicy::Always);
                assert_eq!(
                    d.extra_hosts,
                    vec!["myhost:192.168.1.100", "otherhost:10.0.0.1"]
                );
            }
            _ => panic!("expected Docker variant"),
        }
    }

    #[test]
    fn when_workspace_type_is_docker_minimal_then_optional_fields_absent() {
        let yaml = "type: docker\nimage: \"my-agent:latest\"";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(d.image, "my-agent:latest");
                assert!(d.entrypoint.is_none());
                assert!(d.volumes.is_empty());
                assert!(d.env.is_empty());
                assert!(d.memory_limit.is_none());
                assert!(d.cpu_limit.is_none());
                assert!(d.docker_host.is_none());
                assert!(d.network.is_none());
                assert_eq!(d.pull_policy, PullPolicy::IfNotPresent);
                assert!(d.extra_hosts.is_empty());
            }
            _ => panic!("expected Docker variant"),
        }
    }

    #[test]
    fn when_preopened_dir_readonly_default_flipped_then_serde_defaults_to_true() {
        let yaml = "host: \"/tmp\"\nguest: \"/data\"";
        let dir: PreopenedDir = serde_yaml::from_str(yaml).unwrap();
        assert!(dir.readonly, "readonly should default to true");
    }

    #[test]
    fn when_agent_config_has_local_workspace_then_binary_and_env_parsed() {
        let yaml = r#"
workspace:
  type: local
  binary: "opencode"
  working_dir: "/tmp"
  env:
    MY_KEY: "val"
tools:
  - "system-info"
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.workspace {
            WorkspaceConfig::Local(local) => {
                assert_eq!(local.binary, StringOrArray(vec!["opencode".into()]));
                assert_eq!(local.working_dir, Some(PathBuf::from("/tmp")));
                assert_eq!(local.env["MY_KEY"], "val");
            }
            _ => panic!("expected Local variant"),
        }
        assert_eq!(config.tools, vec!["system-info"]);
        assert!(config.enabled);
    }

    #[test]
    fn when_agent_config_has_docker_workspace_then_image_and_limits_parsed() {
        let yaml = r#"
workspace:
  type: docker
  image: "anyclaw/opencode:latest"
  memory_limit: "512m"
  cpu_limit: "1.5"
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.workspace {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(d.image, "anyclaw/opencode:latest");
                assert_eq!(d.memory_limit, Some("512m".into()));
                assert_eq!(d.cpu_limit, Some("1.5".into()));
            }
            _ => panic!("expected Docker variant"),
        }
    }

    #[test]
    fn when_binary_is_string_then_parses_to_single_element_vec() {
        let yaml = "type: local\nbinary: \"opencode\"";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Local(local) => {
                assert_eq!(local.binary, StringOrArray(vec!["opencode".into()]));
                assert_eq!(local.binary.command_and_args(), ("opencode", &[][..]));
            }
            _ => panic!("expected Local variant"),
        }
    }

    #[test]
    fn when_binary_is_array_then_parses_to_multi_element_vec() {
        let yaml = "type: local\nbinary: [\"opencode\", \"acp\", \"--verbose\"]";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Local(local) => {
                assert_eq!(
                    local.binary,
                    StringOrArray(vec!["opencode".into(), "acp".into(), "--verbose".into()])
                );
                let (cmd, args) = local.binary.command_and_args();
                assert_eq!(cmd, "opencode");
                assert_eq!(args, &["acp", "--verbose"]);
            }
            _ => panic!("expected Local variant"),
        }
    }

    #[test]
    fn when_entrypoint_is_string_then_parses_to_single_element() {
        let yaml = "type: docker\nimage: \"my-agent:latest\"\nentrypoint: \"/usr/bin/agent\"";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(
                    d.entrypoint,
                    Some(StringOrArray(vec!["/usr/bin/agent".into()]))
                );
            }
            _ => panic!("expected Docker variant"),
        }
    }

    #[test]
    fn when_entrypoint_is_array_then_parses_to_multi_element() {
        let yaml =
            "type: docker\nimage: \"my-agent:latest\"\nentrypoint: [\"/usr/bin/agent\", \"serve\"]";
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(
                    d.entrypoint,
                    Some(StringOrArray(vec!["/usr/bin/agent".into(), "serve".into()]))
                );
            }
            _ => panic!("expected Docker variant"),
        }
    }

    #[test]
    fn when_string_or_array_serialized_single_then_emits_string() {
        let val = StringOrArray(vec!["opencode".into()]);
        let yaml = serde_yaml::to_string(&val).unwrap();
        assert!(yaml.contains("opencode"));
        assert!(!yaml.contains('['));
    }

    #[test]
    fn when_string_or_array_serialized_multi_then_emits_array() {
        let val = StringOrArray(vec!["opencode".into(), "acp".into()]);
        let yaml = serde_yaml::to_string(&val).unwrap();
        assert!(yaml.contains("opencode"));
        assert!(yaml.contains("acp"));
    }

    #[rstest]
    fn when_supervisor_has_permission_timeout_then_parses_to_some() {
        let yaml = "permission_timeout_secs: 30";
        let config: SupervisorConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.permission_timeout_secs, Some(30));
    }

    #[rstest]
    fn when_supervisor_has_no_permission_timeout_then_defaults_to_none() {
        let yaml = "";
        let config: SupervisorConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.permission_timeout_secs, None);
    }

    #[test]
    fn when_anyclaw_config_schema_generated_then_has_2020_12_dialect() {
        let schema = schemars::schema_for!(AnyclawConfig);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let schema_field = value.get("$schema").expect("schema has $schema field");
        assert!(
            schema_field.as_str().unwrap_or("").contains("2020-12"),
            "expected 2020-12 dialect, got: {schema_field}"
        );
    }

    #[test]
    fn when_anyclaw_config_schema_generated_then_has_expected_property_keys() {
        let schema = schemars::schema_for!(AnyclawConfig);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let properties = value
            .get("properties")
            .expect("schema has properties")
            .as_object()
            .expect("properties is an object");
        for key in &[
            "log_level",
            "log_format",
            "extensions_dir",
            "agents_manager",
            "channels_manager",
            "tools_manager",
            "supervisor",
            "session_store",
        ] {
            assert!(properties.contains_key(*key), "missing property: {key}");
        }
    }

    #[test]
    fn when_session_store_config_schema_generated_then_has_type_discriminator() {
        let schema = schemars::schema_for!(SessionStoreConfig);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let schema_str = serde_json::to_string(&value).expect("serializes to string");
        assert!(
            schema_str.contains("\"type\""),
            "schema should reference discriminator 'type': {schema_str}"
        );
    }

    #[test]
    fn when_workspace_config_schema_generated_then_has_type_discriminator() {
        let schema = schemars::schema_for!(WorkspaceConfig);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let schema_str = serde_json::to_string(&value).expect("serializes to string");
        assert!(
            schema_str.contains("\"type\""),
            "schema should reference discriminator 'type': {schema_str}"
        );
    }

    #[test]
    fn when_string_or_array_schema_generated_then_has_one_of_with_string_and_array() {
        let schema = schemars::schema_for!(StringOrArray);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let one_of = value
            .get("oneOf")
            .expect("StringOrArray schema has oneOf")
            .as_array()
            .expect("oneOf is an array");
        assert_eq!(one_of.len(), 2, "oneOf should have exactly 2 entries");
        let types: Vec<&str> = one_of
            .iter()
            .filter_map(|v| v.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(types.contains(&"string"), "oneOf should contain string");
        assert!(types.contains(&"array"), "oneOf should contain array");
    }

    #[test]
    fn when_pull_policy_schema_generated_then_is_string_type_with_enum_variants() {
        let schema = schemars::schema_for!(PullPolicy);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        assert_eq!(
            value.get("type").and_then(|t| t.as_str()),
            Some("string"),
            "PullPolicy schema type should be string"
        );
        let enum_values = value
            .get("enum")
            .expect("PullPolicy schema has enum")
            .as_array()
            .expect("enum is array");
        let variants: Vec<&str> = enum_values.iter().filter_map(|v| v.as_str()).collect();
        assert!(variants.contains(&"always"), "enum should contain 'always'");
        assert!(variants.contains(&"never"), "enum should contain 'never'");
        assert!(
            variants.contains(&"if_not_present"),
            "enum should contain 'if_not_present'"
        );
    }

    #[test]
    fn when_pull_policy_schema_generated_then_has_default_if_not_present() {
        let schema = schemars::schema_for!(PullPolicy);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        assert_eq!(
            value.get("default").and_then(|d| d.as_str()),
            Some("if_not_present"),
            "PullPolicy schema default should be 'if_not_present'"
        );
    }

    #[test]
    fn when_local_workspace_config_schema_generated_then_binary_field_references_string_or_array() {
        let schema = schemars::schema_for!(LocalWorkspaceConfig);
        let value = serde_json::to_value(&schema).expect("schema serializes");
        let schema_str = serde_json::to_string(&value).expect("serializes to string");
        assert!(
            schema_str.contains("StringOrArray"),
            "LocalWorkspaceConfig schema should reference StringOrArray: {schema_str}"
        );
    }
}
