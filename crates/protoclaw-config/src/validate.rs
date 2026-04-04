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
        if !binary_exists(&agent.binary) {
            errors.push(ValidationError::BinaryNotFound {
                field: format!("agents-manager.agents.{name}.binary"),
                binary: agent.binary.clone(),
            });
        }
        if let Some(path) = &agent.working_dir {
            if !path.exists() {
                errors.push(ValidationError::WorkingDirNotFound { path: path.clone() });
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
        AgentConfig, AgentsManagerConfig, ChannelConfig, ChannelsManagerConfig, SupervisorConfig,
        ToolsManagerConfig,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn valid_config() -> ProtoclawConfig {
        let mut agents = HashMap::new();
        agents.insert(
            "default".to_string(),
            AgentConfig {
                binary: "echo".to_string(),
                args: vec![],
                enabled: true,
                env: HashMap::new(),
                working_dir: None,
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
        config
            .agents_manager
            .agents
            .get_mut("default")
            .unwrap()
            .binary = "nonexistent-xyz-99999".to_string();
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
        config
            .agents_manager
            .agents
            .get_mut("default")
            .unwrap()
            .working_dir = Some(PathBuf::from("/nonexistent/path/xyz-99999"));
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
}
