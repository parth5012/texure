use futures_util::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use texure_contract::bridge::TreeChangedEvent;
use texure_contract::config::ResolvedDeviceConfig;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// Supervisor managing background WebSocket connection to the Android Bridge.
#[derive(Clone)]
pub struct WsSupervisor {
    connected: Arc<AtomicBool>,
    sender: broadcast::Sender<TreeChangedEvent>,
    config: ResolvedDeviceConfig,
}

impl WsSupervisor {
    pub fn new(config: ResolvedDeviceConfig) -> Self {
        let (sender, _) = broadcast::channel(64);
        Self {
            connected: Arc::new(AtomicBool::new(false)),
            sender,
            config,
        }
    }

    /// Whether the background WebSocket connection is currently active.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Subscribe to incoming tree_changed events.
    pub fn subscribe(&self) -> broadcast::Receiver<TreeChangedEvent> {
        self.sender.subscribe()
    }

    /// Spawn the background supervisor loop.
    pub fn spawn(&self) -> tokio::task::JoinHandle<()> {
        let connected = Arc::clone(&self.connected);
        let sender = self.sender.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut backoff = Duration::from_secs(1);
            let ws_url = format!(
                "{}/ws",
                config
                    .normalized_url()
                    .replace("http://", "ws://")
                    .replace("https://", "wss://")
            );

            loop {
                debug!("Attempting WebSocket dial to {ws_url}...");
                let mut req = match ws_url.clone().into_client_request() {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to create WS request: {e}");
                        tokio::time::sleep(backoff).await;
                        continue;
                    }
                };

                let auth_header = format!("Bearer {}", config.token);
                if let Ok(val) = auth_header.parse() {
                    req.headers_mut().insert("Authorization", val);
                }

                match connect_async(req).await {
                    Ok((ws_stream, _)) => {
                        info!("Connected to bridge WebSocket at {ws_url}");
                        connected.store(true, Ordering::SeqCst);
                        backoff = Duration::from_secs(1);

                        let (_, mut read) = ws_stream.split();
                        while let Some(msg_result) = read.next().await {
                            match msg_result {
                                Ok(Message::Text(text)) => {
                                    if let Ok(event) =
                                        serde_json::from_str::<TreeChangedEvent>(&text)
                                    {
                                        let _ = sender.send(event);
                                    }
                                }
                                Ok(Message::Close(_)) => break,
                                Err(e) => {
                                    warn!("WebSocket read error: {e}");
                                    break;
                                }
                                _ => {}
                            }
                        }

                        connected.store(false, Ordering::SeqCst);
                        warn!("Bridge WebSocket disconnected; beginning reconnection loop");
                    }
                    Err(e) => {
                        connected.store(false, Ordering::SeqCst);
                        debug!("WebSocket connection attempt failed: {e}");
                    }
                }

                // Exponential backoff: 1s -> 5s -> 15s (cap at 15s)
                tokio::time::sleep(backoff).await;
                backoff = match backoff.as_secs() {
                    1 => Duration::from_secs(5),
                    5 => Duration::from_secs(15),
                    _ => Duration::from_secs(15),
                };
            }
        })
    }
}
