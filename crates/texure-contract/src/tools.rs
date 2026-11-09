use crate::error::ErrorCode;
use crate::tree::UiTree;
use serde::{Deserialize, Serialize};

/// The 6 frozen MCP tools.
pub const FROZEN_TOOL_NAMES: [&str; 6] = [
    "device_tree",
    "tap",
    "type",
    "keypress",
    "screenshot",
    "wait_for",
];

/// Parameter payload for the `device_tree` tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceTreeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtree: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Parameter payload for the `tap` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,

    /// Optional semantic node hash from `device_tree` (map #36, #29).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,

    pub rev: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Parameter payload for the `type` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,

    /// Optional semantic node hash from `device_tree` (map #36, #29).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,

    pub text: String,

    pub rev: u64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Permitted key names for the `keypress` tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum KeyPressKey {
    Enter,
    Back,
    Del,
    Tab,
}

impl KeyPressKey {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Enter => "ENTER",
            Self::Back => "BACK",
            Self::Del => "DEL",
            Self::Tab => "TAB",
        }
    }
}

/// Parameter payload for the `keypress` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPressParams {
    pub key: KeyPressKey,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Scale factors allowed for `screenshot`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ScreenshotScale {
    #[serde(rename = "0.25")]
    Quarter,
    #[serde(rename = "0.5")]
    Half,
    #[serde(rename = "1.0")]
    Full,
}

impl ScreenshotScale {
    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Quarter => 0.25,
            Self::Half => 0.5,
            Self::Full => 1.0,
        }
    }

    pub fn query_param(&self) -> &'static str {
        match self {
            Self::Quarter => "scale=0.25",
            Self::Half => "scale=0.5",
            Self::Full => "scale=1.0",
        }
    }
}

/// Parameter payload for the `screenshot` tool.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// Predicate target for `wait_for`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaitForWhere {
    Text,
    Tag,
    Visible,
    Hidden,
}

impl WaitForWhere {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Tag => "tag",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
        }
    }
}

/// Parameter payload for the `wait_for` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForParams {
    #[serde(rename = "where")]
    pub where_: WaitForWhere,

    pub value: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

impl WaitForParams {
    /// Return effective timeout clamped to 30s max (default 10s).
    pub fn effective_timeout_secs(&self) -> u64 {
        self.timeout.unwrap_or(10).min(30)
    }
}

/// Non-error result payload for `wait_for` expiration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitForResult {
    pub ok: bool,
    pub last_rev: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<UiTree>,
}

/// Content element inside an MCP CallToolResult.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: String,
    },
}

impl McpContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn png_image(base64_data: impl Into<String>) -> Self {
        Self::Image {
            data: base64_data.into(),
            mime_type: "image/png".to_string(),
        }
    }
}

/// Envelope returned by MCP `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<McpContent>,
    #[serde(
        default,
        rename = "isError",
        alias = "is_error",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    pub fn success_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::text(text)],
            is_error: None,
        }
    }

    pub fn success_image(base64_png: impl Into<String>) -> Self {
        Self {
            content: vec![McpContent::png_image(base64_png)],
            is_error: None,
        }
    }

    pub fn error(
        code: ErrorCode,
        message: Option<&str>,
        verbose: bool,
        full_error: Option<&serde_json::Value>,
    ) -> Self {
        let text = if verbose {
            if let Some(val) = full_error {
                serde_json::to_string(val).unwrap_or_else(|_| code.formatted_single_line(message))
            } else {
                let err_obj = serde_json::json!({
                    "error": {
                        "code": code.as_str(),
                        "message": message.unwrap_or_else(|| code.default_message()),
                        "hint": code.agent_hint(),
                    }
                });
                serde_json::to_string(&err_obj)
                    .unwrap_or_else(|_| code.formatted_single_line(message))
            }
        } else {
            code.formatted_single_line(message)
        };

        Self {
            content: vec![McpContent::text(text)],
            is_error: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frozen_tools_count() {
        assert_eq!(FROZEN_TOOL_NAMES.len(), 6);
    }

    #[test]
    fn test_wait_for_timeout_clamping() {
        let params = WaitForParams {
            where_: WaitForWhere::Text,
            value: "Send".to_string(),
            timeout: Some(60),
            verbose: None,
        };
        assert_eq!(params.effective_timeout_secs(), 30);

        let default_params = WaitForParams {
            where_: WaitForWhere::Tag,
            value: "btn_send".to_string(),
            timeout: None,
            verbose: None,
        };
        assert_eq!(default_params.effective_timeout_secs(), 10);
    }

    #[test]
    fn test_keypress_key_serialization() {
        let enter = KeyPressKey::Enter;
        let s = serde_json::to_string(&enter).unwrap();
        assert_eq!(s, "\"ENTER\"");
    }

    #[test]
    fn test_tap_params_hash_optional() {
        let params: TapParams =
            serde_json::from_value(serde_json::json!({"node": 1, "rev": 42, "hash": "abc123"}))
                .unwrap();
        assert_eq!(params.hash.as_deref(), Some("abc123"));
        let legacy: TapParams =
            serde_json::from_value(serde_json::json!({"node": 1, "rev": 42})).unwrap();
        assert_eq!(legacy.hash, None);
    }

    #[test]
    fn test_type_params_hash_optional() {
        let params: TypeParams = serde_json::from_value(
            serde_json::json!({"node": 1, "rev": 42, "text": "hi", "hash": "def456"}),
        )
        .unwrap();
        assert_eq!(params.hash.as_deref(), Some("def456"));
        let legacy: TypeParams =
            serde_json::from_value(serde_json::json!({"node": 1, "rev": 42, "text": "hi"}))
                .unwrap();
        assert_eq!(legacy.hash, None);
    }
}
