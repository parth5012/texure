//! Reverse WebSocket tunnel listener (Group R, ticket #60).
//!
//! The desktop server listens for an *inbound* WebSocket connection from the
//! remote companion on `/tunnel`. This is the inverse of `WsSupervisor`, which
//! dials *out* to the in-app bridge's `/ws`. The companion authenticates with a
//! bearer token (HTTP `Authorization` header and/or a first TEXT frame
//! `{"action":"register","token":...}`), after which the connection becomes
//! the single active driver of the paired device.
//!
//! Guardrail G11: in production the endpoint is TLS-encrypted (`wss://`) with
//! bearer auth — TLS is terminated externally (e.g. ngrok). This module uses a
//! raw `TcpListener` + `tokio_tungstenite::accept_async` (no axum/hyper) so it
//! can run plain `ws://` on localhost for in-process tests and local dev.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use texure_contract::error::{BridgeError, BridgeErrorEnvelope, ErrorCode};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, warn};

/// Poison-tolerant mutex access, matching the repo's existing pattern.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Configuration for the reverse tunnel listener.
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    /// HTTP path the companion must request during the upgrade.
    pub path: String,
    /// How often the server emits a heartbeat ping.
    pub heartbeat_interval: Duration,
    /// Connection is torn down if no pong is seen within this window.
    pub pong_timeout: Duration,
    /// Deadline for the companion's first register frame.
    pub handshake_timeout: Duration,
    /// Deadline for a correlated reply to [`TunnelHandle::request`].
    pub request_timeout: Duration,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        let heartbeat_interval = Duration::from_secs(15);
        Self {
            path: crate::pair::DEFAULT_TUNNEL_PATH.to_string(),
            heartbeat_interval,
            // Mirror the companion: >= 2x interval, floor 10s.
            pong_timeout: (heartbeat_interval * 2).max(Duration::from_secs(10)),
            handshake_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// Errors surfaced by the in-process tunnel API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelError {
    Bind(String),
    Handshake(String),
    AuthFailed,
    Busy { owner: String },
    NotConnected,
    Timeout,
    Disconnected,
    InvalidMessage(String),
}

impl TunnelError {
    /// Frozen contract code this error maps to.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::AuthFailed => ErrorCode::AuthFailed,
            Self::Busy { .. } => ErrorCode::Busy,
            Self::NotConnected | Self::Disconnected => ErrorCode::Offline,
            _ => ErrorCode::Offline,
        }
    }
}

impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind(m) => write!(f, "tunnel bind failed: {m}"),
            Self::Handshake(m) => write!(f, "tunnel handshake failed: {m}"),
            Self::AuthFailed => write!(f, "{}", ErrorCode::AuthFailed),
            Self::Busy { owner } => write!(f, "BUSY: tunnel owner {owner}"),
            Self::NotConnected => write!(f, "no tunnel connected"),
            Self::Timeout => write!(f, "tunnel request timed out"),
            Self::Disconnected => write!(f, "tunnel disconnected"),
            Self::InvalidMessage(m) => write!(f, "invalid tunnel message: {m}"),
        }
    }
}

impl std::error::Error for TunnelError {}

/// The active connection's outbound channel, tagged with its owner id so a
/// stale teardown cannot clear a newer driver's slot.
struct ActiveConnection {
    id: u64,
    owner: String,
    outbound: mpsc::Sender<Message>,
}

/// Process-wide state shared by the accept loop, the active session and every
/// cloned [`TunnelHandle`].
struct TunnelShared {
    expected_token: String,
    config: TunnelConfig,
    connected: AtomicBool,
    active: Mutex<Option<ActiveConnection>>,
    pending: Mutex<HashMap<String, oneshot::Sender<String>>>,
    next_id: AtomicU64,
    shutdown: broadcast::Sender<()>,
}

/// Extract a correlation key (`seq` or JSON-RPC `id`) from an envelope.
fn scalar_key(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn correlation_key(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value
        .get("seq")
        .and_then(scalar_key)
        .or_else(|| value.get("id").and_then(scalar_key))
}

/// A frozen-code error frame shared by post-upgrade AUTH_FAILED / BUSY refusals.
fn error_frame(code: ErrorCode, message: &str, owner: Option<&str>) -> String {
    let mut error = BridgeError::new(code, message);
    if let Some(owner) = owner {
        error = error.with_owner(owner);
    }
    serde_json::to_string(&BridgeErrorEnvelope::from_error(error)).unwrap_or_else(|_| {
        format!(
            "{{\"error\":{{\"code\":\"{}\",\"message\":\"{}\"}}}}",
            code.as_str(),
            message
        )
    })
}

/// HTTP error used during the upgrade handshake (401 route/token refusals).
fn http_error(status: StatusCode, code: &str, message: &str) -> ErrorResponse {
    let mut response = ErrorResponse::new(Some(message.to_string()));
    *response.status_mut() = status;
    if let Ok(header) = HeaderValue::from_str(code) {
        response.headers_mut().insert("X-Texure-Code", header);
    }
    response
}

/// Clear state when a session ends. Only the named owner may be released so a
/// late teardown cannot evict a companion that registered after a reconnect.
fn release(shared: &TunnelShared, id: u64) {
    // Hold the `active` lock across ALL state mutations: registration sets
    // `connected` under this same lock, so a stale teardown must not be able
    // to clear a newer owner's flag or its in-flight requests.
    let mut active = lock(&shared.active);
    if active.as_ref().is_some_and(|conn| conn.id == id) {
        *active = None;
        shared.connected.store(false, Ordering::SeqCst);
        // Dropping the senders resolves outstanding requests as disconnected.
        lock(&shared.pending).clear();
    }
}

/// A cloneable handle to a running tunnel listener (the seam for #61/#63).
#[derive(Clone)]
pub struct TunnelHandle {
    shared: Arc<TunnelShared>,
}

impl TunnelHandle {
    /// Whether a registered companion is currently connected.
    pub fn is_connected(&self) -> bool {
        self.shared.connected.load(Ordering::SeqCst)
    }

    /// Identity of the current single-flight owner, if any.
    pub fn owner(&self) -> Option<String> {
        lock(&self.shared.active)
            .as_ref()
            .map(|conn| conn.owner.clone())
    }

    /// Send a gateway/JSON-RPC envelope and await its correlated reply.
    ///
    /// Correlation uses the envelope's `seq` (or JSON-RPC `id`). Senders carry
    /// arbitrary JSON text; #61 builds the gateway envelope on top of this
    /// primitive and #63 starts the listener in the blackbox harness.
    pub async fn request(&self, envelope: &str) -> Result<String, TunnelError> {
        let outbound = {
            let active = lock(&self.shared.active);
            active
                .as_ref()
                .map(|conn| conn.outbound.clone())
                .ok_or(TunnelError::NotConnected)?
        };

        let key = correlation_key(envelope).unwrap_or_else(|| {
            format!(
                "bare-{}",
                self.shared.next_id.fetch_add(1, Ordering::Relaxed)
            )
        });
        let (reply_tx, reply_rx) = oneshot::channel();
        {
            let mut pending = lock(&self.shared.pending);
            if pending.contains_key(&key) {
                return Err(TunnelError::InvalidMessage(format!(
                    "duplicate correlation key {key}"
                )));
            }
            pending.insert(key.clone(), reply_tx);
        }

        if outbound
            .send(Message::Text(envelope.to_string()))
            .await
            .is_err()
        {
            lock(&self.shared.pending).remove(&key);
            return Err(TunnelError::Disconnected);
        }

        match tokio::time::timeout(self.shared.config.request_timeout, reply_rx).await {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(_)) => Err(TunnelError::Disconnected),
            Err(_) => {
                lock(&self.shared.pending).remove(&key);
                Err(TunnelError::Timeout)
            }
        }
    }
}

/// A running reverse tunnel listener. Dropping it stops accepting connections.
pub struct TunnelServer {
    addr: SocketAddr,
    shared: Arc<TunnelShared>,
    accept_task: JoinHandle<()>,
}

impl TunnelServer {
    /// Bind `0.0.0.0:<port>` and start accepting tunnel connections.
    pub async fn bind(port: u16, expected_token: impl Into<String>) -> Result<Self, TunnelError> {
        Self::bind_with_config(port, TunnelConfig::default(), expected_token).await
    }

    /// Bind with explicit heartbeat/handshake configuration (used by tests).
    pub async fn bind_with_config(
        port: u16,
        config: TunnelConfig,
        expected_token: impl Into<String>,
    ) -> Result<Self, TunnelError> {
        let listener = TcpListener::bind(("0.0.0.0", port))
            .await
            .map_err(|error| TunnelError::Bind(error.to_string()))?;
        let addr = listener
            .local_addr()
            .map_err(|error| TunnelError::Bind(error.to_string()))?;

        let (shutdown, _) = broadcast::channel::<()>(4);
        let shared = Arc::new(TunnelShared {
            expected_token: expected_token.into(),
            config,
            connected: AtomicBool::new(false),
            active: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            shutdown,
        });

        let mut shutdown_rx = shared.shutdown.subscribe();
        let accept_shared = Arc::clone(&shared);
        let accept_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        match accepted {
                            Ok((stream, peer)) => {
                                let state = Arc::clone(&accept_shared);
                                tokio::spawn(async move {
                                    serve_connection(stream, peer, state).await;
                                });
                            }
                            Err(error) => {
                                warn!(error = %error, "tunnel listener accept failed");
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

    /// The concrete address the listener is bound to (ephemeral port friendly).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Address suitable for local clients to connect to this server.
    /// If bound to `0.0.0.0` or `::`, returns loopback (`127.0.0.1` or `::1`) with the bound port.
    pub fn connect_addr(&self) -> SocketAddr {
        if self.addr.ip().is_unspecified() {
            match self.addr {
                SocketAddr::V4(v4) => SocketAddr::new(
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                    v4.port(),
                ),
                SocketAddr::V6(v6) => SocketAddr::new(
                    std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
                    v6.port(),
                ),
            }
        } else {
            self.addr
        }
    }

    /// Cloneable in-process handle.
    pub fn handle(&self) -> TunnelHandle {
        TunnelHandle {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Whether a registered companion is currently connected.
    pub fn is_connected(&self) -> bool {
        self.handle().is_connected()
    }

    /// Stop accepting and tear down the active session.
    pub fn stop(&self) {
        let _ = self.shared.shutdown.send(());
    }
}

impl Drop for TunnelServer {
    fn drop(&mut self) {
        self.stop();
        self.accept_task.abort();
    }
}

/// Upgrade + authenticate + drives one companion connection.
// `accept_hdr_async` fixes the handshake callback error type to tungstenite's
// `ErrorResponse` (`http::Response<Option<String>>`); it cannot be boxed here.
#[expect(
    clippy::result_large_err,
    reason = "handshake callback error type is fixed by tokio-tungstenite"
)]
async fn serve_connection(stream: TcpStream, peer: SocketAddr, shared: Arc<TunnelShared>) {
    let expected_token = shared.expected_token.clone();
    let path = shared.config.path.clone();

    // Header check during the HTTP upgrade gives a real 401 when present.
    let callback = |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        if request.uri().path() != path {
            return Err(http_error(
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "tunnel endpoint not found",
            ));
        }
        if let Some(value) = request.headers().get("Authorization") {
            let valid = value
                .to_str()
                .ok()
                .and_then(|raw| raw.strip_prefix("Bearer "))
                .is_some_and(|token| token.trim() == expected_token);
            if !valid {
                return Err(http_error(
                    StatusCode::UNAUTHORIZED,
                    "AUTH_FAILED",
                    "AUTH_FAILED: token mismatch",
                ));
            }
        }
        Ok(response)
    };

    let websocket = match accept_hdr_async(stream, callback).await {
        Ok(websocket) => websocket,
        Err(error) => {
            debug!(error = %error, "tunnel upgrade rejected");
            return;
        }
    };

    let connection_id = shared.next_id.fetch_add(1, Ordering::Relaxed);
    let identity = peer.to_string();
    let (mut write, mut read) = websocket.split();

    // The companion's first TEXT frame is `{"action":"register","token":...}`.
    let first_frame = match tokio::time::timeout(shared.config.handshake_timeout, read.next()).await
    {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(other))) => {
            debug!(?other, "expected register TEXT frame");
            let _ = write
                .send(Message::Text(error_frame(
                    ErrorCode::AuthFailed,
                    "expected register handshake",
                    None,
                )))
                .await;
            return;
        }
        Ok(Some(Err(error))) => {
            debug!(error = %error, "tunnel read failed during register");
            return;
        }
        Ok(None) | Err(_) => {
            debug!("tunnel closed/timed out before register");
            return;
        }
    };

    let register: Value = match serde_json::from_str(&first_frame) {
        Ok(value) => value,
        Err(error) => {
            debug!(error = %error, "malformed register frame");
            let _ = write
                .send(Message::Text(error_frame(
                    ErrorCode::AuthFailed,
                    "malformed register handshake",
                    None,
                )))
                .await;
            return;
        }
    };
    let token_ok = register["token"].as_str() == Some(expected_token.as_str());
    if register["action"].as_str() != Some("register") || !token_ok {
        let _ = write
            .send(Message::Text(error_frame(
                ErrorCode::AuthFailed,
                "token mismatch",
                None,
            )))
            .await;
        let _ = write.close().await;
        return;
    }

    // Single-flight: one active driver. Losers get BUSY with the owner label.
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(64);
    let busy_owner = {
        let mut active = lock(&shared.active);
        match active.as_ref() {
            Some(existing) => Some(existing.owner.clone()),
            None => {
                *active = Some(ActiveConnection {
                    id: connection_id,
                    owner: identity.clone(),
                    outbound: outbound_tx.clone(),
                });
                shared.connected.store(true, Ordering::SeqCst);
                None
            }
        }
    };
    if let Some(owner) = busy_owner {
        let frame = error_frame(ErrorCode::Busy, "second tunnel connection", Some(&owner));
        let _ = write.send(Message::Text(frame)).await;
        let _ = write.close().await;
        return;
    }

    let mut shutdown_rx = shared.shutdown.subscribe();
    let last_pong = Arc::new(Mutex::new(Instant::now()));
    let mut heartbeat = tokio::time::interval(shared.config.heartbeat_interval);
    heartbeat.tick().await; // consume the immediate first tick
    let pong_timeout = shared.config.pong_timeout;

    let failure: Option<String> = loop {
        tokio::select! {
            _ = shutdown_rx.recv() => break None,
            outgoing = outbound_rx.recv() => {
                match outgoing {
                    Some(message) => {
                        if let Err(error) = write.send(message).await {
                            break Some(error.to_string());
                        }
                    }
                    None => break None,
                }
            }
            _ = heartbeat.tick() => {
                if lock(&last_pong).elapsed() > pong_timeout {
                    break Some("pong timeout".to_string());
                }
                // Mirror the companion: native Ping plus text tunnel.ping.
                let _ = outbound_tx.send(Message::Ping(Vec::new())).await;
                let _ = outbound_tx
                    .send(Message::Text("tunnel.ping".to_string()))
                    .await;
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let trimmed = text.trim();
                        if trimmed == "tunnel.ping" {
                            *lock(&last_pong) = Instant::now();
                            let _ = outbound_tx
                                .send(Message::Text("tunnel.pong".to_string()))
                                .await;
                        } else if trimmed == "tunnel.pong" {
                            *lock(&last_pong) = Instant::now();
                        } else if let Some(key) = correlation_key(&text) {
                            let responder = lock(&shared.pending).remove(&key);
                            if let Some(responder) = responder {
                                let _ = responder.send(text);
                            } else {
                                debug!(key = %key, "unmatched tunnel reply");
                            }
                        } else {
                            // Bare reply: hand it to the sole outstanding request.
                            let mut pending = lock(&shared.pending);
                            if pending.len() == 1 {
                                let key = pending.keys().next().cloned();
                                if let Some(key) = key {
                                    if let Some(responder) = pending.remove(&key) {
                                        let _ = responder.send(text);
                                    }
                                }
                            }
                        }
                    }
                    // tungstenite queues Pong replies to native Pings itself.
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        *lock(&last_pong) = Instant::now();
                    }
                    Some(Ok(Message::Close(_))) | None => break None,
                    Some(Err(error)) => break Some(error.to_string()),
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Frame(_))) => {}
                }
            }
        }
    };

    if let Some(reason) = failure {
        warn!(owner = %identity, reason = %reason, "tunnel session ended");
    }
    release(&shared, connection_id);
    let _ = write.close().await;
    debug!(owner = %identity, "tunnel session torn down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::time::Instant;
    use tokio::net::TcpStream;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

    type Client = WebSocketStream<MaybeTlsStream<TcpStream>>;

    /// Minimal companion stand-in: connect, optionally set the bearer header,
    /// then send the first register TEXT frame.
    async fn connect_companion(
        addr: SocketAddr,
        token: &str,
        header: Option<&str>,
    ) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        let addr = if addr.ip().is_unspecified() {
            SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                addr.port(),
            )
        } else {
            addr
        };
        let url = format!("ws://{addr}{}", crate::pair::DEFAULT_TUNNEL_PATH);
        let mut request = url.into_client_request()?;
        if let Some(bearer) = header {
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {bearer}")
                    .parse()
                    .map_err(|e| format!("bad header: {e}"))?,
            );
        }
        let (mut ws, _) = connect_async(request).await?;
        ws.send(Message::Text(
            json!({"action": "register", "token": token}).to_string(),
        ))
        .await?;
        Ok(ws)
    }

    /// Poll `pred` until true or `deadline` elapses (timeouts are short in tests).
    async fn wait_until(mut pred: impl FnMut() -> bool, deadline: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < deadline {
            if pred() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        pred()
    }

    /// Read the next TEXT frame, skipping heartbeats.
    async fn next_text(ws: &mut Client) -> Option<String> {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(t) = msg {
                let trimmed = t.trim();
                if trimmed != "tunnel.ping" && trimmed != "tunnel.pong" {
                    return Some(t);
                }
            }
        }
        None
    }

    fn fast_config() -> TunnelConfig {
        TunnelConfig {
            heartbeat_interval: Duration::from_millis(30),
            pong_timeout: Duration::from_millis(90),
            handshake_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_secs(2),
            ..TunnelConfig::default()
        }
    }

    // A late teardown from an old session must not evict a newer owner.
    #[test]
    fn stale_release_does_not_disconnect_newer_owner_or_clear_pending() {
        let (shutdown, _) = broadcast::channel::<()>(1);
        let shared = TunnelShared {
            expected_token: "good-token".to_string(),
            config: TunnelConfig::default(),
            connected: AtomicBool::new(true),
            active: Mutex::new(Some(ActiveConnection {
                id: 2,
                owner: "new-owner".to_string(),
                outbound: mpsc::channel::<Message>(1).0,
            })),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(3),
            shutdown,
        };
        let (reply_tx, _reply_rx) = oneshot::channel();
        lock(&shared.pending).insert("seq-1".to_string(), reply_tx);

        release(&shared, 1); // stale id from a previous session

        assert!(shared.connected.load(Ordering::SeqCst));
        assert!(lock(&shared.pending).contains_key("seq-1"));
        assert_eq!(lock(&shared.active).as_ref().map(|conn| conn.id), Some(2));
    }

    // 1. Valid registration marks the server connected and records an owner.
    #[tokio::test]
    async fn valid_registration_marks_connected_and_records_owner() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let _client = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("companion connects");

        assert!(
            wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
            "valid register must mark the tunnel connected"
        );
        assert!(
            server.handle().owner().is_some(),
            "an owner identity must be recorded"
        );
    }

    // 2. Mismatched first-frame token is refused with AUTH_FAILED.
    #[tokio::test]
    async fn mismatched_token_is_refused_with_auth_failed() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let mut client = connect_companion(server.local_addr(), "wrong-token", None)
            .await
            .expect("WS upgrade succeeds; rejection is post-upgrade");

        let reply = next_text(&mut client).await.expect("AUTH_FAILED frame");
        let value: Value = serde_json::from_str(&reply).expect("JSON error frame");
        assert_eq!(value["error"]["code"], "AUTH_FAILED");
        assert!(
            !server.is_connected(),
            "no tunnel established on auth failure"
        );
    }

    // 2b. Mismatched Authorization header is refused during the handshake (401).
    #[tokio::test]
    async fn mismatched_authorization_header_is_refused_during_handshake() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let url = format!(
            "ws://{}{}",
            server.connect_addr(),
            crate::pair::DEFAULT_TUNNEL_PATH
        );
        let mut request = url.into_client_request().expect("request");
        request.headers_mut().insert(
            "Authorization",
            "Bearer wrong-token".parse().expect("header"),
        );
        let result = connect_async(request).await;

        let err = result.expect_err("handshake must be rejected");
        let rendered = format!("{err:?}");
        assert!(
            rendered.contains("401") || rendered.contains("AUTH_FAILED"),
            "401/AUTH_FAILED expected, got {rendered}"
        );
        assert!(!server.is_connected());
    }

    // 3. Heartbeat timeout disconnects an unresponsive companion.
    #[tokio::test]
    async fn heartbeat_timeout_disconnects_unresponsive_companion() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let _client = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("companion connects");
        assert!(
            wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
            "register first"
        );

        assert!(
            wait_until(|| !server.is_connected(), Duration::from_secs(2)).await,
            "pong timeout must disconnect the companion"
        );
    }

    // 4. Clean teardown releases the single-flight lock.
    #[tokio::test]
    async fn clean_teardown_releases_single_flight_lock() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        {
            let first = connect_companion(server.local_addr(), "good-token", Some("good-token"))
                .await
                .expect("first companion connects");
            assert!(
                wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
                "first register"
            );
            drop(first);
            assert!(
                wait_until(|| !server.is_connected(), Duration::from_secs(2)).await,
                "teardown clears connected flag"
            );
        }

        let _second = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("second companion connects");
        assert!(
            wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
            "released lock must let a later companion register"
        );
    }

    // 5. Second concurrent connection is refused with BUSY + owner identity.
    #[tokio::test]
    async fn second_connection_is_refused_with_busy_and_owner() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let _owner = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("owner connects");
        assert!(
            wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
            "owner register"
        );
        let owner = server.handle().owner().expect("owner identity");

        let mut loser = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("loser WS upgrade succeeds");
        let reply = next_text(&mut loser).await.expect("BUSY frame");
        let value: Value = serde_json::from_str(&reply).expect("JSON error frame");
        assert_eq!(value["error"]["code"], "BUSY");
        assert_eq!(value["error"]["owner"], owner);
        assert!(server.is_connected(), "owner stays connected");
    }

    // 6. The #61 seam: an envelope with `seq` is replied to correlation-wise.
    #[tokio::test]
    async fn request_correlates_reply_by_seq() {
        let server = TunnelServer::bind_with_config(0, fast_config(), "good-token")
            .await
            .expect("bind ephemeral port");
        let mut client = connect_companion(server.local_addr(), "good-token", None)
            .await
            .expect("companion connects");
        assert!(
            wait_until(|| server.is_connected(), Duration::from_secs(2)).await,
            "register first"
        );

        let handle = server.handle();
        let request = json!({"v": 1, "seq": 7, "payload": {"type": "rpc"}}).to_string();
        let pending = tokio::spawn(async move { handle.request(&request).await });

        // Companion receives the envelope and replies on the same tunnel.
        let envelope = next_text(&mut client).await.expect("envelope from server");
        assert_eq!(serde_json::from_str::<Value>(&envelope).unwrap()["seq"], 7);
        client
            .send(Message::Text(
                json!({"v": 1, "seq": 7, "payload": "ok"}).to_string(),
            ))
            .await
            .expect("reply sent");

        let reply = pending
            .await
            .expect("request task")
            .expect("correlated reply");
        assert_eq!(
            serde_json::from_str::<Value>(&reply).unwrap()["payload"],
            "ok"
        );
    }
}
