use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
}

impl JsonRpcRequest {
    pub fn new(
        method: impl Into<String>,
        id: Option<serde_json::Value>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }

    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn when_creating_request_then_jsonrpc_field_is_2_0() {
        let req = JsonRpcRequest::new("test_method", Some(serde_json::json!(1)), None);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "test_method");
        assert_eq!(json["id"], 1);
    }

    #[test]
    fn when_request_has_no_id_then_id_omitted_from_json() {
        let req = JsonRpcRequest::new("notify", None, None);
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("id").is_none());
    }

    #[test]
    fn when_id_is_none_then_is_notification_returns_true() {
        let req = JsonRpcRequest::new("notify", None, None);
        assert!(req.is_notification());

        let req_with_id = JsonRpcRequest::new("call", Some(serde_json::json!(1)), None);
        assert!(!req_with_id.is_notification());
    }

    #[test]
    fn when_response_has_result_then_error_field_omitted() {
        let resp = JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!("ok"));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["result"], "ok");
        assert!(json.get("error").is_none());
    }

    #[test]
    fn when_response_has_error_then_result_field_omitted() {
        let err = JsonRpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        };
        let resp = JsonRpcResponse::error(Some(serde_json::json!(1)), err);
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["error"]["code"], -32600);
        assert_eq!(json["error"]["message"], "Invalid Request");
        assert!(json.get("result").is_none());
    }

    #[test]
    fn when_error_has_data_then_included_when_none_then_omitted() {
        let err = JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: Some(serde_json::json!({"detail": "no such method"})),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], -32601);
        assert_eq!(json["message"], "Method not found");
        assert_eq!(json["data"]["detail"], "no such method");

        let err_no_data = JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        };
        let json2 = serde_json::to_value(&err_no_data).unwrap();
        assert!(json2.get("data").is_none());
    }

    #[test]
    fn when_json_has_method_field_then_deserializes_as_request() {
        let json_str = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.method, "test");
                assert_eq!(req.id, Some(serde_json::json!(1)));
            }
            _ => panic!("Expected Request variant"),
        }
    }

    #[test]
    fn when_json_has_result_field_then_deserializes_as_response() {
        let json_str = r#"{"jsonrpc":"2.0","id":1,"result":"hello"}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            JsonRpcMessage::Response(resp) => {
                assert_eq!(resp.result, Some(serde_json::json!("hello")));
            }
            _ => panic!("Expected Response variant"),
        }
    }

    #[test]
    fn when_request_serialized_then_deserializes_to_equal_value() {
        let req = JsonRpcRequest::new(
            "test_method",
            Some(serde_json::json!(42)),
            Some(serde_json::json!({"key": "value"})),
        );
        let json_str = serde_json::to_string(&req).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(req, deserialized);
    }
}
