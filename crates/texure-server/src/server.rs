use crate::bridge_client::BridgeClient;
use crate::config::load_resolved_config;
use crate::wait_for::execute_wait_for;
use crate::ws_supervisor::WsSupervisor;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use texure_contract::bridge::{KeyPressRequest, TapRequest, TypeRequest};
use texure_contract::error::ErrorCode;
use texure_contract::rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use texure_contract::tools::{
    CallToolResult, DeviceTreeParams, KeyPressParams, ScreenshotParams, TapParams, TypeParams,
    WaitForParams, FROZEN_TOOL_NAMES,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// Stdio MCP server state.
pub struct McpServer {
    pub custom_config_path: Option<PathBuf>,
    bridge: Arc<Mutex<Option<(BridgeClient, WsSupervisor)>>>,
}

impl McpServer {
    pub fn new(custom_config_path: Option<PathBuf>) -> Self {
        Self {
            custom_config_path,
            bridge: Arc::new(Mutex::new(None)),
        }
    }

    /// Run the main stdin / stdout JSON-RPC 2.0 loop.
    pub async fn run_stdio(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        debug!("McpServer listening on standard I/O...");

        while let Some(line) = reader.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(req) => req,
                Err(_) => {
                    let err_resp = JsonRpcResponse::error(None, JsonRpcError::parse_error());
                    let mut out = serde_json::to_string(&err_resp)?;
                    out.push('\n');
                    stdout.write_all(out.as_bytes()).await?;
                    stdout.flush().await?;
                    continue;
                }
            };

            // Notifications have no id member and must not receive a response per JSON-RPC 2.0
            let req_id = match request.id {
                Some(ref id) => id.clone(),
                None => {
                    debug!("Notification received: {}", request.method);
                    continue;
                }
            };

            let response = self
                .handle_request(Some(req_id.clone()), &request.method, request.params)
                .await;

            if let Some(resp) = response {
                let mut out = serde_json::to_string(&resp)?;
                out.push('\n');
                stdout.write_all(out.as_bytes()).await?;
                stdout.flush().await?;
            }
        }

        info!("Stdin closed; McpServer shutting down cleanly.");
        Ok(())
    }

    async fn handle_request(
        &self,
        id: Option<Value>,
        method: &str,
        params: Option<Value>,
    ) -> Option<JsonRpcResponse> {
        let params_val = params.unwrap_or(Value::Null);

        match method {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "texure-server",
                        "version": "0.1.0"
                    }
                });
                Some(JsonRpcResponse::success(id, result))
            }
            "ping" => Some(JsonRpcResponse::success(id, serde_json::json!({}))),
            "tools/list" => {
                let tools = self.list_tools();
                let result = serde_json::json!({ "tools": tools });
                Some(JsonRpcResponse::success(id, result))
            }
            "tools/call" => match self.call_tool(params_val).await {
                Ok(tool_result) => {
                    let val = serde_json::to_value(&tool_result).unwrap_or(Value::Null);
                    Some(JsonRpcResponse::success(id, val))
                }
                Err(rpc_err) => Some(JsonRpcResponse::error(id, rpc_err)),
            },
            _ => Some(JsonRpcResponse::error(
                id,
                JsonRpcError::method_not_found(method),
            )),
        }
    }

    fn list_tools(&self) -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "device_tree",
                "description": "Fetches accessibility/view hierarchy tree of the active Android window",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "max_depth": { "type": "integer", "description": "Maximum tree traversal depth" },
                        "subtree": { "type": "string", "description": "Optional subtree node id" },
                        "verbose": { "type": "boolean", "description": "Return full error payload on error" }
                    }
                }
            }),
            serde_json::json!({
                "name": "tap",
                "description": "Tap an Android UI node by id with rev check",
                "inputSchema": {
                    "type": "object",
                    "required": ["node", "rev"],
                    "properties": {
                        "node": { "type": "integer" },
                        "index": { "type": "integer" },
                        "rev": { "type": "integer" },
                        "verbose": { "type": "boolean" }
                    }
                }
            }),
            serde_json::json!({
                "name": "type",
                "description": "Type text into an editable Android node",
                "inputSchema": {
                    "type": "object",
                    "required": ["node", "text", "rev"],
                    "properties": {
                        "node": { "type": "integer" },
                        "index": { "type": "integer" },
                        "text": { "type": "string" },
                        "rev": { "type": "integer" },
                        "verbose": { "type": "boolean" }
                    }
                }
            }),
            serde_json::json!({
                "name": "keypress",
                "description": "Send a key event (ENTER, BACK, DEL, TAB)",
                "inputSchema": {
                    "type": "object",
                    "required": ["key"],
                    "properties": {
                        "key": { "type": "string", "enum": ["ENTER", "BACK", "DEL", "TAB"] },
                        "verbose": { "type": "boolean" }
                    }
                }
            }),
            serde_json::json!({
                "name": "screenshot",
                "description": "Capture screenshot of the active window",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "scale": { "type": "number", "enum": [0.25, 0.5, 1.0] },
                        "verbose": { "type": "boolean" }
                    }
                }
            }),
            serde_json::json!({
                "name": "wait_for",
                "description": "Wait for a UI condition to be satisfied",
                "inputSchema": {
                    "type": "object",
                    "required": ["where", "value"],
                    "properties": {
                        "where": { "type": "string", "enum": ["text", "tag", "visible", "hidden"] },
                        "value": { "type": "string" },
                        "timeout": { "type": "integer", "maximum": 30 },
                        "verbose": { "type": "boolean" }
                    }
                }
            }),
        ]
    }

    /// Execute a `tools/call`. Protocol-level problems (unknown tool, malformed
    /// arguments) are `Err` and surface as JSON-RPC -32602; tool outcomes —
    /// including bridge failures — are `Ok(CallToolResult)`.
    async fn call_tool(&self, params: Value) -> Result<CallToolResult, JsonRpcError> {
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("missing tool name"))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let verbose = args
            .get("verbose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !FROZEN_TOOL_NAMES.contains(&name) {
            return Err(JsonRpcError::invalid_params(&format!(
                "Unknown tool: {name}"
            )));
        }

        // Lazy configuration and bridge connection setup
        let (bridge, ws) = match self.get_or_init_bridge().await {
            Ok(pair) => pair,
            Err(code) => {
                let msg = code.default_message();
                return Ok(CallToolResult::error(code, Some(msg), verbose, None));
            }
        };

        match name {
            "device_tree" => {
                let dt_params: DeviceTreeParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid device_tree arguments: {e}"))
                })?;
                match bridge
                    .get_ui_tree(dt_params.max_depth, dt_params.subtree.as_deref())
                    .await
                {
                    Ok(tree) => {
                        let text = serde_json::to_string(&tree).unwrap_or_default();
                        Ok(CallToolResult::success_text(text))
                    }
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            "tap" => {
                let tap_params: TapParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid tap arguments: {e}"))
                })?;

                let mut req = TapRequest::new(tap_params.rev);
                if let Some(n) = tap_params.node {
                    req = req.with_node(n);
                }
                if let Some(i) = tap_params.index {
                    req = req.with_index(i);
                }

                match bridge.tap(&req).await {
                    Ok(resp) => {
                        let text = serde_json::to_string(&resp).unwrap_or_default();
                        Ok(CallToolResult::success_text(text))
                    }
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            "type" => {
                let type_params: TypeParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid type arguments: {e}"))
                })?;

                let mut req = TypeRequest::new(type_params.text, type_params.rev);
                if let Some(n) = type_params.node {
                    req = req.with_node(n);
                }
                if let Some(i) = type_params.index {
                    req = req.with_index(i);
                }

                match bridge.type_text(&req).await {
                    Ok(resp) => {
                        let text = serde_json::to_string(&resp).unwrap_or_default();
                        Ok(CallToolResult::success_text(text))
                    }
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            "keypress" => {
                let key_params: KeyPressParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid keypress arguments: {e}"))
                })?;

                let req = KeyPressRequest::new(key_params.key);
                match bridge.keypress(&req).await {
                    Ok(resp) => {
                        let text = serde_json::to_string(&resp).unwrap_or_default();
                        Ok(CallToolResult::success_text(text))
                    }
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            "screenshot" => {
                let ss_params: ScreenshotParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid screenshot arguments: {e}"))
                })?;
                match bridge.screenshot(ss_params.scale).await {
                    Ok(bytes) => {
                        let base64_png = BASE64.encode(&bytes);
                        let mut res = CallToolResult::success_image(base64_png);
                        if verbose {
                            let meta = serde_json::json!({
                                "bytes_length": bytes.len(),
                                "scale": ss_params.scale.unwrap_or(0.5),
                            });
                            res.content.push(texure_contract::tools::McpContent::text(
                                serde_json::to_string(&meta).unwrap_or_default(),
                            ));
                        }
                        Ok(res)
                    }
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            "wait_for" => {
                let wf_params: WaitForParams = serde_json::from_value(args).map_err(|e| {
                    JsonRpcError::invalid_params(&format!("Invalid wait_for arguments: {e}"))
                })?;

                match execute_wait_for(&wf_params, &bridge, &ws).await {
                    Ok(res) => Ok(res),
                    Err(err) => {
                        Ok(err.to_mcp_result(verbose, &bridge.config.url, ws.is_connected()))
                    }
                }
            }
            _ => Err(JsonRpcError::invalid_params(&format!(
                "Unknown tool: {name}"
            ))),
        }
    }

    async fn get_or_init_bridge(&self) -> Result<(BridgeClient, WsSupervisor), ErrorCode> {
        let mut guard = self.bridge.lock().await;
        if let Some((ref client, ref ws)) = *guard {
            return Ok((client.clone(), ws.clone()));
        }

        let resolved = load_resolved_config(self.custom_config_path.as_deref())?;
        let client = BridgeClient::new(resolved.clone()).map_err(|e| {
            error!("Bridge client creation failed: {e}");
            ErrorCode::Offline
        })?;

        let ws = WsSupervisor::new(resolved);
        ws.spawn();

        *guard = Some((client.clone(), ws.clone()));
        Ok((client, ws))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_initialize_and_tools_list() {
        let server = McpServer::new(None);

        let init_resp = server
            .handle_request(Some(Value::from(1)), "initialize", None)
            .await
            .expect("init response");

        assert_eq!(init_resp.id, Some(Value::from(1)));
        let result = init_resp.result.expect("result");
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "texure-server");

        let tools_resp = server
            .handle_request(Some(Value::from(2)), "tools/list", None)
            .await
            .expect("tools list response");
        let tools = tools_resp.result.expect("tools result")["tools"]
            .as_array()
            .cloned()
            .expect("tools array");
        assert_eq!(tools.len(), 6);

        let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert_eq!(
            tool_names,
            vec![
                "device_tree",
                "tap",
                "type",
                "keypress",
                "screenshot",
                "wait_for"
            ]
        );
    }

    #[tokio::test]
    async fn test_server_no_config_error_response() {
        // Clear any env vars that might resolve
        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");

        let server = McpServer::new(Some(PathBuf::from("/nonexistent/texure_config.toml")));
        let call_params = serde_json::json!({
            "name": "device_tree",
            "arguments": {}
        });

        let resp = server
            .handle_request(Some(Value::from(3)), "tools/call", Some(call_params))
            .await
            .expect("tool call response");

        let result = resp.result.expect("tool call result");
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().expect("error text");
        assert!(text.contains("NO_CONFIG: no config file or env vars"));
    }
}
