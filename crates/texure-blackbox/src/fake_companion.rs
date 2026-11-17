//! In-process fake companion for the `mcp-tunnel` blackbox target (BB-R1..R13).
//!
//! The companion is a real WebSocket *client* of the `texure-server` reverse
//! tunnel listener. It registers with a bearer token, answers gateway RPC
//! envelopes with fixture-driven canned bridge responses, and rides the
//! `tunnel.ping`/`tunnel.pong` heartbeat. Drop/reconnect is real WS teardown +
//! re-registration, so tunnel lifecycle scenarios exercise the real listener.
//! No server internals are imported — this is protocol-only (blackbox law).

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Error, Debug)]
pub enum CompanionError {
    #[error("tunnel connect failed: {0}")]
    Connect(String),
    #[error("tunnel register failed: {0}")]
    Register(String),
    #[error("shizuku command failed: {0}")]
    Shizuku(String),
}

struct Session {
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<()>,
}

pub struct FakeCompanion {
    addr: SocketAddr,
    root_dir: PathBuf,
    seen: Arc<Mutex<Vec<Value>>>,
    replies: Arc<Mutex<Vec<Value>>>,
    session: Mutex<Option<Session>>,
}

impl FakeCompanion {
    pub fn new(addr: SocketAddr) -> Self {
        let root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            addr,
            root_dir,
            seen: Arc::new(Mutex::new(Vec::new())),
            replies: Arc::new(Mutex::new(Vec::new())),
            session: Mutex::new(None),
        }
    }

    /// Register a companion on the real tunnel. `label` is only a local tag for
    /// the scenario (owner vs loser); the server assigns the real owner identity
    /// from the peer address. A failed register (AUTH_FAILED/BUSY) leaves any
    /// existing active session intact — required by BB-R13's owner+loser steps.
    pub async fn connect(&mut self, token: &str, _label: &str) -> Result<Value, CompanionError> {
        let url = format!("ws://{}/tunnel", self.addr);
        let mut request = url
            .clone()
            .into_client_request()
            .map_err(|e| CompanionError::Connect(format!("bad url '{url}': {e}")))?;
        // Contract handshake: bearer token on the HTTP upgrade AND in the
        // first register TEXT frame below.
        let bearer = format!("Bearer {token}")
            .parse()
            .map_err(|e| CompanionError::Connect(format!("invalid bearer header: {e}")))?;
        request.headers_mut().insert("Authorization", bearer);

        let (mut ws, _) =
            match tokio::time::timeout(Duration::from_secs(3), connect_async(request)).await {
                Ok(Ok(pair)) => pair,
                Ok(Err(e)) => {
                    // The listener answers a rejected Authorization header with a
                    // real HTTP 401; surface it as the frozen AUTH_FAILED outcome.
                    let rendered = format!("{e}");
                    if rendered.contains("401")
                        || rendered.contains("403")
                        || rendered.contains("AUTH_FAILED")
                    {
                        return Ok(json!({
                            "connected": false,
                            "code": "AUTH_FAILED",
                            "state": "UNPAIRED",
                        }));
                    }
                    return Err(CompanionError::Connect(rendered));
                }
                Err(_) => return Err(CompanionError::Connect("connect timed out".to_string())),
            };

        let register = json!({ "action": "register", "token": token }).to_string();
        ws.send(Message::Text(register))
            .await
            .map_err(|e| CompanionError::Register(e.to_string()))?;

        // A valid register produces no immediate frame; AUTH_FAILED / BUSY
        // produce a frozen-code error frame before the server closes.
        if let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout(Duration::from_millis(250), ws.next()).await
        {
            let trimmed = text.trim();
            if trimmed != "tunnel.ping" && trimmed != "tunnel.pong" {
                if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(code) = value["error"]["code"].as_str() {
                        let owner = value["error"]["owner"].as_str().map(str::to_string);
                        let state = if code == "AUTH_FAILED" {
                            "UNPAIRED"
                        } else {
                            // BUSY keeps the owner's tunnel connected.
                            "CONNECTED"
                        };
                        let _ = ws.close(None).await;
                        let mut response = json!({
                            "connected": false,
                            "code": code,
                            "state": state,
                        });
                        if let Some(owner) = owner {
                            response["owner"] = json!(owner);
                        }
                        return Ok(response);
                    }
                }
            }
        }

        // Success: this connection becomes the sole active driver.
        self.teardown().await;
        let (shutdown, shutdown_rx) = oneshot::channel();
        let seen = Arc::clone(&self.seen);
        let replies = Arc::clone(&self.replies);
        let root_dir = self.root_dir.clone();
        let handle = tokio::spawn(run_session(ws, shutdown_rx, seen, replies, root_dir));
        *lock(&self.session) = Some(Session { shutdown, handle });

        Ok(json!({ "connected": true, "state": "CONNECTED" }))
    }

    /// Drop the active connection without reconnecting (BB-R3 drop step).
    pub async fn drop_tunnel(&mut self) -> Value {
        self.teardown().await;
        json!({ "dropped": true, "state": "DROPPED" })
    }

    /// Supervisor reconnect after a drop: re-register with a valid token and
    /// report the observed state (BB-R3).
    pub async fn heartbeat(&mut self) -> Result<Value, CompanionError> {
        self.connect("valid-bearer-token", "companion").await?;
        Ok(json!({
            "reconnected": true,
            "connected": true,
            "state": "CONNECTED",
        }))
    }

    /// Whether the most recent gateway envelope carried verify-at-execute — a
    /// fact observed on the tunnel that the server strips when it re-serializes
    /// `BridgeSuccessResponse` (BB-R5).
    pub fn last_envelope_verify_at_execute(&self) -> bool {
        lock(&self.seen)
            .last()
            .and_then(|envelope| envelope["payload"]["verify_at_execute"].as_bool())
            .unwrap_or(false)
    }

    /// Simulate a Shizuku lifecycle command outcome from the frozen schema
    /// fixture (BB-R7..R10). Unknown ops / missing fixture surface as an error
    /// so a scenario cannot pass on invented data.
    pub fn shizuku_cmd(&self, op: &str, args: &Value) -> Result<Value, CompanionError> {
        let path = self
            .root_dir
            .join("contract/fixtures/shizuku_command_schema.json");
        let schema: Value = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| json!({}));

        let entry = schema["ops"]
            .as_array()
            .and_then(|ops| ops.iter().find(|e| e["op"].as_str() == Some(op)));
        match entry {
            Some(entry) if entry["mock_result"]["ok"].as_bool() == Some(true) => {
                let mut response = entry["mock_result"].clone();
                if let Some(map) = response.as_object_mut() {
                    map.insert("op".to_string(), json!(op));
                    map.insert("args".to_string(), args.clone());
                    // Keep the fixture's outcome state (e.g. "screen_on");
                    // only synthesize a state when the fixture omits one so
                    // record_step_context still observes a concrete outcome.
                    if map.get("state").is_none() {
                        map.insert("state".to_string(), json!("CONNECTED"));
                    }
                }
                Ok(response)
            }
            Some(_) => Err(CompanionError::Shizuku(format!(
                "op '{op}' did not succeed in the command schema"
            ))),
            None => Err(CompanionError::Shizuku(format!(
                "unknown shizuku op '{op}' (or missing command schema)"
            ))),
        }
    }

    pub async fn disconnect(&mut self) {
        self.teardown().await;
    }

    async fn teardown(&mut self) {
        let session = lock(&self.session).take();
        if let Some(session) = session {
            let _ = session.shutdown.send(());
            let _ = tokio::time::timeout(Duration::from_millis(500), session.handle).await;
        }
    }
}

impl Drop for FakeCompanion {
    fn drop(&mut self) {
        let session = lock(&self.session).take();
        if let Some(session) = session {
            let _ = session.shutdown.send(());
            session.handle.abort();
        }
    }
}

/// Reader task for one live tunnel session: heartbeat echo + gateway replies.
async fn run_session(
    mut ws: Ws,
    mut shutdown: oneshot::Receiver<()>,
    seen: Arc<Mutex<Vec<Value>>>,
    replies: Arc<Mutex<Vec<Value>>>,
    root_dir: PathBuf,
) {
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            incoming = ws.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
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
                        lock(&seen).push(value.clone());
                        if let Some(reply) = gateway_reply(&value, &root_dir) {
                            lock(&replies).push(reply.clone());
                            let _ = ws.send(Message::Text(reply.to_string())).await;
                        }
                    }
                    // tungstenite answers native Pings itself; Pongs prove liveness.
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
        }
    }
    let _ = ws.close(None).await;
}

/// Build a canned gateway reply from the frozen fixtures. Real envelope in,
/// frozen-shaped payload out, correlation `seq` echoed verbatim.
fn gateway_reply(envelope: &Value, root_dir: &Path) -> Option<Value> {
    let rpc = envelope["payload"]["rpc"].as_str()?;
    let args = &envelope["payload"]["args"];
    let seq = envelope.get("seq").cloned().unwrap_or(json!(1));

    let tree = load_tree(root_dir);
    let current_rev = tree["rev"].as_u64().unwrap_or(42);

    let payload = match rpc {
        "device_tree" => tree,
        "tap" | "type" | "keypress" => {
            let req_rev = args["rev"].as_u64().unwrap_or(0);
            if req_rev < current_rev {
                json!({
                    "error": {
                        "code": "NODE_OUTDATED",
                        "message": "tree changed before execute",
                        "hint": "re-fetch /ui-tree and retry",
                        "relayed": true,
                    }
                })
            } else {
                json!({ "ok": true, "rev": current_rev + 1 })
            }
        }
        "device_state" => json!({ "state": "ONLINE" }),
        _ => return None,
    };

    Some(json!({ "v": 1, "seq": seq, "payload": payload }))
}

fn load_tree(root_dir: &Path) -> Value {
    let path = root_dir.join("contract/fixtures/ui_tree.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| {
            json!({
                "rev": 42,
                "window": "com.velavoice/.MainActivity",
                "nodes": [],
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn companion() -> FakeCompanion {
        FakeCompanion::new("127.0.0.1:1".parse().expect("addr"))
    }

    #[test]
    fn gateway_reply_for_device_tree_echoes_seq_and_returns_fixture_tree() {
        let envelope =
            json!({"v": 1, "seq": 9, "payload": {"type": "rpc", "rpc": "device_tree", "args": {}}});
        let reply = gateway_reply(&envelope, &companion().root_dir).expect("reply");
        assert_eq!(reply["seq"], 9);
        assert_eq!(reply["payload"]["rev"], 42);
        assert_eq!(reply["payload"]["window"], "com.velavoice/.MainActivity");
    }

    #[test]
    fn gateway_reply_relays_node_outdated_for_stale_revision() {
        let envelope = json!({"v": 1, "seq": 2, "payload": {"rpc": "tap", "args": {"node": 3, "rev": 1}, "verify_at_execute": true}});
        let reply = gateway_reply(&envelope, &companion().root_dir).expect("reply");
        assert_eq!(reply["payload"]["error"]["code"], "NODE_OUTDATED");
        assert_eq!(reply["payload"]["error"]["relayed"], true);
        assert_eq!(reply["seq"], 2);
    }

    #[test]
    fn gateway_reply_accepts_current_revision() {
        let envelope =
            json!({"v": 1, "seq": 3, "payload": {"rpc": "tap", "args": {"node": 3, "rev": 42}}});
        let reply = gateway_reply(&envelope, &companion().root_dir).expect("reply");
        assert_eq!(reply["payload"]["ok"], true);
        assert_eq!(reply["payload"]["rev"], 43);
    }

    #[test]
    fn gateway_reply_ignores_unknown_rpc() {
        let envelope = json!({"v": 1, "seq": 1, "payload": {"rpc": "not_a_tool"}});
        assert!(gateway_reply(&envelope, &companion().root_dir).is_none());
    }

    #[test]
    fn shizuku_simulation_is_fixture_driven() {
        let companion = companion();
        let wake = companion.shizuku_cmd("wake", &json!({})).expect("wake");
        assert_eq!(wake["ok"], true);
        assert_eq!(wake["state"], "screen_on");
        assert!(companion.shizuku_cmd("not_an_op", &json!({})).is_err());
    }

    #[test]
    fn verify_at_execute_read_from_last_envelope() {
        let companion = companion();
        assert!(!companion.last_envelope_verify_at_execute());
        lock(&companion.seen)
            .push(json!({"seq": 1, "payload": {"rpc": "tap", "verify_at_execute": true}}));
        assert!(companion.last_envelope_verify_at_execute());
    }
}
