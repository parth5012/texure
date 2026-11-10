//! Workflow recording export + replay core (ticket #24).
//!
//! Converts a bridge `/recording/export` trace into (a) a native blackbox scenario
//! (DSL JSON) and (b) a TypeScript codemode script that drives the 6 frozen MCP
//! tools. Also hosts the deterministic replay runner used by the `replay` CLI
//! command and by the L2 record/replay acceptance test.

use crate::client_http::HttpBridgeClient;
use crate::scenario::{Assertion, Scenario, Step};
use serde::Serialize;
use serde_json::{json, Value};

/// Converts a `/recording/export` trace into a native blackbox scenario.
///
/// Mapping (bridge action → DSL step):
/// - `tap`   → `tap {node, rev: "$tree.rev"}` — rev is re-resolved at
///   verify-at-execute time during replay, never pinned from the recording.
/// - `type`  → `type {node, text, rev: "$tree.rev"}`
/// - `keypress BACK` → `back {}`; other keys → `keypress {key}`
/// - `pause` → `wait_for {ms}` — a logical gap; deterministic replay collapses
///   pauses to ordering (no sleeps in scenarios).
///
/// Recorded node indices are positions in the tree captured at record time;
/// replay verifies they still index an existing node before executing.
pub fn export_to_scenario(trace: &Value, name: &str) -> Result<Scenario, String> {
    let actions = trace
        .get("actions")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "trace has no `actions` array".to_string())?;

    let mut steps = vec![Step {
        action: "device_tree".to_string(),
        args: json!({}),
        expect: Some(json!({ "rev": "> 0" })),
    }];

    for action in actions {
        let kind = action.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let node_index = action
            .get("node")
            .and_then(|n| n.get("index"))
            .and_then(|i| i.as_u64());

        let step = match kind {
            "tap" => {
                let idx = node_index.ok_or_else(|| {
                    format!("tap action seq {:?} has no node index", action.get("seq"))
                })?;
                Step {
                    action: "tap".to_string(),
                    args: json!({ "node": idx, "rev": "$tree.rev" }),
                    expect: Some(json!({ "ok": true })),
                }
            }
            "type" => {
                let idx = node_index.ok_or_else(|| {
                    format!("type action seq {:?} has no node index", action.get("seq"))
                })?;
                let text = action.get("value").and_then(|v| v.as_str()).unwrap_or("");
                Step {
                    action: "type".to_string(),
                    args: json!({ "node": idx, "text": text, "rev": "$tree.rev" }),
                    expect: Some(json!({ "ok": true })),
                }
            }
            "keypress" => {
                let key = action
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ENTER");
                if key == "BACK" {
                    Step {
                        action: "back".to_string(),
                        args: json!({}),
                        expect: Some(json!({ "ok": true })),
                    }
                } else {
                    Step {
                        action: "keypress".to_string(),
                        args: json!({ "key": key }),
                        expect: Some(json!({ "ok": true })),
                    }
                }
            }
            "pause" => {
                let ms = action.get("ms").and_then(|v| v.as_u64()).unwrap_or(0);
                Step {
                    action: "wait_for".to_string(),
                    args: json!({ "ms": ms }),
                    expect: Some(json!({ "ok": true })),
                }
            }
            other => return Err(format!("unsupported recorded action: {other}")),
        };
        steps.push(step);
    }

    Ok(Scenario {
        schema_version: 1,
        name: name.to_string(),
        kind: "scenario".to_string(),
        description: Some(format!(
            "Recorded workflow exported from /recording/export (count {})",
            actions.len()
        )),
        tags: vec!["recorded".to_string()],
        steps,
        assertions: vec![Assertion {
            pred: "state".to_string(),
            target: "bridge".to_string(),
            value: Some(json!("IDLE")),
        }],
    })
}

/// Renders a TypeScript codemode script: an ordered sequence of calls to the
/// 6 frozen MCP tools (`device_tree`, `tap`, `type`, `keypress`, `wait_for`,
/// `back`). Node indices and rev placeholders mirror the scenario export;
/// comments carry the recorded node identity for readability.
pub fn export_to_codemode(trace: &Value, name: &str) -> String {
    let actions = trace
        .get("actions")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = String::new();
    out.push_str(&format!(
        "// Texure workflow replay: {name}\n\
         // Generated by `texure-blackbox record` (ticket #24). Tool set frozen at 6:\n\
         // device_tree, tap, type, keypress, wait_for, back.\n\
         // Node indices refer to the tree returned by the device_tree call below;\n\
         // verify-at-execute re-resolves rev from the live tree at replay time.\n\n\
         export async function runWorkflow(mcp: any): Promise<void> {{\n"
    ));
    out.push_str("  await mcp.callTool(\"device_tree\", {});\n");

    for action in &actions {
        let kind = action.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let seq = action.get("seq").and_then(|v| v.as_u64()).unwrap_or(0);
        let t = action.get("t").and_then(|v| v.as_u64()).unwrap_or(0);

        // Human-readable node trail in a comment (identity snapshot).
        let node_comment = action
            .get("node")
            .map(|n| {
                let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("null");
                let text = n.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let cls = n.get("cls").and_then(|v| v.as_str()).unwrap_or("");
                format!("{id} {cls} \"{text}\"")
            })
            .unwrap_or_default();

        match kind {
            "tap" => {
                let idx = action
                    .get("node")
                    .and_then(|n| n.get("index"))
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0);
                out.push_str(&format!(
                    "  // seq {seq} t={t}ms tap {node_comment}\n\
                     await mcp.callTool(\"tap\", {{ \"node\": {idx}, \"rev\": \"$tree.rev\" }});\n"
                ));
            }
            "type" => {
                let idx = action
                    .get("node")
                    .and_then(|n| n.get("index"))
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0);
                let text = action.get("value").and_then(|v| v.as_str()).unwrap_or("");
                out.push_str(&format!(
                    "  // seq {seq} t={t}ms type {node_comment}\n\
                     await mcp.callTool(\"type\", {{ \"node\": {idx}, \"text\": \"{text}\", \"rev\": \"$tree.rev\" }});\n"
                ));
            }
            "keypress" => {
                let key = action
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("ENTER");
                out.push_str(&format!(
                    "  // seq {seq} t={t}ms keypress {key}\n\
                     await mcp.callTool(\"keypress\", {{ \"key\": \"{key}\" }});\n"
                ));
            }
            "pause" => {
                let ms = action.get("ms").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push_str(&format!(
                    "  // seq {seq} t={t}ms pause gap {ms}ms\n\
                     await mcp.callTool(\"wait_for\", {{ \"ms\": {ms} }});\n"
                ));
            }
            _ => {}
        }
    }

    out.push_str("}\n");
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayStepOutcome {
    pub index: usize,
    pub action: String,
    pub ok: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplayReport {
    pub scenario: String,
    pub total: usize,
    pub passed: usize,
    pub pass_rate: f64,
    pub outcomes: Vec<ReplayStepOutcome>,
}

/// Deterministically replays a scenario against `client`: each step is
/// verified at execute time (fresh tree + rev before tap/type, response
/// `ok` checks after). No sleeps, no randomness — same scenario + same
/// bridge state ⇒ same report.
pub async fn replay_scenario(scenario: &Scenario, client: &HttpBridgeClient) -> ReplayReport {
    let mut outcomes = Vec::new();

    for (idx, step) in scenario.steps.iter().enumerate() {
        let outcome = execute_step(step, client).await;
        let ok = outcome.is_ok();
        outcomes.push(ReplayStepOutcome {
            index: idx,
            action: step.action.clone(),
            ok,
            reason: outcome.err(),
        });
    }

    let passed = outcomes.iter().filter(|o| o.ok).count();
    let total = outcomes.len();
    let pass_rate = if total == 0 {
        1.0
    } else {
        passed as f64 / total as f64
    };

    ReplayReport {
        scenario: scenario.name.clone(),
        total,
        passed,
        pass_rate,
        outcomes,
    }
}

async fn execute_step(step: &Step, client: &HttpBridgeClient) -> Result<(), String> {
    match step.action.as_str() {
        "device_tree" => {
            let max_depth = step
                .args
                .get("max_depth")
                .and_then(|d| d.as_u64())
                .map(|d| d as u32);
            client
                .get_ui_tree(max_depth)
                .await
                .map(|_| ())
                .map_err(|e| format!("device_tree failed: {e}"))
        }
        "tap" => {
            let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
            let tree = client
                .get_ui_tree(None)
                .await
                .map_err(|e| format!("verify-at-execute: tree refresh failed: {e}"))?;
            let rev = tree.get("rev").and_then(|r| r.as_u64()).unwrap_or(0);
            let node_count = tree
                .get("nodes")
                .and_then(|n| n.as_array())
                .map(|a| a.len() as u64)
                .unwrap_or(0);
            if node >= node_count {
                return Err(format!(
                    "verify-at-execute: node index {node} out of range (tree has {node_count} nodes)"
                ));
            }
            client
                .tap(node, rev, None)
                .await
                .map_err(|e| format!("tap failed: {e}"))?;
            Ok(())
        }
        "type" => {
            let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
            let text = step.args.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let tree = client
                .get_ui_tree(None)
                .await
                .map_err(|e| format!("verify-at-execute: tree refresh failed: {e}"))?;
            let rev = tree.get("rev").and_then(|r| r.as_u64()).unwrap_or(0);
            let node_count = tree
                .get("nodes")
                .and_then(|n| n.as_array())
                .map(|a| a.len() as u64)
                .unwrap_or(0);
            if node >= node_count {
                return Err(format!(
                    "verify-at-execute: node index {node} out of range (tree has {node_count} nodes)"
                ));
            }
            client
                .type_text(node, text, rev, None)
                .await
                .map_err(|e| format!("type failed: {e}"))?;
            Ok(())
        }
        "keypress" => {
            let key = step
                .args
                .get("key")
                .and_then(|k| k.as_str())
                .unwrap_or("ENTER");
            client
                .keypress(key)
                .await
                .map_err(|e| format!("keypress failed: {e}"))?;
            Ok(())
        }
        "back" => client
            .back()
            .await
            .map(|_| ())
            .map_err(|e| format!("back failed: {e}")),
        "screenshot" => {
            let scale = step
                .args
                .get("scale")
                .and_then(|s| s.as_f64())
                .map(|s| s as f32);
            let bytes = client
                .screenshot(scale, None)
                .await
                .map_err(|e| format!("screenshot failed: {e}"))?;
            if bytes.len() < 8 || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
                return Err("screenshot did not return PNG magic bytes".to_string());
            }
            Ok(())
        }
        "wait_for" => Ok(()), // logical gap: deterministic replay keeps ordering only
        "recording_start" => client
            .recording_start()
            .await
            .map(|_| ())
            .map_err(|e| format!("recording_start failed: {e}")),
        "recording_stop" => client
            .recording_stop()
            .await
            .map(|_| ())
            .map_err(|e| format!("recording_stop failed: {e}")),
        "recording_export" => client
            .recording_export()
            .await
            .map(|_| ())
            .map_err(|e| format!("recording_export failed: {e}")),
        "recording_clear" => client
            .recording_clear()
            .await
            .map(|_| ())
            .map_err(|e| format!("recording_clear failed: {e}")),
        other => Err(format!("unsupported action in replay: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn demo_trace() -> serde_json::Value {
        json!({
            "recording": false,
            "count": 3,
            "actions": [
                {"seq": 0, "t": 10, "action": "tap",
                 "node": {"index": 2, "id": "btn_go", "text": "Go", "cd": null, "cls": "android.widget.Button", "rev": 42}},
                {"seq": 1, "t": 50, "action": "type",
                 "node": {"index": 3, "id": "input_x", "text": null, "cd": null, "cls": "android.widget.EditText", "rev": 42},
                 "value": "hi"},
                {"seq": 2, "t": 80, "action": "keypress", "node": null, "key": "BACK"},
            ]
        })
    }

    #[test]
    fn export_maps_actions_to_scenario_steps() {
        let sc = export_to_scenario(&demo_trace(), "demo").expect("export");

        let names: Vec<&str> = sc.steps.iter().map(|s| s.action.as_str()).collect();
        assert_eq!(names, ["device_tree", "tap", "type", "back"]);

        // tap resolves rev against the fresh tree at verify-at-execute time
        let tap = &sc.steps[1];
        assert_eq!(tap.args["node"], 2);
        assert_eq!(tap.args["rev"], "$tree.rev");

        let typing = &sc.steps[2];
        assert_eq!(typing.args["node"], 3);
        assert_eq!(typing.args["text"], "hi");

        sc.validate().expect("exported scenario must validate");
    }

    #[test]
    fn codemode_script_contains_tool_sequence() {
        let script = export_to_codemode(&demo_trace(), "demo");
        assert!(script.contains("callTool(\"device_tree\""), "{script}");
        assert!(script.contains("callTool(\"tap\""), "{script}");
        assert!(script.contains("callTool(\"type\""), "{script}");
        // BACK keypress maps to the frozen keypress tool
        assert!(
            script.contains("callTool(\"keypress\", { \"key\": \"BACK\" }"),
            "{script}"
        );
    }

    #[tokio::test]
    async fn l2_record_and_replay_against_fake_bridge() {
        use crate::client_http::HttpBridgeClient;
        use crate::fake_bridge::{FakeBridgeConfig, FakeBridgeServer};

        let workspace_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

        // RECORD: run actions against a FakeBridge with recording active.
        let mut source = FakeBridgeServer::start(FakeBridgeConfig {
            scenario_name: "static_screen".to_string(),
            root_dir: workspace_root.to_string(),
            port: 0,
            token: "test-token".to_string(),
        })
        .await
        .expect("fake bridge start");

        let client = HttpBridgeClient::new(
            format!("http://127.0.0.1:{}", source.addr.port()),
            Some("test-token".to_string()),
        );

        client.recording_start().await.expect("recording start");
        client.tap(0, 42, None).await.expect("recorded tap");
        client.tap(1, 42, None).await.expect("recorded tap 2");
        let export = client.recording_export().await.expect("export");
        client.recording_stop().await.expect("recording stop");
        source.stop();

        assert_eq!(export["count"], 2);

        let scenario = export_to_scenario(&export, "recorded_demo").expect("scenario export");
        scenario.validate().expect("valid scenario");

        // REPLAY: deterministic re-execution against a fresh FakeBridge.
        let mut target = FakeBridgeServer::start(FakeBridgeConfig {
            scenario_name: "static_screen".to_string(),
            root_dir: workspace_root.to_string(),
            port: 0,
            token: "test-token".to_string(),
        })
        .await
        .expect("replay target start");

        let tclient = HttpBridgeClient::new(
            format!("http://127.0.0.1:{}", target.addr.port()),
            Some("test-token".to_string()),
        );

        let report = replay_scenario(&scenario, &tclient).await;
        target.stop();

        assert_eq!(report.total, 3);
        assert_eq!(report.passed, 3, "report: {report:?}");
        assert_eq!(report.pass_rate, 1.0);
    }
}
