/// Configuration errors for protoclaw config loading and validation.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to load config from '{path}': {reason}")]
    LoadFailed { path: String, reason: String },

    #[error("config validation failed: {0}")]
    Validation(String),

    #[error("config parse error: {0}")]
    Parse(#[from] Box<figment::Error>),
}
