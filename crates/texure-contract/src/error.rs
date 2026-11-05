use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// The 16 frozen error codes from docs/bridge-contract.md §Error shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NoConfig,
    Offline,
    AuthFailed,
    Busy,
    NodeOutdated,
    NodeNotClickable,
    NodeNotEditable,
    NodeNotFound,
    InjectFailed,
    CaptureFailed,
    AppNotForeground,
    ScreenOff,
    DeviceLocked,
    HumanSteering,
    BridgeStopped,
    PortInUse,
}

impl ErrorCode {
    /// All 16 frozen error codes in specification order.
    pub const ALL: [ErrorCode; 16] = [
        ErrorCode::NoConfig,
        ErrorCode::Offline,
        ErrorCode::AuthFailed,
        ErrorCode::Busy,
        ErrorCode::NodeOutdated,
        ErrorCode::NodeNotClickable,
        ErrorCode::NodeNotEditable,
        ErrorCode::NodeNotFound,
        ErrorCode::InjectFailed,
        ErrorCode::CaptureFailed,
        ErrorCode::AppNotForeground,
        ErrorCode::ScreenOff,
        ErrorCode::DeviceLocked,
        ErrorCode::HumanSteering,
        ErrorCode::BridgeStopped,
        ErrorCode::PortInUse,
    ];

    /// String identifier for the error code (uppercase snake case).
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoConfig => "NO_CONFIG",
            Self::Offline => "OFFLINE",
            Self::AuthFailed => "AUTH_FAILED",
            Self::Busy => "BUSY",
            Self::NodeOutdated => "NODE_OUTDATED",
            Self::NodeNotClickable => "NODE_NOT_CLICKABLE",
            Self::NodeNotEditable => "NODE_NOT_EDITABLE",
            Self::NodeNotFound => "NODE_NOT_FOUND",
            Self::InjectFailed => "INJECT_FAILED",
            Self::CaptureFailed => "CAPTURE_FAILED",
            Self::AppNotForeground => "APP_NOT_FOREGROUND",
            Self::ScreenOff => "SCREEN_OFF",
            Self::DeviceLocked => "DEVICE_LOCKED",
            Self::HumanSteering => "HUMAN_STEERING",
            Self::BridgeStopped => "BRIDGE_STOPPED",
            Self::PortInUse => "PORT_IN_USE",
        }
    }

    /// Official agent remediation hint per docs/bridge-contract.md §Error shape.
    pub const fn agent_hint(&self) -> &'static str {
        match self {
            Self::NoConfig => {
                "create ~/.config/texure/config.toml or set TEXURE_URL/TEXURE_TOKEN"
            }
            Self::Offline => "same LAN? app open? fg service alive?",
            Self::AuthFailed => "re-paste fresh token from overlay",
            Self::Busy => "wait for owner release / ask owner",
            Self::NodeOutdated => "re-run device_tree, re-target",
            Self::NodeNotClickable => "pick a clickable node from the latest tree",
            Self::NodeNotEditable => "tap the editable field first, then type",
            Self::NodeNotFound => "ids only come from the latest device_tree",
            Self::InjectFailed => "re-fetch tree, retry once; bundle evidence",
            Self::CaptureFailed => "foreground the app; device_tree still works",
            Self::AppNotForeground => "bring app forward explicitly (no auto-launch)",
            Self::ScreenOff => "wake + reopen app (human step)",
            Self::DeviceLocked => "unlock device first",
            Self::HumanSteering => "re-plan signal: device_tree then continue",
            Self::BridgeStopped => "restart service via overlay, reconnect",
            Self::PortInUse => "free the occupier; service stays stopped",
        }
    }

    /// Default human-readable error message.
    pub const fn default_message(&self) -> &'static str {
        match self {
            Self::NoConfig => "no config file or env vars",
            Self::Offline => "device unreachable",
            Self::AuthFailed => "token mismatch",
            Self::Busy => "second client during single-flight",
            Self::NodeOutdated => "tree refreshed since reference; re-run /ui-tree",
            Self::NodeNotClickable => "tap onto non-clickable node",
            Self::NodeNotEditable => "type into non-editable node",
            Self::NodeNotFound => "node id not found",
            Self::InjectFailed => "injection methods failed",
            Self::CaptureFailed => "screenshot capture failed",
            Self::AppNotForeground => "mutation while backgrounded",
            Self::ScreenOff => "display off",
            Self::DeviceLocked => "keyguard locked",
            Self::HumanSteering => "human touch detected mid-action",
            Self::BridgeStopped => "bridge service was stopped mid-action",
            Self::PortInUse => "fixed port occupied on device",
        }
    }

    /// Format single-line error text for standard MCP tool response: `CODE: message (hint: ...)`.
    pub fn formatted_single_line(&self, custom_msg: Option<&str>) -> String {
        let msg = custom_msg.unwrap_or_else(|| self.default_message());
        format!("{}: {}", self.as_str(), msg)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::error::Error for ErrorCode {}

impl FromStr for ErrorCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "NO_CONFIG" => Ok(Self::NoConfig),
            "OFFLINE" => Ok(Self::Offline),
            "AUTH_FAILED" => Ok(Self::AuthFailed),
            "BUSY" => Ok(Self::Busy),
            "NODE_OUTDATED" => Ok(Self::NodeOutdated),
            "NODE_NOT_CLICKABLE" => Ok(Self::NodeNotClickable),
            "NODE_NOT_EDITABLE" => Ok(Self::NodeNotEditable),
            "NODE_NOT_FOUND" => Ok(Self::NodeNotFound),
            "INJECT_FAILED" => Ok(Self::InjectFailed),
            "CAPTURE_FAILED" => Ok(Self::CaptureFailed),
            "APP_NOT_FOREGROUND" => Ok(Self::AppNotForeground),
            "SCREEN_OFF" => Ok(Self::ScreenOff),
            "DEVICE_LOCKED" => Ok(Self::DeviceLocked),
            "HUMAN_STEERING" => Ok(Self::HumanSteering),
            "BRIDGE_STOPPED" => Ok(Self::BridgeStopped),
            "PORT_IN_USE" => Ok(Self::PortInUse),
            other => Err(format!("Unknown error code: {other}")),
        }
    }
}

/// Standard bridge error body contained within error envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

impl BridgeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let hint = Some(code.agent_hint().to_string());
        Self {
            code,
            message: message.into(),
            hint,
            owner: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BridgeError {}

/// HTTP/JSON error envelope `{ "error": { "code": "...", "message": "..." } }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeErrorEnvelope {
    pub error: BridgeError,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ws_connected: Option<bool>,
}

impl BridgeErrorEnvelope {
    pub fn from_error(error: BridgeError) -> Self {
        Self {
            error,
            http_status: None,
            bridge_url: None,
            ws_connected: None,
        }
    }

    pub fn from_code(code: ErrorCode) -> Self {
        Self::from_error(BridgeError::new(code, code.default_message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_16_error_codes_roundtrip() {
        assert_eq!(ErrorCode::ALL.len(), 16);
        for code in &ErrorCode::ALL {
            let json = serde_json::to_string(code).expect("serialize error code");
            let expected_json = format!("\"{}\"", code.as_str());
            assert_eq!(json, expected_json);

            let parsed: ErrorCode =
                serde_json::from_str(&json).expect("deserialize error code");
            assert_eq!(parsed, *code);

            let from_str_res = ErrorCode::from_str(code.as_str()).expect("from_str");
            assert_eq!(from_str_res, *code);

            assert!(!code.agent_hint().is_empty());
            assert!(!code.default_message().is_empty());
        }
    }

    #[test]
    fn test_bridge_error_envelope_serialization() {
        let envelope = BridgeErrorEnvelope::from_code(ErrorCode::NodeOutdated);
        let val = serde_json::to_value(&envelope).expect("to_value");
        assert_eq!(val["error"]["code"], "NODE_OUTDATED");
        assert_eq!(
            val["error"]["message"],
            "tree refreshed since reference; re-run /ui-tree"
        );
    }
}
