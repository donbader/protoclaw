/// Configuration errors for anyclaw config loading and validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file could not be loaded or parsed.
    #[error("failed to load config from '{path}': {reason}")]
    LoadFailed {
        /// Path that was attempted.
        path: String,
        /// Why loading failed.
        reason: String,
    },

    /// Semantic validation of the loaded config failed.
    #[error("config validation failed: {0}")]
    Validation(String),

    /// Figment extraction error.
    #[error("config parse error: {0}")]
    Parse(#[from] Box<figment::Error>),

    /// A Docker memory limit string could not be parsed.
    #[error("invalid memory limit '{value}': {reason}")]
    InvalidMemoryLimit {
        /// The raw memory limit string.
        value: String,
        /// Why parsing failed.
        reason: String,
    },

    /// A Docker CPU limit string could not be parsed.
    #[error("invalid cpu limit '{value}': {reason}")]
    InvalidCpuLimit {
        /// The raw CPU limit string.
        value: String,
        /// Why parsing failed.
        reason: String,
    },
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
