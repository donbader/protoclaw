use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Embedded defaults YAML — loaded as base layer in Figment.
pub const DEFAULTS_YAML: &str = include_str!("defaults.yaml");

/// Top-level protoclaw configuration.
///
/// Loaded from layered providers: defaults → YAML file → environment variables.
/// Manager-hierarchy: each manager owns its children as named maps.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoclawConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_log_format")]
    pub log_format: String,
    #[serde(default = "default_extensions_dir")]
    pub extensions_dir: String,
    #[serde(rename = "agents-manager", default)]
    pub agents_manager: AgentsManagerConfig,
    #[serde(rename = "channels-manager", default)]
    pub channels_manager: ChannelsManagerConfig,
    #[serde(rename = "tools-manager", default)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrashTrackerConfig {
    #[serde(default = "default_crash_max")]
    pub max_crashes: u32,
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
    #[serde(default)]
    pub debounce: DebounceConfig,
    #[serde(default)]
    pub channels: HashMap<String, ChannelConfig>,
}

impl Default for ChannelsManagerConfig {
    fn default() -> Self {
        Self {
            init_timeout_secs: default_init_timeout_secs(),
            debounce: DebounceConfig::default(),
            channels: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ToolsManagerConfig {
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
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
    pub backoff: Option<BackoffConfig>,
    #[serde(default)]
    pub crash_tracker: Option<CrashTrackerConfig>,
    #[serde(default)]
    pub options: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AckConfig {
    #[serde(default)]
    pub reaction: bool,
    #[serde(default)]
    pub typing: bool,
    #[serde(default = "default_reaction_emoji")]
    pub reaction_emoji: String,
    #[serde(default = "default_reaction_lifecycle")]
    pub reaction_lifecycle: String,
}

impl Default for AckConfig {
    fn default() -> Self {
        Self {
            reaction: false,
            typing: false,
            reaction_emoji: default_reaction_emoji(),
            reaction_lifecycle: default_reaction_lifecycle(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebounceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_window_ms")]
    pub window_ms: u64,
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default = "default_mid_response")]
    pub mid_response: String,
}

impl Default for DebounceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_ms: default_window_ms(),
            separator: default_separator(),
            mid_response: default_mid_response(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolConfig {
    #[serde(default = "default_tool_type")]
    pub tool_type: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupervisorConfig {
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
    #[serde(default = "default_health_interval")]
    pub health_check_interval_secs: u64,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
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
fn default_log_format() -> String {
    "pretty".into()
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
fn default_reaction_lifecycle() -> String {
    "remove".into()
}
fn default_window_ms() -> u64 {
    1000
}
fn default_separator() -> String {
    "\n".into()
}
fn default_mid_response() -> String {
    "queue".into()
}
fn default_tool_type() -> String {
    "mcp".into()
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
    fn log_level_defaults_to_info() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn log_level_from_yaml() {
        let yaml = "log_level: \"debug\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn extensions_dir_defaults() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.extensions_dir, "/usr/local/bin");
    }

    #[test]
    fn extensions_dir_from_yaml() {
        let yaml = "extensions_dir: \"/custom/path\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.extensions_dir, "/custom/path");
    }

    #[test]
    fn agents_manager_named_map() {
        let yaml = r#"
agents-manager:
  agents:
    opencode:
      binary: "opencode"
      args:
        - "acp"
      tools:
        - "system-info"
        - "filesystem"
      env:
        ANTHROPIC_API_KEY: "sk-test"
    claude-code:
      binary: "claude"
      enabled: false
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 2);
        let oc = &config.agents_manager.agents["opencode"];
        assert_eq!(oc.binary, "opencode");
        assert_eq!(oc.args, vec!["acp"]);
        assert!(oc.enabled);
        assert_eq!(oc.tools, vec!["system-info", "filesystem"]);
        assert_eq!(oc.env["ANTHROPIC_API_KEY"], "sk-test");
        let cc = &config.agents_manager.agents["claude-code"];
        assert!(!cc.enabled);
    }

    #[test]
    fn channel_config_no_name_field() {
        let yaml = r#"
channels-manager:
  channels:
    debug-http:
      binary: "debug-http"
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert!(config.channels_manager.channels.contains_key("debug-http"));
    }

    #[test]
    fn channel_enabled_defaults_true() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn channel_enabled_false() {
        let yaml = "binary: \"telegram-channel\"\nenabled: false";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn channel_agent_defaults_to_default() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agent, "default");
    }

    #[test]
    fn channel_agent_from_yaml() {
        let yaml = "binary: \"telegram-channel\"\nagent: \"opencode\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.agent, "opencode");
    }

    #[test]
    fn ack_config_defaults() {
        let ack = AckConfig::default();
        assert!(!ack.reaction);
        assert!(!ack.typing);
        assert_eq!(ack.reaction_emoji, "👀");
        assert_eq!(ack.reaction_lifecycle, "remove");
    }

    #[test]
    fn ack_config_from_yaml() {
        let yaml = "reaction: true\ntyping: true\nreaction_emoji: \"⏳\"\nreaction_lifecycle: \"replace_done\"";
        let config: AckConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.reaction);
        assert!(config.typing);
        assert_eq!(config.reaction_emoji, "⏳");
        assert_eq!(config.reaction_lifecycle, "replace_done");
    }

    #[test]
    fn channel_ack_nested() {
        let yaml = r#"
channels-manager:
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
    fn debounce_config_defaults() {
        let debounce = DebounceConfig::default();
        assert!(debounce.enabled);
        assert_eq!(debounce.window_ms, 1000);
        assert_eq!(debounce.separator, "\n");
        assert_eq!(debounce.mid_response, "queue");
    }

    #[test]
    fn debounce_config_from_yaml() {
        let yaml = "enabled: false\nwindow_ms: 2000\nseparator: \" \"\nmid_response: \"cancel\"";
        let config: DebounceConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.window_ms, 2000);
        assert_eq!(config.separator, " ");
        assert_eq!(config.mid_response, "cancel");
    }

    #[test]
    fn tool_config_mcp_type() {
        let yaml = r#"
tools-manager:
  tools:
    filesystem:
      binary: "mcp-server-filesystem"
      args:
        - "--root"
        - "/workspace"
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        let fs = &config.tools_manager.tools["filesystem"];
        assert_eq!(fs.tool_type, "mcp");
        assert_eq!(fs.binary, Some("mcp-server-filesystem".into()));
        assert!(fs.enabled);
    }

    #[test]
    fn tool_config_wasm_type() {
        let yaml = r#"
tools-manager:
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
        assert_eq!(t.tool_type, "wasm");
        assert_eq!(t.module, Some(PathBuf::from("/path/to/tool.wasm")));
        assert_eq!(t.description, "A test tool");
        assert_eq!(t.sandbox.fuel_limit, 500_000);
        assert_eq!(t.sandbox.epoch_timeout_secs, 10);
    }

    #[test]
    fn tool_config_name_not_in_struct() {
        let yaml = "binary: \"mcp-server-filesystem\"";
        let config: ToolConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.binary, Some("mcp-server-filesystem".into()));
    }

    #[test]
    fn log_format_defaults_to_pretty() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, "pretty");
    }

    #[test]
    fn log_format_from_yaml() {
        let yaml = "log_format: \"json\"";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.log_format, "json");
    }

    #[test]
    fn defaults_yaml_parses() {
        let config: ProtoclawConfig = serde_yaml::from_str(DEFAULTS_YAML).unwrap();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.log_format, "pretty");
        assert_eq!(config.extensions_dir, "/usr/local/bin");
        assert!(config.channels_manager.debounce.enabled);
        assert_eq!(config.channels_manager.debounce.window_ms, 1000);
        assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
        assert_eq!(config.agents_manager.acp_timeout_secs, 30);
        assert_eq!(config.agents_manager.shutdown_grace_ms, 100);
        assert_eq!(config.channels_manager.init_timeout_secs, 10);
    }

    #[test]
    fn wasm_sandbox_config_defaults() {
        let config = WasmSandboxConfig::default();
        assert_eq!(config.fuel_limit, 1_000_000);
        assert_eq!(config.epoch_timeout_secs, 30);
        assert_eq!(config.memory_limit_bytes, 67_108_864);
        assert!(config.preopened_dirs.is_empty());
    }

    #[test]
    fn agent_config_defaults() {
        let yaml = "binary: \"test-agent\"";
        let config: AgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert!(config.tools.is_empty());
        assert!(config.env.is_empty());
        assert!(config.args.is_empty());
        assert!(config.acp_timeout_secs.is_none());
        assert!(config.backoff.is_none());
        assert!(config.crash_tracker.is_none());
        assert!(config.options.is_empty());
    }

    #[test]
    fn agent_config_options_from_yaml() {
        let yaml = r#"
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
    fn default_agent_name_returns_first_enabled() {
        let yaml = r#"
agents-manager:
  agents:
    disabled-one:
      binary: "x"
      enabled: false
    enabled-one:
      binary: "y"
"#;
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.default_agent_name(), Some("enabled-one"));
    }

    #[test]
    fn default_agent_name_none_when_empty() {
        let yaml = "";
        let config: ProtoclawConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.default_agent_name(), None);
    }

    #[test]
    fn full_config_shape() {
        let yaml = r#"
log_level: "info"
extensions_dir: "/usr/local/bin"

agents-manager:
  agents:
    opencode:
      binary: "@built-in/opencode"
      tools:
        - "system-info"
      env:
        OPENCODE_API_KEY: "test"

channels-manager:
  debounce:
    enabled: true
    window_ms: 1000
  channels:
    telegram:
      binary: "@built-in/telegram-channel"
      agent: "opencode"
      ack:
        reaction: true
        typing: true
    debug-http:
      binary: "@built-in/debug-http"

tools-manager:
  tools:
    system-info:
      binary: "@built-in/system-info"

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
    fn backoff_config_defaults() {
        let config = BackoffConfig::default();
        assert_eq!(config.base_delay_ms, 100);
        assert_eq!(config.max_delay_secs, 30);
    }

    #[test]
    fn backoff_config_from_yaml() {
        let yaml = "base_delay_ms: 200\nmax_delay_secs: 60";
        let config: BackoffConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.base_delay_ms, 200);
        assert_eq!(config.max_delay_secs, 60);
    }

    #[test]
    fn backoff_config_partial_yaml_uses_defaults() {
        let yaml = "base_delay_ms: 500";
        let config: BackoffConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.base_delay_ms, 500);
        assert_eq!(config.max_delay_secs, 30);
    }

    #[test]
    fn crash_tracker_config_defaults() {
        let config = CrashTrackerConfig::default();
        assert_eq!(config.max_crashes, 5);
        assert_eq!(config.window_secs, 60);
    }

    #[test]
    fn crash_tracker_config_from_yaml() {
        let yaml = "max_crashes: 10\nwindow_secs: 120";
        let config: CrashTrackerConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.max_crashes, 10);
        assert_eq!(config.window_secs, 120);
    }

    #[test]
    fn agents_manager_config_defaults() {
        let config = AgentsManagerConfig::default();
        assert_eq!(config.acp_timeout_secs, 30);
        assert_eq!(config.shutdown_grace_ms, 100);
        assert!(config.agents.is_empty());
    }

    #[test]
    fn channels_manager_config_defaults() {
        let config = ChannelsManagerConfig::default();
        assert_eq!(config.init_timeout_secs, 10);
        assert!(config.channels.is_empty());
    }

    #[test]
    fn per_agent_override_parses() {
        let yaml = r#"
agents-manager:
  agents:
    slow-agent:
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
    fn per_channel_override_parses() {
        let yaml = r#"
channels-manager:
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
    fn channel_config_option_fields_default_none() {
        let yaml = "binary: \"debug-http\"";
        let config: ChannelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.init_timeout_secs.is_none());
        assert!(config.backoff.is_none());
        assert!(config.crash_tracker.is_none());
    }
}
