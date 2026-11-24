//! Transparent tool-call routing over the reverse tunnel (Group R, ticket #61).
//!
//! When a mobile companion is registered on the reverse WebSocket tunnel, all
//! six frozen MCP tools are forwarded to it as gateway RPC envelopes. Replies
//! are translated into the *same* `CallToolResult` shapes the direct LAN bridge
//! path produces, so MCP clients cannot tell the two transports apart.

use crate::bridge_client::BridgeCallError;
use crate::server::ToolArgs;
use crate::tunnel_server::{TunnelError, TunnelHandle};
use crate::wait_for::evaluate_condition;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use texure_contract::bridge::{BridgeSuccessResponse, TapRequest, TypeRequest};
use texure_contract::error::ErrorCode;
use texure_contract::tools::CallToolResult;
use texure_contract::tree::UiTree;
use tokio::time::Instant;

/// Context string used in verbose errors routed through the tunnel. The direct
/// path reports its LAN bridge URL here; the tunnel's analogous descriptor is
/// the companion endpoint.
const TUNNEL_BRIDGE_URL: &str = "tunnel://companion";
/// The tunnel carries its own live session; mirror the direct path's
/// `ws.is_connected() == true` state in verbose error context.
const TUNNEL_WS_CONNECTED: bool = true;

/// Why a tunneled call failed. Transport failures are distinct from tool-level
/// errors so the server can fall back to the next configured transport; a
/// relayed bridge error (e.g. `NODE_OUTDATED`) must stay a tool error and never
/// trigger fallback (guardrail G8 + `transport_order.json`).
#[derive(Debug)]
pub(crate) enum RouteError {
    Transport(TunnelError),
    Tool(CallToolResult),
}

/// Routes frozen tool calls through a connected companion tunnel.
pub struct TunnelRouter {
    handle: TunnelHandle,
    next_seq: AtomicU64,
    lifecycle_probe: bool,
}

impl TunnelRouter {
    /// Build a router over a live tunnel handle. Each router has its own
    /// monotonic `seq` space so replies cannot collide across instances.
    pub fn new(handle: TunnelHandle) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        Self {
            handle,
            next_seq: AtomicU64::new(seed.max(1)),
            lifecycle_probe: false,
        }
    }

    /// Opt in to a best-effort device-state probe + `shizuku_start_app` trigger
    /// before each call (wired via `texure-server --lifecycle-probe`). Off by
    /// default: the `device_state` RPC is not published by the companion yet,
    /// so enabling it before that lands is a harmless no-op with a short probe
    /// timeout. See DECISIONS.md for the dependency and follow-up issue.
    pub fn with_lifecycle_probe(mut self) -> Self {
        self.lifecycle_probe = true;
        self
    }

    pub fn is_connected(&self) -> bool {
        self.handle.is_connected()
    }

    /// Forward one validated tool call and translate the reply. All failures
    /// become `CallToolResult` errors — never panics — so the MCP daemon keeps
    /// running when the tunnel drops mid-call (guardrail G8).
    /// Forward one validated tool call. Transport-level failures are returned
    /// as `RouteError::Transport` so the caller can fall back; every other
    /// outcome is a normal tool result (including relayed bridge errors).
    pub(crate) async fn route_transport(
        &self,
        name: &str,
        parsed: &ToolArgs,
        verbose: bool,
    ) -> Result<CallToolResult, RouteError> {
        if self.lifecycle_probe {
            self.ensure_device_running().await;
        }

        // `wait_for` polls `device_tree` over the same tunnel: there is no
        // separate WS `tree_changed` channel, mirroring the direct path's
        // "WS down => 500ms HTTP poll" branch.
        if let ToolArgs::WaitFor(params) = parsed {
            return self.wait_for_via_tunnel(params, verbose).await;
        }

        let (rpc, args, verify_at_execute) = rpc_for(parsed);
        let envelope = self.envelope(rpc, args, verify_at_execute);
        let reply = self
            .request(&envelope)
            .await
            .map_err(RouteError::Transport)?;

        Ok(match decode_reply(&reply) {
            Ok(Reply::Success(payload)) => match self.success(name, parsed, payload) {
                Ok(result) => result,
                Err(error) => error.to_mcp_result(verbose, TUNNEL_BRIDGE_URL, TUNNEL_WS_CONNECTED),
            },
            Ok(Reply::Error(relayed)) => {
                let code = relayed_code(&relayed);
                let message = relayed["message"]
                    .as_str()
                    .unwrap_or_else(|| code.default_message())
                    .to_string();
                let error = BridgeCallError {
                    code,
                    message,
                    // Keep the full relayed object (code, message, hint,
                    // relayed:true) verbatim for the verbose JSON branch.
                    raw_json: Some(json!({ "error": relayed })),
                    http_status: None,
                };
                error.to_mcp_result(verbose, TUNNEL_BRIDGE_URL, TUNNEL_WS_CONNECTED)
            }
            Err(error) => error.to_mcp_result(verbose, TUNNEL_BRIDGE_URL, TUNNEL_WS_CONNECTED),
        })
    }

    /// Render a tunneled call as a plain tool result. The production path uses
    /// `route_transport` so it can fall back; this convenience wrapper keeps
    /// the focused router tests readable.
    #[cfg(test)]
    pub(crate) async fn route(
        &self,
        name: &str,
        parsed: &ToolArgs,
        verbose: bool,
    ) -> CallToolResult {
        match self.route_transport(name, parsed, verbose).await {
            Ok(result) => result,
            Err(RouteError::Tool(result)) => result,
            Err(RouteError::Transport(error)) => {
                CallToolResult::error(error.code(), Some(&error.to_string()), verbose, None)
            }
        }
    }

    fn success(
        &self,
        name: &str,
        parsed: &ToolArgs,
        payload: Value,
    ) -> Result<CallToolResult, BridgeCallError> {
        match parsed {
            ToolArgs::DeviceTree(_) => {
                let tree: UiTree = serde_json::from_value(payload).map_err(|e| {
                    BridgeCallError::new(
                        ErrorCode::CaptureFailed,
                        format!("Invalid UI tree payload: {e}"),
                    )
                })?;
                let text = serde_json::to_string(&tree).map_err(|e| {
                    BridgeCallError::new(
                        ErrorCode::CaptureFailed,
                        format!("Failed to serialize UI tree: {e}"),
                    )
                })?;
                Ok(CallToolResult::success_text(text))
            }
            ToolArgs::Tap(_) | ToolArgs::TypeText(_) | ToolArgs::KeyPress(_) => {
                let response: BridgeSuccessResponse =
                    serde_json::from_value(payload).map_err(|e| {
                        BridgeCallError::new(
                            ErrorCode::InjectFailed,
                            format!("Invalid {name} response: {e}"),
                        )
                    })?;
                let text = serde_json::to_string(&response).map_err(|e| {
                    BridgeCallError::new(
                        ErrorCode::InjectFailed,
                        format!("Failed to serialize {name} response: {e}"),
                    )
                })?;
                Ok(CallToolResult::success_text(text))
            }
            ToolArgs::Screenshot(params) => {
                let data = payload["image"]
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| {
                        BridgeCallError::new(
                            ErrorCode::CaptureFailed,
                            "screenshot reply missing image data",
                        )
                    })?;
                // Reject a malformed companion payload even in non-verbose mode.
                let bytes_length = base64::engine::general_purpose::STANDARD
                    .decode(&data)
                    .map(|bytes| bytes.len())
                    .map_err(|e| {
                        BridgeCallError::new(
                            ErrorCode::CaptureFailed,
                            format!("Invalid base64 image data from companion: {e}"),
                        )
                    })?;
                let scale = params.scale.unwrap_or(0.5);
                let mut result = CallToolResult::success_image_with_meta(
                    &data,
                    "image/png",
                    bytes_length,
                    scale,
                );
                // Verbose mirror of the direct path's metadata block.
                if params.verbose.unwrap_or(false) {
                    let meta = json!({
                        "bytes_length": bytes_length,
                        "scale": scale,
                    });
                    result
                        .content
                        .push(texure_contract::tools::McpContent::text(meta.to_string()));
                }
                Ok(result)
            }
            ToolArgs::WaitFor(_) => {
                // Handled by `wait_for_via_tunnel` before the generic round-trip.
                Err(BridgeCallError::new(
                    ErrorCode::InjectFailed,
                    "wait_for must be routed through the polling path",
                ))
            }
        }
    }

    /// Poll `device_tree` over the tunnel, matching the direct `wait_for`
    /// fallback: immediate evaluation, 500ms polls, 30s cap, non-error
    /// `{ok:false, last_rev, hint, tree}` payload on timeout.
    async fn wait_for_via_tunnel(
        &self,
        params: &texure_contract::tools::WaitForParams,
        verbose: bool,
    ) -> Result<CallToolResult, RouteError> {
        let deadline = Instant::now() + Duration::from_secs(params.effective_timeout_secs());

        // Immediate evaluation before the first sleep, matching the direct path.
        let initial = self.fetch_tree(verbose).await?;
        if evaluate_condition(&initial, params.where_, &params.value) {
            let ok = json!({"ok": true, "rev": initial.rev});
            return Ok(CallToolResult::success_text(ok.to_string()));
        }

        let mut last_rev = initial.rev;
        let mut final_tree = Some(initial);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500).min(remaining)).await;

            let tree = self.fetch_tree(verbose).await?;
            last_rev = tree.rev;
            if evaluate_condition(&tree, params.where_, &params.value) {
                let ok = json!({"ok": true, "rev": tree.rev});
                return Ok(CallToolResult::success_text(ok.to_string()));
            }
            final_tree = Some(tree);
        }

        // Match the direct path: one final fresh snapshot so the agent can
        // re-plan from the state at the deadline, not a stale poll result.
        // A transport failure must still propagate so fallback can fire.
        let final_tree = match self.fetch_tree(verbose).await {
            Ok(tree) => {
                last_rev = tree.rev;
                Some(tree)
            }
            Err(RouteError::Transport(error)) => return Err(RouteError::Transport(error)),
            Err(RouteError::Tool(_)) => final_tree,
        };

        let timeout = json!({
            "ok": false,
            "last_rev": last_rev,
            "hint": "re-fetch device_tree and re-plan",
            "tree": final_tree,
        });
        Ok(CallToolResult::success_text(timeout.to_string()))
    }

    /// Fetch one tree snapshot over the tunnel for `wait_for` polling.
    async fn fetch_tree(&self, verbose: bool) -> Result<UiTree, RouteError> {
        let envelope = self.envelope("device_tree", json!({}), false);
        let reply = self
            .request(&envelope)
            .await
            .map_err(RouteError::Transport)?;
        match decode_reply(&reply) {
            Ok(Reply::Success(payload)) => serde_json::from_value(payload).map_err(|e| {
                let error = BridgeCallError::new(
                    ErrorCode::CaptureFailed,
                    format!("Invalid UI tree payload: {e}"),
                );
                RouteError::Tool(error.to_mcp_result(
                    verbose,
                    TUNNEL_BRIDGE_URL,
                    TUNNEL_WS_CONNECTED,
                ))
            }),
            Ok(Reply::Error(relayed)) => {
                let code = relayed_code(&relayed);
                let message = relayed["message"]
                    .as_str()
                    .unwrap_or_else(|| code.default_message())
                    .to_string();
                let error = BridgeCallError {
                    code,
                    message,
                    raw_json: Some(json!({ "error": relayed })),
                    http_status: None,
                };
                Err(RouteError::Tool(error.to_mcp_result(
                    verbose,
                    TUNNEL_BRIDGE_URL,
                    TUNNEL_WS_CONNECTED,
                )))
            }
            Err(error) => Err(RouteError::Tool(error.to_mcp_result(
                verbose,
                TUNNEL_BRIDGE_URL,
                TUNNEL_WS_CONNECTED,
            ))),
        }
    }

    /// Best-effort remote lifecycle bootstrap. A state probe reporting
    /// `STOPPED` triggers `shizuku_start_app`; every failure is swallowed so a
    /// trigger problem can never break the actual tool call.
    async fn ensure_device_running(&self) {
        const PROBE_TIMEOUT: Duration = Duration::from_millis(750);
        let probe = self.envelope("device_state", json!({}), false);
        let state = match tokio::time::timeout(PROBE_TIMEOUT, self.request(&probe)).await {
            Ok(Ok(reply)) => match decode_reply(&reply) {
                Ok(Reply::Success(payload)) => payload["state"]
                    .as_str()
                    .or_else(|| payload["device_state"].as_str())
                    .map(str::to_string),
                Ok(Reply::Error(_)) | Err(_) => None,
            },
            Ok(Err(_)) | Err(_) => None,
        };
        let stopped = state.is_some_and(|s| s.eq_ignore_ascii_case("stopped"));
        if stopped {
            let start = self.envelope("shizuku_start_app", json!({}), false);
            let _ = tokio::time::timeout(PROBE_TIMEOUT, self.request(&start)).await;
        }
    }

    fn next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, Ordering::Relaxed)
    }

    fn envelope(&self, rpc: &str, args: Value, verify_at_execute: bool) -> String {
        json!({
            "v": 1,
            "seq": self.next_seq(),
            "payload": {
                "type": "rpc",
                "rpc": rpc,
                "args": args,
                "verify_at_execute": verify_at_execute,
            }
        })
        .to_string()
    }

    async fn request(&self, envelope: &str) -> Result<String, TunnelError> {
        self.handle.request(envelope).await
    }
}

/// The gateway RPC name, arguments and revision-check flag for a parsed tool.
/// Mutation arguments are built from the same contract request structs the
/// direct bridge path uses, so the wire shapes cannot drift.
fn rpc_for(parsed: &ToolArgs) -> (&'static str, Value, bool) {
    match parsed {
        ToolArgs::DeviceTree(params) => {
            let mut args = json!({});
            if let Some(depth) = params.max_depth {
                args["max_depth"] = json!(depth);
            }
            if let Some(subtree) = &params.subtree {
                args["subtree"] = json!(subtree);
            }
            ("device_tree", args, false)
        }
        ToolArgs::Tap(params) => {
            let mut request = TapRequest::new(params.rev);
            if let Some(node) = params.node {
                request = request.with_node(node);
            }
            if let Some(index) = params.index {
                request = request.with_index(index);
            }
            if let Some(hash) = &params.hash {
                request = request.with_hash(hash);
            }
            ("tap", to_value_or_empty(request), true)
        }
        ToolArgs::TypeText(params) => {
            let mut request = TypeRequest::new(&params.text, params.rev);
            if let Some(node) = params.node {
                request = request.with_node(node);
            }
            if let Some(index) = params.index {
                request = request.with_index(index);
            }
            if let Some(hash) = &params.hash {
                request = request.with_hash(hash);
            }
            ("type", to_value_or_empty(request), true)
        }
        ToolArgs::KeyPress(params) => ("keypress", json!({"key": params.key.as_str()}), true),
        ToolArgs::Screenshot(params) => {
            let mut args = json!({});
            if let Some(scale) = params.scale {
                args["scale"] = json!(scale);
            }
            ("screenshot", args, false)
        }
        ToolArgs::WaitFor(_) => ("wait_for", json!({}), false),
    }
}

fn to_value_or_empty(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap_or_else(|_| json!({}))
}

/// A decoded tunnel reply: either a success payload or a relayed error body.
enum Reply {
    Success(Value),
    Error(Value),
}

fn decode_reply(text: &str) -> Result<Reply, BridgeCallError> {
    let value: Value = serde_json::from_str(text).map_err(|e| {
        BridgeCallError::new(
            ErrorCode::BridgeStopped,
            format!("invalid tunnel reply: {e}"),
        )
    })?;

    // Gateway envelope: `{v, seq, payload: <result|{error}>}`.
    if let Some(payload) = value.get("payload") {
        if let Some(error) = payload.get("error") {
            return Ok(Reply::Error(error.clone()));
        }
        return Ok(Reply::Success(payload.clone()));
    }

    // JSON-RPC 2.0: `{jsonrpc, id, result|error}`.
    if let Some(error) = value.get("error") {
        let detail = error.get("data").cloned().unwrap_or_else(|| error.clone());
        return Ok(Reply::Error(detail));
    }
    if let Some(result) = value.get("result") {
        return Ok(Reply::Success(result.clone()));
    }

    Err(BridgeCallError::new(
        ErrorCode::BridgeStopped,
        "tunnel reply missing payload or result",
    ))
}

/// Resolve a relayed error code (string contract code or JSON-RPC numeric
/// code) to one of the 16 frozen error codes. No new codes are introduced.
fn relayed_code(error: &Value) -> ErrorCode {
    if let Some(code) = error["code"].as_str() {
        if let Ok(code) = code.parse::<ErrorCode>() {
            return code;
        }
    }
    match error["code"].as_i64() {
        Some(-32001) => ErrorCode::Offline,
        Some(-32002) => ErrorCode::NodeOutdated,
        _ => ErrorCode::BridgeStopped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair::DEFAULT_TUNNEL_PATH;
    use crate::server::{parse_tool_args, McpServer};
    use crate::tunnel_server::{TunnelConfig, TunnelServer};
    use futures_util::{SinkExt, StreamExt};
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use texure_contract::tools::McpContent;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;

    type Responder = Arc<dyn Fn(&Value) -> Option<Value> + Send + Sync>;

    fn fast_config() -> TunnelConfig {
        TunnelConfig {
            heartbeat_interval: Duration::from_millis(30),
            pong_timeout: Duration::from_millis(120),
            handshake_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
            ..TunnelConfig::default()
        }
    }

    /// In-process companion stand-in: registers on a real `TunnelServer`,
    /// records every gateway envelope it sees, and answers via a closure.
    struct FakeCompanion {
        server: TunnelServer,
        seen: Arc<Mutex<Vec<Value>>>,
        task: JoinHandle<()>,
    }

    impl FakeCompanion {
        async fn spawn(responder: Responder) -> Self {
            let server = TunnelServer::bind_with_config(0, fast_config(), "tok")
                .await
                .expect("bind ephemeral port");
            let addr: SocketAddr = server.connect_addr();
            let url = format!("ws://{addr}{DEFAULT_TUNNEL_PATH}");
            let request = url.into_client_request().expect("client request");
            let (mut ws, _) = connect_async(request).await.expect("companion connects");
            ws.send(Message::Text(
                json!({"action": "register", "token": "tok"}).to_string(),
            ))
            .await
            .expect("register sent");

            let start = Instant::now();
            while !server.is_connected() && start.elapsed() < Duration::from_secs(2) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            assert!(server.is_connected(), "fake companion must register");

            let seen = Arc::new(Mutex::new(Vec::new()));
            let recorder = Arc::clone(&seen);
            let task = tokio::spawn(async move {
                while let Some(Ok(message)) = ws.next().await {
                    let Message::Text(text) = message else {
                        continue;
                    };
                    let trimmed = text.trim();
                    if trimmed == "tunnel.ping" {
                        let _ = ws.send(Message::Text("tunnel.pong".to_string())).await;
                        continue;
                    }
                    if trimmed == "tunnel.pong" {
                        continue;
                    }
                    let value: Value = match serde_json::from_str(trimmed) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    if value["action"].as_str() == Some("register") {
                        continue;
                    }
                    recorder.lock().expect("seen lock").push(value.clone());
                    if let Some(mut reply) = responder(&value) {
                        if let Some(seq) = value.get("seq") {
                            if reply.get("seq").is_none() {
                                reply["seq"] = seq.clone();
                            }
                        }
                        let _ = ws.send(Message::Text(reply.to_string())).await;
                    }
                }
            });
            Self { server, seen, task }
        }

        fn handle(&self) -> TunnelHandle {
            self.server.handle()
        }

        fn seen(&self) -> Vec<Value> {
            self.seen.lock().expect("seen lock").clone()
        }
    }

    impl Drop for FakeCompanion {
        fn drop(&mut self) {
            self.task.abort();
            self.server.stop();
        }
    }

    fn text_of(result: &CallToolResult) -> String {
        match result.content.first().expect("at least one content block") {
            McpContent::Text { text } => text.clone(),
            McpContent::Image { .. } => panic!("expected a text content block"),
        }
    }

    async fn route_args(
        router: &TunnelRouter,
        name: &str,
        args: Value,
        verbose: bool,
    ) -> CallToolResult {
        let parsed = parse_tool_args(name, args).expect("valid tool args");
        router.route(name, &parsed, verbose).await
    }

    #[tokio::test]
    async fn device_tree_builds_gateway_envelope_and_returns_tree() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"rev": 7, "window": "App", "nodes": []}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let result = route_args(&router, "device_tree", json!({}), false).await;

        let seen = companion.seen();
        assert_eq!(seen.len(), 1, "exactly one gateway envelope expected");
        let envelope = &seen[0];
        assert_eq!(envelope["v"], 1);
        assert!(envelope["seq"].is_u64(), "seq must be a unique number");
        assert_eq!(envelope["payload"]["type"], "rpc");
        assert_eq!(envelope["payload"]["rpc"], "device_tree");
        assert_eq!(envelope["payload"]["args"], json!({}));
        assert_eq!(result.is_error, None);
        let tree: Value = serde_json::from_str(&text_of(&result)).expect("tree JSON");
        assert_eq!(tree["rev"], 7);
    }

    #[tokio::test]
    async fn tap_round_trip_sets_verify_at_execute_and_serializes_success() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"ok": true, "rev": 8, "verify_at_execute": true}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let result = route_args(&router, "tap", json!({"node": 3, "rev": 7}), false).await;

        let seen = companion.seen();
        assert_eq!(seen[0]["payload"]["rpc"], "tap");
        assert_eq!(seen[0]["payload"]["args"]["node"], 3);
        assert_eq!(seen[0]["payload"]["args"]["rev"], 7);
        assert_eq!(seen[0]["payload"]["verify_at_execute"], true);
        assert_eq!(result.is_error, None);
        assert_eq!(text_of(&result), r#"{"ok":true,"rev":8}"#);
    }

    #[tokio::test]
    async fn relayed_node_outdated_keeps_code_message_and_frozen_hint() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"error": {
                "code": "NODE_OUTDATED",
                "message": "tree changed before execute",
                "hint": "re-fetch /ui-tree and retry",
                "relayed": true,
            }}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let one_line = route_args(&router, "tap", json!({"node": 3, "rev": 1}), false).await;
        assert_eq!(one_line.is_error, Some(true));
        assert_eq!(
            text_of(&one_line),
            "NODE_OUTDATED: tree changed before execute; re-run device_tree, re-target"
        );

        let verbose = route_args(&router, "tap", json!({"node": 3, "rev": 1}), true).await;
        let full: Value = serde_json::from_str(&text_of(&verbose)).expect("verbose JSON");
        assert_eq!(full["error"]["code"], "NODE_OUTDATED");
        assert_eq!(full["error"]["message"], "tree changed before execute");
        assert_eq!(full["error"]["relayed"], true);
        assert_eq!(full["error"]["hint"], "re-fetch /ui-tree and retry");
    }

    #[tokio::test]
    async fn json_rpc_result_shape_is_translated_to_success() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            // Correlation via `id` matching the envelope's numeric `seq`.
            Some(json!({"jsonrpc": "2.0", "id": 1, "result": {"ok": true, "rev": 9}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let result = route_args(&router, "keypress", json!({"key": "ENTER"}), false).await;

        assert_eq!(result.is_error, None);
        assert_eq!(text_of(&result), r#"{"ok":true,"rev":9}"#);
    }

    #[tokio::test]
    async fn tunnel_drop_mid_call_maps_to_offline_without_panicking() {
        // Companion registers, then drops the socket without replying.
        let server = TunnelServer::bind_with_config(0, fast_config(), "tok")
            .await
            .expect("bind ephemeral port");
        let url = format!("ws://{}{DEFAULT_TUNNEL_PATH}", server.connect_addr());
        let request = url.into_client_request().expect("client request");
        let (mut ws, _) = connect_async(request).await.expect("companion connects");
        ws.send(Message::Text(
            json!({"action": "register", "token": "tok"}).to_string(),
        ))
        .await
        .expect("register sent");
        let start = Instant::now();
        while !server.is_connected() && start.elapsed() < Duration::from_secs(2) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(server.is_connected());

        let router = TunnelRouter::new(server.handle());
        let handle = tokio::spawn(async move {
            // Consume the envelope, then drop the connection mid-call.
            let _ = ws.next().await;
            drop(ws);
        });
        let result = route_args(&router, "device_tree", json!({}), false).await;
        handle.await.expect("drop task");

        assert_eq!(result.is_error, Some(true));
        assert!(
            text_of(&result).starts_with("OFFLINE:"),
            "tunnel drop must become OFFLINE, got: {}",
            text_of(&result)
        );
    }

    #[tokio::test]
    async fn silent_tunnel_is_a_transport_failure_so_fallback_can_fire() {
        // Registered companion that accepts frames but never replies: the
        // contract's fallback_on(heartbeat_timeout / tunnel_unreachable) case.
        let companion = FakeCompanion::spawn(Arc::new(|_| None)).await;
        let router = TunnelRouter::new(companion.handle());
        let parsed = parse_tool_args("device_tree", json!({})).expect("valid args");

        let outcome = router.route_transport("device_tree", &parsed, false).await;

        assert!(
            matches!(outcome, Err(RouteError::Transport(_))),
            "a silent tunnel must be reported as a transport failure, not OFFLINE-as-tool-error"
        );
    }

    #[tokio::test]
    async fn stopped_device_triggers_shizuku_start_app_before_the_call() {
        let companion = FakeCompanion::spawn(Arc::new(|envelope| {
            let rpc = envelope["payload"]["rpc"].as_str().unwrap_or("");
            match rpc {
                "device_state" => Some(json!({"payload": {"state": "STOPPED"}})),
                "shizuku_start_app" => Some(json!({"payload": {"ok": true}})),
                "device_tree" => Some(json!({"payload": {"rev": 3, "window": "App", "nodes": []}})),
                _ => None,
            }
        }))
        .await;
        let router = TunnelRouter::new(companion.handle()).with_lifecycle_probe();

        let result = route_args(&router, "device_tree", json!({}), false).await;

        assert_eq!(
            result.is_error, None,
            "best-effort probe must not break the call"
        );
        let seen = companion.seen();
        let rpcs: Vec<&str> = seen
            .iter()
            .filter_map(|e| e["payload"]["rpc"].as_str())
            .collect();
        assert_eq!(
            rpcs,
            vec!["device_state", "shizuku_start_app", "device_tree"],
            "start_app RPC must precede the actual tool call"
        );
    }

    #[tokio::test]
    async fn seq_is_unique_across_calls() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"rev": 1, "window": "App", "nodes": []}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        for _ in 0..3 {
            let _ = route_args(&router, "device_tree", json!({}), false).await;
        }

        let seen = companion.seen();
        let seqs: Vec<u64> = seen
            .iter()
            .map(|e| e["seq"].as_u64().expect("seq"))
            .collect();
        let mut sorted = seqs.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "each call must use a distinct seq");
    }

    #[tokio::test]
    async fn all_six_frozen_tools_map_to_a_gateway_rpc() {
        let cases = vec![
            ("device_tree", json!({}), "device_tree", false),
            ("tap", json!({"node": 1, "rev": 1}), "tap", true),
            ("type", json!({"text": "hi", "rev": 1}), "type", true),
            ("keypress", json!({"key": "BACK"}), "keypress", true),
            ("screenshot", json!({}), "screenshot", false),
            // wait_for has no WS channel over the tunnel, so it polls
            // device_tree (the direct path's WS-down fallback).
            (
                "wait_for",
                json!({"where": "text", "value": "Send", "timeout": 1}),
                "device_tree",
                false,
            ),
        ];
        for (name, args, expected_rpc, expected_verify) in cases {
            let companion =
                FakeCompanion::spawn(Arc::new(|_| Some(json!({"payload": {"ok": true}})))).await;
            let router = TunnelRouter::new(companion.handle());
            let _ = route_args(&router, name, args, false).await;
            let seen = companion.seen();
            let first = seen.first().expect("gateway envelope");
            assert_eq!(first["payload"]["rpc"], expected_rpc, "rpc for {name}");
            assert_eq!(
                first["payload"]["verify_at_execute"], expected_verify,
                "verify flag for {name}"
            );
        }
    }

    #[tokio::test]
    async fn mcp_server_routes_through_a_connected_tunnel_without_lan_config() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"rev": 12, "window": "Remote", "nodes": []}}))
        }))
        .await;
        let server = McpServer::new(Some(PathBuf::from("/nonexistent/texure_config.toml")), None)
            .with_tunnel(companion.handle());

        let result = server
            .call_tool(json!({"name": "device_tree", "arguments": {}}))
            .await
            .expect("tool result");

        assert_eq!(result.is_error, None);
        let tree: Value = serde_json::from_str(&text_of(&result)).expect("tree JSON");
        assert_eq!(
            tree["rev"], 12,
            "reply came over the tunnel, not the LAN bridge"
        );
        assert_eq!(companion.seen().len(), 1, "exactly one gateway proxy hop");
    }

    #[test]
    fn disconnected_tunnel_falls_back_to_the_direct_lan_path() {
        // Sync test + block_on so the env lock is never held across an await
        // (clippy::await_holding_lock), matching server.rs's env test.
        let _env = crate::test_env_lock::lock();
        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(async {
            let server = TunnelServer::bind_with_config(0, fast_config(), "tok")
                .await
                .expect("bind ephemeral port");
            assert!(!server.is_connected(), "no companion registered yet");

            let mcp = McpServer::new(Some(PathBuf::from("/nonexistent/texure_config.toml")), None)
                .with_tunnel(server.handle());
            let result = mcp
                .call_tool(json!({"name": "device_tree", "arguments": {}}))
                .await
                .expect("tool result");

            assert_eq!(result.is_error, Some(true));
            assert!(
                text_of(&result).starts_with("NO_CONFIG:"),
                "absent tunnel must keep the direct-path NO_CONFIG behavior, got: {}",
                text_of(&result)
            );
        });
    }

    #[tokio::test]
    async fn invalid_base64_screenshot_is_a_capture_failed_error() {
        let companion = FakeCompanion::spawn(Arc::new(|_| {
            Some(json!({"payload": {"image": "!!!not-base64!!!", "mime": "image/png"}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let result = route_args(&router, "screenshot", json!({}), false).await;

        assert_eq!(result.is_error, Some(true));
        assert!(text_of(&result).starts_with("CAPTURE_FAILED:"));
    }

    #[tokio::test]
    async fn screenshot_reply_becomes_an_image_content_block() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"PNGDATA");
        let image = encoded.clone();
        let companion = FakeCompanion::spawn(Arc::new(move |_| {
            Some(json!({"payload": {"image": image.clone(), "mime": "image/png"}}))
        }))
        .await;
        let router = TunnelRouter::new(companion.handle());

        let result = route_args(&router, "screenshot", json!({}), false).await;

        assert_eq!(result.is_error, None);
        assert_eq!(
            result.structured_content,
            Some(serde_json::json!({
                "format": "image/png",
                "bytes_length": 7,
                "scale": 0.5
            }))
        );
        match result.content.first().expect("image block") {
            McpContent::Image { data, mime_type } => {
                assert_eq!(data, &encoded);
                assert_eq!(mime_type, "image/png");
            }
            other => panic!("expected image content, got {other:?}"),
        }
    }
}
