use serde::{Deserialize, Serialize};

/// A single option in a permission prompt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub label: String,
}

/// Permission request from agent, forwarded to channel for user decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    pub request_id: String,
    pub description: String,
    pub options: Vec<PermissionOption>,
}

/// User's response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    pub request_id: String,
    pub option_id: String,
}

/// Protoclaw → Channel: show permission prompt to user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRequestPermission {
    pub request_id: String,
    pub session_id: String,
    pub description: String,
    pub options: Vec<PermissionOption>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_option_round_trip() {
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
    fn permission_request_round_trip() {
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
    fn permission_response_round_trip() {
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
    fn channel_request_permission_round_trip() {
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
