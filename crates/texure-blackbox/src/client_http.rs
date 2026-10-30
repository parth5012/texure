use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HttpBridgeError {
    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Bridge returned error status {status}: {body}")]
    StatusError { status: u16, body: String },
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("WebSocket error: {0}")]
    WsError(String),
    #[error("Connection timeout")]
    Timeout,
}

pub struct HttpBridgeClient {
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl HttpBridgeClient {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            client,
        }
    }

    fn headers(&self) -> Result<HeaderMap, HttpBridgeError> {
        let mut headers = HeaderMap::new();
        if let Some(token) = &self.token {
            let val = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| HttpBridgeError::InvalidHeader(format!("Invalid auth token header: {e}")))?;
            headers.insert(AUTHORIZATION, val);
        }
        Ok(headers)
    }

    pub async fn get_ui_tree(&self, max_depth: Option<u32>) -> Result<Value, HttpBridgeError> {
        let mut url = format!("{}/ui-tree", self.base_url);
        if let Some(depth) = max_depth {
            url.push_str(&format!("?max_depth={depth}"));
        }

        let resp = self
            .client
            .get(&url)
            .headers(self.headers()?)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn tap(&self, node: u64, rev: u64) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/tap", self.base_url);
        let payload = serde_json::json!({
            "node": node,
            "rev": rev
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn type_text(&self, node: u64, text: &str, rev: u64) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/type", self.base_url);
        let payload = serde_json::json!({
            "node": node,
            "text": text,
            "rev": rev
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn keypress(&self, key: &str) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/keypress", self.base_url);
        let payload = serde_json::json!({
            "key": key
        });

        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&payload)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn back(&self) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/back", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .send()
            .await?;

        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn screenshot(&self, scale: Option<f32>) -> Result<Vec<u8>, HttpBridgeError> {
        let mut url = format!("{}/screenshot", self.base_url);
        if let Some(s) = scale {
            url.push_str(&format!("?scale={s}"));
        }

        let resp = self
            .client
            .get(&url)
            .headers(self.headers()?)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status != 200 {
            let body = resp.text().await.unwrap_or_default();
            return Err(HttpBridgeError::StatusError { status, body });
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }
}
