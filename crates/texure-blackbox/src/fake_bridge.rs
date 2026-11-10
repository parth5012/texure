use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

pub struct FakeBridgeConfig {
    pub scenario_name: String,
    pub root_dir: String,
    pub port: u16,
    pub token: String,
}

impl Default for FakeBridgeConfig {
    fn default() -> Self {
        Self {
            scenario_name: "static_screen".to_string(),
            root_dir: ".".to_string(),
            port: 0,
            token: "test-token".to_string(),
        }
    }
}

pub struct FakeBridgeState {
    pub rev: AtomicU64,
    pub has_mutated_stale: AtomicBool,
    pub active_owner: tokio::sync::Mutex<Option<String>>,
    pub ws_sender: broadcast::Sender<String>,
    pub scenario_name: String,
    pub root_dir: String,
    pub expected_token: String,
    pub recording: tokio::sync::Mutex<RecordingState>,
    pub tags: tokio::sync::Mutex<std::collections::HashMap<String, u64>>,
}

/// In-memory workflow-recording session (ticket #24 L2 mock).
///
/// Mirrors the bridge contract: while `active`, mutating requests
/// (`/tap`, `/type`, `/keypress`, `/back`) append one synthetic action each
/// with a node reference and timestamp offset. The real bridge excludes
/// agent-injected events instead (human-only trace); this mock records
/// agent traffic so L2 can verify export/replay deterministically.
pub struct RecordingState {
    pub active: bool,
    pub started_at: Option<std::time::Instant>,
    pub next_seq: u64,
    pub actions: Vec<Value>,
}

pub struct FakeBridgeServer {
    pub addr: SocketAddr,
    _state: Arc<FakeBridgeState>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FakeBridgeServer {
    pub async fn start(
        config: FakeBridgeConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", config.port)).await?;
        let addr = listener.local_addr()?;
        let (ws_sender, _) = broadcast::channel(32);

        let initial_rev = if config.scenario_name == "token_budget" {
            100
        } else {
            42
        };

        let state = Arc::new(FakeBridgeState {
            rev: AtomicU64::new(initial_rev),
            has_mutated_stale: AtomicBool::new(false),
            active_owner: tokio::sync::Mutex::new(None),
            ws_sender,
            scenario_name: config.scenario_name,
            root_dir: config.root_dir,
            expected_token: config.token,
            recording: tokio::sync::Mutex::new(RecordingState {
                active: false,
                started_at: None,
                next_seq: 0,
                actions: Vec::new(),
            }),
            tags: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        });

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, client_addr)) => {
                                let st = Arc::clone(&state_clone);
                                tokio::spawn(async move {
                                    handle_connection(stream, client_addr, st).await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Ok(Self {
            addr,
            _state: state,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for FakeBridgeServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    client_addr: SocketAddr,
    state: Arc<FakeBridgeState>,
) {
    let mut buf = vec![0u8; 8192];
    let n = match stream.peek(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);

    // Check if this is a WebSocket upgrade request
    if req_str.contains("Upgrade: websocket") || req_str.contains("upgrade: websocket") {
        let expected_auth = format!("Bearer {}", state.expected_token);
        let has_valid_auth = req_str.lines().any(|l| {
            let l_trimmed = l.trim();
            if l_trimmed.to_lowercase().starts_with("authorization:") {
                let val = l_trimmed["authorization:".len()..].trim();
                val == expected_auth
            } else {
                false
            }
        });

        if !has_valid_auth || state.scenario_name == "auth_reject" {
            let err = json!({
                "error": {
                    "code": "AUTH_FAILED",
                    "message": "Authorization header missing or invalid on WebSocket upgrade",
                    "hint": "re-paste fresh token from overlay"
                }
            });
            let body = serde_json::to_vec(&err).unwrap();
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                String::from_utf8_lossy(&body)
            );
            let _ = stream.write_all(resp.as_bytes()).await;
            let _ = stream.flush().await;
            return;
        }

        if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
            let mut ws_rx = state.ws_sender.subscribe();
            loop {
                tokio::select! {
                    msg_res = ws_rx.recv() => {
                        match msg_res {
                            Ok(msg) => {
                                if ws_stream.send(Message::Text(msg)).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    client_msg = ws_stream.next() => {
                        match client_msg {
                            Some(Ok(Message::Close(_))) | None => break,
                            _ => {}
                        }
                    }
                }
            }
        }
        return;
    }

    // Standard HTTP request handling
    let mut read_buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut header_end = None;

    while header_end.is_none() {
        if read_buf.len() > 64 * 1024 {
            // Header too large
            return;
        }
        match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(bytes_read) => {
                read_buf.extend_from_slice(&chunk[..bytes_read]);
                if let Some(pos) = find_header_end(&read_buf) {
                    header_end = Some(pos);
                }
            }
            Err(_) => return,
        }
    }

    let header_pos = match header_end {
        Some(p) => p,
        None => return,
    };

    let header_str = String::from_utf8_lossy(&read_buf[..header_pos]);
    let body_bytes = read_buf[header_pos..].to_vec();

    let mut lines = header_str.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let full_path = parts[1];

    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }

    let mut content_length = 0;
    if let Some(cl) = headers.get("content-length") {
        content_length = cl.parse::<usize>().unwrap_or(0);
    }
    // Cap content length to 16MB to avoid unbounded allocation
    if content_length > 16 * 1024 * 1024 {
        return;
    }

    let mut final_body = body_bytes;
    while final_body.len() < content_length {
        let mut extra = vec![0u8; content_length - final_body.len()];
        match stream.read(&mut extra).await {
            Ok(0) => break,
            Ok(r) => final_body.extend_from_slice(&extra[..r]),
            Err(_) => break,
        }
    }

    let (path, query_str) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };

    let auth_header = headers.get("authorization").cloned().unwrap_or_default();
    let client_ip = headers
        .get("remote-addr")
        .cloned()
        .unwrap_or_else(|| client_addr.ip().to_string());

    let (status, resp_headers, resp_body) = route_request(
        method,
        path,
        query_str,
        &auth_header,
        &client_ip,
        &final_body,
        &state,
    )
    .await;

    let mut response = format!(
        "HTTP/1.1 {}\r\nConnection: close\r\nContent-Length: {}\r\n",
        status,
        resp_body.len()
    );
    for (k, v) in resp_headers {
        response.push_str(&format!("{k}: {v}\r\n"));
    }
    response.push_str("\r\n");

    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.write_all(&resp_body).await;
    let _ = stream.flush().await;
}

fn find_header_end(data: &[u8]) -> Option<usize> {
    for i in 0..data.len().saturating_sub(3) {
        if &data[i..i + 4] == b"\r\n\r\n" {
            return Some(i + 4);
        }
    }
    None
}

async fn capture_recording_action(
    state: &Arc<FakeBridgeState>,
    action: &str,
    node_index: Option<u64>,
    text: Option<&str>,
    key: Option<&str>,
) {
    let mut rec = state.recording.lock().await;
    if !rec.active {
        return;
    }
    let t = rec
        .started_at
        .map(|s| s.elapsed().as_millis() as u64)
        .unwrap_or(0);
    let rev = state.rev.load(Ordering::SeqCst);
    let node = node_index.map(|idx| {
        json!({
            "index": idx,
            "id": null,
            "text": null,
            "cd": null,
            "cls": "synthetic",
            "rev": rev
        })
    });

    let mut entry = json!({
        "seq": rec.next_seq,
        "t": t,
        "action": action,
        "node": node,
    });
    rec.next_seq += 1;
    if let Some(e) = entry.as_object_mut() {
        if let Some(v) = text {
            e.insert("value".to_string(), json!(v));
        }
        if let Some(k) = key {
            e.insert("key".to_string(), json!(k));
        }
    }
    rec.actions.push(entry);
}

/// Inject deterministic semantic hashes into FakeBridge `/ui-tree` nodes
/// (map #36, #29). FNV-1a 64 over ancestor path/id/text/cls (hex, 16 chars).
fn inject_hashes(nodes: &mut Value, ancestor: &[String]) {
    if let Some(arr) = nodes.as_array_mut() {
        for node in arr.iter_mut() {
            if let Some(obj) = node.as_object_mut() {
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let cls = obj.get("cls").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let mut path = ancestor.to_vec();
                // children: path includes this node's segment
                let seg = if id.is_empty() { cls.clone() } else { id.clone() };
                let hash = fnv1a_64_hex(&path, &id, &text, &cls);
                obj.insert("hash".to_string(), json!(hash));
                if let Some(children) = obj.get_mut("children") {
                    let mut child_path = path.clone();
                    child_path.push(seg);
                    inject_hashes(children, &child_path);
                }
            }
        }
    }
}

fn load_fixture_nodes(path: &Path) -> Vec<Value> {
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Value>(&content) {
                if let Some(nodes) = v.get("nodes").and_then(|n| n.as_array()) {
                    return nodes.clone();
                }
            }
        }
    }
    vec![]
}

fn node_hash(node: &Value) -> String {
    let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let cls = node.get("cls").and_then(|v| v.as_str()).unwrap_or("");
    // ancestor path empty at root for FakeBridge
    fnv1a_64_hex(&[], id, text, cls)
}

fn fnv1a_64_hex(ancestor: &[String], id: &str, text: &str, cls: &str) -> String {
    let mut input = String::new();
    input.push_str(&ancestor.join("/"));
    input.push('\n');
    input.push_str(id);
    input.push('\n');
    input.push_str(text.trim().chars().take(1024).collect::<String>().as_str());
    input.push('\n');
    input.push_str(cls);
    let mut h: u64 = 0xcbf29ce484222325;
    let prime: u64 = 0x100000001b3;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(prime);
    }
    format!("{:016x}", h)
}

async fn route_request(
    method: &str,
    path: &str,
    query_str: &str,
    auth_header: &str,
    client_ip: &str,
    body: &[u8],
    state: &Arc<FakeBridgeState>,
) -> (&'static str, Vec<(&'static str, String)>, Vec<u8>) {
    // 0. Device offline check
    if state.scenario_name == "device_offline" {
        let err = json!({
            "error": {
                "code": "OFFLINE",
                "message": "device unreachable",
                "hint": "same LAN? app open? fg service alive?"
            }
        });
        return (
            "503 Service Unavailable",
            vec![("Content-Type", "application/json".to_string())],
            serde_json::to_vec(&err).unwrap(),
        );
    }

    // 1. Auth check
    let expected_auth = format!("Bearer {}", state.expected_token);
    let is_auth_reject = state.scenario_name == "auth_reject"
        || auth_header.is_empty()
        || auth_header != expected_auth;

    if is_auth_reject && state.scenario_name == "auth_reject" {
        let err = json!({
            "error": {
                "code": "AUTH_FAILED",
                "message": "token mismatch (stale token after rotation)",
                "hint": "re-paste fresh token from overlay"
            }
        });
        return (
            "401 Unauthorized",
            vec![("Content-Type", "application/json".to_string())],
            serde_json::to_vec(&err).unwrap(),
        );
    }

    if auth_header != expected_auth && state.scenario_name != "auth_reject" {
        let err = json!({
            "error": {
                "code": "AUTH_FAILED",
                "message": "Authorization header missing or invalid",
                "hint": "re-paste fresh token from overlay"
            }
        });
        return (
            "401 Unauthorized",
            vec![("Content-Type", "application/json".to_string())],
            serde_json::to_vec(&err).unwrap(),
        );
    }

    // 2. Single-flight lock check
    if state.scenario_name == "busy_second_client" {
        let err = json!({
            "error": {
                "code": "BUSY",
                "message": "second client during single-flight",
                "hint": "wait for owner release / ask owner",
                "owner": "client-1"
            }
        });
        return (
            "409 Conflict",
            vec![("Content-Type", "application/json".to_string())],
            serde_json::to_vec(&err).unwrap(),
        );
    }

    {
        let mut owner_guard = state.active_owner.lock().await;
        if let Some(ref owner) = *owner_guard {
            if owner != client_ip && state.scenario_name != "lock_release" {
                let err = json!({
                    "error": {
                        "code": "BUSY",
                        "message": "second client during single-flight",
                        "hint": "wait for owner release / ask owner",
                        "owner": owner.clone()
                    }
                });
                return (
                    "409 Conflict",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }
        } else {
            *owner_guard = Some(client_ip.to_string());
        }
    }

    match (method, path) {
        ("GET", "/ui-tree") => {
            let fixture_file = if state.scenario_name == "token_budget" {
                "contract/fixtures/deep_gauntlet_tree.json"
            } else {
                "contract/fixtures/ui_tree.json"
            };

            let fixture_path = Path::new(&state.root_dir).join(fixture_file);
            let mut tree_json: Value = if fixture_path.exists() {
                serde_json::from_str(&fs::read_to_string(&fixture_path).unwrap_or_default())
                    .unwrap_or_else(|_| json!({"rev": 42, "window": "com.velavoice/.MainActivity", "nodes": []}))
            } else {
                json!({
                    "rev": 42,
                    "window": "com.velavoice/.MainActivity",
                    "nodes": [
                        {
                            "id": "btn_send",
                            "text": "Send",
                            "cls": "android.widget.Button",
                            "bounds": [12, 340, 108, 56],
                            "clickable": true,
                            "editable": false
                        }
                    ]
                })
            };

            let current_rev = state.rev.load(Ordering::SeqCst);
            if let Some(obj) = tree_json.as_object_mut() {
                obj.insert("rev".to_string(), json!(current_rev));
                // Apply runtime tags (map #36, #31): tagged nodes surface the tag as id.
                let tags: std::collections::HashMap<String, u64> = state.tags.lock().await.clone();
                if let Some(nodes) = obj.get_mut("nodes").and_then(|n| n.as_array_mut()) {
                    for (tag, idx) in &tags {
                        if let Some(node) = nodes.get_mut(*idx as usize) {
                            if let Some(o) = node.as_object_mut() {
                                o.insert("id".to_string(), json!(tag));
                            }
                        }
                    }
                }
                // Inject semantic hashes after tagging so tagged nodes' hashes rotate.
                if let Some(nodes) = obj.get_mut("nodes") {
                    inject_hashes(nodes, &[]);
                }
            }

            // In stale_screen scenario, after returning the tree once, simulate a background mutation
            if state.scenario_name == "stale_screen"
                && !state.has_mutated_stale.swap(true, Ordering::SeqCst)
            {
                state.rev.fetch_add(1, Ordering::SeqCst);
            }

            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&tree_json).unwrap(),
            )
        }
        ("POST", "/tap") => {
            let body_json: Value = serde_json::from_slice(body).unwrap_or(json!({}));
            let req_node = body_json.get("node").and_then(|v| v.as_u64()).unwrap_or(0);
            let req_hash = body_json.get("hash").and_then(|v| v.as_str()).map(String::from);

            // Hash targeting (map #36, #29): validate/resolve by hash when present.
            if let Some(hash) = &req_hash {
                let fixture_file = if state.scenario_name == "token_budget" {
                    "contract/fixtures/deep_gauntlet_tree.json"
                } else {
                    "contract/fixtures/ui_tree.json"
                };
                let fixture_path = Path::new(&state.root_dir).join(fixture_file);
                let nodes = load_fixture_nodes(&fixture_path);
                let mut resolved: Option<usize> = None;
                if (req_node as usize) < nodes.len() && node_hash(&nodes[req_node as usize]) == *hash {
                    resolved = Some(req_node as usize);
                } else {
                    for (i, n) in nodes.iter().enumerate() {
                        if node_hash(n) == *hash {
                            resolved = Some(i);
                            break;
                        }
                    }
                }
                if resolved.is_none() {
                    let err = json!({
                        "error": {
                            "code": "NODE_OUTDATED",
                            "message": "tree refreshed since reference; re-run /ui-tree",
                            "hint": "re-run device_tree, re-target"
                        }
                    });
                    return (
                        "400 Bad Request",
                        vec![("Content-Type", "application/json".to_string())],
                        serde_json::to_vec(&err).unwrap(),
                    );
                }
            } else if req_node == 99999 || state.scenario_name == "stale_screen" {
                let err = json!({
                    "error": {
                        "code": "NODE_OUTDATED",
                        "message": "tree refreshed since reference; re-run /ui-tree",
                        "hint": "re-run device_tree, re-target"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }

            let new_rev = state.rev.fetch_add(1, Ordering::SeqCst) + 1;
            let event = json!({
                "type": "tree_changed",
                "rev": new_rev
            });
            let _ = state.ws_sender.send(event.to_string());

            capture_recording_action(state, "tap", Some(req_node), None, None).await;

            let resp = json!({ "ok": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/type") => {
            let body_json: Value = serde_json::from_slice(body).unwrap_or(json!({}));
            let req_node = body_json.get("node").and_then(|v| v.as_u64()).unwrap_or(0);
            let req_text = body_json
                .get("text")
                .and_then(|v| v.as_str())
                .map(String::from);
            let req_hash = body_json.get("hash").and_then(|v| v.as_str()).map(String::from);

            if let Some(hash) = &req_hash {
                let fixture_file = if state.scenario_name == "token_budget" {
                    "contract/fixtures/deep_gauntlet_tree.json"
                } else {
                    "contract/fixtures/ui_tree.json"
                };
                let fixture_path = Path::new(&state.root_dir).join(fixture_file);
                let nodes = load_fixture_nodes(&fixture_path);
                let mut resolved: Option<usize> = None;
                if (req_node as usize) < nodes.len() && node_hash(&nodes[req_node as usize]) == *hash {
                    resolved = Some(req_node as usize);
                } else {
                    for (i, n) in nodes.iter().enumerate() {
                        if node_hash(n) == *hash {
                            resolved = Some(i);
                            break;
                        }
                    }
                }
                if resolved.is_none() {
                    let err = json!({
                        "error": {
                            "code": "NODE_OUTDATED",
                            "message": "tree refreshed since reference; re-run /ui-tree",
                            "hint": "re-run device_tree, re-target"
                        }
                    });
                    return (
                        "400 Bad Request",
                        vec![("Content-Type", "application/json".to_string())],
                        serde_json::to_vec(&err).unwrap(),
                    );
                }
            } else if state.scenario_name == "stale_screen" {
                let err = json!({
                    "error": {
                        "code": "NODE_OUTDATED",
                        "message": "tree refreshed since reference; re-run /ui-tree",
                        "hint": "re-run device_tree, re-target"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }

            let new_rev = state.rev.fetch_add(1, Ordering::SeqCst) + 1;
            let event = json!({
                "type": "tree_changed",
                "rev": new_rev
            });
            let _ = state.ws_sender.send(event.to_string());

            capture_recording_action(state, "type", Some(req_node), req_text.as_deref(), None).await;

            let resp = json!({ "ok": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/keypress") => {
            let body_json: Value = serde_json::from_slice(body).unwrap_or(json!({}));
            let key = body_json
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("ENTER");
            capture_recording_action(
                state,
                "keypress",
                body_json.get("node").and_then(|v| v.as_u64()),
                None,
                Some(key),
            )
            .await;
            let resp = json!({ "ok": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/back") => {
            capture_recording_action(state, "keypress", None, None, Some("BACK")).await;
            let resp = json!({ "ok": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/recording/start") => {
            let mut rec = state.recording.lock().await;
            rec.active = true;
            rec.started_at = Some(std::time::Instant::now());
            let resp = json!({ "ok": true, "recording": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/recording/stop") => {
            let mut rec = state.recording.lock().await;
            if !rec.active {
                let err = json!({
                    "error": {
                        "code": "RECORDING_IDLE",
                        "message": "no active recording session",
                        "hint": "start with POST /recording/start first"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }
            rec.active = false;
            let count = rec.actions.len();
            let resp = json!({ "ok": true, "recording": false, "count": count });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("GET", "/recording/export") => {
            let rec = state.recording.lock().await;
            let resp = json!({
                "recording": rec.active,
                "count": rec.actions.len(),
                "actions": rec.actions.clone(),
            });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("DELETE", "/recording") => {
            let mut rec = state.recording.lock().await;
            rec.actions.clear();
            rec.next_seq = 0;
            let resp = json!({ "ok": true, "count": 0 });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("POST", "/tag") => {
            let body_json: Value = serde_json::from_slice(body).unwrap_or(json!({}));
            let target = body_json.get("target").cloned().unwrap_or(json!({}));
            let target_index = target.get("index").and_then(|v| v.as_u64()).unwrap_or(0);
            let target_rev = target.get("rev").and_then(|v| v.as_u64()).unwrap_or(0);
            let tag = body_json.get("tag").and_then(|v| v.as_str()).unwrap_or("").to_string();

            if tag.trim().is_empty() || tag.len() > 256 {
                let err = json!({
                    "error": {
                        "code": "BAD_REQUEST",
                        "message": "Invalid tag: must not be blank or exceed 256 characters",
                        "hint": "provide a non-blank tag of at most 256 characters"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }

            let current_rev = state.rev.load(Ordering::SeqCst);
            if target_rev != current_rev {
                let err = json!({
                    "error": {
                        "code": "NODE_OUTDATED",
                        "message": "tree refreshed since reference; re-run /ui-tree",
                        "hint": "re-run device_tree, re-target"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }

            // Validate index bounds against fixture
            let fixture_file = if state.scenario_name == "token_budget" {
                "contract/fixtures/deep_gauntlet_tree.json"
            } else {
                "contract/fixtures/ui_tree.json"
            };
            let fixture_path = Path::new(&state.root_dir).join(fixture_file);
            let nodes = load_fixture_nodes(&fixture_path);
            if target_index as usize >= nodes.len() {
                let err = json!({
                    "error": {
                        "code": "NODE_OUTDATED",
                        "message": "tree refreshed since reference; re-run /ui-tree",
                        "hint": "re-run device_tree, re-target"
                    }
                });
                return (
                    "400 Bad Request",
                    vec![("Content-Type", "application/json".to_string())],
                    serde_json::to_vec(&err).unwrap(),
                );
            }

            // Mutate in-memory tag registry (survives refreshes) by storing on state.
            // Tag the node: store mapping tag -> index in a registry; the /ui-tree
            // response will reflect the tag as the node's id on next fetch.
            {
                let mut tags = state.tags.lock().await;
                tags.insert(tag.clone(), target_index);
            }

            let new_rev = state.rev.fetch_add(1, Ordering::SeqCst) + 1;
            let event = json!({
                "type": "tree_changed",
                "rev": new_rev
            });
            let _ = state.ws_sender.send(event.to_string());

            // Update the cached fixture? We can't easily mutate fixture file.
            // Instead, /ui-tree handler applies tags from state.tags to nodes.
            let resp = json!({ "ok": true });
            (
                "200 OK",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&resp).unwrap(),
            )
        }
        ("GET", "/screenshot") => {
            // PNG magic bytes + valid minimal PNG chunks
            let scale = if query_str.contains("scale=1.0") {
                1.0
            } else if query_str.contains("scale=0.25") {
                0.25
            } else {
                0.5
            };

            // Region crop (map #36, #30): region=x,y,w,h → crop dimensions.
            let (rw, rh) = parse_region(query_str).unwrap_or((0, 0));
            // Vary byte size based on scale (and region if present) to satisfy BB-A4
            let png_bytes = if rw > 0 && rh > 0 {
                make_png_bytes_region(scale, rw, rh)
            } else {
                make_png_bytes(scale)
            };
            (
                "200 OK",
                vec![("Content-Type", "image/png".to_string())],
                png_bytes,
            )
        }
        ("GET", "/last-crash") => {
            let err = json!({
                "error": {
                    "code": "NOT_FOUND",
                    "message": "no crash bundle available"
                }
            });
            (
                "404 Not Found",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&err).unwrap(),
            )
        }
        _ => {
            let err = json!({
                "error": {
                    "code": "NOT_FOUND",
                    "message": format!("Route not found: {method} {path}")
                }
            });
            (
                "404 Not Found",
                vec![("Content-Type", "application/json".to_string())],
                serde_json::to_vec(&err).unwrap(),
            )
        }
    }
}

fn parse_region(q: &str) -> Option<(u32, u32)> {
    for part in q.split('&') {
        if let Some(val) = part.strip_prefix("region=") {
            let nums: Vec<&str> = val.split(',').collect();
            if nums.len() == 4 {
                if let (Ok(_x), Ok(_y), Ok(w), Ok(h)) =
                    (nums[0].parse::<i32>(), nums[1].parse::<i32>(), nums[2].parse::<u32>(), nums[3].parse::<u32>())
                {
                    if w > 0 && h > 0 {
                        return Some((w, h));
                    }
                }
            }
        }
    }
    None
}

fn make_png_bytes_region(scale: f64, rw: u32, rh: u32) -> Vec<u8> {
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let width: u32 = ((rw as f64) * scale) as u32;
    let height: u32 = ((rh as f64) * scale) as u32;
    let width = width.max(1);
    let height = height.max(1);

    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8);
    ihdr_data.push(6);
    ihdr_data.push(0);
    ihdr_data.push(0);
    ihdr_data.push(0);

    write_png_chunk(&mut bytes, b"IHDR", &ihdr_data);

    let tag = format!("region={}x{} scale={}", rw, rh, scale);
    write_png_chunk(&mut bytes, b"tEXt", tag.as_bytes());

    let idat_data = [0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01];
    write_png_chunk(&mut bytes, b"IDAT", &idat_data);

    write_png_chunk(&mut bytes, b"IEND", &[]);
    bytes
}

fn make_png_bytes(scale: f64) -> Vec<u8> {
    // 8-byte PNG signature
    let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    // IHDR chunk (13 bytes)
    let width: u32 = (1080.0 * scale) as u32;
    let height: u32 = (2400.0 * scale) as u32;

    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.push(8); // bit depth
    ihdr_data.push(6); // RGBA
    ihdr_data.push(0); // compression
    ihdr_data.push(0); // filter
    ihdr_data.push(0); // interlace

    write_png_chunk(&mut bytes, b"IHDR", &ihdr_data);

    // Add extra comment bytes based on scale so byte sizes strictly differ across scales
    let tag = format!("scale={scale}");
    write_png_chunk(&mut bytes, b"tEXt", tag.as_bytes());

    // Minimal valid IDAT chunk (zlib header + empty deflated block)
    let idat_data = [0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01];
    write_png_chunk(&mut bytes, b"IDAT", &idat_data);

    // IEND chunk
    write_png_chunk(&mut bytes, b"IEND", &[]);

    bytes
}

fn write_png_chunk(dst: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    dst.extend_from_slice(&len.to_be_bytes());
    dst.extend_from_slice(chunk_type);
    dst.extend_from_slice(data);

    // Compute CRC32 over chunk_type + data
    let mut crc = 0xffffffffu32;
    for &b in chunk_type.iter().chain(data.iter()) {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xedb88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^= 0xffffffff;
    dst.extend_from_slice(&crc.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fake_bridge_ui_tree_and_tap() {
        let mut server = FakeBridgeServer::start(FakeBridgeConfig {
            scenario_name: "static_screen".to_string(),
            root_dir: ".".to_string(),
            port: 0,
            token: "test-token".to_string(),
        })
        .await
        .expect("server start");

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}", server.addr.port());

        // Test GET /ui-tree
        let resp = client
            .get(format!("{url}/ui-tree"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("get ui-tree");
        assert_eq!(resp.status(), 200);
        let tree: Value = resp.json().await.expect("json");
        assert!(tree.get("rev").unwrap().as_u64().unwrap() > 0);

        // Test POST /tap
        let tap_resp = client
            .post(format!("{url}/tap"))
            .header("Authorization", "Bearer test-token")
            .json(&json!({"node": 1, "rev": 42}))
            .send()
            .await
            .expect("tap");
        assert_eq!(tap_resp.status(), 200);

        // Test GET /screenshot
        let ss_resp = client
            .get(format!("{url}/screenshot"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("screenshot");
        assert_eq!(ss_resp.status(), 200);
        let bytes = ss_resp.bytes().await.expect("bytes");
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );

        // Test auth failure
        let auth_fail = client
            .get(format!("{url}/ui-tree"))
            .header("Authorization", "Bearer wrong-token")
            .send()
            .await
            .expect("auth fail");
        assert_eq!(auth_fail.status(), 401);

        server.stop();
    }

    #[tokio::test]
    async fn test_fake_bridge_recording_lifecycle() {
        let mut server = FakeBridgeServer::start(FakeBridgeConfig {
            scenario_name: "static_screen".to_string(),
            root_dir: ".".to_string(),
            port: 0,
            token: "test-token".to_string(),
        })
        .await
        .expect("server start");

        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}", server.addr.port());

        // Fresh server: export shape (BB-B15) + stop while idle (BB-B12)
        let export = client
            .get(format!("{url}/recording/export"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("export");
        assert_eq!(export.status(), 200);
        let export_json: Value = export.json().await.expect("json");
        assert_eq!(export_json["recording"], false);
        assert_eq!(export_json["count"], 0);
        assert!(export_json["actions"].is_array());

        let idle_stop = client
            .post(format!("{url}/recording/stop"))
            .header("Authorization", "Bearer test-token")
            .json(&json!({}))
            .send()
            .await
            .expect("stop while idle");
        assert_eq!(idle_stop.status(), 400);
        let idle_json: Value = idle_stop.json().await.expect("json");
        assert_eq!(idle_json["error"]["code"], "RECORDING_IDLE");

        // Start is idempotent; stop reports count (BB-B12)
        let start = client
            .post(format!("{url}/recording/start"))
            .header("Authorization", "Bearer test-token")
            .json(&json!({}))
            .send()
            .await
            .expect("start");
        assert_eq!(start.status(), 200);
        let start_json: Value = start.json().await.expect("json");
        assert_eq!(start_json["ok"], true);
        assert_eq!(start_json["recording"], true);

        let start_again = client
            .post(format!("{url}/recording/start"))
            .header("Authorization", "Bearer test-token")
            .json(&json!({}))
            .send()
            .await
            .expect("start again");
        assert_eq!(start_again.status(), 200);

        // Actions while recording are captured in order (BB-B14)
        for (path, body) in [
            ("/tap", json!({"node": 1, "rev": 42})),
            ("/type", json!({"node": 2, "text": "hi", "rev": 42})),
            ("/keypress", json!({"key": "ENTER"})),
            ("/back", json!({})),
        ] {
            let resp = client
                .post(format!("{url}{path}"))
                .header("Authorization", "Bearer test-token")
                .json(&body)
                .send()
                .await
                .expect("mutation while recording");
            assert_eq!(resp.status(), 200, "{path}");
        }

        let export = client
            .get(format!("{url}/recording/export"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("export after mutations");
        assert_eq!(export.status(), 200);
        let export_json: Value = export.json().await.expect("json");
        assert_eq!(export_json["recording"], true);
        assert_eq!(export_json["count"], 4);
        let actions = export_json["actions"].as_array().expect("actions");
        let kinds: Vec<&str> = actions
            .iter()
            .map(|a| a["action"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["tap", "type", "keypress", "keypress"]);
        assert_eq!(actions[1]["value"], "hi");

        let stop = client
            .post(format!("{url}/recording/stop"))
            .header("Authorization", "Bearer test-token")
            .json(&json!({}))
            .send()
            .await
            .expect("stop");
        assert_eq!(stop.status(), 200);
        let stop_json: Value = stop.json().await.expect("json");
        assert_eq!(stop_json["ok"], true);
        assert_eq!(stop_json["recording"], false);
        assert_eq!(stop_json["count"], 4);

        // DELETE clears the buffer (BB-B15)
        let clear = client
            .delete(format!("{url}/recording"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("delete recording");
        assert_eq!(clear.status(), 200);

        let export = client
            .get(format!("{url}/recording/export"))
            .header("Authorization", "Bearer test-token")
            .send()
            .await
            .expect("export after delete");
        assert_eq!(export.status(), 200);
        let export_json: Value = export.json().await.expect("json");
        assert_eq!(export_json["count"], 0);
        assert_eq!(export_json["actions"].as_array().expect("actions").len(), 0);

        server.stop();
    }
}
