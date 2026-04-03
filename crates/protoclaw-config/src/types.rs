use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Embedded defaults TOML — loaded as base layer in Figment.
pub const DEFAULTS_TOML: &str = include_str!("defaults.toml");

/// Top-level protoclaw configuration.
///
/// Loaded from layered providers: defaults → TOML file → environment variables.
/// Manager-hierarchy: each manager owns its children as named maps.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtoclawConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
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

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AgentsManagerConfig {
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelsManagerConfig {
    #[serde(default)]
    pub debounce: DebounceConfig,
    #[serde(default)]
    pub channels: HashMap<String, ChannelConfig>,
}

impl Default for ChannelsManagerConfig {
    fn default() -> Self {
        Self {
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
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default)]
    pub tools: Vec<String>,
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
        let toml = r#""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn log_level_from_toml() {
        let toml = r#"log_level = "debug""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn extensions_dir_defaults() {
        let toml = r#""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.extensions_dir, "/usr/local/bin");
    }

    #[test]
    fn extensions_dir_from_toml() {
        let toml = r#"extensions_dir = "/custom/path""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.extensions_dir, "/custom/path");
    }

    #[test]
    fn agents_manager_named_map() {
        let toml = r#"
            [agents-manager.agents.opencode]
            binary = "opencode"
            args = ["acp"]
            tools = ["system-info", "filesystem"]

            [agents-manager.agents.opencode.env]
            ANTHROPIC_API_KEY = "sk-test"

            [agents-manager.agents.claude-code]
            binary = "claude"
            enabled = false
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
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
        let toml = r#"
            [channels-manager.channels.debug-http]
            binary = "debug-http"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.channels_manager.channels.len(), 1);
        assert!(config.channels_manager.channels.contains_key("debug-http"));
    }

    #[test]
    fn channel_enabled_defaults_true() {
        let toml = r#"
            binary = "debug-http"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn channel_enabled_false() {
        let toml = r#"
            binary = "telegram-channel"
            enabled = false
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert!(!config.enabled);
    }

    #[test]
    fn channel_agent_defaults_to_default() {
        let toml = r#"
            binary = "debug-http"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.agent, "default");
    }

    #[test]
    fn channel_agent_from_toml() {
        let toml = r#"
            binary = "telegram-channel"
            agent = "opencode"
        "#;
        let config: ChannelConfig = toml::from_str(toml).unwrap();
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
    fn ack_config_from_toml() {
        let toml = r#"
            reaction = true
            typing = true
            reaction_emoji = "⏳"
            reaction_lifecycle = "replace_done"
        "#;
        let config: AckConfig = toml::from_str(toml).unwrap();
        assert!(config.reaction);
        assert!(config.typing);
        assert_eq!(config.reaction_emoji, "⏳");
        assert_eq!(config.reaction_lifecycle, "replace_done");
    }

    #[test]
    fn channel_ack_nested() {
        let toml = r#"
            [channels-manager.channels.telegram]
            binary = "telegram-channel"
            agent = "opencode"
            [channels-manager.channels.telegram.ack]
            reaction = true
            typing = true
            reaction_emoji = "👀"
            reaction_lifecycle = "remove"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
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
    fn debounce_config_from_toml() {
        let toml = r#"
            enabled = false
            window_ms = 2000
            separator = " "
            mid_response = "cancel"
        "#;
        let config: DebounceConfig = toml::from_str(toml).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.window_ms, 2000);
        assert_eq!(config.separator, " ");
        assert_eq!(config.mid_response, "cancel");
    }

    #[test]
    fn tool_config_mcp_type() {
        let toml = r#"
            [tools-manager.tools.filesystem]
            binary = "mcp-server-filesystem"
            args = ["--root", "/workspace"]
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        let fs = &config.tools_manager.tools["filesystem"];
        assert_eq!(fs.tool_type, "mcp");
        assert_eq!(fs.binary, Some("mcp-server-filesystem".into()));
        assert!(fs.enabled);
    }

    #[test]
    fn tool_config_wasm_type() {
        let toml = r#"
            [tools-manager.tools.my-tool]
            tool_type = "wasm"
            module = "/path/to/tool.wasm"
            description = "A test tool"
            input_schema = '{"type": "object"}'
            [tools-manager.tools.my-tool.sandbox]
            fuel_limit = 500000
            epoch_timeout_secs = 10
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        let t = &config.tools_manager.tools["my-tool"];
        assert_eq!(t.tool_type, "wasm");
        assert_eq!(t.module, Some(PathBuf::from("/path/to/tool.wasm")));
        assert_eq!(t.description, "A test tool");
        assert_eq!(t.sandbox.fuel_limit, 500_000);
        assert_eq!(t.sandbox.epoch_timeout_secs, 10);
    }

    #[test]
    fn tool_config_name_not_in_struct() {
        let toml = r#"
            binary = "mcp-server-filesystem"
        "#;
        let config: ToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.binary, Some("mcp-server-filesystem".into()));
    }

    #[test]
    fn defaults_toml_parses() {
        let config: ProtoclawConfig = toml::from_str(DEFAULTS_TOML).unwrap();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.extensions_dir, "/usr/local/bin");
        assert!(config.channels_manager.debounce.enabled);
        assert_eq!(config.channels_manager.debounce.window_ms, 1000);
        assert_eq!(config.supervisor.shutdown_timeout_secs, 30);
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
        let toml = r#"
            binary = "test-agent"
        "#;
        let config: AgentConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert!(config.tools.is_empty());
        assert!(config.env.is_empty());
        assert!(config.args.is_empty());
    }

    #[test]
    fn default_agent_name_returns_first_enabled() {
        let toml = r#"
            [agents-manager.agents.disabled-one]
            binary = "x"
            enabled = false

            [agents-manager.agents.enabled-one]
            binary = "y"
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.default_agent_name(), Some("enabled-one"));
    }

    #[test]
    fn default_agent_name_none_when_empty() {
        let toml = r#""#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.default_agent_name(), None);
    }

    #[test]
    fn full_config_shape() {
        let toml = r#"
            log_level = "info"
            extensions_dir = "/usr/local/bin"

            [agents-manager.agents.opencode]
            binary = "@built-in/opencode"
            tools = ["system-info"]
            [agents-manager.agents.opencode.env]
            OPENCODE_API_KEY = "test"

            [channels-manager.debounce]
            enabled = true
            window_ms = 1000

            [channels-manager.channels.telegram]
            binary = "@built-in/telegram-channel"
            agent = "opencode"
            [channels-manager.channels.telegram.ack]
            reaction = true
            typing = true

            [channels-manager.channels.debug-http]
            binary = "@built-in/debug-http"

            [tools-manager.tools.system-info]
            binary = "@built-in/system-info"

            [supervisor]
            shutdown_timeout_secs = 15
        "#;
        let config: ProtoclawConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.agents_manager.agents.len(), 1);
        assert_eq!(config.channels_manager.channels.len(), 2);
        assert_eq!(config.tools_manager.tools.len(), 1);
        assert_eq!(config.supervisor.shutdown_timeout_secs, 15);
        let tg = &config.channels_manager.channels["telegram"];
        assert!(tg.ack.reaction);
        assert_eq!(tg.agent, "opencode");
    }
}
