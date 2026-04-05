use crate::ProtoclawConfig;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("{field}: binary '{binary}' not found on PATH or as absolute path")]
    BinaryNotFound { field: String, binary: String },
    #[error("agent.working_dir: directory '{}' does not exist", path.display())]
    WorkingDirNotFound { path: std::path::PathBuf },
    #[error("{field}: invalid memory limit '{value}': {reason}")]
    InvalidMemoryLimit {
        field: String,
        value: String,
        reason: String,
    },
    #[error("{field}: invalid cpu limit '{value}': {reason}")]
    InvalidCpuLimit {
        field: String,
        value: String,
        reason: String,
    },
    #[error("{field}: invalid docker_host URI '{value}' (expected unix:// or tcp://)")]
    InvalidDockerHost { field: String, value: String },
    #[error("{field}: volume entry '{value}' missing ':' separator")]
    InvalidVolumeMount { field: String, value: String },
}

#[derive(Debug, Clone)]
pub enum ValidationWarning {
    BinaryNotOnPath {
        field: String,
        binary: String,
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

pub fn validate_config(config: &ProtoclawConfig) -> ValidationResult {
    let mut errors = Vec::new();
    let warnings = Vec::new();

    for (name, agent) in &config.agents_manager.agents {
        match &agent.workspace {
            crate::WorkspaceConfig::Local(local) => {
                if !binary_exists(&local.binary) {
                    errors.push(ValidationError::BinaryNotFound {
                        field: format!("agents-manager.agents.{name}.workspace.binary"),
                        binary: local.binary.clone(),
                    });
                }
                if let Some(path) = &local.working_dir {
                    if !path.exists() {
                        errors.push(ValidationError::WorkingDirNotFound { path: path.clone() });
                    }
                }
            }
            crate::WorkspaceConfig::Docker(docker) => {
                if let Some(ref mem) = docker.memory_limit {
                    if let Err(e) = crate::parse_memory_limit(mem) {
                        errors.push(ValidationError::InvalidMemoryLimit {
                            field: format!("agents-manager.agents.{name}.workspace.memory_limit"),
                            value: mem.clone(),
                            reason: e.to_string(),
                        });
                    }
                }
                if let Some(ref cpu) = docker.cpu_limit {
                    if let Err(e) = crate::parse_cpu_limit(cpu) {
                        errors.push(ValidationError::InvalidCpuLimit {
                            field: format!("agents-manager.agents.{name}.workspace.cpu_limit"),
                            value: cpu.clone(),
                            reason: e.to_string(),
                        });
                    }
                }
                if let Some(ref host) = docker.docker_host {
                    if !host.starts_with("unix://") && !host.starts_with("tcp://") {
                        errors.push(ValidationError::InvalidDockerHost {
                            field: format!("agents-manager.agents.{name}.workspace.docker_host"),
                            value: host.clone(),
                        });
                    }
                }
                for vol in &docker.volumes {
                    if !vol.contains(':') {
                        errors.push(ValidationError::InvalidVolumeMount {
                            field: format!("agents-manager.agents.{name}.workspace.volumes"),
                            value: vol.clone(),
                        });
                    }
                }
            }
        }
    }

    for (name, ch) in &config.channels_manager.channels {
        if let Some(ref bin) = Some(&ch.binary) {
            if !binary_exists(bin) {
                errors.push(ValidationError::BinaryNotFound {
                    field: format!("channels-manager.channels.{name}.binary"),
                    binary: ch.binary.clone(),
                });
            }
        }
    }

    for (name, tool) in &config.tools_manager.tools {
        if let Some(ref bin) = tool.binary {
            if !binary_exists(bin) {
                errors.push(ValidationError::BinaryNotFound {
                    field: format!("tools-manager.tools.{name}.binary"),
                    binary: bin.clone(),
                });
            }
        }
    }

    ValidationResult { errors, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentConfig, AgentsManagerConfig, ChannelConfig, ChannelsManagerConfig,
        DockerWorkspaceConfig, LocalWorkspaceConfig, PullPolicy, SupervisorConfig,
        ToolsManagerConfig, WorkspaceConfig,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn valid_config() -> ProtoclawConfig {
        let mut agents = HashMap::new();
        agents.insert(
            "default".to_string(),
            AgentConfig {
                workspace: WorkspaceConfig::Local(LocalWorkspaceConfig {
                    binary: "echo".to_string(),
                    working_dir: None,
                    env: HashMap::new(),
                }),
                args: vec![],
                enabled: true,
                tools: vec![],
                acp_timeout_secs: None,
                backoff: None,
                crash_tracker: None,
                options: HashMap::new(),
            },
        );
        ProtoclawConfig {
            log_level: "info".into(),
            log_format: "pretty".into(),
            extensions_dir: "/usr/local/bin".into(),
            agents_manager: AgentsManagerConfig {
                acp_timeout_secs: 30,
                shutdown_grace_ms: 100,
                agents,
            },
            channels_manager: ChannelsManagerConfig::default(),
            tools_manager: ToolsManagerConfig::default(),
            supervisor: SupervisorConfig::default(),
        }
    }

    #[test]
    fn valid_config_has_no_errors() {
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
    fn missing_agent_binary_is_error() {
        let mut config = valid_config();
        if let WorkspaceConfig::Local(ref mut local) = config
            .agents_manager
            .agents
            .get_mut("default")
            .unwrap()
            .workspace
        {
            local.binary = "nonexistent-xyz-99999".to_string();
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
    fn nonexistent_working_dir_is_error() {
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
    fn missing_channel_binary_is_error() {
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
    fn missing_tool_binary_is_error() {
        let mut config = valid_config();
        config.tools_manager.tools.insert(
            "fs".to_string(),
            crate::ToolConfig {
                tool_type: "mcp".into(),
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
    fn is_ok_true_when_no_errors() {
        let result = ValidationResult {
            errors: vec![],
            warnings: vec![],
        };
        assert!(result.is_ok());
    }

    #[test]
    fn is_ok_false_when_has_errors() {
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
    fn docker_agent_skips_binary_check() {
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
                }),
                args: vec![],
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
    fn docker_invalid_memory_limit_is_error() {
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
                }),
                args: vec![],
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
    fn docker_invalid_cpu_limit_is_error() {
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
                }),
                args: vec![],
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
    fn docker_volume_missing_colon_is_error() {
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
                }),
                args: vec![],
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
}
