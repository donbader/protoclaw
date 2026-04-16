use crate::AnyclawConfig;
use std::path::Path;
use std::process::{Command, Stdio};

/// Outcome of [`validate_config`] — collects errors and warnings separately.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Hard errors that should prevent startup.
    pub errors: Vec<ValidationError>,
    /// Soft warnings that are informational but not blocking.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Returns `true` if there are no errors (warnings are allowed).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// A hard validation error that should prevent startup.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    /// A configured binary was not found on PATH or as an absolute path.
    #[error("{field}: binary '{binary}' not found on PATH or as absolute path")]
    BinaryNotFound {
        /// Config field path (e.g. `"agents_manager.agents.default.workspace.binary"`).
        field: String,
        /// The binary name or path that was not found.
        binary: String,
    },
    /// A configured working directory does not exist on disk.
    #[error("agent.working_dir: directory '{}' does not exist", path.display())]
    WorkingDirNotFound {
        /// The missing directory path.
        path: std::path::PathBuf,
    },
    /// A Docker memory limit string could not be parsed.
    #[error("{field}: invalid memory limit '{value}': {reason}")]
    InvalidMemoryLimit {
        /// Config field path.
        field: String,
        /// The raw memory limit string.
        value: String,
        /// Why parsing failed.
        reason: String,
    },
    /// A Docker CPU limit string could not be parsed.
    #[error("{field}: invalid cpu limit '{value}': {reason}")]
    InvalidCpuLimit {
        /// Config field path.
        field: String,
        /// The raw CPU limit string.
        value: String,
        /// Why parsing failed.
        reason: String,
    },
    /// A Docker host URI does not use a recognized scheme.
    #[error("{field}: invalid docker_host URI '{value}' (expected unix:// or tcp://)")]
    InvalidDockerHost {
        /// Config field path.
        field: String,
        /// The invalid URI.
        value: String,
    },
    /// A volume mount entry is missing the required `:` separator.
    #[error("{field}: volume entry '{value}' missing ':' separator")]
    InvalidVolumeMount {
        /// Config field path.
        field: String,
        /// The invalid volume string.
        value: String,
    },
    /// The tools server host is not a valid hostname or IP address.
    #[error("{field}: invalid hostname or IP '{value}'")]
    InvalidToolsServerHost {
        /// Config field path.
        field: String,
        /// The invalid host string.
        value: String,
    },
}

/// A soft validation warning (informational, does not block startup).
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    /// A binary was found at an absolute path but is not on the system PATH.
    BinaryNotOnPath {
        /// Config field path.
        field: String,
        /// The binary name.
        binary: String,
        /// Where the binary was found.
        found_at: String,
    },
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryNotOnPath {
                field,
                binary,
                found_at,
            } => {
                write!(
                    f,
                    "{field}: binary '{binary}' found at {found_at} but not on PATH"
                )
            }
        }
    }
}

fn binary_exists(binary: &str) -> bool {
    if Path::new(binary).is_absolute() {
        Path::new(binary).is_file()
    } else {
        Command::new(binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }
}

fn is_valid_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    // Accept raw IP addresses (IPv4 and IPv6).
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    // RFC 1123 hostname: total length ≤ 253, labels separated by '.', each label
    // 1–63 chars, only alphanumerics and hyphens, no leading/trailing hyphen.
    if host.len() > 253 {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

fn validate_tools_server_host(config: &AnyclawConfig, errors: &mut Vec<ValidationError>) {
    if !is_valid_host(&config.tools_manager.tools_server_host) {
        errors.push(ValidationError::InvalidToolsServerHost {
            field: "tools_manager.tools_server_host".to_string(),
            value: config.tools_manager.tools_server_host.clone(),
        });
    }
}

fn validate_local_agent(
    name: &str,
    local: &crate::LocalWorkspaceConfig,
    errors: &mut Vec<ValidationError>,
) {
    if !binary_exists(&local.binary.0[0]) {
        errors.push(ValidationError::BinaryNotFound {
            field: format!("agents_manager.agents.{name}.workspace.binary"),
            binary: local.binary.0[0].clone(),
        });
    }
    if let Some(path) = &local.working_dir
        && !path.exists()
    {
        errors.push(ValidationError::WorkingDirNotFound { path: path.clone() });
    }
}

fn validate_docker_agent(
    name: &str,
    docker: &crate::DockerWorkspaceConfig,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(mem) = &docker.memory_limit
        && let Err(e) = crate::parse_memory_limit(mem)
    {
        errors.push(ValidationError::InvalidMemoryLimit {
            field: format!("agents_manager.agents.{name}.workspace.memory_limit"),
            value: mem.clone(),
            reason: e.to_string(),
        });
    }
    if let Some(cpu) = &docker.cpu_limit
        && let Err(e) = crate::parse_cpu_limit(cpu)
    {
        errors.push(ValidationError::InvalidCpuLimit {
            field: format!("agents_manager.agents.{name}.workspace.cpu_limit"),
            value: cpu.clone(),
            reason: e.to_string(),
        });
    }
    if let Some(host) = &docker.docker_host
        && !host.starts_with("unix://")
        && !host.starts_with("tcp://")
    {
        errors.push(ValidationError::InvalidDockerHost {
            field: format!("agents_manager.agents.{name}.workspace.docker_host"),
            value: host.clone(),
        });
    }
    for volume in &docker.volumes {
        if !volume.contains(':') {
            errors.push(ValidationError::InvalidVolumeMount {
                field: format!("agents_manager.agents.{name}.workspace.volumes"),
                value: volume.clone(),
            });
        }
    }
}

fn validate_agents(config: &AnyclawConfig, errors: &mut Vec<ValidationError>) {
    for (name, agent) in &config.agents_manager.agents {
        match &agent.workspace {
            crate::WorkspaceConfig::Local(local) => validate_local_agent(name, local, errors),
            crate::WorkspaceConfig::Docker(docker) => validate_docker_agent(name, docker, errors),
        }
    }
}

fn validate_channel_binaries(config: &AnyclawConfig, errors: &mut Vec<ValidationError>) {
    for (name, channel) in &config.channels_manager.channels {
        if !binary_exists(&channel.binary) {
            errors.push(ValidationError::BinaryNotFound {
                field: format!("channels_manager.channels.{name}.binary"),
                binary: channel.binary.clone(),
            });
        }
    }
}

fn validate_tool_binaries(config: &AnyclawConfig, errors: &mut Vec<ValidationError>) {
    for (name, tool) in &config.tools_manager.tools {
        if let Some(binary) = &tool.binary
            && !binary_exists(binary)
        {
            errors.push(ValidationError::BinaryNotFound {
                field: format!("tools_manager.tools.{name}.binary"),
                binary: binary.clone(),
            });
        }
    }
}

/// Validate a YAML config string against the JSON Schema generated from the Rust types.
///
/// Returns a list of schema violation messages. An empty list means the YAML is schema-valid.
#[allow(clippy::disallowed_types)] // Schema validation operates on untyped YAML→JSON
pub fn validate_schema(yaml_content: &str) -> Vec<String> {
    let yaml_value: serde_json::Value = match serde_yaml::from_str(yaml_content) {
        Ok(v) => v,
        Err(e) => return vec![format!("YAML parse error: {e}")],
    };
    let schema_value = crate::generate_schema();
    let validator = match jsonschema::validator_for(&schema_value) {
        Ok(v) => v,
        Err(e) => return vec![format!("Schema compilation error: {e}")],
    };
    validator
        .iter_errors(&yaml_value)
        .map(|error| {
            let path = error.instance_path.to_string();
            if path.is_empty() {
                error.to_string()
            } else {
                format!("{path}: {error}")
            }
        })
        .collect()
}

/// Compare top-level YAML keys against the JSON Schema `properties` and return unknown key names.
///
/// Only checks the top level — nested unknown keys are ignored.
/// Returns an empty vec if the YAML is not a mapping or the schema has no properties.
#[allow(clippy::disallowed_types)] // Compares raw YAML keys against schema properties
pub fn check_unknown_keys(yaml_content: &str) -> Vec<String> {
    let yaml_value: serde_json::Value = match serde_yaml::from_str(yaml_content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let Some(yaml_obj) = yaml_value.as_object() else {
        return vec![];
    };
    let schema = crate::generate_schema();
    let known_keys: std::collections::HashSet<&str> = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|obj| obj.keys().map(String::as_str).collect())
        .unwrap_or_default();

    yaml_obj
        .keys()
        .filter(|k| !known_keys.contains(k.as_str()))
        .cloned()
        .collect()
}

/// Validate the loaded configuration: check binary existence, working dirs, Docker limits, and host format.
pub fn validate_config(config: &AnyclawConfig) -> ValidationResult {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    validate_tools_server_host(config, &mut errors);
    validate_agents(config, &mut errors);
    validate_channel_binaries(config, &mut errors);
    validate_tool_binaries(config, &mut errors);

    ValidationResult { errors, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentConfig, AgentsManagerConfig, ChannelConfig, ChannelsManagerConfig,
        DockerWorkspaceConfig, LocalWorkspaceConfig, LogFormat, PullPolicy, StringOrArray,
        SupervisorConfig, ToolType, ToolsManagerConfig, WorkspaceConfig,
    };
    use rstest::rstest;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn valid_config() -> AnyclawConfig {
        let mut agents = HashMap::new();
        agents.insert(
            "default".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                    binary: StringOrArray::from("echo"),
                    working_dir: None,
                    env: HashMap::new(),
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        AnyclawConfig {
            log_level: "info".into(),
            log_format: LogFormat::Pretty,
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                acp_timeout_secs: 30,
                shutdown_grace_ms: 100,
                agents,
            },
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
            session_store: Default::default(),
        }
    }

    #[test]
    fn when_all_binaries_exist_then_validation_has_no_errors() {
        let config = valid_config();
        let result = validate_config(&config);
        assert!(
            result.errors.is_empty(),
            "expected no errors, got: {:?}",
            result.errors
        );
        assert!(result.is_ok());
    }

    #[test]
    fn when_agent_binary_not_on_path_then_binary_not_found_error() {
        let mut config = valid_config();
        if let WorkspaceConfig::Local(ref mut local) = config
            .agents_manager
            .agents
            .get_mut("default")
            .unwrap()
            .workspace
        {
            local.binary = StringOrArray::from("nonexistent-xyz-99999");
        }
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::BinaryNotFound { field, binary }
                if field.contains("default") && binary == "nonexistent-xyz-99999"
            )
        });
        assert!(
            has_error,
            "expected BinaryNotFound, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_agent_working_dir_does_not_exist_then_working_dir_error() {
        let mut config = valid_config();
        if let WorkspaceConfig::Local(ref mut local) = config
            .agents_manager
            .agents
            .get_mut("default")
            .unwrap()
            .workspace
        {
            local.working_dir = Some(PathBuf::from("/nonexistent/path/xyz-99999"));
        }
        let result = validate_config(&config);
        let has_error = result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::WorkingDirNotFound { .. }));
        assert!(
            has_error,
            "expected WorkingDirNotFound, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_channel_binary_not_on_path_then_binary_not_found_error() {
        let mut config = valid_config();
        config.channels_manager.channels.insert(
            "ch".to_string(),
            ChannelConfig {
                binary: "nonexistent-xyz-99999".to_string(),
                args: vec![],
                enabled: true,
                agent: "default".into(),
                ack: Default::default(),
                init_timeout_secs: None,
                exit_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(
            |e| matches!(e, ValidationError::BinaryNotFound { field, .. } if field.contains("ch")),
        );
        assert!(
            has_error,
            "expected BinaryNotFound, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_tool_binary_not_on_path_then_binary_not_found_error() {
        let mut config = valid_config();
        config.tools_manager.tools.insert(
            "fs".to_string(),
            crate::ToolConfig {
                tool_type: ToolType::Mcp,
                binary: Some("nonexistent-xyz-99999".into()),
                args: vec![],
                enabled: true,
                module: None,
                description: String::new(),
                input_schema: None,
                sandbox: Default::default(),
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(
            |e| matches!(e, ValidationError::BinaryNotFound { field, .. } if field.contains("fs")),
        );
        assert!(
            has_error,
            "expected BinaryNotFound, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_validation_result_has_no_errors_then_is_ok_returns_true() {
        let result = ValidationResult {
            errors: vec![],
            warnings: vec![],
        };
        assert!(result.is_ok());
    }

    #[test]
    fn when_validation_result_has_errors_then_is_ok_returns_false() {
        let result = ValidationResult {
            errors: vec![ValidationError::BinaryNotFound {
                field: "agent.binary".to_string(),
                binary: "missing".to_string(),
            }],
            warnings: vec![],
        };
        assert!(!result.is_ok());
    }

    #[test]
    fn when_agent_workspace_is_docker_then_no_binary_check_performed() {
        let mut config = valid_config();
        config.agents_manager.agents.insert(
            "docker-agent".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                    image: "some-nonexistent-image:latest".to_string(),
                    entrypoint: None,
                    volumes: vec!["/host:/container".to_string()],
                    env: HashMap::new(),
                    memory_limit: Some("512m".to_string()),
                    cpu_limit: Some("1.5".to_string()),
                    docker_host: Some("unix:///var/run/docker.sock".to_string()),
                    network: None,
                    pull_policy: PullPolicy::IfNotPresent,
                    working_dir: None,
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_binary_error = result.errors.iter().any(|e| {
            matches!(e, ValidationError::BinaryNotFound { field, .. } if field.contains("docker-agent"))
        });
        assert!(
            !has_binary_error,
            "docker agent should not produce BinaryNotFound, got: {:?}",
            result.errors
        );
        assert!(
            result.is_ok(),
            "expected no errors, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_docker_memory_limit_unparseable_then_invalid_memory_error() {
        let mut config = valid_config();
        config.agents_manager.agents.insert(
            "docker-agent".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                    image: "my-agent:latest".to_string(),
                    entrypoint: None,
                    volumes: vec![],
                    env: HashMap::new(),
                    memory_limit: Some("notvalid".to_string()),
                    cpu_limit: None,
                    docker_host: None,
                    network: None,
                    pull_policy: PullPolicy::IfNotPresent,
                    working_dir: None,
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::InvalidMemoryLimit { field, value, .. }
                if field.contains("docker-agent") && value == "notvalid"
            )
        });
        assert!(
            has_error,
            "expected InvalidMemoryLimit, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_docker_cpu_limit_unparseable_then_invalid_cpu_error() {
        let mut config = valid_config();
        config.agents_manager.agents.insert(
            "docker-agent".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                    image: "my-agent:latest".to_string(),
                    entrypoint: None,
                    volumes: vec![],
                    env: HashMap::new(),
                    memory_limit: None,
                    cpu_limit: Some("badcpu".to_string()),
                    docker_host: None,
                    network: None,
                    pull_policy: PullPolicy::IfNotPresent,
                    working_dir: None,
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::InvalidCpuLimit { field, value, .. }
                if field.contains("docker-agent") && value == "badcpu"
            )
        });
        assert!(
            has_error,
            "expected InvalidCpuLimit, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_docker_volume_has_no_colon_separator_then_invalid_volume_error() {
        let mut config = valid_config();
        config.agents_manager.agents.insert(
            "docker-agent".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Docker(DockerWorkspaceConfig {
                    image: "my-agent:latest".to_string(),
                    entrypoint: None,
                    volumes: vec!["nocolon".to_string()],
                    env: HashMap::new(),
                    memory_limit: None,
                    cpu_limit: None,
                    docker_host: None,
                    network: None,
                    pull_policy: PullPolicy::IfNotPresent,
                    working_dir: None,
                }),
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::InvalidVolumeMount { field, value }
                if field.contains("docker-agent") && value == "nocolon"
            )
        });
        assert!(
            has_error,
            "expected InvalidVolumeMount, got: {:?}",
            result.errors
        );
    }

    #[rstest]
    #[case::ipv4_loopback("127.0.0.1")]
    #[case::ipv4_any("0.0.0.0")]
    #[case::ipv6_loopback("::1")]
    #[case::localhost("localhost")]
    #[case::fqdn("my-host.example.com")]
    #[case::docker_service_name("anyclaw")]
    #[case::internal_service("tools.internal")]
    fn when_host_is_valid_then_is_valid_host_returns_true(#[case] host: &str) {
        assert!(is_valid_host(host), "expected valid host: {:?}", host);
    }

    #[rstest]
    #[case::with_space("not a hostname")]
    #[case::empty("")]
    #[case::at_sign("host@name")]
    #[case::leading_hyphen("-invalid")]
    #[case::trailing_hyphen("invalid-")]
    #[case::with_slash("host/path")]
    fn when_host_is_invalid_then_is_valid_host_returns_false(#[case] host: &str) {
        assert!(!is_valid_host(host), "expected invalid host: {:?}", host);
    }

    #[test]
    fn when_tools_server_host_is_invalid_then_validation_returns_error() {
        let mut config = valid_config();
        config.tools_manager.tools_server_host = "not a hostname".to_string();
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::InvalidToolsServerHost { field, value }
                if field == "tools_manager.tools_server_host" && value == "not a hostname"
            )
        });
        assert!(
            has_error,
            "expected InvalidToolsServerHost, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn when_tools_server_host_is_valid_then_validation_produces_no_host_error() {
        let mut config = valid_config();
        config.tools_manager.tools_server_host = "127.0.0.1".to_string();
        let result = validate_config(&config);
        let has_error = result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidToolsServerHost { .. }));
        assert!(
            !has_error,
            "expected no InvalidToolsServerHost error, got: {:?}",
            result.errors
        );
    }

    #[rstest]
    #[case::binary_not_on_path(
        ValidationWarning::BinaryNotOnPath {
            field: "agents_manager.agents.default.workspace.binary".into(),
            binary: "mybin".into(),
            found_at: "/opt/mybin".into(),
        },
        "agents_manager.agents.default.workspace.binary: binary 'mybin' found at /opt/mybin but not on PATH"
    )]
    fn when_validation_warning_displayed_then_message_matches_expected(
        #[case] warning: ValidationWarning,
        #[case] expected: &str,
    ) {
        assert_eq!(warning.to_string(), expected);
    }

    #[rstest]
    #[case::binary_not_found(
        ValidationError::BinaryNotFound {
            field: "agents_manager.agents.x.workspace.binary".into(),
            binary: "missing".into(),
        },
        "agents_manager.agents.x.workspace.binary: binary 'missing' not found on PATH or as absolute path"
    )]
    #[case::working_dir_not_found(
        ValidationError::WorkingDirNotFound { path: std::path::PathBuf::from("/no/such/dir") },
        "agent.working_dir: directory '/no/such/dir' does not exist"
    )]
    #[case::invalid_memory_limit(
        ValidationError::InvalidMemoryLimit {
            field: "agents_manager.agents.x.workspace.memory_limit".into(),
            value: "bad".into(),
            reason: "unrecognised suffix".into(),
        },
        "agents_manager.agents.x.workspace.memory_limit: invalid memory limit 'bad': unrecognised suffix"
    )]
    #[case::invalid_cpu_limit(
        ValidationError::InvalidCpuLimit {
            field: "agents_manager.agents.x.workspace.cpu_limit".into(),
            value: "notnum".into(),
            reason: "not a float".into(),
        },
        "agents_manager.agents.x.workspace.cpu_limit: invalid cpu limit 'notnum': not a float"
    )]
    #[case::invalid_docker_host(
        ValidationError::InvalidDockerHost {
            field: "agents_manager.agents.x.workspace.docker_host".into(),
            value: "http://bad".into(),
        },
        "agents_manager.agents.x.workspace.docker_host: invalid docker_host URI 'http://bad' (expected unix:// or tcp://)"
    )]
    #[case::invalid_volume_mount(
        ValidationError::InvalidVolumeMount {
            field: "agents_manager.agents.x.workspace.volumes".into(),
            value: "nocolon".into(),
        },
        "agents_manager.agents.x.workspace.volumes: volume entry 'nocolon' missing ':' separator"
    )]
    #[case::invalid_tools_server_host(
        ValidationError::InvalidToolsServerHost {
            field: "tools_manager.tools_server_host".into(),
            value: "not a hostname".into(),
        },
        "tools_manager.tools_server_host: invalid hostname or IP 'not a hostname'"
    )]
    fn when_validation_error_displayed_then_message_matches_expected(
        #[case] error: ValidationError,
        #[case] expected: &str,
    ) {
        assert_eq!(error.to_string(), expected);
    }

    #[test]
    fn when_valid_yaml_given_then_validate_schema_returns_no_errors() {
        let yaml = r#"
log_level: info
agents_manager:
  agents:
    default:
      workspace:
        type: local
        binary: echo
"#;
        let errors = super::validate_schema(yaml);
        assert!(
            errors.is_empty(),
            "expected no schema errors, got: {errors:?}"
        );
    }

    #[test]
    fn when_log_level_is_wrong_type_then_validate_schema_returns_error_mentioning_log_level() {
        let yaml = r#"
log_level: 123
"#;
        let errors = super::validate_schema(yaml);
        assert!(
            !errors.is_empty(),
            "expected schema errors for invalid log_level type"
        );
        let mentions_log_level = errors.iter().any(|e| e.contains("log_level"));
        assert!(
            mentions_log_level,
            "expected error to mention 'log_level', got: {errors:?}"
        );
    }

    #[test]
    fn when_unknown_top_level_keys_present_then_validate_schema_returns_no_errors() {
        let yaml = r#"
log_level: info
totally_unknown_key: whatever
"#;
        let errors = super::validate_schema(yaml);
        assert!(
            errors.is_empty(),
            "unknown top-level keys should be allowed, got: {errors:?}"
        );
    }

    #[test]
    fn when_acp_timeout_secs_is_string_then_validate_schema_returns_type_error() {
        let yaml = r#"
agents_manager:
  acp_timeout_secs: "not_a_number"
"#;
        let errors = super::validate_schema(yaml);
        assert!(
            !errors.is_empty(),
            "expected schema errors for acp_timeout_secs string value"
        );
        let mentions_type = errors.iter().any(|e| {
            e.contains("acp_timeout_secs")
                || e.contains("integer")
                || e.contains("type")
                || e.contains("agents_manager")
        });
        assert!(
            mentions_type,
            "expected error to mention type mismatch, got: {errors:?}"
        );
    }

    #[test]
    fn when_yaml_has_unknown_top_level_key_then_check_unknown_keys_returns_it() {
        let yaml = "log_level: info\ntotally_unknown: foo\n";
        let unknown = super::check_unknown_keys(yaml);
        assert_eq!(unknown, vec!["totally_unknown"]);
    }

    #[test]
    fn when_yaml_has_only_known_keys_then_check_unknown_keys_returns_empty() {
        let yaml = "log_level: info\n";
        let unknown = super::check_unknown_keys(yaml);
        assert!(
            unknown.is_empty(),
            "expected no unknown keys, got: {unknown:?}"
        );
    }

    #[test]
    fn when_yaml_has_multiple_unknown_keys_then_check_unknown_keys_returns_all() {
        let yaml = "log_level: info\nfoo: 1\nbar: 2\n";
        let mut unknown = super::check_unknown_keys(yaml);
        unknown.sort();
        assert_eq!(unknown, vec!["bar", "foo"]);
    }

    #[test]
    fn when_yaml_is_not_a_mapping_then_check_unknown_keys_returns_empty() {
        let yaml = "\"just a string\"";
        let unknown = super::check_unknown_keys(yaml);
        assert!(unknown.is_empty());
    }
}
