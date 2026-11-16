use crate::proxy::ProxyRouter;
use crate::tunnel::{ReverseTunnel, TunnelConfig, TunnelHandle, TunnelState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::info;

/// Pairing configuration received from frontend (e.g. via QR code).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingConfig {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub token: String,
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl PairingConfig {
    pub fn to_ws_url(&self) -> String {
        if let Some(ref u) = self.url {
            if let Ok(parsed) = url::Url::parse(u) {
                if parsed.scheme() == "ws" || parsed.scheme() == "wss" {
                    return u.clone();
                }
            }
        }
        let scheme = if self.scheme == "https" || self.scheme == "wss" {
            "wss"
        } else {
            "ws"
        };
        let host = if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let path = if self.path.starts_with('/') {
            self.path.clone()
        } else {
            format!("/{}", self.path)
        };
        format!("{}://{}:{}{}", scheme, host, self.port, path)
    }
}

/// Status payload returned by `get_tunnel_status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TunnelStatusResponse {
    pub state: TunnelState,
    pub uptime_secs: u64,
    pub latency_ms: u32,
    pub connected: bool,
}

/// Status payload returned by `get_target_status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetStatusResponse {
    pub responsive: bool,
    pub url: String,
}

/// Streaming event forwarded to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendLogEvent {
    pub id: String,
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub source: String,
}

pub type EventSink = Arc<dyn Fn(&str, Value) + Send + Sync>;

/// Shared backend application state.
pub struct AppState {
    pub tunnel_handle: Mutex<Option<TunnelHandle>>,
    pub proxy_router: Arc<ProxyRouter>,
    pub started_at: Mutex<Option<Instant>>,
    pub last_pairing: Mutex<Option<PairingConfig>>,
    pub event_sink: Arc<std::sync::Mutex<Option<EventSink>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tunnel_handle: Mutex::new(None),
            proxy_router: Arc::new(ProxyRouter::default()),
            started_at: Mutex::new(None),
            last_pairing: Mutex::new(None),
            event_sink: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn with_proxy_router(proxy_router: ProxyRouter) -> Self {
        Self {
            tunnel_handle: Mutex::new(None),
            proxy_router: Arc::new(proxy_router),
            started_at: Mutex::new(None),
            last_pairing: Mutex::new(None),
            event_sink: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn emit_event(&self, event: &str, payload: Value) {
        let sink = {
            let guard = self.event_sink.lock().unwrap_or_else(|e| e.into_inner());
            guard.clone()
        };
        if let Some(ref emit_fn) = sink {
            emit_fn(event, payload);
        }
    }

    pub fn set_event_sink(&self, sink: impl Fn(&str, Value) + Send + Sync + 'static) {
        let mut guard = self.event_sink.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some(Arc::new(sink));
    }
}

/// Command: start reverse WebSocket tunnel with pairing config.
pub async fn start_tunnel_impl(
    state: &AppState,
    config: PairingConfig,
) -> Result<TunnelStatusResponse, String> {
    info!(host = %config.host, port = config.port, "Starting reverse tunnel from IPC command");
    let ws_url = config.to_ws_url();

    // Stop existing tunnel if already running (awaiting completion)
    let mut handle_guard = state.tunnel_handle.lock().await;
    if let Some(existing) = handle_guard.take() {
        existing.stop().await;
    }

    let tunnel_config = TunnelConfig {
        url: ws_url.clone(),
        token: config.token.clone(),
        heartbeat_interval_secs: 15,
        backoff_delays_secs: vec![1, 3, 10],
    };

    let router = Arc::new(
        state
            .proxy_router
            .as_ref()
            .clone()
            .with_auth_token(&config.token),
    );
    let sink_holder = state.event_sink.clone();
    let url_clone = ws_url.clone();

    let tunnel = ReverseTunnel::new(tunnel_config)
        .with_proxy_router(router)
        .with_state_listener(move |st| {
            let sink = {
                let guard = sink_holder.lock().unwrap_or_else(|e| e.into_inner());
                guard.clone()
            };
            if let Some(ref emit_fn) = sink {
                emit_fn(
                    "tunnel_state_changed",
                    json!({
                        "state": st.to_string(),
                        "url": url_clone,
                    }),
                );
            }
        });
    let handle = tunnel.start();

    let initial_state = handle.current_state();
    *handle_guard = Some(handle);

    let mut started_guard = state.started_at.lock().await;
    *started_guard = Some(Instant::now());

    let mut pairing_guard = state.last_pairing.lock().await;
    *pairing_guard = Some(config);

    // Emit initial event to frontend
    state.emit_event(
        "tunnel_state_changed",
        json!({
            "state": initial_state.to_string(),
            "url": ws_url
        }),
    );

    Ok(TunnelStatusResponse {
        state: initial_state,
        uptime_secs: 0,
        latency_ms: 0,
        connected: initial_state == TunnelState::Connected,
    })
}

/// Command: stop active tunnel.
pub async fn stop_tunnel_impl(state: &AppState) -> Result<TunnelStatusResponse, String> {
    info!("Stopping reverse tunnel from IPC command");
    let mut handle_guard = state.tunnel_handle.lock().await;
    if let Some(handle) = handle_guard.take() {
        handle.stop().await;
    }

    let mut started_guard = state.started_at.lock().await;
    *started_guard = None;

    state.emit_event(
        "tunnel_state_changed",
        json!({
            "state": TunnelState::Disconnected.to_string()
        }),
    );

    Ok(TunnelStatusResponse {
        state: TunnelState::Disconnected,
        uptime_secs: 0,
        latency_ms: 0,
        connected: false,
    })
}

/// Command: check current tunnel status.
pub async fn get_tunnel_status_impl(state: &AppState) -> Result<TunnelStatusResponse, String> {
    let handle_guard = state.tunnel_handle.lock().await;
    let (current_state, connected) = if let Some(ref handle) = *handle_guard {
        let st = handle.current_state();
        (st, st == TunnelState::Connected)
    } else {
        (TunnelState::Disconnected, false)
    };

    let started_guard = state.started_at.lock().await;
    let uptime_secs = if let Some(started) = *started_guard {
        if connected {
            started.elapsed().as_secs()
        } else {
            0
        }
    } else {
        0
    };

    let latency_ms = if connected {
        handle_guard.as_ref().map(|h| h.latency_ms()).unwrap_or(0)
    } else {
        0
    };

    Ok(TunnelStatusResponse {
        state: current_state,
        uptime_secs,
        latency_ms,
        connected,
    })
}

/// Command: check target bridge status at 127.0.0.1:8765.
pub async fn get_target_status_impl(state: &AppState) -> Result<TargetStatusResponse, String> {
    let responsive = state.proxy_router.check_target_status().await;
    Ok(TargetStatusResponse {
        responsive,
        url: "http://127.0.0.1:8765".to_string(),
    })
}

// -----------------------------------------------------------------------------
// Tauri IPC command handlers (when `tauri-app` feature is enabled)
// -----------------------------------------------------------------------------

#[cfg(feature = "tauri-app")]
#[tauri::command]
pub async fn start_tunnel(
    state: tauri::State<'_, AppState>,
    config: PairingConfig,
) -> Result<TunnelStatusResponse, String> {
    start_tunnel_impl(&state, config).await
}

#[cfg(feature = "tauri-app")]
#[tauri::command]
pub async fn stop_tunnel(
    state: tauri::State<'_, AppState>,
) -> Result<TunnelStatusResponse, String> {
    stop_tunnel_impl(&state).await
}

#[cfg(feature = "tauri-app")]
#[tauri::command]
pub async fn get_tunnel_status(
    state: tauri::State<'_, AppState>,
) -> Result<TunnelStatusResponse, String> {
    get_tunnel_status_impl(&state).await
}

#[cfg(feature = "tauri-app")]
#[tauri::command]
pub async fn get_target_status(
    state: tauri::State<'_, AppState>,
) -> Result<TargetStatusResponse, String> {
    get_target_status_impl(&state).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_commands_lifecycle() {
        // Start mock WS server to accept tunnel
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            use futures_util::StreamExt;
            if let Ok((stream, _)) = listener.accept().await {
                if let Ok(mut ws_stream) = tokio_tungstenite::accept_async(stream).await {
                    while let Some(Ok(msg)) = ws_stream.next().await {
                        if msg.is_close() {
                            break;
                        }
                    }
                }
            }
        });

        let closed_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_port = closed_listener.local_addr().unwrap().port();
        drop(closed_listener);

        let state = AppState::with_proxy_router(ProxyRouter::new(format!(
            "http://127.0.0.1:{}",
            closed_port
        )));

        let event_counter = Arc::new(AtomicUsize::new(0));
        let ec_clone = event_counter.clone();
        state.set_event_sink(move |_event, _payload| {
            ec_clone.fetch_add(1, Ordering::SeqCst);
        });

        let config = PairingConfig {
            scheme: "ws".to_string(),
            host: "127.0.0.1".to_string(),
            port,
            path: "/tunnel".to_string(),
            token: "pairing-tok-123".to_string(),
            app: Some("texure".to_string()),
            url: None,
        };

        // 1. start_tunnel
        let start_res = start_tunnel_impl(&state, config)
            .await
            .expect("start tunnel");
        assert!(!start_res.connected); // initial state connecting

        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let st = get_tunnel_status_impl(&state).await.expect("tunnel status");
            if st.connected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // 2. get_tunnel_status
        let status = get_tunnel_status_impl(&state).await.expect("tunnel status");
        assert_eq!(status.state, TunnelState::Connected);
        assert!(status.connected);

        // 3. get_target_status (target bridge is not running on 8765 in test)
        let target_status = get_target_status_impl(&state).await.expect("target status");
        assert!(!target_status.responsive);
        assert_eq!(target_status.url, "http://127.0.0.1:8765");

        // 4. stop_tunnel
        let stop_res = stop_tunnel_impl(&state).await.expect("stop tunnel");
        assert_eq!(stop_res.state, TunnelState::Disconnected);
        assert!(!stop_res.connected);

        let status_after_stop = get_tunnel_status_impl(&state)
            .await
            .expect("tunnel status after stop");
        assert_eq!(status_after_stop.state, TunnelState::Disconnected);
        assert!(!status_after_stop.connected);

        // Verify events were emitted
        assert!(event_counter.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn test_pairing_config_url_normalization() {
        // 1. IPv6 host gets bracketed
        let ipv6_cfg = PairingConfig {
            scheme: "ws".to_string(),
            host: "::1".to_string(),
            port: 8080,
            path: "tunnel".to_string(),
            token: "tok".to_string(),
            app: None,
            url: None,
        };
        assert_eq!(ipv6_cfg.to_ws_url(), "ws://[::1]:8080/tunnel");

        // 2. HTTPS maps to WSS
        let https_cfg = PairingConfig {
            scheme: "https".to_string(),
            host: "gateway.example.com".to_string(),
            port: 443,
            path: "/tunnel".to_string(),
            token: "tok".to_string(),
            app: None,
            url: None,
        };
        assert_eq!(
            https_cfg.to_ws_url(),
            "wss://gateway.example.com:443/tunnel"
        );

        // 3. Valid WSS override is preserved
        let override_cfg = PairingConfig {
            scheme: "ws".to_string(),
            host: "desktop.local".to_string(),
            port: 8080,
            path: "/tunnel".to_string(),
            token: "tok".to_string(),
            app: None,
            url: Some("wss://secure.example.com:9000/custom".to_string()),
        };
        assert_eq!(
            override_cfg.to_ws_url(),
            "wss://secure.example.com:9000/custom"
        );

        // 4. Invalid scheme override is rejected and falls back to safe scheme
        let bad_override = PairingConfig {
            scheme: "wss".to_string(),
            host: "desktop.local".to_string(),
            port: 8080,
            path: "/tunnel".to_string(),
            token: "tok".to_string(),
            app: None,
            url: Some("javascript:alert(1)".to_string()),
        };
        assert_eq!(bad_override.to_ws_url(), "wss://desktop.local:8080/tunnel");
    }
}
