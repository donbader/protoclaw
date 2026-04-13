use serde::{Deserialize, Serialize};

/// A single option in a permission prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    /// Machine-readable identifier for this option (e.g. `"allow_once"`).
    pub option_id: String,
    /// Human-readable label shown to the user.
    ///
    /// Accepts `"name"` as an alias on deserialization for compatibility with
    /// agents that use `name` instead of `label` (e.g. OpenCode).
    #[serde(alias = "name")]
    pub label: String,
}

/// Permission request from agent, forwarded to channel for user decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    /// Unique identifier for correlating the response.
    pub request_id: String,
    /// Human-readable description of what the agent is requesting.
    pub description: String,
    /// Available choices the user can select from.
    pub options: Vec<PermissionOption>,
}

/// User's response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    /// Identifier matching the original [`PermissionRequest`].
    pub request_id: String,
    /// The `option_id` the user selected.
    pub option_id: String,
}

/// Anyclaw → Channel: show permission prompt to user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRequestPermission {
    /// Unique identifier for correlating the response.
    pub request_id: String,
    /// Session the permission request belongs to.
    pub session_id: String,
    /// Human-readable description of what is being requested.
    pub description: String,
    /// Available choices the user can select from.
    pub options: Vec<PermissionOption>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_serializing_permission_option_then_uses_camel_case() {
        let opt = PermissionOption {
            option_id: "allow_once".into(),
            label: "Allow once".into(),
        };
        let json = serde_json::to_value(&opt).unwrap();
        assert_eq!(json["optionId"], "allow_once");
        assert_eq!(json["label"], "Allow once");
        assert!(json.get("option_id").is_none());
        let deser: PermissionOption = serde_json::from_value(json).unwrap();
        assert_eq!(deser, opt);
    }

    #[test]
    fn when_serializing_permission_request_then_uses_camel_case() {
        let req = PermissionRequest {
            request_id: "perm-1".into(),
            description: "Allow file write?".into(),
            options: vec![
                PermissionOption {
                    option_id: "allow".into(),
                    label: "Allow".into(),
                },
                PermissionOption {
                    option_id: "deny".into(),
                    label: "Deny".into(),
                },
            ],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["requestId"], "perm-1");
        assert_eq!(json["description"], "Allow file write?");
        assert_eq!(json["options"].as_array().unwrap().len(), 2);
        assert!(json.get("request_id").is_none());
        let deser: PermissionRequest = serde_json::from_value(json).unwrap();
        assert_eq!(deser, req);
    }

    #[test]
    fn when_serializing_permission_response_then_uses_camel_case() {
        let resp = PermissionResponse {
            request_id: "perm-1".into(),
            option_id: "allow_once".into(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["requestId"], "perm-1");
        assert_eq!(json["optionId"], "allow_once");
        assert!(json.get("request_id").is_none());
        let deser: PermissionResponse = serde_json::from_value(json).unwrap();
        assert_eq!(deser, resp);
    }

    #[test]
    fn when_deserializing_permission_option_with_name_alias_then_maps_to_label() {
        let json = serde_json::json!({"optionId": "once", "name": "Allow once"});
        let opt: PermissionOption = serde_json::from_value(json).unwrap();
        assert_eq!(opt.label, "Allow once");
        assert_eq!(opt.option_id, "once");
    }

    #[test]
    fn when_serializing_channel_request_permission_then_uses_camel_case() {
        let req = ChannelRequestPermission {
            request_id: "req-1".into(),
            session_id: "sess-1".into(),
            description: "Allow file write?".into(),
            options: vec![PermissionOption {
                option_id: "allow".into(),
                label: "Allow".into(),
            }],
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["requestId"], "req-1");
        assert_eq!(json["sessionId"], "sess-1");
        assert!(json.get("request_id").is_none());
        assert!(json.get("session_id").is_none());
        let deser: ChannelRequestPermission = serde_json::from_value(json).unwrap();
        assert_eq!(deser, req);
    }
}
