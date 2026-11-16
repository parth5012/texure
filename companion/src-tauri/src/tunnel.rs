use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// Connection state of the reverse WebSocket tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TunnelState {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

impl std::fmt::Display for TunnelState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelState::Connected => write!(f, "CONNECTED"),
            TunnelState::Connecting => write!(f, "CONNECTING"),
            TunnelState::Disconnected => write!(f, "DISCONNECTED"),
            TunnelState::Error => write!(f, "ERROR"),
        }
    }
}

/// Initial registration handshake sent upon WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterHandshake {
    pub action: String,
    pub token: String,
}

impl RegisterHandshake {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            action: "register".to_string(),
            token: token.into(),
        }
    }
}

/// Configuration for the reverse WebSocket tunnel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub url: String,
    pub token: String,
    #[serde(default = "default_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_backoff_delays")]
    pub backoff_delays_secs: Vec<u64>,
}

fn default_heartbeat_interval_secs() -> u64 {
    15
}

fn default_backoff_delays() -> Vec<u64> {
    vec![1, 3, 10]
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            url: "ws://127.0.0.1:8766/tunnel".to_string(),
            token: String::new(),
            heartbeat_interval_secs: default_heartbeat_interval_secs(),
            backoff_delays_secs: default_backoff_delays(),
        }
    }
}

pub type MessageHandler = Arc<dyn Fn(String) -> Option<String> + Send + Sync>;
pub type StateListener = Arc<dyn Fn(TunnelState) + Send + Sync>;

/// Handle to control the running tunnel supervisor.
pub struct TunnelHandle {
    stop_tx: watch::Sender<bool>,
    state_rx: watch::Receiver<TunnelState>,
    running: Arc<AtomicBool>,
    join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    latency_ms: Arc<std::sync::atomic::AtomicU32>,
}

impl TunnelHandle {
    /// Request graceful shutdown of the tunnel and await task completion (with bounded timeout).
    pub async fn stop(&self) {
        let _ = self.stop_tx.send(true);
        let mut guard = self.join_handle.lock().await;
        if let Some(mut handle) = guard.take() {
            if tokio::time::timeout(Duration::from_secs(5), &mut handle)
                .await
                .is_err()
            {
                handle.abort();
            }
        }
    }

    /// Check current connection state.
    pub fn current_state(&self) -> TunnelState {
        *self.state_rx.borrow()
    }

    /// Obtain a receiver to watch state changes.
    pub fn watch_state(&self) -> watch::Receiver<TunnelState> {
        self.state_rx.clone()
    }

    /// Whether supervisor is actively running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Measured heartbeat round-trip latency in milliseconds.
    pub fn latency_ms(&self) -> u32 {
        self.latency_ms.load(Ordering::Relaxed)
    }
}

/// Outbound Reverse WebSocket Tunnel Client.
pub struct ReverseTunnel {
    config: TunnelConfig,
    state_tx: watch::Sender<TunnelState>,
    state_rx: watch::Receiver<TunnelState>,
    state_listener: Option<StateListener>,
    message_handler: Option<MessageHandler>,
    proxy_router: Option<Arc<crate::proxy::ProxyRouter>>,
}

impl ReverseTunnel {
    pub fn new(config: TunnelConfig) -> Self {
        let (state_tx, state_rx) = watch::channel(TunnelState::Disconnected);
        Self {
            config,
            state_tx,
            state_rx,
            state_listener: None,
            message_handler: None,
            proxy_router: None,
        }
    }

    pub fn with_state_listener(
        mut self,
        listener: impl Fn(TunnelState) + Send + Sync + 'static,
    ) -> Self {
        self.state_listener = Some(Arc::new(listener));
        self
    }

    pub fn with_message_handler(
        mut self,
        handler: impl Fn(String) -> Option<String> + Send + Sync + 'static,
    ) -> Self {
        self.message_handler = Some(Arc::new(handler));
        self
    }

    pub fn with_proxy_router(mut self, proxy_router: Arc<crate::proxy::ProxyRouter>) -> Self {
        self.proxy_router = Some(proxy_router);
        self
    }

    fn notify_state(&self, state: TunnelState) {
        let _ = self.state_tx.send(state);
        if let Some(ref listener) = self.state_listener {
            listener(state);
        }
    }

    /// Launch supervisor background loop and return control handle.
    pub fn start(self) -> TunnelHandle {
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let state_rx = self.state_rx.clone();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let latency_ms = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let latency_clone = latency_ms.clone();

        let join_handle = tokio::spawn(async move {
            let backoff_schedule = if self.config.backoff_delays_secs.is_empty() {
                vec![1, 3, 10]
            } else {
                self.config.backoff_delays_secs.clone()
            };

            let mut backoff_idx = 0;

            while !*stop_rx.borrow() {
                self.notify_state(TunnelState::Connecting);
                info!(url = %self.config.url, "Attempting reverse WebSocket connection");

                let connect_fut = async {
                    let mut request = match self.config.url.as_str().into_client_request() {
                        Ok(r) => r,
                        Err(e) => {
                            return Err(tokio_tungstenite::tungstenite::Error::Io(
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    e.to_string(),
                                ),
                            ));
                        }
                    };
                    if !self.config.token.is_empty() {
                        match format!("Bearer {}", self.config.token).parse() {
                            Ok(val) => {
                                request.headers_mut().insert("Authorization", val);
                            }
                            Err(e) => {
                                warn!(error = %e, "Failed to construct Authorization header for tunnel dial");
                            }
                        }
                    }
                    tokio_tungstenite::connect_async(request).await
                };

                let connect_timed = tokio::time::timeout(Duration::from_secs(10), connect_fut);

                let connect_result = tokio::select! {
                    res = stop_rx.changed() => {
                        if res.is_err() || *stop_rx.borrow() {
                            break;
                        }
                        continue;
                    }
                    res = connect_timed => res,
                };

                match connect_result {
                    Ok(Ok((ws_stream, _response))) => {
                        info!("Reverse WebSocket connection established, sending handshake");
                        self.notify_state(TunnelState::Connected);
                        backoff_idx = 0; // Reset backoff on successful connect

                        let (mut write, mut read) = ws_stream.split();
                        let (outbound_tx, mut outbound_rx) =
                            tokio::sync::mpsc::channel::<Message>(64);

                        // 1. Send register handshake
                        let handshake = RegisterHandshake::new(&self.config.token);
                        let handshake_json = serde_json::to_string(&handshake)
                            .unwrap_or_else(|_| "{\"action\":\"register\"}".to_string());

                        if let Err(e) = write.send(Message::Text(handshake_json)).await {
                            error!(error = %e, "Failed to send register handshake");
                            self.notify_state(TunnelState::Error);
                        } else {
                            // 2. Run session loop with heartbeat ping/pong & pong deadline
                            let mut ping_interval = tokio::time::interval(Duration::from_secs(
                                self.config.heartbeat_interval_secs.max(1),
                            ));
                            ping_interval.tick().await; // Initial tick
                            let mut last_pong = std::time::Instant::now();
                            let mut last_ping_sent: Option<std::time::Instant> = None;
                            let pong_timeout =
                                Duration::from_secs(self.config.heartbeat_interval_secs.max(1) * 2)
                                    .max(Duration::from_secs(10));

                            let mut session_active = true;

                            while session_active && !*stop_rx.borrow() {
                                tokio::select! {
                                    res = stop_rx.changed() => {
                                        if res.is_err() || *stop_rx.borrow() {
                                            info!("Stop signal received, closing WebSocket tunnel");
                                            let _ = write.send(Message::Close(None)).await;
                                            session_active = false;
                                        }
                                    }
                                    Some(out_msg) = outbound_rx.recv() => {
                                        if let Err(e) = write.send(out_msg).await {
                                            warn!(error = %e, "Failed to write outbound WebSocket message");
                                            session_active = false;
                                        }
                                    }
                                    _ = ping_interval.tick() => {
                                        if last_pong.elapsed() > pong_timeout {
                                            warn!("Heartbeat pong timeout exceeded; terminating tunnel to reconnect");
                                            session_active = false;
                                        } else {
                                            debug!("Sending heartbeat ping");
                                            last_ping_sent = Some(std::time::Instant::now());
                                            if let Err(e) = write.send(Message::Ping(vec![])).await {
                                                warn!(error = %e, "Failed to send ping heartbeat");
                                                session_active = false;
                                            }
                                        }
                                    }
                                    msg = read.next() => {
                                        match msg {
                                            Some(Ok(Message::Text(txt))) => {
                                                let trimmed = txt.trim();
                                                if trimmed == "tunnel.ping" {
                                                    debug!("Received text-framed heartbeat ping; replying with tunnel.pong");
                                                    last_pong = std::time::Instant::now();
                                                    let _ = outbound_tx.send(Message::Text("tunnel.pong".to_string())).await;
                                                } else if trimmed == "tunnel.pong" {
                                                    debug!("Received text-framed heartbeat pong ACK");
                                                    last_pong = std::time::Instant::now();
                                                    if let Some(sent) = last_ping_sent.take() {
                                                        let rtt = sent.elapsed().as_millis() as u32;
                                                        latency_clone.store(rtt, Ordering::Relaxed);
                                                    }
                                                } else {
                                                    debug!(len = txt.len(), "Received RPC message over tunnel");
                                                    let router = self.proxy_router.clone();
                                                    let handler = self.message_handler.clone();
                                                    let tx = outbound_tx.clone();
                                                    tokio::spawn(async move {
                                                        if let Some(ref r) = router {
                                                            if let Some(reply) = r.handle_message(&txt).await {
                                                                let _ = tx.send(Message::Text(reply)).await;
                                                            }
                                                        } else if let Some(ref h) = handler {
                                                            if let Some(reply) = h(txt) {
                                                                let _ = tx.send(Message::Text(reply)).await;
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                            Some(Ok(Message::Ping(payload))) => {
                                                debug!("Received ping, replying with pong");
                                                let _ = outbound_tx.send(Message::Pong(payload)).await;
                                            }
                                            Some(Ok(Message::Pong(_))) => {
                                                debug!("Received pong heartbeat ACK");
                                                last_pong = std::time::Instant::now();
                                                if let Some(sent) = last_ping_sent.take() {
                                                    let rtt = sent.elapsed().as_millis() as u32;
                                                    latency_clone.store(rtt, Ordering::Relaxed);
                                                }
                                            }
                                            Some(Ok(Message::Close(reason))) => {
                                                info!(?reason, "Server closed WebSocket connection");
                                                session_active = false;
                                            }
                                            Some(Err(e)) => {
                                                warn!(error = %e, "WebSocket stream error");
                                                session_active = false;
                                            }
                                            None => {
                                                info!("WebSocket stream EOF reached");
                                                session_active = false;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(error = %e, "Failed to connect reverse WebSocket tunnel");
                        self.notify_state(TunnelState::Error);
                    }
                    Err(_) => {
                        warn!("Reverse WebSocket connection attempt timed out");
                        self.notify_state(TunnelState::Error);
                    }
                }

                if *stop_rx.borrow() {
                    break;
                }

                // Disconnected from server
                self.notify_state(TunnelState::Disconnected);

                // Compute exponential backoff delay
                let delay_secs = backoff_schedule
                    .get(backoff_idx)
                    .copied()
                    .unwrap_or(*backoff_schedule.last().unwrap_or(&10));

                if backoff_idx + 1 < backoff_schedule.len() {
                    backoff_idx += 1;
                }

                info!(delay_secs, "Backing off before reconnect attempt");
                tokio::select! {
                    res = stop_rx.changed() => {
                        if res.is_err() || *stop_rx.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(delay_secs)) => {}
                }
            }

            self.notify_state(TunnelState::Disconnected);
            running_clone.store(false, Ordering::SeqCst);
            info!("Reverse WebSocket tunnel supervisor stopped");
        });

        TunnelHandle {
            stop_tx,
            state_rx,
            running,
            join_handle: Arc::new(tokio::sync::Mutex::new(Some(join_handle))),
            latency_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn test_handshake_and_ping_pong() {
        // Start mock WS server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_url = format!("ws://127.0.0.1:{}", port);

        let received_handshake = Arc::new(Mutex::new(None));
        let received_handshake_clone = received_handshake.clone();
        let ping_count = Arc::new(AtomicUsize::new(0));
        let ping_count_clone = ping_count.clone();

        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
                    while let Some(msg) = ws_stream.next().await {
                        match msg {
                            Ok(Message::Text(txt)) => {
                                let mut slot = received_handshake_clone.lock().await;
                                *slot = Some(txt);
                            }
                            Ok(Message::Ping(_)) => {
                                ping_count_clone.fetch_add(1, Ordering::SeqCst);
                                let _ = ws_stream.send(Message::Pong(vec![])).await;
                            }
                            Ok(Message::Close(_)) => break,
                            _ => {}
                        }
                    }
                }
            }
        });

        // Start client tunnel with 1s heartbeat
        let config = TunnelConfig {
            url: server_url,
            token: "test-auth-token-42".to_string(),
            heartbeat_interval_secs: 1,
            backoff_delays_secs: vec![1, 2],
        };

        let tunnel = ReverseTunnel::new(config);
        let handle = tunnel.start();

        // Wait for connect and handshake
        let start = std::time::Instant::now();
        while handle.current_state() != TunnelState::Connected
            && start.elapsed() < Duration::from_secs(5)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(handle.current_state(), TunnelState::Connected);

        while received_handshake.lock().await.is_none() && start.elapsed() < Duration::from_secs(5)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let handshake_val = received_handshake
            .lock()
            .await
            .clone()
            .expect("handshake received");
        let parsed: RegisterHandshake =
            serde_json::from_str(&handshake_val).expect("valid handshake json");
        assert_eq!(parsed.action, "register");
        assert_eq!(parsed.token, "test-auth-token-42");

        // Wait for at least one ping
        while ping_count.load(Ordering::SeqCst) < 1 && start.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(ping_count.load(Ordering::SeqCst) >= 1);

        // Stop tunnel cleanly
        handle.stop().await;
        let stop_start = std::time::Instant::now();
        while (handle.is_running() || handle.current_state() != TunnelState::Disconnected)
            && stop_start.elapsed() < Duration::from_secs(5)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(handle.current_state(), TunnelState::Disconnected);
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn test_reconnect_backoff_schedule() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_url = format!("ws://127.0.0.1:{}", port);
        drop(listener); // Immediately drop so connections fail

        let state_transitions = Arc::new(std::sync::Mutex::new(Vec::new()));
        let st_clone = state_transitions.clone();

        let config = TunnelConfig {
            url: server_url,
            token: "tok".to_string(),
            heartbeat_interval_secs: 5,
            backoff_delays_secs: vec![1, 2],
        };

        let tunnel = ReverseTunnel::new(config).with_state_listener(move |st| {
            let mut list = st_clone.lock().unwrap();
            list.push(st);
        });

        let handle = tunnel.start();

        // Dynamically wait for failure transitions (Connecting -> Error -> Disconnected)
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let list = state_transitions.lock().unwrap().clone();
            if list.contains(&TunnelState::Connecting)
                && list.contains(&TunnelState::Error)
                && list.contains(&TunnelState::Disconnected)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let list = state_transitions.lock().unwrap().clone();
        // Should have transitioned through Connecting -> Error -> Disconnected
        assert!(list.contains(&TunnelState::Connecting));
        assert!(list.contains(&TunnelState::Error));
        assert!(list.contains(&TunnelState::Disconnected));

        handle.stop().await;
        let stop_start = std::time::Instant::now();
        while (handle.is_running() || handle.current_state() != TunnelState::Disconnected)
            && stop_start.elapsed() < Duration::from_secs(5)
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(handle.current_state(), TunnelState::Disconnected);
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn test_drop_handle_terminates_supervisor() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let config = TunnelConfig {
            url: format!("ws://127.0.0.1:{}", port),
            token: "tok".to_string(),
            heartbeat_interval_secs: 5,
            backoff_delays_secs: vec![1, 2],
        };

        let tunnel = ReverseTunnel::new(config);
        let handle = tunnel.start();
        let running_clone = handle.running.clone();
        assert!(running_clone.load(Ordering::SeqCst));

        // Dropping the handle drops stop_tx, which must terminate the supervisor
        drop(handle);

        let start = std::time::Instant::now();
        while running_clone.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(5) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(!running_clone.load(Ordering::SeqCst));
    }
}
