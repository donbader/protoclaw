//! ACP wire types — re-exported from anyclaw-sdk-types for backward compatibility.
//!
//! Canonical location: `anyclaw_sdk_types::acp`
pub use anyclaw_sdk_types::acp::*;

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn when_initialize_params_reexported_then_constructible() {
        let params = InitializeParams {
            protocol_version: 1,
            capabilities: ClientCapabilities { experimental: None },
            options: None,
        };
        assert_eq!(params.protocol_version, 1);
    }

    #[rstest]
    fn when_session_update_event_reexported_then_constructible() {
        let event = SessionUpdateEvent {
            session_id: "ses-1".into(),
            update: SessionUpdateType::Result {
                content: Some("done".into()),
            },
        };
        assert_eq!(event.session_id, "ses-1");
    }

    #[rstest]
    fn when_content_part_reexported_then_constructible() {
        let part = ContentPart::text("hello");
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["text"], "hello");
    }

    #[rstest]
    fn when_session_new_params_reexported_then_constructible() {
        let params = SessionNewParams {
            session_id: None,
            cwd: "/tmp".into(),
            mcp_servers: vec![],
        };
        assert_eq!(params.cwd, "/tmp");
    }
}
