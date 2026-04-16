//! Property-based tests for all public wire types in anyclaw-sdk-types.
//!
//! Uses proptest to verify serialize→deserialize round-trip identity
//! and no-panic guarantees for arbitrary inputs.

use proptest::prelude::*;
use std::collections::HashMap;

use crate::acp::*;
use crate::channel::*;
use crate::permission::*;
use crate::session_key::SessionKey;

// ── Helpers ────────────────────────────────────────────────────────────

fn arb_json_value() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        any::<i64>().prop_map(|n| serde_json::Value::Number(n.into())),
        "[a-zA-Z0-9_-]{0,20}".prop_map(serde_json::Value::String),
    ]
}

fn arb_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,20}"
}

fn arb_opt_string() -> impl Strategy<Value = Option<String>> {
    proptest::option::of("[a-zA-Z0-9_-]{1,20}")
}

fn arb_string_value_map() -> impl Strategy<Value = HashMap<String, serde_json::Value>> {
    proptest::collection::hash_map("[a-z]{1,5}", arb_json_value(), 0..3)
}

// ── ACP type strategies ────────────────────────────────────────────────

prop_compose! {
    fn arb_client_capabilities()(experimental in proptest::option::of(arb_string_value_map())) -> ClientCapabilities {
        ClientCapabilities { experimental }
    }
}

prop_compose! {
    fn arb_initialize_params()(
        protocol_version in any::<u32>(),
        capabilities in arb_client_capabilities(),
        options in proptest::option::of(arb_string_value_map()),
    ) -> InitializeParams {
        InitializeParams { protocol_version, capabilities, options }
    }
}

prop_compose! {
    fn arb_initialize_result()(protocol_version in any::<u32>()) -> InitializeResult {
        InitializeResult { protocol_version, agent_capabilities: None, defaults: None }
    }
}

prop_compose! {
    fn arb_mcp_server_info()(
        name in arb_string(),
        server_type in prop_oneof![Just("stdio".to_string()), Just("sse".to_string())],
        url in arb_string(),
        command in arb_string(),
        args in proptest::collection::vec(arb_string(), 0..3),
        env in proptest::collection::vec(arb_string(), 0..3),
        headers in proptest::collection::vec(
            proptest::collection::vec(arb_string(), 2..=2), 0..2
        ),
    ) -> McpServerInfo {
        McpServerInfo { name, server_type, url, command, args, env, headers }
    }
}

prop_compose! {
    fn arb_session_new_params()(
        session_id in arb_opt_string(),
        cwd in arb_string(),
        mcp_servers in proptest::collection::vec(arb_mcp_server_info(), 0..2),
    ) -> SessionNewParams {
        SessionNewParams { session_id, cwd, mcp_servers }
    }
}

prop_compose! {
    fn arb_session_new_result()(session_id in arb_string()) -> SessionNewResult {
        SessionNewResult { session_id }
    }
}

fn arb_content_part() -> impl Strategy<Value = ContentPart> {
    prop_oneof![
        arb_string().prop_map(|text| ContentPart::Text { text }),
        arb_string().prop_map(|url| ContentPart::Image { url }),
    ]
}

prop_compose! {
    fn arb_session_prompt_params()(
        session_id in arb_string(),
        prompt in proptest::collection::vec(arb_content_part(), 0..3),
    ) -> SessionPromptParams {
        SessionPromptParams { session_id, prompt }
    }
}

prop_compose! {
    fn arb_session_cancel_params()(session_id in arb_string()) -> SessionCancelParams {
        SessionCancelParams { session_id }
    }
}

prop_compose! {
    fn arb_session_fork_params()(session_id in arb_string()) -> SessionForkParams {
        SessionForkParams { session_id }
    }
}

prop_compose! {
    fn arb_session_fork_result()(session_id in arb_string()) -> SessionForkResult {
        SessionForkResult { session_id }
    }
}

prop_compose! {
    fn arb_session_list_params()(_dummy in Just(())) -> SessionListParams {
        SessionListParams {}
    }
}

prop_compose! {
    fn arb_session_info()(
        session_id in arb_string(),
        metadata in arb_string_value_map(),
    ) -> SessionInfo {
        SessionInfo { session_id, metadata }
    }
}

prop_compose! {
    fn arb_session_list_result()(
        sessions in proptest::collection::vec(arb_session_info(), 0..3),
    ) -> SessionListResult {
        SessionListResult { sessions }
    }
}

prop_compose! {
    fn arb_session_load_params()(
        session_id in arb_string(),
        cwd in arb_opt_string(),
        mcp_servers in proptest::option::of(proptest::collection::vec(arb_mcp_server_info(), 0..2)),
    ) -> SessionLoadParams {
        SessionLoadParams { session_id, cwd, mcp_servers }
    }
}

fn arb_tool_call_status() -> impl Strategy<Value = ToolCallStatus> {
    prop_oneof![
        Just(ToolCallStatus::Pending),
        Just(ToolCallStatus::InProgress),
        Just(ToolCallStatus::Completed),
        Just(ToolCallStatus::Failed),
    ]
}

// ── Channel type strategies ─────────────────────────────────────────────

prop_compose! {
    fn arb_channel_capabilities()(streaming in any::<bool>(), rich_text in any::<bool>()) -> ChannelCapabilities {
        ChannelCapabilities { streaming, rich_text }
    }
}

prop_compose! {
    fn arb_channel_ack_config()(
        reaction in any::<bool>(),
        typing in any::<bool>(),
        reaction_emoji in arb_string(),
        reaction_lifecycle in arb_string(),
    ) -> ChannelAckConfig {
        ChannelAckConfig { reaction, typing, reaction_emoji, reaction_lifecycle }
    }
}

prop_compose! {
    fn arb_channel_initialize_params()(
        protocol_version in any::<u32>(),
        channel_id in arb_string(),
        ack in proptest::option::of(arb_channel_ack_config()),
        options in arb_string_value_map(),
    ) -> ChannelInitializeParams {
        ChannelInitializeParams { protocol_version, channel_id, ack, options }
    }
}

prop_compose! {
    fn arb_channel_initialize_result()(
        protocol_version in any::<u32>(),
        capabilities in arb_channel_capabilities(),
    ) -> ChannelInitializeResult {
        ChannelInitializeResult { protocol_version, capabilities, defaults: None }
    }
}

prop_compose! {
    fn arb_deliver_message()(
        session_id in arb_string(),
        content in arb_json_value(),
    ) -> DeliverMessage {
        DeliverMessage { session_id, content }
    }
}

prop_compose! {
    fn arb_peer_info()(
        channel_name in arb_string(),
        peer_id in arb_string(),
        kind in arb_string(),
    ) -> PeerInfo {
        PeerInfo { channel_name, peer_id, kind }
    }
}

prop_compose! {
    fn arb_channel_send_message()(
        peer_info in arb_peer_info(),
        content in arb_string(),
    ) -> ChannelSendMessage {
        ChannelSendMessage { peer_info, content }
    }
}

prop_compose! {
    fn arb_thought_content()(
        session_id in arb_string(),
        update_type in arb_string(),
        content in arb_string(),
    ) -> ThoughtContent {
        ThoughtContent { session_id, update_type, content }
    }
}

prop_compose! {
    fn arb_ack_notification()(
        session_id in arb_string(),
        channel_name in arb_string(),
        peer_id in arb_string(),
        message_id in arb_opt_string(),
    ) -> AckNotification {
        AckNotification { session_id, channel_name, peer_id, message_id }
    }
}

prop_compose! {
    fn arb_ack_lifecycle_notification()(
        session_id in arb_string(),
        action in arb_string(),
        stop_reason in proptest::option::of(prop_oneof![
            Just(StopReason::EndTurn),
            Just(StopReason::MaxTokens),
            Just(StopReason::MaxTurnRequests),
            Just(StopReason::Refusal),
            Just(StopReason::Cancelled),
        ]),
    ) -> AckLifecycleNotification {
        AckLifecycleNotification { session_id, action, stop_reason }
    }
}

prop_compose! {
    fn arb_channel_respond_permission()(
        request_id in arb_string(),
        option_id in arb_string(),
    ) -> ChannelRespondPermission {
        ChannelRespondPermission { request_id, option_id }
    }
}

prop_compose! {
    fn arb_session_created()(
        session_id in arb_string(),
        peer_info in arb_peer_info(),
    ) -> SessionCreated {
        SessionCreated { session_id, peer_info }
    }
}

// ── Permission type strategies ──────────────────────────────────────────

prop_compose! {
    fn arb_permission_option()(
        option_id in arb_string(),
        label in arb_string(),
    ) -> PermissionOption {
        PermissionOption { option_id, label }
    }
}

prop_compose! {
    fn arb_permission_request()(
        request_id in arb_string(),
        description in arb_string(),
        options in proptest::collection::vec(arb_permission_option(), 0..3),
    ) -> PermissionRequest {
        PermissionRequest { request_id, description, options }
    }
}

prop_compose! {
    fn arb_permission_response()(
        request_id in arb_string(),
        option_id in arb_string(),
    ) -> PermissionResponse {
        PermissionResponse { request_id, option_id }
    }
}

prop_compose! {
    fn arb_channel_request_permission()(
        request_id in arb_string(),
        session_id in arb_string(),
        description in arb_string(),
        options in proptest::collection::vec(arb_permission_option(), 0..3),
    ) -> ChannelRequestPermission {
        ChannelRequestPermission { request_id, session_id, description, options }
    }
}

// ── SessionKey strategy ────────────────────────────────────────────────

prop_compose! {
    fn arb_session_key()(
        channel in "[a-zA-Z0-9_-]{1,10}",
        kind in "[a-zA-Z0-9_-]{1,10}",
        peer in "[a-zA-Z0-9_-]{1,10}",
    ) -> SessionKey {
        SessionKey::new(&channel, &kind, &peer)
    }
}

// ── ACP round-trip property tests ───────────────────────────────────────

/// Helper macro: serialize → deserialize round-trip assertion.
macro_rules! assert_round_trip {
    ($val:expr, $ty:ty) => {{
        let json = serde_json::to_string(&$val).unwrap();
        let restored: $ty = serde_json::from_str(&json).unwrap();
        assert_eq!($val, restored);
    }};
}

proptest! {
    #[test]
    fn acp_client_capabilities_round_trips(val in arb_client_capabilities()) {
        assert_round_trip!(val, ClientCapabilities);
    }

    #[test]
    fn acp_initialize_params_round_trips(val in arb_initialize_params()) {
        assert_round_trip!(val, InitializeParams);
    }

    #[test]
    fn acp_initialize_result_round_trips(val in arb_initialize_result()) {
        assert_round_trip!(val, InitializeResult);
    }

    #[test]
    fn acp_mcp_server_info_round_trips(val in arb_mcp_server_info()) {
        assert_round_trip!(val, McpServerInfo);
    }

    #[test]
    fn acp_session_new_params_round_trips(val in arb_session_new_params()) {
        assert_round_trip!(val, SessionNewParams);
    }

    #[test]
    fn acp_session_new_result_round_trips(val in arb_session_new_result()) {
        assert_round_trip!(val, SessionNewResult);
    }

    #[test]
    fn acp_content_part_round_trips(val in arb_content_part()) {
        assert_round_trip!(val, ContentPart);
    }

    #[test]
    fn acp_session_prompt_params_round_trips(val in arb_session_prompt_params()) {
        assert_round_trip!(val, SessionPromptParams);
    }

    #[test]
    fn acp_session_cancel_params_round_trips(val in arb_session_cancel_params()) {
        assert_round_trip!(val, SessionCancelParams);
    }

    #[test]
    fn acp_session_fork_params_round_trips(val in arb_session_fork_params()) {
        assert_round_trip!(val, SessionForkParams);
    }

    #[test]
    fn acp_session_fork_result_round_trips(val in arb_session_fork_result()) {
        assert_round_trip!(val, SessionForkResult);
    }

    #[test]
    fn acp_session_list_params_round_trips(val in arb_session_list_params()) {
        assert_round_trip!(val, SessionListParams);
    }

    #[test]
    fn acp_session_info_round_trips(val in arb_session_info()) {
        assert_round_trip!(val, SessionInfo);
    }

    #[test]
    fn acp_session_list_result_round_trips(val in arb_session_list_result()) {
        assert_round_trip!(val, SessionListResult);
    }

    #[test]
    fn acp_session_load_params_round_trips(val in arb_session_load_params()) {
        assert_round_trip!(val, SessionLoadParams);
    }

    #[test]
    fn acp_tool_call_status_round_trips(val in arb_tool_call_status()) {
        assert_round_trip!(val, ToolCallStatus);
    }
}

// ── Channel round-trip property tests ───────────────────────────────────

proptest! {
    #[test]
    fn channel_capabilities_round_trips(val in arb_channel_capabilities()) {
        assert_round_trip!(val, ChannelCapabilities);
    }

    #[test]
    fn channel_ack_config_round_trips(val in arb_channel_ack_config()) {
        assert_round_trip!(val, ChannelAckConfig);
    }

    #[test]
    fn channel_initialize_params_round_trips(val in arb_channel_initialize_params()) {
        assert_round_trip!(val, ChannelInitializeParams);
    }

    #[test]
    fn channel_initialize_result_round_trips(val in arb_channel_initialize_result()) {
        assert_round_trip!(val, ChannelInitializeResult);
    }

    #[test]
    fn channel_deliver_message_round_trips(val in arb_deliver_message()) {
        assert_round_trip!(val, DeliverMessage);
    }

    #[test]
    fn channel_peer_info_round_trips(val in arb_peer_info()) {
        assert_round_trip!(val, PeerInfo);
    }

    #[test]
    fn channel_send_message_round_trips(val in arb_channel_send_message()) {
        assert_round_trip!(val, ChannelSendMessage);
    }

    #[test]
    fn channel_thought_content_round_trips(val in arb_thought_content()) {
        assert_round_trip!(val, ThoughtContent);
    }

    #[test]
    fn channel_ack_notification_round_trips(val in arb_ack_notification()) {
        assert_round_trip!(val, AckNotification);
    }

    #[test]
    fn channel_ack_lifecycle_notification_round_trips(val in arb_ack_lifecycle_notification()) {
        assert_round_trip!(val, AckLifecycleNotification);
    }

    #[test]
    fn channel_respond_permission_round_trips(val in arb_channel_respond_permission()) {
        assert_round_trip!(val, ChannelRespondPermission);
    }

    #[test]
    fn channel_session_created_round_trips(val in arb_session_created()) {
        assert_round_trip!(val, SessionCreated);
    }
}

// ── Permission + SessionKey round-trip property tests ────────────────────

proptest! {
    #[test]
    fn permission_option_round_trips(val in arb_permission_option()) {
        assert_round_trip!(val, PermissionOption);
    }

    #[test]
    fn permission_request_round_trips(val in arb_permission_request()) {
        assert_round_trip!(val, PermissionRequest);
    }

    #[test]
    fn permission_response_round_trips(val in arb_permission_response()) {
        assert_round_trip!(val, PermissionResponse);
    }

    #[test]
    fn channel_request_permission_round_trips(val in arb_channel_request_permission()) {
        assert_round_trip!(val, ChannelRequestPermission);
    }

    #[test]
    fn session_key_round_trips(val in arb_session_key()) {
        assert_round_trip!(val, SessionKey);
    }

    #[test]
    fn session_key_display_from_str_round_trips(val in arb_session_key()) {
        let displayed = val.to_string();
        let parsed: SessionKey = displayed.parse().unwrap();
        assert_eq!(val, parsed);
    }
}
