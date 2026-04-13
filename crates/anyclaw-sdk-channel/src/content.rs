/// Extract displayable text from a content JSON value.
///
/// Handles OpenCode's `{"type": "text", "text": "actual text"}` wrapper format,
/// plain string values, and falls back to JSON serialization for other types.
pub fn content_to_string(content: &serde_json::Value) -> String {
    if let Some(text) = content.get("text").and_then(|t| t.as_str()) {
        return text.to_string();
    }
    match content {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to serialize content value to string, using empty string");
            String::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_content_is_plain_string_then_returns_string() {
        let val = serde_json::Value::String("hello".into());
        assert_eq!(content_to_string(&val), "hello");
    }

    #[rstest]
    fn when_content_is_opencode_wrapper_then_extracts_text() {
        let val = serde_json::json!({"type": "text", "text": "hello"});
        assert_eq!(content_to_string(&val), "hello");
    }

    #[rstest]
    fn when_content_is_object_without_text_then_returns_json() {
        let val = serde_json::json!({"key": "value"});
        let result = content_to_string(&val);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[rstest]
    fn when_content_is_number_then_returns_stringified() {
        let val = serde_json::json!(42);
        assert_eq!(content_to_string(&val), "42");
    }
}
