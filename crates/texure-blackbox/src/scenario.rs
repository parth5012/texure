use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScenarioError {
    #[error("IO error reading scenario: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Schema version mismatch: expected 1, found {0}")]
    InvalidSchemaVersion(u32),
    #[error("Unknown or forbidden action: {0}")]
    InvalidAction(String),
    #[error("Unknown predicate: {0}")]
    InvalidPredicate(String),
    #[error("Unresolvable reference: {0}")]
    UnresolvableRef(String),
}

/// Scenario DSL v0 definition (per docs/blackbox-plan.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub schema_version: u32,
    pub name: String,
    pub kind: String, // "scenario" | "fixture"
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub action: String,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub expect: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub pred: String,
    pub target: String,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

impl Scenario {
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, ScenarioError> {
        let content = std::fs::read_to_string(path)?;
        let scenario: Self = serde_json::from_str(&content)?;
        scenario.validate()?;
        Ok(scenario)
    }

    pub fn validate(&self) -> Result<(), ScenarioError> {
        if self.schema_version != 1 {
            return Err(ScenarioError::InvalidSchemaVersion(self.schema_version));
        }

        let allowed_actions = [
            "device_tree",
            "tap",
            "type",
            "keypress",
            "back",
            "screenshot",
            "wait_for",
            "mcp_handshake",
            "raw_mcp",
        ];

        for step in &self.steps {
            if step.action == "sleep" {
                return Err(ScenarioError::InvalidAction(
                    "sleep is strictly forbidden in scenarios; use wait_for with predicates"
                        .to_string(),
                ));
            }
            if !allowed_actions.contains(&step.action.as_str()) {
                return Err(ScenarioError::InvalidAction(step.action.clone()));
            }
        }

        let allowed_preds = ["exists", "missing", "count_lt", "state", "bounds_of"];
        for assertion in &self.assertions {
            if !allowed_preds.contains(&assertion.pred.as_str()) {
                return Err(ScenarioError::InvalidPredicate(assertion.pred.clone()));
            }
        }

        Ok(())
    }

    pub fn is_device_only(&self) -> bool {
        self.tags.iter().any(|t| t == "device")
    }
}

/// Evaluates `$tree.*` variable references against a captured device_tree JSON
pub fn resolve_variable(expr: &str, context: &HashMap<String, serde_json::Value>) -> Result<serde_json::Value, ScenarioError> {
    if let Some(stripped) = expr.strip_prefix("$tree.") {
        if let Some(tree) = context.get("tree") {
            if stripped == "rev" {
                return tree.get("rev").cloned().ok_or_else(|| {
                    ScenarioError::UnresolvableRef("$tree.rev missing in tree".to_string())
                });
            }
            if let Some(node_idx_str) = stripped.strip_prefix("nodes[") {
                if let Some(end_idx) = node_idx_str.find(']') {
                    let idx_str = &node_idx_str[..end_idx];
                    let suffix = &node_idx_str[end_idx + 1..];
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if let Some(nodes) = tree.get("nodes").and_then(|n| n.as_array()) {
                            if let Some(node) = nodes.get(idx) {
                                if suffix.is_empty() {
                                    return Ok(node.clone());
                                } else if let Some(path) = suffix.strip_prefix('.') {
                                    let mut curr = node;
                                    for part in path.split('.') {
                                        curr = curr.get(part).ok_or_else(|| {
                                            ScenarioError::UnresolvableRef(expr.to_string())
                                        })?;
                                    }
                                    return Ok(curr.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err(ScenarioError::UnresolvableRef(expr.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_scenario_validation() {
        let sc = Scenario {
            schema_version: 1,
            name: "test_scenario".to_string(),
            kind: "scenario".to_string(),
            description: None,
            tags: vec![],
            steps: vec![Step {
                action: "device_tree".to_string(),
                args: serde_json::json!({}),
                expect: None,
            }],
            assertions: vec![Assertion {
                pred: "exists".to_string(),
                target: "node".to_string(),
                value: None,
            }],
        };
        assert!(sc.validate().is_ok());
    }

    #[test]
    fn test_invalid_schema_version() {
        let sc = Scenario {
            schema_version: 2,
            name: "bad_version".to_string(),
            kind: "scenario".to_string(),
            description: None,
            tags: vec![],
            steps: vec![],
            assertions: vec![],
        };
        assert!(sc.validate().is_err());
    }

    #[test]
    fn test_reject_sleep_action() {
        let sc = Scenario {
            schema_version: 1,
            name: "sleep_forbidden".to_string(),
            kind: "scenario".to_string(),
            description: None,
            tags: vec![],
            steps: vec![Step {
                action: "sleep".to_string(),
                args: serde_json::json!({"duration": 5}),
                expect: None,
            }],
            assertions: vec![],
        };
        assert!(sc.validate().is_err());
    }

    #[test]
    fn test_resolve_tree_variable() {
        let mut context = HashMap::new();
        context.insert(
            "tree".to_string(),
            serde_json::json!({
                "rev": 42,
                "nodes": [
                    { "id": "btn_1", "text": "Click me" }
                ]
            }),
        );

        let rev = resolve_variable("$tree.rev", &context).unwrap();
        assert_eq!(rev, 42);

        let node_id = resolve_variable("$tree.nodes[0].id", &context).unwrap();
        assert_eq!(node_id, "btn_1");

        let mut deep_context = HashMap::new();
        deep_context.insert(
            "tree".to_string(),
            serde_json::json!({
                "nodes": [
                    { "id": "btn_1", "extra": { "sub": "deep_val" } }
                ]
            }),
        );
        let node_subfield = resolve_variable("$tree.nodes[0].extra.sub", &deep_context).unwrap();
        assert_eq!(node_subfield, "deep_val");
    }
}

