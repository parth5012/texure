//! Inspector HTTP server and WebSocket live synchronizer.

use std::io::Cursor;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use texure_contract::bridge::{KeyPressRequest, TapRequest, TypeRequest};
use texure_contract::error::ErrorCode;
use texure_contract::tools::KeyPressKey;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

use crate::bridge_client::BridgeClient;
use crate::ws_supervisor::WsSupervisor;

/// Embedded single-page Inspector assets.
pub const INSPECT_HTML: &str = include_str!("../assets/inspect.html");

/// An async stream wrapper that prepends buffered bytes before reading from the underlying stream.
pub struct PrefixedStream<S> {
    prefix: Cursor<Vec<u8>>,
    stream: S,
}

impl<S: AsyncRead + Unpin> AsyncRead for PrefixedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let pos = self.prefix.position() as usize;
        let total = self.prefix.get_ref().len();
        if pos < total {
            let before = buf.filled().len();
            let mut slice = &self.prefix.get_ref()[pos..];
            let read_len = std::io::Read::read(&mut slice, buf.initialize_unfilled())?;
            self.prefix.set_position((pos + read_len) as u64);
            buf.advance(read_len);
            if buf.filled().len() > before {
                return Poll::Ready(Ok(()));
            }
        }
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for PrefixedStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

/// Request payload for `POST /inspect/action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectActionRequest {
    pub action: String,
    #[serde(default)]
    pub index: Option<u64>,
    #[serde(default)]
    pub node: Option<u64>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub rev: Option<u64>,
}

struct InspectorShared {
    bridge: BridgeClient,
    supervisor: Option<WsSupervisor>,
    active_connections: AtomicUsize,
    shutdown: broadcast::Sender<()>,
    single_flight: Mutex<()>,
}

/// Live Inspector web server serving HTTP assets and live WebSocket stream.
pub struct InspectorServer {
    addr: SocketAddr,
    shared: Arc<InspectorShared>,
    accept_task: JoinHandle<()>,
}

impl InspectorServer {
    /// Bind to a TCP port and start serving inspector assets and WebSocket stream.
    pub async fn bind(
        port: u16,
        bridge: BridgeClient,
        supervisor: Option<WsSupervisor>,
    ) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        let addr = listener.local_addr()?;
        let (shutdown, _) = broadcast::channel(16);

        let shared = Arc::new(InspectorShared {
            bridge,
            supervisor,
            active_connections: AtomicUsize::new(0),
            shutdown,
            single_flight: Mutex::new(()),
        });

        let accept_shared = Arc::clone(&shared);
        let mut shutdown_rx = shared.shutdown.subscribe();

        let accept_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        match accepted {
                            Ok((stream, peer)) => {
                                let state = Arc::clone(&accept_shared);
                                tokio::spawn(async move {
                                    if let Err(e) = serve_connection(stream, peer, state).await {
                                        debug!("Inspector connection error from {peer}: {e}");
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Inspector listener accept failed: {e}");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        });

        Ok(Self {
            addr,
            shared,
            accept_task,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn connect_addr(&self) -> SocketAddr {
        if self.addr.ip().is_unspecified() {
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                self.addr.port(),
            )
        } else {
            self.addr
        }
    }

    pub fn active_connections(&self) -> usize {
        self.shared.active_connections.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        let _ = self.shared.shutdown.send(());
        self.accept_task.abort();
    }
}

impl Drop for InspectorServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    _peer: SocketAddr,
    shared: Arc<InspectorShared>,
) -> std::io::Result<()> {
    let mut header_buf = Vec::new();
    let mut tmp = [0u8; 1024];

    let end = loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        header_buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = header_buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if header_buf.len() > 65536 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Header too large",
            ));
        }
    };
    let header_bytes = &header_buf[..end];
    let header_str = String::from_utf8_lossy(header_bytes);
    let mut lines = header_str.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path_and_query = parts.next().unwrap_or("");
    let path = path_and_query.split('?').next().unwrap_or("");

    // WebSocket upgrade check
    let is_ws = path == "/inspect/ws" || header_str.to_lowercase().contains("upgrade: websocket");
    if is_ws {
        let prefixed = PrefixedStream {
            prefix: Cursor::new(header_buf),
            stream,
        };
        handle_websocket(prefixed, shared).await;
        return Ok(());
    }

    // Serve single-page HTML
    if method == "GET" && (path == "/inspect" || path == "/") {
        let body = INSPECT_HTML.as_bytes();
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(resp.as_bytes()).await?;
        stream.write_all(body).await?;
        stream.flush().await?;
        return Ok(());
    }

    // UI tree proxy
    if method == "GET" && path == "/ui-tree" {
        match shared.bridge.get_ui_tree(None, None).await {
            Ok(tree) => {
                let json = serde_json::to_vec(&tree).unwrap_or_default();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    json.len()
                );
                stream.write_all(resp.as_bytes()).await?;
                stream.write_all(&json).await?;
                stream.flush().await?;
            }
            Err(err) => {
                let err_json = serde_json::json!({ "error": err.message }).to_string();
                let resp = format!(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_json.len(),
                    err_json
                );
                stream.write_all(resp.as_bytes()).await?;
                stream.flush().await?;
            }
        }
        return Ok(());
    }

    // Live Drive action handler
    if method == "POST" && path == "/inspect/action" {
        let content_length: usize = lines
            .find(|l| l.to_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);

        let mut body_bytes = header_buf[end..].to_vec();
        while body_bytes.len() < content_length {
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&tmp[..n]);
        }
        body_bytes.truncate(content_length);

        handle_action(stream, &body_bytes, shared).await?;
        return Ok(());
    }

    // 404 fallback
    let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found";
    stream.write_all(resp.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn handle_websocket<S: AsyncRead + AsyncWrite + Unpin>(stream: S, shared: Arc<InspectorShared>) {
    let mut ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("Failed to accept inspector WebSocket: {e}");
            return;
        }
    };

    shared.active_connections.fetch_add(1, Ordering::SeqCst);
    debug!("Inspector WebSocket client connected");

    // Push initial snapshot
    let tree_res = shared.bridge.get_ui_tree(None, None).await;
    let shot_res = shared.bridge.screenshot(Some(0.5)).await;
    if let (Ok(tree), Ok(shot)) = (tree_res, shot_res) {
        let b64 = BASE64.encode(&shot);
        let payload = serde_json::json!({
            "tree": tree,
            "screenshot": b64,
        });
        let _ = ws.send(Message::Text(payload.to_string())).await;
    }

    let mut tree_rx = shared.supervisor.as_ref().map(|s| s.subscribe());
    let mut shutdown_rx = shared.shutdown.subscribe();

    loop {
        tokio::select! {
            client_msg = ws.next() => {
                match client_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = ws.send(Message::Pong(payload)).await;
                    }
                    _ => {}
                }
            }
            event = async {
                match &mut tree_rx {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                match event {
                    Ok(_ev) => {
                        let tree_res = shared.bridge.get_ui_tree(None, None).await;
                        let shot_res = shared.bridge.screenshot(Some(0.5)).await;
                        if let (Ok(tree), Ok(shot)) = (tree_res, shot_res) {
                            let b64 = BASE64.encode(&shot);
                            let payload = serde_json::json!({
                                "tree": tree,
                                "screenshot": b64,
                            });
                            if ws.send(Message::Text(payload.to_string())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = shutdown_rx.recv() => break,
        }
    }

    shared.active_connections.fetch_sub(1, Ordering::SeqCst);
    debug!("Inspector WebSocket client disconnected");
}

async fn handle_action(
    mut stream: TcpStream,
    body: &[u8],
    shared: Arc<InspectorShared>,
) -> std::io::Result<()> {
    let action_req: InspectActionRequest = match serde_json::from_slice(body) {
        Ok(req) => req,
        Err(e) => {
            let err_body = serde_json::json!({ "error": format!("Invalid JSON: {e}") }).to_string();
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                err_body.len(),
                err_body
            );
            stream.write_all(resp.as_bytes()).await?;
            stream.flush().await?;
            return Ok(());
        }
    };

    let _guard = shared.single_flight.lock().await;

    let res = match action_req.action.as_str() {
        "tap" => {
            let index = action_req.index.or(action_req.node).unwrap_or(0);
            let rev = action_req.rev.unwrap_or(0);
            let mut tap = TapRequest::new(rev).with_index(index);
            if let Some(hash) = action_req.hash {
                tap = tap.with_hash(hash);
            }
            shared.bridge.tap(&tap).await.map(|_| ())
        }
        "type" => {
            let text = action_req.text.unwrap_or_default();
            let rev = action_req.rev.unwrap_or(0);
            let mut typ = TypeRequest::new(text, rev);
            if let Some(idx) = action_req.index.or(action_req.node) {
                typ = typ.with_index(idx);
            }
            if let Some(hash) = action_req.hash {
                typ = typ.with_hash(hash);
            }
            shared.bridge.type_text(&typ).await.map(|_| ())
        }
        "keypress" => {
            let key_str = action_req.key.as_deref().unwrap_or("enter");
            let key = match key_str.to_lowercase().as_str() {
                "back" => KeyPressKey::Back,
                _ => KeyPressKey::Enter,
            };
            let kp = KeyPressRequest { key };
            shared.bridge.keypress(&kp).await.map(|_| ())
        }
        other => {
            let err_body =
                serde_json::json!({ "error": format!("Unknown action: {other}") }).to_string();
            let resp = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                err_body.len(),
                err_body
            );
            stream.write_all(resp.as_bytes()).await?;
            stream.flush().await?;
            return Ok(());
        }
    };

    match res {
        Ok(()) => {
            let ok_body = serde_json::json!({ "status": "ok" }).to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                ok_body.len(),
                ok_body
            );
            stream.write_all(resp.as_bytes()).await?;
            stream.flush().await?;
        }
        Err(err) => {
            let (status_code, status_text) = if err.code == ErrorCode::NodeOutdated {
                ("409 Conflict", "NODE_OUTDATED")
            } else {
                ("500 Internal Server Error", err.code.as_str())
            };
            let err_body = serde_json::json!({
                "error": status_text,
                "message": err.message
            })
            .to_string();
            let resp = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status_code,
                err_body.len(),
                err_body
            );
            stream.write_all(resp.as_bytes()).await?;
            stream.flush().await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;
    use texure_contract::bridge::TreeChangedEvent;
    use texure_contract::config::ResolvedDeviceConfig;
    use texure_contract::tree::{UiNode, UiTree};

    // Minimal 1x1 valid PNG bytes (67 bytes)
    const MOCK_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
        0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
        0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
        0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    struct MockBridgeServer {
        addr: SocketAddr,
        rev: Arc<AtomicU64>,
        shutdown: broadcast::Sender<()>,
        accept_task: JoinHandle<()>,
    }

    impl MockBridgeServer {
        async fn start() -> Self {
            let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let addr = listener.local_addr().unwrap();
            let rev = Arc::new(AtomicU64::new(42));
            let (shutdown, _) = broadcast::channel(4);
            let mut shutdown_rx = shutdown.subscribe();

            let rev_clone = Arc::clone(&rev);
            let accept_task = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        Ok((mut stream, _)) = listener.accept() => {
                            let rev = Arc::clone(&rev_clone);
                            tokio::spawn(async move {
                                let mut buf = [0u8; 1024];
                                if let Ok(n) = stream.read(&mut buf).await {
                                    let req = String::from_utf8_lossy(&buf[..n]);
                                    if req.starts_with("GET /ui-tree") {
                                        let tree = UiTree {
                                            rev: rev.load(Ordering::SeqCst),
                                            window: "com.example/.MainActivity".to_string(),
                                            nodes: vec![UiNode {
                                                id: Some("test_node".to_string()),
                                                text: Some("Test".to_string()),
                                                cd: None,
                                                cls: Some("android.widget.TextView".to_string()),
                                                bounds: Some([0, 0, 100, 50]),
                                                clickable: true,
                                                editable: false,
                                                scrollable: false,
                                                index: Some(0),
                                                hash: Some("abc".to_string()),
                                                children: vec![],
                                            }],
                                        };
                                        let json = serde_json::to_vec(&tree).unwrap();
                                        let resp = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                            json.len()
                                        );
                                        let _ = stream.write_all(resp.as_bytes()).await;
                                        let _ = stream.write_all(&json).await;
                                    } else if req.starts_with("GET /screenshot") {
                                        let resp = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                            MOCK_PNG.len()
                                        );
                                        let _ = stream.write_all(resp.as_bytes()).await;
                                        let _ = stream.write_all(MOCK_PNG).await;
                                    } else if req.starts_with("POST /tap") {
                                        if req.contains("\"rev\":999") || req.contains("\"rev\": 999") {
                                            let err_resp = serde_json::json!({
                                                "error": {
                                                    "code": "NODE_OUTDATED",
                                                    "message": "Node revision outdated"
                                                }
                                            }).to_string();
                                            let resp = format!(
                                                "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                err_resp.len(),
                                                err_resp
                                            );
                                            let _ = stream.write_all(resp.as_bytes()).await;
                                        } else {
                                            let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";
                                            let _ = stream.write_all(resp.as_bytes()).await;
                                        }
                                    } else if req.starts_with("POST /type") {
                                        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";
                                        let _ = stream.write_all(resp.as_bytes()).await;
                                    } else if req.starts_with("POST /keypress") {
                                        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}";
                                        let _ = stream.write_all(resp.as_bytes()).await;
                                    } else {
                                        let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                                        let _ = stream.write_all(resp.as_bytes()).await;
                                    }
                                }
                            });
                        }
                        _ = shutdown_rx.recv() => break,
                    }
                }
            });

            Self {
                addr,
                rev,
                shutdown,
                accept_task,
            }
        }

        fn bump_rev(&self, new_rev: u64) {
            self.rev.store(new_rev, Ordering::SeqCst);
        }

        fn stop(&self) {
            let _ = self.shutdown.send(());
            self.accept_task.abort();
        }
    }

    impl Drop for MockBridgeServer {
        fn drop(&mut self) {
            self.stop();
        }
    }

    #[test]
    fn test_inspect_html_non_empty_and_valid() {
        assert!(!INSPECT_HTML.is_empty(), "INSPECT_HTML must not be empty");
        assert!(INSPECT_HTML.contains("<!DOCTYPE html>"));
        assert!(INSPECT_HTML.contains("<html"));
        assert!(INSPECT_HTML.contains("<head>"));
        assert!(INSPECT_HTML.contains("<body>"));
        assert!(INSPECT_HTML.contains("</html>"));
        assert!(INSPECT_HTML.contains("screen-canvas") || INSPECT_HTML.contains("canvas"));
        assert!(INSPECT_HTML.contains("Node detail") || INSPECT_HTML.contains("node-detail"));
        assert!(INSPECT_HTML.contains("Copy Tool Call") || INSPECT_HTML.contains("copy-tool-call"));
    }

    #[test]
    fn test_inspect_html_no_external_resources() {
        assert!(!INSPECT_HTML.contains("http://"));
        assert!(!INSPECT_HTML.contains("https://"));
        assert!(!INSPECT_HTML.contains("<link rel=\"stylesheet\" href=\"//"));
        assert!(!INSPECT_HTML.contains("<script src=\"//"));
    }

    #[tokio::test]
    async fn test_inspect_http_serving() {
        let dummy_config = ResolvedDeviceConfig {
            url: "http://127.0.0.1:9".to_string(),
            token: "tok".to_string(),
            timeout_secs: 1,
        };
        let bridge = BridgeClient::new(dummy_config).unwrap();
        let server = InspectorServer::bind(0, bridge, None).await.unwrap();

        let inspect_url = format!("http://{}/inspect", server.local_addr());
        let resp = reqwest::get(&inspect_url)
            .await
            .expect("connect to inspect server");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html; charset=utf-8"
        );
        let body = resp.text().await.expect("read inspect response body");
        assert_eq!(body, INSPECT_HTML);
    }

    #[tokio::test]
    async fn test_inspect_websocket_sync_and_cleanup() {
        let mock_bridge = MockBridgeServer::start().await;
        let config = ResolvedDeviceConfig {
            url: format!("http://{}", mock_bridge.addr),
            token: "tok".to_string(),
            timeout_secs: 2,
        };
        let bridge = BridgeClient::new(config.clone()).unwrap();
        let supervisor = WsSupervisor::new(config);

        let server = InspectorServer::bind(0, bridge, Some(supervisor.clone()))
            .await
            .unwrap();

        let ws_url = format!("ws://{}/inspect/ws", server.local_addr());
        let (mut ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
            .await
            .expect("connect to inspector WS");

        assert_eq!(server.active_connections(), 1);

        // 1. Initial snapshot check
        let msg = ws_stream.next().await.expect("initial WS message").unwrap();
        if let Message::Text(text) = msg {
            let val: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(val["tree"]["rev"], 42);
            assert!(val["screenshot"].is_string());
        } else {
            panic!("Expected text message");
        }

        // 2. Tree changed notification trigger
        mock_bridge.bump_rev(43);
        let event = TreeChangedEvent::new(43);
        supervisor.notify(event).unwrap();

        // Receive push update on WebSocket
        let updated_msg = tokio::time::timeout(Duration::from_secs(2), ws_stream.next())
            .await
            .expect("timeout waiting for push update")
            .expect("stream message")
            .unwrap();

        if let Message::Text(text) = updated_msg {
            let val: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(val["tree"]["rev"], 43);
            assert!(val["screenshot"].is_string());
        } else {
            panic!("Expected text message for push update");
        }

        // 3. Tab close cleanup check
        drop(ws_stream);
        // Wait for cleanup
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(server.active_connections(), 0);
    }

    #[tokio::test]
    async fn test_inspect_action_handlers_and_rev_validation() {
        let mock_bridge = MockBridgeServer::start().await;
        let config = ResolvedDeviceConfig {
            url: format!("http://{}", mock_bridge.addr),
            token: "tok".to_string(),
            timeout_secs: 2,
        };
        let bridge = BridgeClient::new(config.clone()).unwrap();
        let server = InspectorServer::bind(0, bridge, None).await.unwrap();

        let client = reqwest::Client::new();
        let action_url = format!("http://{}/inspect/action", server.local_addr());

        // 1. Valid tap action
        let tap_body = serde_json::json!({
            "action": "tap",
            "index": 0,
            "rev": 42
        });
        let resp = client.post(&action_url).json(&tap_body).send().await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        // 2. Outdated rev tap action -> expects 409 Conflict with NODE_OUTDATED
        let outdated_tap = serde_json::json!({
            "action": "tap",
            "index": 0,
            "rev": 999
        });
        let resp = client.post(&action_url).json(&outdated_tap).send().await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::CONFLICT);
        let err_json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(err_json["error"], "NODE_OUTDATED");

        // 3. Remote keyboard type action
        let type_body = serde_json::json!({
            "action": "type",
            "index": 0,
            "text": "Hello World",
            "rev": 42
        });
        let resp = client.post(&action_url).json(&type_body).send().await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        // 4. Remote keypress action
        let key_body = serde_json::json!({
            "action": "keypress",
            "key": "enter"
        });
        let resp = client.post(&action_url).json(&key_body).send().await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
    }
}
