use crate::bridge_client::{BridgeCallError, BridgeClient};
use crate::ws_supervisor::WsSupervisor;
use std::time::Duration;
use texure_contract::tools::{CallToolResult, WaitForParams, WaitForWhere};
use texure_contract::tree::UiTree;
use tokio::time::Instant;

/// Evaluate a wait_for condition against a given UI tree.
pub fn evaluate_condition(tree: &UiTree, where_: WaitForWhere, value: &str) -> bool {
    match where_ {
        WaitForWhere::Text | WaitForWhere::Visible => {
            !tree.find_by_text_substring(value).is_empty()
        }
        WaitForWhere::Tag => tree
            .find_by_id(value)
            .map(|n| n.is_visible())
            .unwrap_or(false),
        WaitForWhere::Hidden => tree.find_by_text_substring(value).is_empty(),
    }
}

/// Execute `wait_for` logic combining immediate evaluation, WS events, and 500ms HTTP fallback.
pub async fn execute_wait_for(
    params: &WaitForParams,
    bridge: &BridgeClient,
    ws: &WsSupervisor,
) -> Result<CallToolResult, BridgeCallError> {
    let timeout_secs = params.effective_timeout_secs();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    // Subscribe before the initial fetch: a tree change landing between the
    // snapshot and a later subscribe would be broadcast to nobody.
    let mut ws_rx = ws.subscribe();

    // 1. Initial snapshot check
    let initial_tree = bridge.get_ui_tree(None, None).await?;
    if evaluate_condition(&initial_tree, params.where_, &params.value) {
        let ok_payload = serde_json::json!({
            "ok": true,
            "rev": initial_tree.rev
        });
        return Ok(CallToolResult::success_text(ok_payload.to_string()));
    }

    let mut last_rev = initial_tree.rev;

    // 2. Loop until deadline
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        if ws.is_connected() {
            // WS active: wait for tree_changed broadcast, but tick at most
            // 500ms so a disconnect mid-wait re-enters the loop and reaches
            // the HTTP poll fallback instead of sleeping to the deadline.
            let tick = Duration::from_millis(500).min(remaining);
            tokio::select! {
                _ = tokio::time::sleep(tick) => continue,
                msg = ws_rx.recv() => {
                    match msg {
                        Ok(_event) => {
                            match bridge.get_ui_tree(None, None).await {
                                Ok(tree) => {
                                    last_rev = tree.rev;
                                    if evaluate_condition(&tree, params.where_, &params.value) {
                                        let ok_payload = serde_json::json!({
                                            "ok": true,
                                            "rev": tree.rev
                                        });
                                        return Ok(CallToolResult::success_text(ok_payload.to_string()));
                                    }
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        Err(_) => {
                            // Flap / lagging; fall through to small sleep
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        } else {
            // WS disconnected: fall back to 500ms HTTP polling
            let poll_delay = Duration::from_millis(500).min(remaining);
            tokio::time::sleep(poll_delay).await;

            match bridge.get_ui_tree(None, None).await {
                Ok(tree) => {
                    last_rev = tree.rev;
                    if evaluate_condition(&tree, params.where_, &params.value) {
                        let ok_payload = serde_json::json!({
                            "ok": true,
                            "rev": tree.rev
                        });
                        return Ok(CallToolResult::success_text(ok_payload.to_string()));
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    // 3. Deadline reached: fetch one final snapshot and return non-error timeout payload
    let final_tree = bridge.get_ui_tree(None, None).await.ok();
    if let Some(ref tree) = final_tree {
        last_rev = tree.rev;
    }

    let timeout_payload = serde_json::json!({
        "ok": false,
        "last_rev": last_rev,
        "hint": "re-fetch device_tree and re-plan",
        "tree": final_tree
    });

    Ok(CallToolResult::success_text(timeout_payload.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use texure_contract::tree::UiNode;

    #[test]
    fn test_predicate_evaluation() {
        let tree = UiTree::new(
            42,
            "MainActivity",
            vec![
                UiNode {
                    id: Some("btn_login".to_string()),
                    text: Some("Log in now".to_string()),
                    cd: None,
                    cls: Some("Button".to_string()),
                    bounds: Some([0, 0, 100, 50]),
                    clickable: true,
                    editable: false,
                    scrollable: false,
                    index: Some(0),
                    hash: None,
                    children: Vec::new(),
                },
                UiNode {
                    id: Some("spinner".to_string()),
                    text: Some("Loading...".to_string()),
                    cd: None,
                    cls: Some("ProgressBar".to_string()),
                    bounds: Some([0, 50, 100, 50]),
                    clickable: false,
                    editable: false,
                    scrollable: false,
                    index: Some(1),
                    hash: None,
                    children: Vec::new(),
                },
            ],
        );

        // Text condition
        assert!(evaluate_condition(&tree, WaitForWhere::Text, "log in"));
        assert!(!evaluate_condition(&tree, WaitForWhere::Text, "Sign Out"));

        // Tag condition
        assert!(evaluate_condition(&tree, WaitForWhere::Tag, "btn_login"));
        assert!(!evaluate_condition(&tree, WaitForWhere::Tag, "nonexistent"));

        // Hidden condition
        assert!(!evaluate_condition(&tree, WaitForWhere::Hidden, "Loading"));
        assert!(evaluate_condition(
            &tree,
            WaitForWhere::Hidden,
            "MissingSpinner"
        ));
    }
}
