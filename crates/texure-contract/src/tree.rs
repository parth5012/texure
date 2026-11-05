use serde::{Deserialize, Serialize};

/// Accessibility and view hierarchy tree returned by `GET /ui-tree`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiTree {
    pub rev: u64,
    pub window: String,
    #[serde(default)]
    pub nodes: Vec<UiNode>,
}

impl UiTree {
    pub fn new(rev: u64, window: impl Into<String>, nodes: Vec<UiNode>) -> Self {
        Self {
            rev,
            window: window.into(),
            nodes,
        }
    }

    /// Search for a node by tag / id (exact match, recursive).
    pub fn find_by_id(&self, id: &str) -> Option<&UiNode> {
        fn search<'a>(nodes: &'a [UiNode], id: &str) -> Option<&'a UiNode> {
            for node in nodes {
                if node.id.as_deref() == Some(id) {
                    return Some(node);
                }
                if let Some(found) = search(&node.children, id) {
                    return Some(found);
                }
            }
            None
        }
        search(&self.nodes, id)
    }

    /// Search for a node by index (recursive).
    pub fn find_by_index(&self, index: usize) -> Option<&UiNode> {
        fn search(nodes: &[UiNode], index: usize) -> Option<&UiNode> {
            for node in nodes {
                if node.index == Some(index) {
                    return Some(node);
                }
                if let Some(found) = search(&node.children, index) {
                    return Some(found);
                }
            }
            None
        }
        search(&self.nodes, index)
    }

    /// Find all visible nodes whose text or contentDescription matches a case-insensitive predicate (recursive).
    pub fn find_by_text_substring(&self, text: &str) -> Vec<&UiNode> {
        let needle = text.to_lowercase();
        let mut results = Vec::new();

        fn collect<'a>(nodes: &'a [UiNode], needle: &str, results: &mut Vec<&'a UiNode>) {
            for node in nodes {
                if node.is_visible() {
                    let matches = node
                        .text
                        .as_deref()
                        .map(|t| t.to_lowercase().contains(needle))
                        .unwrap_or(false)
                        || node
                            .cd
                            .as_deref()
                            .map(|cd| cd.to_lowercase().contains(needle))
                            .unwrap_or(false);

                    if matches {
                        results.push(node);
                    }
                }
                collect(&node.children, needle, results);
            }
        }

        collect(&self.nodes, &needle, &mut results);
        results
    }
}

/// A single node in the Android view tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNode {
    /// View resource ID or test tag.
    #[serde(default, alias = "tag", alias = "view_id")]
    pub id: Option<String>,

    /// Displayed text content.
    #[serde(default)]
    pub text: Option<String>,

    /// Content description for accessibility.
    #[serde(default, alias = "contentDescription", alias = "content_description")]
    pub cd: Option<String>,

    /// Android view class name (e.g. android.widget.Button).
    #[serde(default, alias = "className", alias = "class_name")]
    pub cls: Option<String>,

    /// Node bounds in dp: `[x, y, w, h]`.
    #[serde(default)]
    pub bounds: Option<[i32; 4]>,

    /// Whether this node can be clicked.
    #[serde(default)]
    pub clickable: bool,

    /// Whether this node can receive text input.
    #[serde(default)]
    pub editable: bool,

    /// Whether this node is scrollable.
    #[serde(default)]
    pub scrollable: bool,

    /// Positional index in tree traversal.
    #[serde(default)]
    pub index: Option<usize>,

    /// Child nodes (for hierarchical representation if used).
    #[serde(default)]
    pub children: Vec<UiNode>,
}

impl UiNode {
    /// Check if node has non-zero positive dimensions.
    pub fn is_visible(&self) -> bool {
        match self.bounds {
            Some([_, _, w, h]) => w > 0 && h > 0,
            None => true,
        }
    }

    /// Primary text or description accessor.
    pub fn display_text(&self) -> Option<&str> {
        self.text.as_deref().or(self.cd.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_tree_deserialization_from_fixture() {
        let json_str = include_str!("../../../contract/fixtures/ui_tree.json");
        let tree: UiTree = serde_json::from_str(json_str).expect("deserialize ui_tree fixture");
        assert_eq!(tree.rev, 42);
        assert_eq!(tree.window, "com.velavoice/.MainActivity");
        assert_eq!(tree.nodes.len(), 2);

        let node0 = &tree.nodes[0];
        assert_eq!(node0.id.as_deref(), Some("btn_send"));
        assert_eq!(node0.text.as_deref(), Some("Send"));
        assert_eq!(node0.cls.as_deref(), Some("android.widget.Button"));
        assert_eq!(node0.bounds, Some([12, 340, 108, 56]));
        assert!(node0.clickable);
        assert!(!node0.editable);

        let node1 = &tree.nodes[1];
        assert_eq!(node1.id.as_deref(), Some("input_message"));
        assert!(node1.editable);
    }

    #[test]
    fn test_deep_gauntlet_fixture() {
        let json_str = include_str!("../../../contract/fixtures/deep_gauntlet_tree.json");
        let tree: UiTree = serde_json::from_str(json_str).expect("deserialize deep_gauntlet fixture");
        assert_eq!(tree.rev, 100);
        assert_eq!(tree.nodes.len(), 5);
        assert_eq!(tree.nodes[0].id.as_deref(), Some("root_container"));
    }
}
