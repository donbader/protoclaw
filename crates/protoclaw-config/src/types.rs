use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Embedded defaults YAML — loaded as base layer in Figment.
pub const DEFAULTS_YAML: &str = include_str!("defaults.yaml");

/// Output format for tracing/log output.
///
/// `pretty` is the default and suitable for development. `json` emits
/// structured JSON lines, which is preferable in production environments
/// where log aggregators (e.g., Datadog, CloudWatch) ingest structured output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
}

/// Top-level protoclaw configuration.
///
/// Loaded from layered providers: defaults → YAML file → environment variables.
/// Manager-hierarchy: each manager owns its children as named maps.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoclawConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub log_format: LogFormat,
    #[serde(default = "default_extensions_dir")]
    pub extensions_dir: String,
    #[serde(alias = "agents-manager", default)]
    pub agents_manager: AgentsManagerConfig,
    #[serde(alias = "channels-manager", default)]
    pub channels_manager: ChannelsManagerConfig,
    #[serde(alias = "tools-manager", default)]
    pub tools_manager: ToolsManagerConfig,
    #[serde(default)]
    pub supervisor: SupervisorConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackoffConfig {
    #[serde(default = "default_backoff_base_ms")]
    pub base_delay_ms: u64,
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
#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentsManagerConfig {
    #[serde(default = "default_acp_timeout_secs")]
    pub acp_timeout_secs: u64,
    #[serde(default = "default_shutdown_grace_ms")]
    pub shutdown_grace_ms: u64,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
}

impl Default for AgentsManagerConfig {
    fn default() -> Self {
        Self {
            acp_timeout_secs: default_acp_timeout_secs(),
            shutdown_grace_ms: default_shutdown_grace_ms(),
            agents: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelsManagerConfig {
    #[serde(default = "default_init_timeout_secs")]
    pub init_timeout_secs: u64,
    #[serde(default = "default_exit_timeout_secs")]
    pub exit_timeout_secs: u64,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolsManagerConfig {
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
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

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub enum PullPolicy {
    Always,
    #[default]
    IfNotPresent,
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LocalWorkspaceConfig {
    pub binary: String,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DockerWorkspaceConfig {
    pub image: String,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub memory_limit: Option<String>,
    #[serde(default)]
    pub cpu_limit: Option<String>,
    #[serde(default)]
    pub docker_host: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub pull_policy: PullPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceConfig {
    Local(LocalWorkspaceConfig),
    Docker(DockerWorkspaceConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub acp_timeout_secs: Option<u64>,
    #[serde(default)]
    pub backoff: Option<BackoffConfig>,
    #[serde(default)]
    pub crash_tracker: Option<CrashTrackerConfig>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default)]
    pub ack: AckConfig,
    #[serde(default)]
    pub init_timeout_secs: Option<u64>,
    #[serde(default)]
    pub exit_timeout_secs: Option<u64>,
    #[serde(default)]
    pub backoff: Option<BackoffConfig>,
    #[serde(default)]
    pub crash_tracker: Option<CrashTrackerConfig>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

/// What happens to the reaction emoji after the agent finishes responding.
///
/// - `remove`: the reaction is deleted once the response is sent
/// - `replace_done`: the in-progress reaction is swapped for a "done" checkmark emoji
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReactionLifecycle {
    #[default]
    Remove,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AckConfig {
    #[serde(default)]
    pub reaction: bool,
    #[serde(default)]
    pub typing: bool,
    #[serde(default = "default_reaction_emoji")]
    pub reaction_emoji: String,
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

impl From<AckConfig> for protoclaw_sdk_types::ChannelAckConfig {
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    #[default]
    Mcp,
    Wasm,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolConfig {
    #[serde(default)]
    pub tool_type: ToolType,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub module: Option<PathBuf>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: Option<String>,
    #[serde(default)]
    pub sandbox: WasmSandboxConfig,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmSandboxConfig {
    #[serde(default = "default_fuel_limit")]
    pub fuel_limit: u64,
    #[serde(default = "default_epoch_timeout")]
    pub epoch_timeout_secs: u64,
    #[serde(default = "default_memory_limit")]
    pub memory_limit_bytes: u64,
    #[serde(default)]
    pub preopened_dirs: Vec<PreopenedDir>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PreopenedDir {
    pub host: PathBuf,
    pub guest: String,
    #[serde(default)]
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupervisorConfig {
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    /// Maximum number of manager restart attempts within `restart_window_secs`.
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    /// Rolling window in seconds over which manager crash attempts are counted.
    #[serde(default = "default_restart_window")]
    pub restart_window_secs: u64,
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
    "info".into()
}
fn default_extensions_dir() -> String {
    "/usr/local/bin".into()
}
fn default_true() -> bool {
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
fn default_acp_timeout_secs() -> u64 {
    30
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
        }
    }
}

impl ProtoclawConfig {
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

    #[test]
    fn when_no_log_level_set_then_defaults_to_info() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn when_log_level_in_yaml_then_uses_provided_value() {
        let yaml = "log_level: \"debug\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn when_no_extensions_dir_set_then_defaults_to_usr_local_bin() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.extensions_dir, "/usr/local/bin");
    }

    #[test]
    fn when_extensions_dir_in_yaml_then_uses_provided_path() {
        let yaml = "extensions_dir: \"/custom/path\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
      args:
        - "acp"
      tools:
        - "system-info"
        - "filesystem"
    claude-code:
      workspace:
        type: local
        binary: "claude"
      enabled: false
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 2);
        let oc = &config.agents_manager.agents["opencode"];
        if let WorkspaceConfig::Local(ref local) = oc.workspace {
            assert_eq!(local.binary, "opencode");
            assert_eq!(local.env["ANTHROPIC_API_KEY"], "sk-test");
        } else {
            panic!("expected Local workspace");
        }
        assert_eq!(oc.args, vec!["acp"]);
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
    fn when_channel_has_nested_ack_config_then_parses_correctly() {
        let yaml = r#"
channels_manager:
  channels:
    telegram:
      binary: "telegram-channel"
      agent: "opencode"
      ack:
        reaction: true
        typing: true
        reaction_emoji: "👀"
        reaction_lifecycle: "remove"
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        let tg = &config.channels_manager.channels["telegram"];
        assert!(tg.ack.reaction);
        assert!(tg.ack.typing);
        assert_eq!(tg.ack.reaction_emoji, "👀");
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, LogFormat::Pretty);
    }

    #[test]
    fn when_log_format_in_yaml_then_uses_provided_value() {
        let yaml = "log_format: \"json\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, LogFormat::Json);
    }

    #[test]
    fn when_parsing_defaults_yaml_then_all_expected_values_present() {
        let config: ProtoclawConfig = serde_yaml::from_str(DEFAULTS_YAML).unwrap();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.log_format, LogFormat::Pretty);
        assert_eq!(config.extensions_dir, "/usr/local/bin");
        assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
        assert_eq!(config.agents_manager.acp_timeout_secs, 30);
        assert_eq!(config.agents_manager.shutdown_grace_ms, 100);
        assert_eq!(config.channels_manager.init_timeout_secs, 10);
        assert_eq!(config.channels_manager.exit_timeout_secs, 5);
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
        assert!(config.args.is_empty());
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.default_agent_name(), Some("enabled-one"));
    }

    #[test]
    fn when_no_agents_configured_then_default_agent_name_returns_none() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert_eq!(config.tools_manager.tools.len(), 1);
    }

    #[test]
    fn when_manager_keys_are_hyphenated_then_backward_compat_alias_parses_correctly() {
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert_eq!(config.tools_manager.tools.len(), 1);
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
                assert_eq!(local.binary, "opencode");
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
                assert_eq!(local.binary, "agent");
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
        binary: "@built-in/agents/opencode-wrapper"
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 2);
        assert_eq!(config.tools_manager.tools.len(), 1);
        assert_eq!(config.supervisor.shutdown_timeout_secs, 15);
        let tg = &config.channels_manager.channels["telegram"];
        assert!(tg.ack.reaction);
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
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
image: "protoclaw/opencode:latest"
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
"#;
        let ws: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        match ws {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(d.image, "protoclaw/opencode:latest");
                assert_eq!(d.entrypoint, Some("/usr/bin/opencode".into()));
                assert_eq!(d.volumes, vec!["/workspace:/workspace", "/tmp:/tmp:ro"]);
                assert_eq!(d.env["MODEL"], "claude");
                assert_eq!(d.memory_limit, Some("512m".into()));
                assert_eq!(d.cpu_limit, Some("1.5".into()));
                assert_eq!(d.docker_host, Some("unix:///var/run/docker.sock".into()));
                assert_eq!(d.network, Some("my-net".into()));
                assert_eq!(d.pull_policy, PullPolicy::Always);
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
            }
            _ => panic!("expected Docker variant"),
        }
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
args:
  - "acp"
tools:
  - "system-info"
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.workspace {
            WorkspaceConfig::Local(local) => {
                assert_eq!(local.binary, "opencode");
                assert_eq!(local.working_dir, Some(PathBuf::from("/tmp")));
                assert_eq!(local.env["MY_KEY"], "val");
            }
            _ => panic!("expected Local variant"),
        }
        assert_eq!(config.args, vec!["acp"]);
        assert_eq!(config.tools, vec!["system-info"]);
        assert!(config.enabled);
    }

    #[test]
    fn when_agent_config_has_docker_workspace_then_image_and_limits_parsed() {
        let yaml = r#"
workspace:
  type: docker
  image: "protoclaw/opencode:latest"
  memory_limit: "512m"
  cpu_limit: "1.5"
args:
  - "acp"
"#;
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        match &config.workspace {
            WorkspaceConfig::Docker(d) => {
                assert_eq!(d.image, "protoclaw/opencode:latest");
                assert_eq!(d.memory_limit, Some("512m".into()));
                assert_eq!(d.cpu_limit, Some("1.5".into()));
            }
            _ => panic!("expected Docker variant"),
        }
        assert_eq!(config.args, vec!["acp"]);
    }
}
