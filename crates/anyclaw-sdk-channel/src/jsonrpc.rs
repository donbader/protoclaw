//! Minimal JSON-RPC 2.0 types for the channel harness.
//!
//! Private to this crate — avoids depending on the internal `anyclaw-jsonrpc`
//! crate which is `publish = false`. Only the subset needed by `ChannelHarness`
//! is defined here.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request/response id — String or Number.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum RequestId {
    /// Numeric id (most common in practice).
    Number(i64),
    /// String id (used by some agent implementations).
    String(String),
}

// D-03 extensible: params schema varies per JSON-RPC method
#[allow(clippy::disallowed_types)]
/// A JSON-RPC 2.0 request (or notification if `id` is `None`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

// D-03 extensible: result schema varies per JSON-RPC method
#[allow(clippy::disallowed_types)]
/// A JSON-RPC 2.0 response (success or error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

// D-03 extensible: error data is implementation-defined
#[allow(clippy::disallowed_types)]
/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    #[allow(clippy::disallowed_types)]
    pub fn new(
        method: impl Into<String>,
        id: Option<RequestId>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

impl JsonRpcResponse {
    #[allow(clippy::disallowed_types)]
    pub fn success(id: Option<RequestId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<RequestId>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}
