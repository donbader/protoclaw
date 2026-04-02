use crate::ProtoclawConfig;
use std::collections::HashSet;
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
    #[error("duplicate {kind} name: '{name}'")]
    DuplicateName { kind: String, name: String },
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

    if !binary_exists(&config.agent.binary) {
        errors.push(ValidationError::BinaryNotFound {
            field: "agent.binary".to_string(),
            binary: config.agent.binary.clone(),
        });
    }

    if let Some(path) = &config.agent.working_dir {
        if !path.exists() {
            errors.push(ValidationError::WorkingDirNotFound { path: path.clone() });
        }
    }

    for (i, ch) in config.channels.iter().enumerate() {
        if !binary_exists(&ch.binary) {
            errors.push(ValidationError::BinaryNotFound {
                field: format!("channels[{i}].binary"),
                binary: ch.binary.clone(),
            });
        }
    }

    for (i, mcp) in config.mcp_servers.iter().enumerate() {
        if !binary_exists(&mcp.binary) {
            errors.push(ValidationError::BinaryNotFound {
                field: format!("mcp_servers[{i}].binary"),
                binary: mcp.binary.clone(),
            });
        }
    }

    let mut seen_channels: HashSet<&str> = HashSet::new();
    for ch in &config.channels {
        if !seen_channels.insert(ch.name.as_str()) {
            errors.push(ValidationError::DuplicateName {
                kind: "channel".to_string(),
                name: ch.name.clone(),
            });
        }
    }

    let mut seen_mcps: HashSet<&str> = HashSet::new();
    for mcp in &config.mcp_servers {
        if !seen_mcps.insert(mcp.name.as_str()) {
            errors.push(ValidationError::DuplicateName {
                kind: "mcp_server".to_string(),
                name: mcp.name.clone(),
            });
        }
    }

    ValidationResult { errors, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentConfig, ChannelConfig, McpServerConfig, SupervisorConfig};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn valid_config() -> ProtoclawConfig {
        ProtoclawConfig {
            agent: AgentConfig {
                binary: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
            },
            channels: vec![],
            mcp_servers: vec![],
            wasm_tools: vec![],
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
        config.agent.binary = "nonexistent-xyz-99999".to_string();
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::BinaryNotFound { field, binary }
                if field == "agent.binary" && binary == "nonexistent-xyz-99999"
            )
        });
        assert!(
            has_error,
            "expected BinaryNotFound for agent.binary, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn duplicate_channel_names_is_error() {
        let mut config = valid_config();
        config.channels = vec![
            ChannelConfig {
                name: "debug-http".to_string(),
                binary: "echo".to_string(),
                args: vec![],
            },
            ChannelConfig {
                name: "debug-http".to_string(),
                binary: "echo".to_string(),
                args: vec![],
            },
        ];
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::DuplicateName { kind, name }
                if kind == "channel" && name == "debug-http"
            )
        });
        assert!(
            has_error,
            "expected DuplicateName for channel 'debug-http', got: {:?}",
            result.errors
        );
    }

    #[test]
    fn duplicate_mcp_server_names_is_error() {
        let mut config = valid_config();
        config.mcp_servers = vec![
            McpServerConfig {
                name: "fs".to_string(),
                binary: "echo".to_string(),
                args: vec![],
            },
            McpServerConfig {
                name: "fs".to_string(),
                binary: "echo".to_string(),
                args: vec![],
            },
        ];
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::DuplicateName { kind, name }
                if kind == "mcp_server" && name == "fs"
            )
        });
        assert!(
            has_error,
            "expected DuplicateName for mcp_server 'fs', got: {:?}",
            result.errors
        );
    }

    #[test]
    fn nonexistent_working_dir_is_error() {
        let mut config = valid_config();
        config.agent.working_dir = Some(PathBuf::from("/nonexistent/path/xyz-99999"));
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
        config.channels = vec![ChannelConfig {
            name: "ch".to_string(),
            binary: "nonexistent-xyz-99999".to_string(),
            args: vec![],
        }];
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::BinaryNotFound { field, .. }
                if field == "channels[0].binary"
            )
        });
        assert!(
            has_error,
            "expected BinaryNotFound for channels[0].binary, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn missing_mcp_server_binary_is_error() {
        let mut config = valid_config();
        config.mcp_servers = vec![McpServerConfig {
            name: "fs".to_string(),
            binary: "nonexistent-xyz-99999".to_string(),
            args: vec![],
        }];
        let result = validate_config(&config);
        let has_error = result.errors.iter().any(|e| {
            matches!(
                e,
                ValidationError::BinaryNotFound { field, .. }
                if field == "mcp_servers[0].binary"
            )
        });
        assert!(
            has_error,
            "expected BinaryNotFound for mcp_servers[0].binary, got: {:?}",
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
