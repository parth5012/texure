use crate::tools::KeyPressKey;
use crate::tree::UiTree;
use serde::{Deserialize, Serialize};

/// Request body for `POST /tap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,

    pub rev: u64,
}

impl TapRequest {
    pub fn new(rev: u64) -> Self {
        Self {
            node: None,
            index: None,
            rev,
        }
    }

    pub fn with_node(mut self, node: u64) -> Self {
        self.node = Some(node);
        self
    }

    pub fn with_index(mut self, index: u64) -> Self {
        self.index = Some(index);
        self
    }
}

/// Request body for `POST /type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u64>,

    pub text: String,

    pub rev: u64,
}

impl TypeRequest {
    pub fn new(text: impl Into<String>, rev: u64) -> Self {
        Self {
            node: None,
            index: None,
            text: text.into(),
            rev,
        }
    }

    pub fn with_node(mut self, node: u64) -> Self {
        self.node = Some(node);
        self
    }

    pub fn with_index(mut self, index: u64) -> Self {
        self.index = Some(index);
        self
    }
}

/// Request body for `POST /keypress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPressRequest {
    pub key: KeyPressKey,
}

impl KeyPressRequest {
    pub fn new(key: KeyPressKey) -> Self {
        Self { key }
    }
}

/// Standard success envelope returned by mutation endpoints (`/tap`, `/type`, `/keypress`, `/back`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeSuccessResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
}

impl BridgeSuccessResponse {
    pub fn ok() -> Self {
        Self {
            ok: true,
            rev: None,
        }
    }

    pub fn ok_with_rev(rev: u64) -> Self {
        Self {
            ok: true,
            rev: Some(rev),
        }
    }
}

/// Broadcast event pushed over `ws://<host>:<port>/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeChangedEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub rev: u64,
}

impl TreeChangedEvent {
    pub const EVENT_TYPE: &'static str = "tree_changed";

    pub fn new(rev: u64) -> Self {
        Self {
            event_type: Self::EVENT_TYPE.to_string(),
            rev,
        }
    }
}

/// Build information metadata in crash bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub application_id: String,
    pub version_name: String,
    pub bridge_rev: String,
}

/// Logged action preceding a crash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastAction {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<u64>,
    pub rev: u64,
    pub result: String,
}

/// Debug crash/ANR bundle returned by `GET /last-crash`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrashBundle {
    pub timestamp: u64,
    pub stack_trace: String,
    pub last_tree: UiTree,
    #[serde(default)]
    pub last_actions: Vec<LastAction>,
    pub build_info: BuildInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tap_response_fixture_deserialization() {
        let json_str = include_str!("../../../contract/fixtures/tap_response.json");
        let resp: BridgeSuccessResponse =
            serde_json::from_str(json_str).expect("deserialize tap_response");
        assert!(resp.ok);
    }

    #[test]
    fn test_tree_changed_event_serialization() {
        let event = TreeChangedEvent::new(43);
        let val = serde_json::to_value(&event).unwrap();
        assert_eq!(val["type"], "tree_changed");
        assert_eq!(val["rev"], 43);
    }
}
