/// Configuration errors for protoclaw config loading and validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load config from '{path}': {reason}")]
    LoadFailed { path: String, reason: String },

    #[error("config validation failed: {0}")]
    Validation(String),

    #[error("config parse error: {0}")]
    Parse(#[from] Box<figment::Error>),

    #[error("invalid memory limit '{value}': {reason}")]
    InvalidMemoryLimit { value: String, reason: String },

    #[error("invalid cpu limit '{value}': {reason}")]
    InvalidCpuLimit { value: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::load_failed(
        ConfigError::LoadFailed { path: "/etc/foo.yaml".into(), reason: "not found".into() },
        "failed to load config from '/etc/foo.yaml': not found"
    )]
    #[case::validation(
        ConfigError::Validation("some rule violated".into()),
        "config validation failed: some rule violated"
    )]
    #[case::invalid_memory_limit(
        ConfigError::InvalidMemoryLimit { value: "badmem".into(), reason: "unrecognised suffix".into() },
        "invalid memory limit 'badmem': unrecognised suffix"
    )]
    #[case::invalid_cpu_limit(
        ConfigError::InvalidCpuLimit { value: "notanumber".into(), reason: "not a float".into() },
        "invalid cpu limit 'notanumber': not a float"
    )]
    fn when_config_error_displayed_then_message_matches_expected(
        #[case] error: ConfigError,
        #[case] expected: &str,
    ) {
        assert_eq!(error.to_string(), expected);
    }
}
