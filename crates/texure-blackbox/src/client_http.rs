use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::{json, Value};
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
            let val = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|e| {
                HttpBridgeError::InvalidHeader(format!("Invalid auth token header: {e}"))
            })?;
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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn tap(
        &self,
        node: u64,
        rev: u64,
        hash: Option<&str>,
    ) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/tap", self.base_url);
        let mut payload = serde_json::json!({
            "node": node,
            "rev": rev
        });
        if let Some(h) = hash {
            payload["hash"] = serde_json::json!(h);
        }

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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn type_text(
        &self,
        node: u64,
        text: &str,
        rev: u64,
        hash: Option<&str>,
    ) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/type", self.base_url);
        let mut payload = serde_json::json!({
            "node": node,
            "text": text,
            "rev": rev
        });
        if let Some(h) = hash {
            payload["hash"] = serde_json::json!(h);
        }

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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn tag(&self, index: u64, rev: u64, tag: &str) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/tag", self.base_url);
        let payload = serde_json::json!({
            "target": { "index": index, "rev": rev },
            "tag": tag
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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
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

        let json: Value =
            serde_json::from_str(&body).map_err(|e| HttpBridgeError::StatusError {
                status,
                body: format!("Invalid JSON response: {e}"),
            })?;
        Ok(json)
    }

    pub async fn screenshot(
        &self,
        scale: Option<f32>,
        region: Option<&str>,
    ) -> Result<Vec<u8>, HttpBridgeError> {
        let mut url = format!("{}/screenshot", self.base_url);
        let mut q = Vec::new();
        if let Some(s) = scale {
            q.push(format!("scale={s}"));
        }
        if let Some(r) = region {
            q.push(format!("region={}", r));
        }
        if !q.is_empty() {
            url.push('?');
            url.push_str(&q.join("&"));
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

    /// Generic JSON request for the /recording/* control routes.
    pub async fn recording_request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let mut req = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "DELETE" => self.client.delete(&url),
            other => {
                return Err(HttpBridgeError::StatusError {
                    status: 0,
                    body: format!("unsupported method: {other}"),
                })
            }
        };
        if let Some(b) = body {
            req = req.json(&b);
        }

        let resp = req.headers(self.headers()?).send().await?;

        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }

        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn recording_start(&self) -> Result<Value, HttpBridgeError> {
        self.recording_request("POST", "/recording/start", Some(json!({})))
            .await
    }

    pub async fn recording_stop(&self) -> Result<Value, HttpBridgeError> {
        self.recording_request("POST", "/recording/stop", Some(json!({})))
            .await
    }

    pub async fn recording_export(&self) -> Result<Value, HttpBridgeError> {
        self.recording_request("GET", "/recording/export", None)
            .await
    }

    pub async fn recording_clear(&self) -> Result<Value, HttpBridgeError> {
        self.recording_request("DELETE", "/recording", None).await
    }

    pub async fn network_mock(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/network/mocks", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn network_clear_mocks(&self) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/network/mocks", self.base_url);
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers()?)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn network_history(
        &self,
        url_contains: Option<&str>,
    ) -> Result<Value, HttpBridgeError> {
        let mut req = self
            .client
            .get(format!("{}/network/history", self.base_url));
        if let Some(sub) = url_contains {
            req = req.query(&[("url_contains", sub)]);
        }
        let resp = req.headers(self.headers()?).send().await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn network_clear_history(&self) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/network/history", self.base_url);
        let resp = self
            .client
            .delete(&url)
            .headers(self.headers()?)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn network_mode(&self, enabled: bool) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/network/mode", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&json!({ "enabled": enabled }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn network_request(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/network/execute", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_reset(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/reset", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_prefs(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/prefs", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_prefs_get(&self, name: Option<&str>) -> Result<Value, HttpBridgeError> {
        let mut req = self
            .client
            .get(format!("{}/state/prefs", self.base_url))
            .headers(self.headers()?);
        if let Some(n) = name {
            req = req.query(&[("name", n)]);
        }
        let resp = req.send().await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_db_tables(&self, db: Option<&str>) -> Result<Value, HttpBridgeError> {
        let mut req = self
            .client
            .get(format!("{}/state/db/tables", self.base_url))
            .headers(self.headers()?);
        if let Some(d) = db {
            req = req.query(&[("db", d)]);
        }
        let resp = req.send().await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_db_query(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/db/query", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_db_exec(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/db/exec", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_snapshot_save(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/snapshots/save", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_snapshot_restore(&self, payload: &Value) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/snapshots/restore", self.base_url);
        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(payload)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn state_snapshots_list(&self) -> Result<Value, HttpBridgeError> {
        let url = format!("{}/state/snapshots", self.base_url);
        let resp = self
            .client
            .get(&url)
            .headers(self.headers()?)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let body_text = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError {
                status,
                body: body_text,
            });
        }
        serde_json::from_str(&body_text).map_err(|e| HttpBridgeError::StatusError {
            status,
            body: format!("Invalid JSON response: {e}"),
        })
    }

    pub async fn get_inspect(&self) -> Result<String, HttpBridgeError> {
        let url = format!("{}/inspect", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let status = resp.status().as_u16();
        let body = resp.text().await?;
        if status != 200 {
            return Err(HttpBridgeError::StatusError { status, body });
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_blackbox_get_inspect_integration() {
        use tokio::io::AsyncReadExt;
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let html = "<!DOCTYPE html><html><head><title>Texure Live Web Inspector</title></head><body><div id=\"screen-canvas\"></div></body></html>";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(resp.as_bytes()).await;
                let _ = stream.flush().await;
            }
        });

        let client = HttpBridgeClient::new(format!("http://{addr}"), None);
        let body = client.get_inspect().await.expect("GET /inspect succeeds");
        assert!(body.contains("<!DOCTYPE html>"));
        assert!(body.contains("Texure Live Web Inspector"));
        assert!(body.contains("screen-canvas"));
    }
}
