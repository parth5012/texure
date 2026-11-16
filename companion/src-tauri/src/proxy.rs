use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use texure_contract::error::ErrorCode;
use tracing::{debug, warn};

/// Gateway tunnel RPC request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRequest {
    #[serde(default = "default_version")]
    pub v: u32,
    pub seq: Option<u64>,
    #[serde(default)]
    pub auth: Option<Value>,
    pub payload: Option<GatewayPayload>,
    // JSON-RPC fallback fields
    #[serde(default)]
    pub jsonrpc: Option<String>,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
}

fn default_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayPayload {
    #[serde(rename = "type", default = "default_rpc_type")]
    pub payload_type: String,
    pub rpc: String,
    #[serde(default)]
    pub args: Option<Value>,
    #[serde(default)]
    pub verify_at_execute: Option<bool>,
}

fn default_rpc_type() -> String {
    "rpc".to_string()
}

/// Local loopback proxy router forwarding remote agent tool calls to target bridge at 127.0.0.1:8765.
#[derive(Clone)]
pub struct ProxyRouter {
    target_base_url: String,
    auth_token: Option<String>,
    client: Client,
}

impl Default for ProxyRouter {
    fn default() -> Self {
        Self::new("http://127.0.0.1:8765")
    }
}

impl ProxyRouter {
    pub fn new(target_base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_millis(800))
            .build()
            .expect("create reqwest client");

        Self {
            target_base_url: target_base_url.into().trim_end_matches('/').to_string(),
            auth_token: None,
            client,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Check whether local bridge is responsive (GET /ui-tree or health).
    pub async fn check_target_status(&self) -> bool {
        let url = format!("{}/ui-tree", self.target_base_url);
        let mut req = self.client.get(&url);
        if let Some(ref tok) = self.auth_token {
            req = req.bearer_auth(tok);
        }
        match req.send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Route incoming message string (from reverse tunnel) and return response string.
    pub async fn handle_message(&self, msg: &str) -> Option<String> {
        let parsed: Result<GatewayRequest, _> = serde_json::from_str(msg);
        let Ok(req) = parsed else {
            return None;
        };

        // Determine if this is a Gateway RPC envelope or JSON-RPC 2.0
        if let Some(ref payload) = req.payload {
            let rpc = &payload.rpc;
            let args = payload.args.clone().unwrap_or_else(|| json!({}));
            let seq = req.seq.unwrap_or(0);

            let result = self.dispatch_tool_call(rpc, args).await;
            let reply = match result {
                Ok(val) => json!({
                    "v": req.v,
                    "seq": seq,
                    "payload": val,
                }),
                Err(err_payload) => json!({
                    "v": req.v,
                    "seq": seq,
                    "payload": {
                        "error": err_payload
                    }
                }),
            };
            return Some(serde_json::to_string(&reply).unwrap_or_default());
        }

        if let Some(ref method) = req.method {
            let id = req.id.clone().unwrap_or(json!(null));
            let params = req.params.clone().unwrap_or_else(|| json!({}));

            let result = self.dispatch_tool_call(method, params).await;
            let reply = match result {
                Ok(val) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": val,
                }),
                Err(err_payload) => {
                    let code = if err_payload["code"] == "OFFLINE" {
                        -32001
                    } else {
                        -32603
                    };
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": code,
                            "message": err_payload["message"],
                            "data": err_payload
                        }
                    })
                }
            };
            return Some(serde_json::to_string(&reply).unwrap_or_default());
        }

        None
    }

    /// Dispatch tool call to local HTTP bridge.
    async fn dispatch_tool_call(&self, rpc: &str, args: Value) -> Result<Value, Value> {
        let (endpoint, is_post) = match rpc {
            "device_tree" | "ui_tree" | "/ui-tree" => ("/ui-tree".to_string(), false),
            "tap" | "/tap" => ("/tap".to_string(), true),
            "type" | "/type" => ("/type".to_string(), true),
            "screenshot" | "/screenshot" => ("/screenshot".to_string(), false),
            other => {
                let path = if other.starts_with('/') {
                    other.to_string()
                } else {
                    format!("/{}", other)
                };
                let has_args = !args.is_null() && args != json!({});
                (path, has_args)
            }
        };

        let target_url = format!("{}{}", self.target_base_url, endpoint);
        debug!(rpc, url = %target_url, "Proxying tool call to target bridge");

        let mut request_builder = if is_post {
            self.client.post(&target_url).json(&args)
        } else {
            self.client.get(&target_url)
        };

        if let Some(ref token) = self.auth_token {
            request_builder = request_builder.bearer_auth(token);
        }

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();
                let bytes = match response.bytes().await {
                    Ok(b) => b,
                    Err(e) => {
                        warn!(error = %e, "Failed to read response body from bridge");
                        return Err(json!({
                            "code": ErrorCode::CaptureFailed.as_str(),
                            "message": format!("Failed reading bridge response body: {}", e),
                            "hint": "target bridge dropped connection mid-read",
                            "relayed": true,
                        }));
                    }
                };
                let parsed: Result<Value, _> = serde_json::from_slice(&bytes);

                match parsed {
                    Ok(val) => {
                        if status.is_success() {
                            Ok(val)
                        } else {
                            // Target returned error envelope (e.g. NODE_OUTDATED)
                            let mut err_obj = if let Some(e) = val.get("error").cloned() {
                                e
                            } else {
                                val
                            };
                            if let Some(map) = err_obj.as_object_mut() {
                                map.insert("relayed".to_string(), json!(true));
                            }
                            Err(err_obj)
                        }
                    }
                    Err(_) => {
                        if status.is_success() {
                            if endpoint == "/screenshot" || bytes.starts_with(b"\x89PNG") {
                                use base64::engine::general_purpose::STANDARD;
                                use base64::Engine;
                                Ok(json!({
                                    "image": STANDARD.encode(&bytes),
                                    "mime": "image/png"
                                }))
                            } else {
                                Ok(json!({ "raw": String::from_utf8_lossy(&bytes) }))
                            }
                        } else {
                            Err(json!({
                                "code": ErrorCode::InjectFailed.as_str(),
                                "message": format!("HTTP error: status {}", status),
                                "hint": "check target bridge logs",
                                "relayed": true,
                            }))
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Target bridge connection failed or timed out -> OFFLINE");
                Err(json!({
                    "code": ErrorCode::Offline.as_str(),
                    "message": "target bridge unreachable (app backgrounded or stopped)",
                    "hint": "bring target app to foreground or relaunch",
                    "relayed": false,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_proxy_relay_device_tree_and_tap() {
        // Mock target HTTP bridge
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        tokio::spawn(async move {
            use bytes::Bytes;
            use http_body_util::Full;
            use hyper::service::service_fn;
            use hyper::{Request, Response, StatusCode};
            use hyper_util::rt::TokioIo;

            while let Ok((stream, _)) = listener.accept().await {
                let io = TokioIo::new(stream);
                tokio::spawn(async move {
                    let service = service_fn(|req: Request<hyper::body::Incoming>| async move {
                        let path = req.uri().path();
                        if path == "/ui-tree" {
                            let body = json!({
                                "rev": 8,
                                "nodes": [{ "id": 1, "text": "Home" }]
                            });
                            Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(
                                serde_json::to_vec(&body).unwrap(),
                            ))))
                        } else if path == "/tap" {
                            let body = json!({
                                "ok": true,
                                "rev": 9,
                                "verify_at_execute": true
                            });
                            Ok::<_, Infallible>(Response::new(Full::new(Bytes::from(
                                serde_json::to_vec(&body).unwrap(),
                            ))))
                        } else {
                            let mut not_found = Response::new(Full::new(Bytes::from("not found")));
                            *not_found.status_mut() = StatusCode::NOT_FOUND;
                            Ok::<_, Infallible>(not_found)
                        }
                    });

                    let _ = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    )
                    .serve_connection(io, service)
                    .await;
                });
            }
        });

        let proxy = ProxyRouter::new(&base_url);

        // 1. Gateway envelope GET ui-tree
        let tree_req = json!({
            "v": 1,
            "seq": 101,
            "payload": {
                "type": "rpc",
                "rpc": "device_tree",
                "args": {}
            }
        });
        let tree_res = proxy
            .handle_message(&tree_req.to_string())
            .await
            .expect("response");
        let tree_val: Value = serde_json::from_str(&tree_res).unwrap();
        assert_eq!(tree_val["v"], 1);
        assert_eq!(tree_val["seq"], 101);
        assert_eq!(tree_val["payload"]["rev"], 8);
        assert_eq!(tree_val["payload"]["nodes"][0]["text"], "Home");

        // 2. Gateway envelope POST tap
        let tap_req = json!({
            "v": 1,
            "seq": 102,
            "payload": {
                "type": "rpc",
                "rpc": "tap",
                "args": { "node": 1, "rev": 8 },
                "verify_at_execute": true
            }
        });
        let tap_res = proxy
            .handle_message(&tap_req.to_string())
            .await
            .expect("response");
        let tap_val: Value = serde_json::from_str(&tap_res).unwrap();
        assert_eq!(tap_val["v"], 1);
        assert_eq!(tap_val["seq"], 102);
        assert_eq!(tap_val["payload"]["ok"], true);
        assert_eq!(tap_val["payload"]["verify_at_execute"], true);
    }

    #[tokio::test]
    async fn test_proxy_error_propagation_and_offline() {
        // Test OFFLINE detection when target port is closed
        let closed_port_url = "http://127.0.0.1:49151";
        let proxy = ProxyRouter::new(closed_port_url);

        let req = json!({
            "v": 1,
            "seq": 201,
            "payload": {
                "type": "rpc",
                "rpc": "tap",
                "args": { "node": 1, "rev": 1 }
            }
        });

        let res = proxy
            .handle_message(&req.to_string())
            .await
            .expect("response");
        let val: Value = serde_json::from_str(&res).unwrap();
        assert_eq!(val["seq"], 201);
        assert_eq!(val["payload"]["error"]["code"], "OFFLINE");
        assert_eq!(val["payload"]["error"]["relayed"], false);

        assert!(!proxy.check_target_status().await);
    }
}
