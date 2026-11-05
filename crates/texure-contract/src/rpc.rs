use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Standard JSON-RPC 2.0 Request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

// Distinguish an absent `id` (notification) from an explicit `"id": null`
// (a request that must receive a response per JSON-RPC 2.0).
impl<'de> Deserialize<'de> for JsonRpcRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawWithId {
            jsonrpc: String,
            id: Value,
            method: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            params: Option<Value>,
        }
        #[derive(Deserialize)]
        struct RawWithoutId {
            jsonrpc: String,
            method: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            params: Option<Value>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Envelope {
            WithId(RawWithId),
            WithoutId(RawWithoutId),
        }

        match Envelope::deserialize(deserializer)? {
            Envelope::WithId(r) => Ok(Self {
                jsonrpc: r.jsonrpc,
                id: Some(r.id),
                method: r.method,
                params: r.params,
            }),
            Envelope::WithoutId(r) => Ok(Self {
                jsonrpc: r.jsonrpc,
                id: None,
                method: r.method,
                params: r.params,
            }),
        }
    }
}

impl JsonRpcRequest {
    pub fn new(id: Option<Value>, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }

    pub fn with_numeric_id(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self::new(Some(Value::from(id)), method, params)
    }

    pub fn notification(method: impl Into<String>, params: Option<Value>) -> Self {
        Self::new(None, method, params)
    }
}

/// Standard JSON-RPC 2.0 Response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn matches_numeric_id(&self, target_id: u64) -> bool {
        match self.id {
            Some(Value::Number(ref n)) => n.as_u64() == Some(target_id),
            _ => false,
        }
    }
}

/// Standard JSON-RPC 2.0 Error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {method}"))
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self::new(-32602, format!("Invalid params: {msg}"))
    }

    pub fn internal_error(msg: &str) -> Self {
        Self::new(-32603, format!("Internal error: {msg}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_id_is_a_request_not_a_notification() {
        let req: JsonRpcRequest =
            serde_json::from_str(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#)
                .expect("parse null-id request");
        assert_eq!(req.id, Some(Value::Null));

        let notif: JsonRpcRequest = serde_json::from_str(r#"{"jsonrpc":"2.0","method":"ping"}"#)
            .expect("parse notification");
        assert_eq!(notif.id, None);
    }

    #[test]
    fn test_jsonrpc_request_response_roundtrip() {
        let req = JsonRpcRequest::with_numeric_id(1, "tools/list", None);
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
        assert!(s.contains("\"method\":\"tools/list\""));

        let resp = JsonRpcResponse::success(Some(Value::from(1)), serde_json::json!({"tools": []}));
        let s_resp = serde_json::to_string(&resp).unwrap();
        assert!(s_resp.contains("\"result\":{\"tools\":[]}"));
    }
}
