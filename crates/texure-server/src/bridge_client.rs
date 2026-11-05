use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use std::time::Duration;
use texure_contract::bridge::{
    BridgeSuccessResponse, KeyPressRequest, TapRequest, TypeRequest,
};
use texure_contract::config::ResolvedDeviceConfig;
use texure_contract::error::{BridgeErrorEnvelope, ErrorCode};
use texure_contract::tools::{CallToolResult, ScreenshotScale};
use texure_contract::tree::UiTree;

/// Error returned from a bridge HTTP call.
#[derive(Debug, Clone)]
pub struct BridgeCallError {
    pub code: ErrorCode,
    pub message: String,
    pub raw_json: Option<serde_json::Value>,
    pub http_status: Option<u16>,
}

impl BridgeCallError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            raw_json: None,
            http_status: None,
        }
    }

    pub fn offline(details: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Offline,
            message: details.into(),
            raw_json: None,
            http_status: None,
        }
    }

    pub fn to_mcp_result(
        &self,
        verbose: bool,
        bridge_url: &str,
        ws_connected: bool,
    ) -> CallToolResult {
        if !verbose {
            return CallToolResult::error(self.code, Some(&self.message), false, None);
        }

        // LLD §4.1 verbose shape: the bridge error payload (verbatim when the
        // bridge sent an envelope) plus connection context. Server-origin
        // errors carry no http_status (LLD §4.2 marks it "—").
        let error_obj = match &self.raw_json {
            Some(raw) => raw
                .get("error")
                .cloned()
                .unwrap_or_else(|| self.fabricated_error()),
            None => self.fabricated_error(),
        };
        let mut full_err = serde_json::json!({
            "error": error_obj,
            "bridge_url": bridge_url,
            "ws_connected": ws_connected,
        });
        if let Some(status) = self.http_status {
            full_err["http_status"] = serde_json::json!(status);
        }
        CallToolResult::error(self.code, Some(&self.message), true, Some(&full_err))
    }

    fn fabricated_error(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code.as_str(),
            "message": &self.message,
            "hint": self.code.agent_hint(),
        })
    }
}

/// HTTP client for interacting with the Android Bridge.
#[derive(Clone)]
pub struct BridgeClient {
    client: Client,
    pub config: ResolvedDeviceConfig,
}

impl BridgeClient {
    pub fn new(config: ResolvedDeviceConfig) -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        let auth_val = format!("Bearer {}", config.token);
        let header_val = HeaderValue::from_str(&auth_val)
            .map_err(|e| format!("Invalid auth token header: {e}"))?;
        headers.insert(AUTHORIZATION, header_val);

        let client = Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

        Ok(Self { client, config })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.normalized_url(), path)
    }

    pub async fn get_ui_tree(
        &self,
        max_depth: Option<u32>,
        subtree: Option<&str>,
    ) -> Result<UiTree, BridgeCallError> {
        let mut url = self.url("/ui-tree");
        let mut query = Vec::new();
        if let Some(depth) = max_depth {
            query.push(format!("max_depth={depth}"));
        }
        if let Some(sub) = subtree {
            query.push(format!("subtree={sub}"));
        }
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query.join("&"));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !resp.status().is_success() {
            return Err(self.map_http_error(resp).await);
        }

        resp.json::<UiTree>()
            .await
            .map_err(|e| BridgeCallError::new(ErrorCode::CaptureFailed, format!("Invalid UI tree payload: {e}")))
    }

    pub async fn tap(&self, req: &TapRequest) -> Result<BridgeSuccessResponse, BridgeCallError> {
        let url = self.url("/tap");
        let resp = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !resp.status().is_success() {
            return Err(self.map_http_error(resp).await);
        }

        resp.json::<BridgeSuccessResponse>()
            .await
            .map_err(|e| BridgeCallError::new(ErrorCode::InjectFailed, format!("Invalid tap response: {e}")))
    }

    pub async fn type_text(&self, req: &TypeRequest) -> Result<BridgeSuccessResponse, BridgeCallError> {
        let url = self.url("/type");
        let resp = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !resp.status().is_success() {
            return Err(self.map_http_error(resp).await);
        }

        resp.json::<BridgeSuccessResponse>()
            .await
            .map_err(|e| BridgeCallError::new(ErrorCode::InjectFailed, format!("Invalid type response: {e}")))
    }

    pub async fn keypress(
        &self,
        req: &KeyPressRequest,
    ) -> Result<BridgeSuccessResponse, BridgeCallError> {
        let url = self.url("/keypress");
        let resp = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !resp.status().is_success() {
            return Err(self.map_http_error(resp).await);
        }

        resp.json::<BridgeSuccessResponse>()
            .await
            .map_err(|e| BridgeCallError::new(ErrorCode::InjectFailed, format!("Invalid keypress response: {e}")))
    }

    pub async fn screenshot(&self, scale: Option<f64>) -> Result<Vec<u8>, BridgeCallError> {
        let scale_enum = match scale {
            Some(s) if (s - 0.25).abs() < 0.01 => ScreenshotScale::Quarter,
            Some(s) if (s - 1.0).abs() < 0.01 => ScreenshotScale::Full,
            _ => ScreenshotScale::Half,
        };

        let url = format!("{}/screenshot?{}", self.config.normalized_url(), scale_enum.query_param());
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| self.map_reqwest_error(e))?;

        if !resp.status().is_success() {
            return Err(self.map_http_error(resp).await);
        }

        if !resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.contains("image/png"))
            .unwrap_or(false)
        {
            return Err(BridgeCallError::new(
                ErrorCode::CaptureFailed,
                "screenshot response was not a PNG",
            ));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| BridgeCallError::new(ErrorCode::CaptureFailed, format!("Failed to read screenshot bytes: {e}")))?;

        Ok(bytes.to_vec())
    }

    fn map_reqwest_error(&self, err: reqwest::Error) -> BridgeCallError {
        if err.is_connect() || err.is_timeout() {
            BridgeCallError::offline("device unreachable")
        } else {
            BridgeCallError::new(
                ErrorCode::BridgeStopped,
                "connection died mid-action",
            )
        }
    }

    async fn map_http_error(&self, resp: reqwest::Response) -> BridgeCallError {
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await.unwrap_or_default();

        if let Ok(envelope) = serde_json::from_slice::<BridgeErrorEnvelope>(&bytes) {
            return BridgeCallError {
                code: envelope.error.code,
                message: envelope.error.message,
                raw_json: serde_json::from_slice(&bytes).ok(),
                http_status: Some(status),
            };
        }

        // Fallback mapping based on HTTP status code
        let code = match status {
            401 => ErrorCode::AuthFailed,
            409 => ErrorCode::Busy,
            s if (500..600).contains(&s) => ErrorCode::BridgeStopped,
            _ => ErrorCode::InjectFailed,
        };

        let msg = String::from_utf8_lossy(&bytes).to_string();
        let message = if msg.trim().is_empty() {
            code.default_message().to_string()
        } else {
            msg
        };

        BridgeCallError {
            code,
            message,
            raw_json: None,
            http_status: Some(status),
        }
    }
}
