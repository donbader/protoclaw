#[derive(Debug, thiserror::Error)]
pub enum FramingError {
    #[error("invalid Content-Length header: {reason}")]
    InvalidHeader { reason: String },
    #[error("frame of {size} bytes exceeds maximum of {max} bytes")]
    FrameTooLarge { size: usize, max: usize },
    #[error("invalid JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("invalid UTF-8 in header: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn when_framing_errors_created_then_display_matches_template() {
        let err = FramingError::InvalidHeader {
            reason: "bad format".to_string(),
        };
        assert_eq!(err.to_string(), "invalid Content-Length header: bad format");

        let err = FramingError::FrameTooLarge {
            size: 64_000_000,
            max: 32_000_000,
        };
        assert_eq!(
            err.to_string(),
            "frame of 64000000 bytes exceeds maximum of 32000000 bytes"
        );

        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = FramingError::InvalidJson(json_err);
        assert!(err.to_string().starts_with("invalid JSON payload:"));
    }
}
