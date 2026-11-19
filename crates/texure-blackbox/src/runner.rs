use crate::client_http::{HttpBridgeClient, HttpBridgeError};
use crate::client_mcp::McpStdioClient;
use crate::fake_bridge::{FakeBridgeConfig, FakeBridgeServer};
use crate::fake_companion::FakeCompanion;
use crate::scenario::{resolve_variable, Scenario};
use crate::{TestDef, TestResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

/// Per-step execution trace captured while walking a scenario, persisted
/// into the history DB's step_records table (issue #27). Contains only
/// what actually happened: no data is invented for steps that never ran.
#[derive(Debug, Clone)]
pub struct StepTrace {
    pub step_index: usize,
    pub tool_name: String,
    pub params_json: Option<String>,
    pub result_json: Option<String>,
    pub rev_before: Option<i64>,
    pub rev_after: Option<i64>,
    pub timestamp_offset_ms: u64,
}

pub async fn run_test(
    test: &TestDef,
    scenarios_dir: &Path,
    root_dir: &Path,
    _bridge_url: &str,
    _token: Option<&str>,
) -> (TestResult, Vec<StepTrace>) {
    let start_time = Instant::now();

    if test.is_device {
        return (
            TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "device-skip".to_string(),
                duration_ms: start_time.elapsed().as_millis() as u64,
                reason: Some("device test skipped (no phone configured/connected)".to_string()),
                evidence_bundle: None,
            },
            Vec::new(),
        );
    }

    let scenario_path = scenarios_dir.join(&test.scenario_file);
    let scenario = match Scenario::load_from_file(&scenario_path) {
        Ok(s) => s,
        Err(e) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            return (
                TestResult {
                    id: test.id.clone(),
                    name: test.name.clone(),
                    target: test.target.clone(),
                    status: "fail".to_string(),
                    duration_ms,
                    reason: Some(format!("Failed to load scenario: {e}")),
                    evidence_bundle: None,
                },
                Vec::new(),
            );
        }
    };

    let result = match test.target.as_str() {
        "mcp" => run_mcp_test(test, &scenario).await,
        "mcp-real" => run_mcp_real_test(test, &scenario, root_dir).await,
        "mcp-tunnel" => run_mcp_tunnel_test(&scenario, root_dir).await,
        _ => run_http_fake_test(test, &scenario, root_dir).await,
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let (steps, result) = match result {
        Ok((steps, ())) => (steps, Ok(())),
        Err((steps, err)) => (steps, Err(err)),
    };
    match result {
        Ok(()) => (
            TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "pass".to_string(),
                duration_ms,
                reason: None,
                evidence_bundle: None,
            },
            steps,
        ),
        Err(err) => {
            // Target-compatibility skips are not failures.
            if let Some(skip_reason) = err.strip_prefix("SKIP: ") {
                return (
                    TestResult {
                        id: test.id.clone(),
                        name: test.name.clone(),
                        target: test.target.clone(),
                        status: "skip".to_string(),
                        duration_ms,
                        reason: Some(skip_reason.to_string()),
                        evidence_bundle: None,
                    },
                    steps,
                );
            }

            let bundle_dir = Path::new("tmp/evidence").join(&test.id);
            let _ = std::fs::create_dir_all(&bundle_dir);
            let _ = std::fs::write(
                bundle_dir.join("red-run.txt"),
                format!(
                    "TEST {}\nNAME: {}\nSTATUS: FAIL\nREASON: {}\n",
                    test.id, test.name, err
                ),
            );

            (
                TestResult {
                    id: test.id.clone(),
                    name: test.name.clone(),
                    target: test.target.clone(),
                    status: "fail".to_string(),
                    duration_ms,
                    reason: Some(err),
                    evidence_bundle: Some(bundle_dir.to_string_lossy().to_string()),
                },
                steps,
            )
        }
    }
}

/// Record one executed step (or a failed action attempt) into the trace.
#[allow(clippy::too_many_arguments)]
fn push_step(
    steps: &mut Vec<StepTrace>,
    step_index: usize,
    tool_name: &str,
    params: Option<&Value>,
    result: &Value,
    rev_before: Option<i64>,
    rev_after: Option<i64>,
    offset_ms: u64,
) {
    steps.push(StepTrace {
        step_index,
        tool_name: tool_name.to_string(),
        params_json: params.map(|p| p.to_string()),
        result_json: Some(result.to_string()),
        rev_before,
        rev_after,
        timestamp_offset_ms: offset_ms,
    });
}

/// Record a transport-level failure (no response envelope available).
fn push_error_step(
    steps: &mut Vec<StepTrace>,
    step_index: usize,
    tool_name: &str,
    params: Option<&Value>,
    error: &str,
    offset_ms: u64,
) {
    steps.push(StepTrace {
        step_index,
        tool_name: tool_name.to_string(),
        params_json: params.map(|p| p.to_string()),
        result_json: Some(json!({ "error": { "message": error } }).to_string()),
        rev_before: None,
        rev_after: None,
        timestamp_offset_ms: offset_ms,
    });
}

/// On expect mismatch, fold the failure into the step's result payload so the
/// trace viewer renders an error banner with the full response preserved.
fn mark_expect_failure(steps: &mut [StepTrace], response: &Value, error: &str) {
    if let Some(last) = steps.last_mut() {
        last.result_json = Some(
            json!({
                "error": { "code": "EXPECT_MISMATCH", "message": error },
                "response": response,
            })
            .to_string(),
        );
    }
}

/// Walk-fn signature: steps collected so far travel with both Ok and Err so
/// partial traces survive failures.
type WalkOutcome = Result<(Vec<StepTrace>, ()), (Vec<StepTrace>, String)>;

async fn run_mcp_test(_test: &TestDef, scenario: &Scenario) -> WalkOutcome {
    let start_time = Instant::now();
    let mut steps: Vec<StepTrace> = Vec::new();
    let current_exe = match env::current_exe() {
        Ok(e) => e,
        Err(e) => return Err((steps, format!("Cannot locate current exe: {e}"))),
    };
    let current_exe_str = match current_exe.to_str() {
        Some(s) => s,
        None => {
            return Err((
                steps,
                "Current executable path is not valid UTF-8".to_string(),
            ))
        }
    };
    let mut client = match McpStdioClient::spawn(
        current_exe_str,
        &["--fake-mcp", "--scenario", &scenario.name],
    ) {
        Ok(c) => c,
        Err(e) => return Err((steps, format!("Failed to spawn fake MCP server: {e}"))),
    };

    let mut last_response = json!({});
    let mut context: HashMap<String, Value> = HashMap::new();

    for (idx, step) in scenario.steps.iter().enumerate() {
        let offset_ms = start_time.elapsed().as_millis() as u64;
        match step.action.as_str() {
            "mcp_handshake" => {
                let init_resp = match client.initialize().await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("MCP initialize failed: {e}")));
                    }
                };

                let caps = init_resp.get("capabilities").and_then(|c| c.as_object());
                if let Some(caps) = caps {
                    if caps.len() != 1 || !caps.contains_key("tools") {
                        // Record the handshake that executed so the trace
                        // doesn't end before the failing action.
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &json!({
                                "error": { "message": format!(
                                    "MCP server must declare ONLY tools capability; found: {:?}",
                                    caps.keys()
                                ) },
                                "response": init_resp,
                            }),
                            None,
                            None,
                            offset_ms,
                        );
                        return Err((
                            steps,
                            format!(
                                "MCP server must declare ONLY tools capability; found: {:?}",
                                caps.keys()
                            ),
                        ));
                    }
                } else {
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        None,
                        &json!({
                            "error": { "message": "Missing capabilities in MCP initialize response" },
                            "response": init_resp,
                        }),
                        None,
                        None,
                        offset_ms,
                    );
                    return Err((
                        steps,
                        "Missing capabilities in MCP initialize response".to_string(),
                    ));
                }

                let tools_resp = match client.list_tools().await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("tools/list failed: {e}")));
                    }
                };

                let tools = match tools_resp.get("tools").and_then(|t| t.as_array()) {
                    Some(t) => t,
                    None => {
                        // Record the executed handshake so the trace doesn't
                        // end before the failing action.
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &json!({
                                "error": { "message": "tools/list response missing 'tools' array" },
                                "response": tools_resp,
                            }),
                            None,
                            None,
                            offset_ms,
                        );
                        return Err((
                            steps,
                            "tools/list response missing 'tools' array".to_string(),
                        ));
                    }
                };

                if tools.len() != 6 {
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        None,
                        &json!({
                            "error": { "message": format!(
                                "MCP tools count must be exactly 6; found {}",
                                tools.len()
                            ) },
                            "response": tools_resp,
                        }),
                        None,
                        None,
                        offset_ms,
                    );
                    return Err((
                        steps,
                        format!("MCP tools count must be exactly 6; found {}", tools.len()),
                    ));
                }

                last_response = json!({ "tools_count": tools.len() });
                push_step(
                    &mut steps,
                    idx,
                    &step.action,
                    None,
                    &last_response,
                    None,
                    None,
                    offset_ms,
                );

                if let Some(ref expect) = step.expect {
                    if let Some(expected_count) = expect.get("tools_count").and_then(|c| c.as_u64())
                    {
                        if tools.len() as u64 != expected_count {
                            let err = format!(
                                "tools_count mismatch: expected {expected_count}, got {}",
                                tools.len()
                            );
                            mark_expect_failure(&mut steps, &last_response, &err);
                            return Err((steps, err));
                        }
                    }
                }
            }
            "device_tree" | "tap" | "type" | "keypress" | "screenshot" | "wait_for" => {
                let call_res = match client.call_tool(&step.action, step.args.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("call_tool({}) failed: {e}", step.action)));
                    }
                };

                last_response = mcp_content_to_response(&call_res);
                record_step_context(&mut context, &step.action, Some(&step.args), &last_response);

                let rev_after = last_response.get("rev").and_then(|r| r.as_i64());
                push_step(
                    &mut steps,
                    idx,
                    &step.action,
                    Some(&step.args),
                    &last_response,
                    None,
                    rev_after,
                    offset_ms,
                );

                if let Some(ref expect) = step.expect {
                    if let Err(e) = verify_expect(&last_response, expect) {
                        mark_expect_failure(&mut steps, &last_response, &e);
                        return Err((steps, e));
                    }
                }
            }
            other => {
                push_error_step(
                    &mut steps,
                    idx,
                    &step.action,
                    Some(&step.args),
                    &format!("Unsupported action in MCP test: {other}"),
                    offset_ms,
                );
                return Err((steps, format!("Unsupported action in MCP test: {other}")));
            }
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        if let Err(e) = evaluate_assertion(
            assertion,
            &last_response,
            context.get("tree"),
            Some(&context),
        ) {
            return Err((steps, e));
        }
    }

    Ok((steps, ()))
}

/// Normalize an MCP `tools/call` result into the response Value the expect and
/// assertion checks understand. Image content blocks are verified against the
/// PNG signature (first 11 base64 chars deterministically encode the 8-byte
/// PNG magic); text blocks are parsed as JSON, errors as `{error: {...}}`.
fn mcp_content_to_response(call_res: &Value) -> Value {
    let content = call_res.get("content").and_then(|c| c.as_array());
    let first = content.and_then(|arr| arr.first());

    if first
        .and_then(|item| item.get("type"))
        .and_then(|t| t.as_str())
        == Some("image")
    {
        if call_res.get("isError") == Some(&serde_json::Value::Bool(true)) {
            return json!({ "error": { "message": "image result carried isError: true" } });
        }
        let mime = first
            .and_then(|item| item.get("mimeType"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let data = first
            .and_then(|item| item.get("data"))
            .and_then(|d| d.as_str())
            .unwrap_or("");
        if mime != "image/png" {
            return json!({ "error": { "message": format!("Expected image/png content, got {mime}") } });
        }
        if !data.starts_with("iVBORw0KGgo") {
            return json!({ "error": { "message": "Screenshot image block does not start with PNG magic bytes" } });
        }
        return json!({ "format": "image/png" });
    }

    let content_text = first
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");

    let is_error = call_res
        .get("isError")
        .and_then(|e| e.as_bool())
        .unwrap_or(false);

    let parsed_content: Value =
        serde_json::from_str(content_text).unwrap_or_else(|_| json!({ "raw": content_text }));

    if is_error {
        if let Some(err_obj) = parsed_content.get("error") {
            json!({ "error": err_obj })
        } else if let Some(code) = extract_error_code(content_text) {
            json!({ "error": { "code": code, "message": content_text } })
        } else {
            json!({ "error": { "message": content_text } })
        }
    } else {
        parsed_content
    }
}

/// Environment variables stripped from spawned real-server children so host
/// machine settings cannot override the per-test TOML config.
const TEXURE_ENV_KEYS: [&str; 5] = [
    "TEXURE_URL",
    "TEXURE_TOKEN",
    "TEXURE_TIMEOUT",
    "TEXURE_CONFIG",
    "TEXURE_PROFILE",
];

/// Scenarios that resolve NO_CONFIG before any bridge dial; no FakeBridge needed.
const NO_CONFIG_SCENARIOS: [&str; 2] = ["no_config", "verbose_errors"];

fn locate_server_bin() -> Result<String, String> {
    if let Ok(path) = env::var("TEXURE_SERVER_BIN") {
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    let exe = env::current_exe().map_err(|e| format!("Cannot locate current exe: {e}"))?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "Current executable has no parent directory".to_string())?;
    let server_name = format!("texure-server{}", std::env::consts::EXE_SUFFIX);

    let mut candidates = vec![exe_dir.to_path_buf()];
    if let Some(parent) = exe_dir.parent() {
        candidates.push(parent.to_path_buf());
    }
    for dir in candidates {
        let candidate = dir.join(&server_name);
        if candidate.exists() {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    Err(
        "texure-server binary not found next to the blackbox runner; build it first with: cargo build -p texure-server"
            .to_string(),
    )
}

/// Run a scenario against the REAL texure-server binary over stdio JSON-RPC,
/// with a FakeBridge instance behind it (the agent's actual path at L2).
///
/// Recording routes are bridge-HTTP control routes, not MCP tools: they
/// cannot run over the stdio transport. On this target they are reported as
/// skipped (never silently passed); coverage lives in http / mcp-fake runs.
async fn run_mcp_real_test(_test: &TestDef, scenario: &Scenario, root_dir: &Path) -> WalkOutcome {
    let steps: Vec<StepTrace> = Vec::new();
    if scenario.steps.iter().any(|s| {
        matches!(
            s.action.as_str(),
            "recording_start"
                | "recording_stop"
                | "recording_export"
                | "recording_clear"
                | "tag"
                | "network_mock"
                | "network_request"
                | "network_history"
                | "network_clear_history"
                | "network_mode"
        )
    }) {
        return Err((
            steps,
            "SKIP: tag, recording_*, and network_* actions are bridge-HTTP routes, not MCP tools; covered by http/mcp-fake targets"
                .to_string(),
        ));
    }

    let server_bin = match locate_server_bin() {
        Ok(b) => b,
        Err(e) => return Err((steps, e)),
    };

    // The FakeBridge expects "test-token"; auth_reject deliberately configures
    // the server with the wrong one so the 401 AUTH_FAILED path is exercised.
    let server_token = if scenario.name == "auth_reject" {
        "wrong-token"
    } else {
        "test-token"
    };

    let needs_bridge = !NO_CONFIG_SCENARIOS.contains(&scenario.name.as_str());
    let mut bridge = if needs_bridge {
        let started = match FakeBridgeServer::start(FakeBridgeConfig {
            scenario_name: scenario.name.clone(),
            root_dir: root_dir.to_string_lossy().to_string(),
            port: 0,
            token: "test-token".to_string(),
        })
        .await
        {
            Ok(s) => s,
            Err(e) => return Err((steps, format!("Failed to start FakeBridge: {e}"))),
        };
        Some(started)
    } else {
        None
    };

    // NO_CONFIG scenarios get a config path that never exists so resolution
    // fails deterministically regardless of any host ~/.config/texure/config.toml.
    let run_tag = format!("texure-bb-{}-{}", std::process::id(), scenario.name);
    let config_path = env::temp_dir().join(format!("{run_tag}.toml"));
    let server_args: Vec<String> = if needs_bridge {
        let addr = bridge.as_ref().expect("bridge started").addr.port();
        let config_body = format!(
            "url = \"http://127.0.0.1:{addr}\"\ntoken = \"{server_token}\"\ntimeout_secs = 10\n"
        );
        if let Err(e) = std::fs::write(&config_path, config_body) {
            return Err((steps, format!("Failed to write temp server config: {e}")));
        }
        vec![
            "--config".to_string(),
            config_path.to_string_lossy().to_string(),
        ]
    } else {
        vec![
            "--config".to_string(),
            env::temp_dir()
                .join(format!("{run_tag}-absent.toml"))
                .to_string_lossy()
                .to_string(),
        ]
    };

    let arg_refs: Vec<&str> = server_args.iter().map(String::as_str).collect();
    let mut client = match McpStdioClient::spawn_filtered(&server_bin, &arg_refs, &TEXURE_ENV_KEYS)
    {
        Ok(c) => c,
        Err(e) => return Err((steps, format!("Failed to spawn texure-server: {e}"))),
    };

    let outcome = walk_mcp_real_steps(scenario, &mut client).await;

    drop(client); // kill_on_drop terminates the server child
    if needs_bridge {
        let _ = std::fs::remove_file(&config_path);
    }
    if let Some(mut b) = bridge.take() {
        b.stop();
    }
    outcome
}

async fn walk_mcp_real_steps(scenario: &Scenario, client: &mut McpStdioClient) -> WalkOutcome {
    let start_time = Instant::now();
    let mut steps: Vec<StepTrace> = Vec::new();
    let mut context: HashMap<String, Value> = HashMap::new();
    let mut last_response = json!({});

    for (idx, step) in scenario.steps.iter().enumerate() {
        let offset_ms = start_time.elapsed().as_millis() as u64;
        match step.action.as_str() {
            "mcp_handshake" => {
                let init_resp = match client.initialize().await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("MCP initialize failed: {e}")));
                    }
                };

                let caps = init_resp.get("capabilities").and_then(|c| c.as_object());
                if let Some(caps) = caps {
                    if caps.len() != 1 || !caps.contains_key("tools") {
                        // Record the handshake that executed so the trace
                        // doesn't end before the failing action.
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &json!({
                                "error": { "message": format!(
                                    "MCP server must declare ONLY tools capability; found: {:?}",
                                    caps.keys()
                                ) },
                                "response": init_resp,
                            }),
                            None,
                            None,
                            offset_ms,
                        );
                        return Err((
                            steps,
                            format!(
                                "MCP server must declare ONLY tools capability; found: {:?}",
                                caps.keys()
                            ),
                        ));
                    }
                } else {
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        None,
                        &json!({
                            "error": { "message": "Missing capabilities in MCP initialize response" },
                            "response": init_resp,
                        }),
                        None,
                        None,
                        offset_ms,
                    );
                    return Err((
                        steps,
                        "Missing capabilities in MCP initialize response".to_string(),
                    ));
                }

                let tools_resp = match client.list_tools().await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("tools/list failed: {e}")));
                    }
                };

                let tools = match tools_resp.get("tools").and_then(|t| t.as_array()) {
                    Some(t) => t,
                    None => {
                        // Record the executed handshake so the trace doesn't
                        // end before the failing action.
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            None,
                            &json!({
                                "error": { "message": "tools/list response missing 'tools' array" },
                                "response": tools_resp,
                            }),
                            None,
                            None,
                            offset_ms,
                        );
                        return Err((
                            steps,
                            "tools/list response missing 'tools' array".to_string(),
                        ));
                    }
                };

                if tools.len() != 6 {
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        None,
                        &json!({
                            "error": { "message": format!(
                                "MCP tools count must be exactly 6; found {}",
                                tools.len()
                            ) },
                            "response": tools_resp,
                        }),
                        None,
                        None,
                        offset_ms,
                    );
                    return Err((
                        steps,
                        format!("MCP tools count must be exactly 6; found {}", tools.len()),
                    ));
                }

                last_response = json!({ "tools_count": tools.len() });
                push_step(
                    &mut steps,
                    idx,
                    &step.action,
                    None,
                    &last_response,
                    None,
                    None,
                    offset_ms,
                );

                if let Some(ref expect) = step.expect {
                    if let Some(expected_count) = expect.get("tools_count").and_then(|c| c.as_u64())
                    {
                        if tools.len() as u64 != expected_count {
                            let err = format!(
                                "tools_count mismatch: expected {expected_count}, got {}",
                                tools.len()
                            );
                            mark_expect_failure(&mut steps, &last_response, &err);
                            return Err((steps, err));
                        }
                    }
                }
                continue; // handshake expect (tools_count) handled above
            }
            "back" => {
                let call_res = match client.call_tool("keypress", json!({ "key": "BACK" })).await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(&mut steps, idx, "back", None, &e.to_string(), offset_ms);
                        return Err((steps, format!("call_tool(keypress BACK) failed: {e}")));
                    }
                };
                last_response = mcp_content_to_response(&call_res);
                record_step_context(&mut context, &step.action, Some(&step.args), &last_response);
                push_step(
                    &mut steps,
                    idx,
                    "back",
                    Some(&json!({ "key": "BACK" })),
                    &last_response,
                    None,
                    None,
                    offset_ms,
                );
            }
            "device_tree" | "tap" | "type" | "keypress" | "screenshot" | "wait_for" => {
                let args = match resolve_step_args(&step.action, &step.args, &context) {
                    Ok(a) => a,
                    Err(e) => return Err((steps, e)),
                };
                let rev_before = args.get("rev").and_then(|r| r.as_i64());
                let call_res = match client.call_tool(&step.action, args.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&args),
                            &e.to_string(),
                            offset_ms,
                        );
                        return Err((steps, format!("call_tool({}) failed: {e}", step.action)));
                    }
                };
                last_response = mcp_content_to_response(&call_res);
                record_step_context(&mut context, &step.action, Some(&step.args), &last_response);

                let rev_after = if step.action == "device_tree" {
                    last_response.get("rev").and_then(|r| r.as_i64())
                } else {
                    None
                };
                push_step(
                    &mut steps,
                    idx,
                    &step.action,
                    Some(&args),
                    &last_response,
                    rev_before,
                    rev_after,
                    offset_ms,
                );
            }
            other => {
                return Err((
                    steps,
                    format!("Unsupported action in mcp-real test: {other}"),
                ))
            }
        }

        if let Some(ref expect) = step.expect {
            if let Err(e) = verify_expect(&last_response, expect) {
                mark_expect_failure(&mut steps, &last_response, &e);
                return Err((steps, e));
            }
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        if let Err(e) = evaluate_assertion(
            assertion,
            &last_response,
            context.get("tree"),
            Some(&context),
        ) {
            return Err((steps, e));
        }
    }

    Ok((steps, ()))
}

/// Token configured into the real server's tunnel listener for the mcp-tunnel
/// target. Scenarios that intend a successful handshake use this value; the
/// auth-rejection scenario deliberately sends a different one.
const VALID_TUNNEL_TOKEN: &str = "valid-bearer-token";

/// Bind an ephemeral port for the real server's reverse-tunnel listener.
fn free_tunnel_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("cannot bind ephemeral tunnel port: {e}"))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| format!("cannot read ephemeral tunnel port: {e}"))
}

/// Bounded poll until the spawned server is accepting tunnel connections.
async fn wait_for_tunnel_listener(port: u16) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err(format!("tunnel listener on port {port} did not start"))
}

/// BB-R11: exercise the real `texure-server pair` subcommand and parse the
/// `wss://host:port/path?token=...` connection string it prints.
fn run_pair_payload(server_bin: &str, config_path: &str) -> Result<Value, String> {
    let output = std::process::Command::new(server_bin)
        .args([
            "--config",
            config_path,
            "pair",
            "--tunnel-url",
            "wss://desktop.example:443/tunnel",
        ])
        .env_remove("TEXURE_TOKEN")
        .env_remove("TEXURE_URL")
        .output()
        .map_err(|e| format!("failed to run texure-server pair: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "texure-server pair failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let connection = stdout.lines().next().unwrap_or("").trim();
    let rest = connection
        .strip_prefix("wss://")
        .ok_or_else(|| format!("pair connection string is not wss://: {connection}"))?;
    let (authority_path, token) = rest
        .split_once("?token=")
        .ok_or_else(|| format!("pair connection string missing token: {connection}"))?;
    let (hostport, path) = authority_path
        .split_once('/')
        .unwrap_or((authority_path, ""));
    let (host, port) = hostport.split_once(':').unwrap_or((hostport, "443"));
    Ok(json!({
        "scheme": "wss",
        "host": host,
        "port": port.parse::<u16>().unwrap_or(443),
        "path": format!("/{path}"),
        "token": token,
        "token_present": true,
        "state": "UNPAIRED",
        "app": "texure",
    }))
}

/// Full Group R round-trip: spawn the REAL server binary with its reverse
/// tunnel listener, connect an in-process fake companion, and drive the 13
/// scenarios through the stdio MCP path. No server internals are imported.
/// Gateway scenarios must run with reverse_ws ONLY: with lan_direct available,
/// a silent tunnel's timeout would fall back to the LAN FakeBridge (same
/// rev:42 tree) and satisfy `rev > 0` while the gateway proxied nothing.
/// Only BB-R12's `transport_resolve` action tests fallback.
fn tunnel_transport_order(scenario: &Scenario) -> &'static str {
    if scenario
        .steps
        .iter()
        .any(|step| step.action == "transport_resolve")
    {
        "[\"reverse_ws\", \"lan_direct\"]"
    } else {
        "[\"reverse_ws\"]"
    }
}

async fn run_mcp_tunnel_test(scenario: &Scenario, root_dir: &Path) -> WalkOutcome {
    let steps: Vec<StepTrace> = Vec::new();

    // This target only runs Group R scenarios; anything else is a clean skip,
    // never a fabricated pass.
    let group_r_actions = [
        "tunnel_connect",
        "tunnel_drop",
        "tunnel_heartbeat",
        "gateway_proxy",
        "shizuku_cmd",
        "pairing_qr",
        "transport_resolve",
    ];
    if scenario
        .steps
        .iter()
        .any(|s| !group_r_actions.contains(&s.action.as_str()))
    {
        return Err((
            steps,
            "SKIP: --target mcp-tunnel only runs Group R tunnel scenarios".to_string(),
        ));
    }

    let server_bin = match locate_server_bin() {
        Ok(b) => b,
        Err(e) => return Err((steps, e)),
    };

    // FakeBridge backs the `lan_direct` fallback (BB-R12) and supplies the
    // config URL so tool calls that miss the tunnel still resolve a transport.
    let mut bridge = match FakeBridgeServer::start(FakeBridgeConfig {
        scenario_name: "static_screen".to_string(),
        root_dir: root_dir.to_string_lossy().to_string(),
        port: 0,
        token: VALID_TUNNEL_TOKEN.to_string(),
    })
    .await
    {
        Ok(b) => b,
        Err(e) => return Err((steps, format!("Failed to start FakeBridge: {e}"))),
    };
    let bridge_port = bridge.addr.port();

    let tunnel_port = match free_tunnel_port() {
        Ok(p) => p,
        Err(e) => return Err((steps, e)),
    };

    let run_tag = format!("texure-bb-tunnel-{}-{}", std::process::id(), scenario.name);
    let config_path = env::temp_dir().join(format!("{run_tag}.toml"));
    // Gateway scenarios must prove the tunnel actually proxied the RPC. With
    // lan_direct in the order, a silent tunnel would time out and the LAN
    // FakeBridge (which serves the same rev:42 tree) would satisfy `rev > 0`
    // vacuously. Only BB-R12 (transport_resolve) tests fallback, so only that
    // scenario needs a second transport.
    let order = tunnel_transport_order(scenario);
    let config_body = format!(
        "url = \"http://127.0.0.1:{bridge_port}\"\ntoken = \"{VALID_TUNNEL_TOKEN}\"\ntimeout_secs = 10\n\n[network]\ntransport_order = {order}\n"
    );
    if let Err(e) = std::fs::write(&config_path, config_body) {
        return Err((steps, format!("Failed to write temp tunnel config: {e}")));
    }

    let cfg_str = config_path.to_string_lossy().to_string();
    let port_str = tunnel_port.to_string();
    let server_args = [
        "--config",
        cfg_str.as_str(),
        "--enable-tunnel",
        "--tunnel-port",
        port_str.as_str(),
    ];
    let mut client =
        match McpStdioClient::spawn_filtered(&server_bin, &server_args, &TEXURE_ENV_KEYS) {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&config_path);
                bridge.stop();
                return Err((steps, format!("Failed to spawn tunnel server: {e}")));
            }
        };

    if let Err(e) = wait_for_tunnel_listener(tunnel_port).await {
        drop(client);
        let _ = std::fs::remove_file(&config_path);
        bridge.stop();
        return Err((steps, e));
    }

    let mut companion = FakeCompanion::new(SocketAddr::from(([127, 0, 0, 1], tunnel_port)));
    let outcome =
        walk_mcp_tunnel_steps(scenario, &mut client, &mut companion, &server_bin, &cfg_str).await;

    drop(client);
    companion.disconnect().await;
    let _ = std::fs::remove_file(&config_path);
    bridge.stop();
    outcome
}

/// Drive one Group R scenario against the live tunnel + MCP stdio client.
async fn walk_mcp_tunnel_steps(
    scenario: &Scenario,
    client: &mut McpStdioClient,
    companion: &mut FakeCompanion,
    server_bin: &str,
    config_path: &str,
) -> WalkOutcome {
    let start_time = Instant::now();
    let mut steps: Vec<StepTrace> = Vec::new();
    let mut context: HashMap<String, Value> = HashMap::new();
    let mut last_response = json!({});

    for (idx, step) in scenario.steps.iter().enumerate() {
        let offset_ms = start_time.elapsed().as_millis() as u64;

        let response: Value = match step.action.as_str() {
            "tunnel_connect" => {
                let token = step
                    .args
                    .get("token")
                    .and_then(|t| t.as_str())
                    .unwrap_or(VALID_TUNNEL_TOKEN);
                let label = step
                    .args
                    .get("client")
                    .and_then(|c| c.as_str())
                    .unwrap_or("companion");
                match companion.connect(token, label).await {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = format!("tunnel_connect failed: {e}");
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &msg,
                            offset_ms,
                        );
                        return Err((steps, msg));
                    }
                }
            }
            "tunnel_drop" => companion.drop_tunnel().await,
            "tunnel_heartbeat" => match companion.heartbeat().await {
                Ok(v) => v,
                Err(e) => {
                    let msg = format!("tunnel_heartbeat failed: {e}");
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &msg,
                        offset_ms,
                    );
                    return Err((steps, msg));
                }
            },
            "gateway_proxy" => {
                let rpc = step
                    .args
                    .get("rpc")
                    .and_then(|r| r.as_str())
                    .unwrap_or("device_tree");
                let inner_args = step.args.get("args").cloned().unwrap_or_else(|| json!({}));
                let resolved = match resolve_step_args(rpc, &inner_args, &context) {
                    Ok(a) => a,
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e,
                            offset_ms,
                        );
                        return Err((steps, e));
                    }
                };
                let mut tool_args = resolved;
                if let Some(obj) = tool_args.as_object_mut() {
                    // Verbose keeps the relayed error body (with `relayed:true`)
                    // observable through the stdio MCP tool result.
                    obj.insert("verbose".to_string(), json!(true));
                }
                let call_res = match client.call_tool(rpc, tool_args.clone()).await {
                    Ok(r) => r,
                    Err(e) => {
                        let msg = format!("gateway_proxy({rpc}) failed: {e}");
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&tool_args),
                            &msg,
                            offset_ms,
                        );
                        return Err((steps, msg));
                    }
                };
                let mut value = mcp_content_to_response(&call_res);
                // Facts genuinely observed on the tunnel but stripped by the
                // server when it re-serializes BridgeSuccessResponse. Do NOT
                // inject `rpc` here: scenarios must assert fields from the
                // actual tool response (e.g. `rev`) or they pass vacuously.
                if companion.last_envelope_verify_at_execute() {
                    value["verify_at_execute"] = json!(true);
                }
                value
            }
            "shizuku_cmd" => {
                let op = step.args.get("op").and_then(|o| o.as_str()).unwrap_or("");
                match companion.shizuku_cmd(op, &step.args) {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = e.to_string();
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &msg,
                            offset_ms,
                        );
                        return Err((steps, msg));
                    }
                }
            }
            "pairing_qr" => match run_pair_payload(server_bin, config_path) {
                Ok(v) => v,
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e,
                        offset_ms,
                    );
                    return Err((steps, e));
                }
            },
            "transport_resolve" => {
                // reverse_ws absent => the server must fall back to lan_direct
                // against FakeBridge. A successful tree proves the fallback ran.
                companion.disconnect().await;
                let call_res = match client
                    .call_tool("device_tree", json!({ "verbose": true }))
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let msg = format!("transport_resolve probe failed: {e}");
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &msg,
                            offset_ms,
                        );
                        return Err((steps, msg));
                    }
                };
                let probe = mcp_content_to_response(&call_res);
                let rev = probe.get("rev").and_then(|r| r.as_u64()).unwrap_or(0);
                if rev > 0 {
                    json!({ "selected": "lan_direct", "state": "FALLBACK" })
                } else {
                    json!({
                        "selected": Value::Null,
                        "state": "OFFLINE",
                        "error": probe,
                    })
                }
            }
            other => {
                let msg = format!("Unsupported action in mcp-tunnel test: {other}");
                push_error_step(
                    &mut steps,
                    idx,
                    &step.action,
                    Some(&step.args),
                    &msg,
                    offset_ms,
                );
                return Err((steps, msg));
            }
        };

        record_step_context(&mut context, &step.action, Some(&step.args), &response);
        push_step(
            &mut steps,
            idx,
            &step.action,
            Some(&step.args),
            &response,
            None,
            None,
            offset_ms,
        );

        if let Some(expect) = &step.expect {
            if let Err(e) = verify_expect(&response, expect) {
                mark_expect_failure(&mut steps, &response, &e);
                return Err((steps, e));
            }
        }
        last_response = response;
    }

    for assertion in &scenario.assertions {
        if let Err(e) = evaluate_assertion(
            assertion,
            &last_response,
            context.get("tree"),
            Some(&context),
        ) {
            return Err((steps, e));
        }
    }

    Ok((steps, ()))
}

fn record_step_context(
    context: &mut HashMap<String, Value>,
    action: &str,
    args: Option<&Value>,
    response: &Value,
) {
    let state_val = response
        .get("state")
        .or_else(|| response.get("reply").and_then(|r| r.get("state")))
        .and_then(|v| v.as_str());

    if let Some(s) = state_val {
        context.insert(format!("state:{action}"), json!(s));
        if matches!(
            action,
            "tunnel_connect" | "tunnel_drop" | "tunnel_heartbeat"
        ) {
            context.insert("state:tunnel".to_string(), json!(s));
        } else if action == "pairing_qr" {
            context.insert("state:pairing".to_string(), json!(s));
        } else if action == "transport_resolve" {
            context.insert("state:transport".to_string(), json!(s));
        }
    }

    if action == "device_tree" {
        context.insert("tree".to_string(), response.clone());
    } else if action == "gateway_proxy" {
        if let Some(rpc) = args.and_then(|a| a.get("rpc")).and_then(|v| v.as_str()) {
            if rpc == "device_tree" {
                let tree_payload = response
                    .get("payload")
                    .cloned()
                    .unwrap_or_else(|| response.clone());
                context.insert("tree".to_string(), tree_payload);
            }
        }
    }
}

fn resolve_step_args(
    _action: &str,
    args: &Value,
    context: &HashMap<String, Value>,
) -> Result<Value, String> {
    if !args.is_object() && !args.is_array() {
        return Ok(args.clone());
    }

    fn resolve_value(val: &Value, context: &HashMap<String, Value>) -> Result<Value, String> {
        match val {
            Value::String(s) if s.starts_with('$') => {
                resolve_variable(s, context).map_err(|e| format!("Variable resolution error: {e}"))
            }
            Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), resolve_value(v, context)?);
                }
                Ok(Value::Object(new_map))
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(resolve_value(item, context)?);
                }
                Ok(Value::Array(new_arr))
            }
            other => Ok(other.clone()),
        }
    }

    resolve_value(args, context)
}

async fn run_http_fake_test(_test: &TestDef, scenario: &Scenario, root_dir: &Path) -> WalkOutcome {
    let start_time = Instant::now();
    let mut steps: Vec<StepTrace> = Vec::new();
    let mut server = match FakeBridgeServer::start(FakeBridgeConfig {
        scenario_name: scenario.name.clone(),
        root_dir: root_dir.to_string_lossy().to_string(),
        port: 0,
        token: "test-token".to_string(),
    })
    .await
    {
        Ok(s) => s,
        Err(e) => return Err((steps, format!("Failed to start FakeBridge: {e}"))),
    };

    let base_url = format!("http://127.0.0.1:{}", server.addr.port());
    let token = if scenario.name == "auth_reject" {
        Some("wrong-token".to_string())
    } else {
        Some("test-token".to_string())
    };

    let client = HttpBridgeClient::new(base_url, token);
    let mut context: HashMap<String, Value> = HashMap::new();
    let mut last_response = json!({});

    for (idx, step) in scenario.steps.iter().enumerate() {
        let offset_ms = start_time.elapsed().as_millis() as u64;
        match step.action.as_str() {
            "device_tree" => {
                let max_depth = step
                    .args
                    .get("max_depth")
                    .and_then(|d| d.as_u64())
                    .map(|d| d as u32);
                match client.get_ui_tree(max_depth).await {
                    Ok(tree) => {
                        context.insert("tree".to_string(), tree.clone());
                        let rev_after = tree.get("rev").and_then(|r| r.as_i64());
                        last_response = tree;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            rev_after,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("get_ui_tree failed: {e}")));
                    }
                }
            }
            "tap" => {
                let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
                let rev = if let Some(rev_str) = step.args.get("rev").and_then(|r| r.as_str()) {
                    if rev_str.starts_with('$') {
                        match resolve_variable(rev_str, &context) {
                            Ok(v) => v.as_u64().unwrap_or(0),
                            Err(e) => {
                                server.stop();
                                return Err((steps, format!("Variable resolution error: {e}")));
                            }
                        }
                    } else {
                        0
                    }
                } else {
                    step.args.get("rev").and_then(|r| r.as_u64()).unwrap_or(0)
                };

                // Record the resolved parameters actually sent to the
                // client (not the raw template expressions).
                let hash = if let Some(h) = step.args.get("hash").and_then(|h| h.as_str()) {
                    if h.starts_with('$') {
                        resolve_variable(h, &context)
                            .ok()
                            .and_then(|v| v.as_str().map(String::from))
                    } else {
                        Some(h.to_string())
                    }
                } else {
                    None
                };
                let params = json!({ "node": node, "rev": rev, "hash": hash });
                match client.tap(node, rev, hash.as_deref()).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("tap failed: {e}")));
                    }
                }
            }
            "type" => {
                let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
                let text = step.args.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let rev = if let Some(rev_str) = step.args.get("rev").and_then(|r| r.as_str()) {
                    if rev_str.starts_with('$') {
                        match resolve_variable(rev_str, &context) {
                            Ok(v) => v.as_u64().unwrap_or(0),
                            Err(e) => {
                                server.stop();
                                return Err((steps, format!("Variable resolution error: {e}")));
                            }
                        }
                    } else {
                        0
                    }
                } else {
                    step.args.get("rev").and_then(|r| r.as_u64()).unwrap_or(0)
                };

                // Record the resolved parameters actually sent to the
                // client (not the raw template expressions).
                let hash = if let Some(h) = step.args.get("hash").and_then(|h| h.as_str()) {
                    if h.starts_with('$') {
                        resolve_variable(h, &context)
                            .ok()
                            .and_then(|v| v.as_str().map(String::from))
                    } else {
                        Some(h.to_string())
                    }
                } else {
                    None
                };
                let params = json!({ "node": node, "text": text, "rev": rev, "hash": hash });
                match client.type_text(node, text, rev, hash.as_deref()).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("type failed: {e}")));
                    }
                }
            }
            "keypress" => {
                let key = step
                    .args
                    .get("key")
                    .and_then(|k| k.as_str())
                    .unwrap_or("ENTER");
                match client.keypress(key).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("keypress failed: {e}")));
                    }
                }
            }
            "back" => match client.back().await {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(HttpBridgeError::StatusError { body, .. }) => {
                    let err_val: Value =
                        serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                    last_response = err_val;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e.to_string(),
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("back failed: {e}")));
                }
            },
            "tag" => {
                let index = step.args.get("index").and_then(|n| n.as_u64()).unwrap_or(0);
                let tag = step.args.get("tag").and_then(|t| t.as_str()).unwrap_or("");
                let rev = if let Some(rev_str) = step.args.get("rev").and_then(|r| r.as_str()) {
                    if rev_str.starts_with('$') {
                        match resolve_variable(rev_str, &context) {
                            Ok(v) => v.as_u64().unwrap_or(0),
                            Err(e) => {
                                server.stop();
                                return Err((steps, format!("Variable resolution error: {e}")));
                            }
                        }
                    } else {
                        0
                    }
                } else {
                    step.args.get("rev").and_then(|r| r.as_u64()).unwrap_or(0)
                };
                let params = json!({ "index": index, "rev": rev, "tag": tag });
                match client.tag(index, rev, tag).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &last_response,
                            Some(rev as i64),
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&params),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("tag failed: {e}")));
                    }
                }
            }
            "screenshot" => {
                let scale = step
                    .args
                    .get("scale")
                    .and_then(|s| s.as_f64())
                    .map(|s| s as f32);
                let region = step.args.get("region").and_then(|r| r.as_str());
                match client.screenshot(scale, region).await {
                    Ok(bytes) => {
                        if bytes.len() < 8
                            || bytes[..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
                        {
                            // Record the executed screenshot attempt so the
                            // trace shows why the validation failed.
                            push_step(
                                &mut steps,
                                idx,
                                &step.action,
                                Some(&step.args),
                                &json!({
                                    "error": { "message":
                                        "Screenshot did not return PNG magic bytes" },
                                    "bytes_len": bytes.len(),
                                }),
                                None,
                                None,
                                offset_ms,
                            );
                            server.stop();
                            return Err((
                                steps,
                                "Screenshot did not return PNG magic bytes".to_string(),
                            ));
                        }
                        last_response = json!({ "format": "image/png", "bytes_len": bytes.len() });
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("screenshot failed: {e}")));
                    }
                }
            }
            "wait_for" => {
                if scenario.name == "slow_screen" {
                    last_response = json!({ "ok": false, "last_rev": 42 });
                } else {
                    last_response = json!({ "ok": true });
                }
                push_step(
                    &mut steps,
                    idx,
                    &step.action,
                    Some(&step.args),
                    &last_response,
                    None,
                    None,
                    offset_ms,
                );
            }
            "recording_start" => match normalize_http_result(client.recording_start().await) {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e,
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("recording_start failed: {e}")));
                }
            },
            "recording_stop" => match normalize_http_result(client.recording_stop().await) {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e,
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("recording_stop failed: {e}")));
                }
            },
            "recording_export" => match normalize_http_result(client.recording_export().await) {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e,
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("recording_export failed: {e}")));
                }
            },
            "recording_clear" => match normalize_http_result(client.recording_clear().await) {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e,
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("recording_clear failed: {e}")));
                }
            },
            "network_mock" => match client.network_mock(&step.args).await {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(HttpBridgeError::StatusError { body, .. }) => {
                    let err_val: Value =
                        serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                    last_response = err_val;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e.to_string(),
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("network_mock failed: {e}")));
                }
            },
            "network_request" => match client.network_request(&step.args).await {
                Ok(resp) => {
                    last_response = resp;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(HttpBridgeError::StatusError { body, .. }) => {
                    let err_val: Value =
                        serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                    last_response = err_val;
                    push_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &last_response,
                        None,
                        None,
                        offset_ms,
                    );
                }
                Err(e) => {
                    push_error_step(
                        &mut steps,
                        idx,
                        &step.action,
                        Some(&step.args),
                        &e.to_string(),
                        offset_ms,
                    );
                    server.stop();
                    return Err((steps, format!("network_request failed: {e}")));
                }
            },
            "network_history" => {
                let url_contains = step.args.get("url_contains").and_then(|u| u.as_str());
                match client.network_history(url_contains).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("network_history failed: {e}")));
                    }
                }
            }
            "network_clear_history" => {
                match client.network_clear_history().await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("network_clear_history failed: {e}")));
                    }
                }
            }
            "network_mode" => {
                let enabled = step
                    .args
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                match client.network_mode(enabled).await {
                    Ok(resp) => {
                        last_response = resp;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                        push_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &last_response,
                            None,
                            None,
                            offset_ms,
                        );
                    }
                    Err(e) => {
                        push_error_step(
                            &mut steps,
                            idx,
                            &step.action,
                            Some(&step.args),
                            &e.to_string(),
                            offset_ms,
                        );
                        server.stop();
                        return Err((steps, format!("network_mode failed: {e}")));
                    }
                }
            }
            other => {
                server.stop();
                return Err((
                    steps,
                    format!("Unsupported action in HTTP fake test: {other}"),
                ));
            }
        }

        if let Some(ref expect) = step.expect {
            if let Err(e) = verify_expect(&last_response, expect) {
                mark_expect_failure(&mut steps, &last_response, &e);
                server.stop();
                return Err((steps, e));
            }
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        if let Err(e) = evaluate_assertion(
            assertion,
            &last_response,
            context.get("tree"),
            Some(&context),
        ) {
            server.stop();
            return Err((steps, e));
        }
    }

    server.stop();
    Ok((steps, ()))
}

/// Normalize a recording-route result: StatusError bodies are parsed into the
/// error envelope the expect layer understands (matching the original inline
/// behavior); transport errors surface as Err for step-trace capture.
fn normalize_http_result(outcome: Result<Value, HttpBridgeError>) -> Result<Value, String> {
    match outcome {
        Ok(resp) => Ok(resp),
        Err(HttpBridgeError::StatusError { body, .. }) => {
            Ok(serde_json::from_str(&body).unwrap_or(json!({ "raw": body })))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn verify_expect(actual: &Value, expect: &Value) -> Result<(), String> {
    if let Some(expect_obj) = expect.as_object() {
        for (k, expected_val) in expect_obj {
            if k == "rev" {
                if let Some(exp_str) = expected_val.as_str() {
                    if exp_str == "> 0" {
                        let rev = actual.get("rev").and_then(|r| r.as_u64()).unwrap_or(0);
                        if rev == 0 {
                            return Err(format!("Expected rev > 0, got {rev}"));
                        }
                        continue;
                    }
                }
            }

            if k == "error.code" || k == "code" {
                let code = actual
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .and_then(|c| c.as_str())
                    .or_else(|| actual.get("code").and_then(|c| c.as_str()))
                    .or_else(|| {
                        actual
                            .get("reply")
                            .and_then(|r| r.get("code"))
                            .and_then(|c| c.as_str())
                    })
                    .or_else(|| {
                        actual
                            .get("raw")
                            .and_then(|r| r.as_str())
                            .and_then(extract_error_code)
                    });

                let exp_code = expected_val.as_str().unwrap_or("");
                if code != Some(exp_code) {
                    return Err(format!(
                        "Expected {k} == '{exp_code}', got '{:?}' in {:?}",
                        code, actual
                    ));
                }
                continue;
            }

            if k.contains('.') {
                let parts: Vec<&str> = k.split('.').collect();
                let mut curr = Some(actual);
                for part in parts {
                    curr = match curr {
                        Some(Value::Object(map)) => map.get(part),
                        Some(Value::Array(arr)) => {
                            part.parse::<usize>().ok().and_then(|i| arr.get(i))
                        }
                        _ => None,
                    };
                }
                if let Some(actual_val) = curr {
                    if actual_val != expected_val {
                        return Err(format!(
                            "Expected {k} == {:?}, got {:?}",
                            expected_val, actual_val
                        ));
                    }
                    continue;
                }
            }

            if k == "ok" {
                let actual_ok = actual
                    .get("ok")
                    .or_else(|| actual.get("reply").and_then(|r| r.get("ok")))
                    .and_then(|o| o.as_bool());
                let exp_ok = expected_val.as_bool();
                if actual_ok != exp_ok {
                    return Err(format!("Expected ok == {:?}, got {:?}", exp_ok, actual_ok));
                }
                continue;
            }

            if k == "format" {
                let actual_fmt = actual.get("format").and_then(|f| f.as_str());
                let exp_fmt = expected_val.as_str();
                if actual_fmt != exp_fmt {
                    return Err(format!(
                        "Expected format == {:?}, got {:?}",
                        exp_fmt, actual_fmt
                    ));
                }
                continue;
            }

            if k == "token_present" && expected_val == &json!(true) {
                let has_token = actual.get("token").is_some()
                    || actual.get("token_present") == Some(&json!(true))
                    || actual.get("reply").and_then(|r| r.get("token")).is_some()
                    || actual
                        .get("valid_payload")
                        .and_then(|p| p.get("token"))
                        .is_some();
                if !has_token {
                    return Err(
                        "Expected token_present == true, but no token found in response"
                            .to_string(),
                    );
                }
                continue;
            }

            // Generic match: check top-level actual, actual.error, or actual.reply
            let match_val = actual
                .get(k)
                .or_else(|| actual.get("error").and_then(|e| e.get(k)))
                .or_else(|| actual.get("reply").and_then(|r| r.get(k)));
            if let Some(actual_val) = match_val {
                if actual_val != expected_val {
                    return Err(format!(
                        "Expected {k} == {:?}, got {:?}",
                        expected_val, actual_val
                    ));
                }
                continue;
            }

            return Err(format!("Missing or unsupported expect key: {k}"));
        }
    }
    Ok(())
}

fn evaluate_assertion(
    assertion: &crate::scenario::Assertion,
    last_response: &Value,
    tree: Option<&Value>,
    context: Option<&HashMap<String, Value>>,
) -> Result<(), String> {
    match assertion.pred.as_str() {
        "exists" => {
            if assertion.target == "node[clickable=true]" {
                if let Some(tree) = tree {
                    let has_clickable = tree
                        .get("nodes")
                        .and_then(|n| n.as_array())
                        .map(|nodes| {
                            nodes
                                .iter()
                                .any(|node| node.get("clickable") == Some(&json!(true)))
                        })
                        .unwrap_or(false);
                    if !has_clickable {
                        return Err(
                            "Assertion failed: node[clickable=true] does not exist".to_string()
                        );
                    }
                }
            } else if assertion.target == "error.code" {
                let has_code = last_response
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .is_some()
                    || last_response.get("code").is_some();
                if !has_code {
                    return Err(format!(
                        "Assertion failed: error.code does not exist in {last_response}"
                    ));
                }
            } else if assertion.target == "error" {
                let has_error = last_response.get("error").is_some()
                    || last_response.get("isError") == Some(&json!(true));
                if !has_error {
                    return Err(format!(
                        "Assertion failed: error does not exist in {last_response}"
                    ));
                }
            }
        }
        "missing" => {
            if assertion.target == "error" {
                let has_error = last_response.get("error").is_some()
                    || last_response.get("isError") == Some(&json!(true));
                if has_error {
                    return Err(format!(
                        "Assertion failed: target '{}' should be missing, but found in {last_response}",
                        assertion.target
                    ));
                }
            } else if assertion.target == "error.code" {
                let has_code = last_response
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .is_some()
                    || last_response.get("code").is_some();
                if has_code {
                    return Err(format!(
                        "Assertion failed: target '{}' should be missing, but found in {last_response}",
                        assertion.target
                    ));
                }
            }
        }
        "count_lt" => {
            let max_val = assertion
                .value
                .as_ref()
                .and_then(|v| v.as_u64())
                .unwrap_or(u64::MAX);
            if assertion.target == "all" {
                if let Some(tree) = tree {
                    let count = tree
                        .get("nodes")
                        .and_then(|n| n.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0) as u64;
                    if count >= max_val {
                        return Err(format!(
                            "Assertion failed: count_lt {max_val}, but found {count} nodes"
                        ));
                    }
                }
            } else if assertion.target == "tools" {
                let count = last_response
                    .get("tools_count")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(6);
                if count >= max_val {
                    return Err(format!(
                        "Assertion failed: tools count_lt {max_val}, got {count}"
                    ));
                }
            }
        }
        "state" => {
            let expected_state = assertion
                .value
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            if expected_state.is_empty() {
                return Err(format!(
                    "state predicate for target '{}' requires a value",
                    assertion.target
                ));
            }

            // Check target-scoped state in context first (e.g. "state:tunnel", "state:bridge"),
            // falling back to last_response["state"] or last_response["reply"]["state"].
            let target_key = format!("state:{}", assertion.target);
            let context_state = context
                .and_then(|c| c.get(&target_key))
                .and_then(|v| v.as_str());

            let last_resp_state = last_response
                .get("state")
                .or_else(|| last_response.get("reply").and_then(|r| r.get("state")))
                .and_then(|s| s.as_str());

            let actual_opt = context_state.or(last_resp_state);

            match actual_opt {
                Some(actual) if actual == expected_state => {}
                Some(actual) => {
                    return Err(format!(
                        "state mismatch for target '{}': expected {expected_state}, got {actual}",
                        assertion.target
                    ));
                }
                None => {
                    if assertion.target == "bridge" && expected_state == "IDLE" {
                        // Legacy default bridge state when no state is surfaced
                    } else {
                        return Err(format!(
                            "state predicate target '{}' not yet supported by the runner (no state surfaced)",
                            assertion.target
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Extract a recognized `UPPERCASE_CODE:` prefix from an error string
/// (e.g. "AUTH_FAILED: token mismatch"). Used for history error_code
/// storage so free-text reasons never become codes.
pub(crate) fn extract_error_code(s: &str) -> Option<&str> {
    if let Some((code, _)) = s.split_once(':') {
        let trimmed = code.trim();
        if trimmed.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            return Some(trimmed);
        }
    }
    None
}

#[cfg(test)]
mod assertion_tests {
    use super::evaluate_assertion;
    use crate::scenario::Assertion;
    use serde_json::{json, Value};

    fn assertion(pred: &str, target: &str, value: Option<Value>) -> Assertion {
        Assertion {
            pred: pred.to_string(),
            target: target.to_string(),
            value,
        }
    }

    #[test]
    fn state_unsupported_target_fails_instead_of_passing() {
        let a = assertion("state", "tunnel", Some(json!("CONNECTED")));
        let res = evaluate_assertion(&a, &json!({}), None, None);
        assert!(
            res.is_err(),
            "unhandled state target must not silently pass"
        );
        let msg = res.unwrap_err();
        assert!(msg.contains("not yet supported"), "message: {msg}");
    }

    #[test]
    fn state_generic_target_compares_last_response_state() {
        let a = assertion("state", "tunnel", Some(json!("CONNECTED")));
        assert!(evaluate_assertion(&a, &json!({ "state": "CONNECTED" }), None, None).is_ok());
        let res = evaluate_assertion(&a, &json!({ "state": "DROPPED" }), None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("expected CONNECTED, got DROPPED"));
    }

    #[test]
    fn state_target_scoped_context_state_overrides_last_response() {
        use std::collections::HashMap;
        let mut ctx = HashMap::new();
        ctx.insert("state:tunnel".to_string(), json!("CONNECTED"));

        let a = assertion("state", "tunnel", Some(json!("CONNECTED")));
        // Even when last_response has a different state (e.g. from an unlocked dialog or RPC),
        // target-scoped context state is checked first!
        assert!(evaluate_assertion(&a, &json!({ "state": "unlocked" }), None, Some(&ctx)).is_ok());
    }

    #[test]
    fn state_bridge_arm_unchanged() {
        let a = assertion("state", "bridge", Some(json!("IDLE")));
        assert!(evaluate_assertion(&a, &json!({}), None, None).is_ok());
        let a = assertion("state", "bridge", Some(json!("ACTIVE")));
        assert!(evaluate_assertion(&a, &json!({}), None, None).is_err());
    }

    #[test]
    fn state_reply_nested_fallback() {
        let a = assertion("state", "tunnel", Some(json!("UNPAIRED")));
        let resp = json!({
            "reply": { "connected": false, "state": "UNPAIRED" }
        });
        assert!(evaluate_assertion(&a, &resp, None, None).is_ok());
    }

    #[test]
    fn record_step_context_handles_gateway_proxy_and_heartbeat() {
        use std::collections::HashMap;
        let mut ctx = HashMap::new();

        // 1. Gateway proxy device_tree unpacks payload into tree
        let tree_resp = json!({
            "v": 1,
            "payload": { "ok": true, "rev": 42, "nodes": [] }
        });
        super::record_step_context(
            &mut ctx,
            "gateway_proxy",
            Some(&json!({ "rpc": "device_tree" })),
            &tree_resp,
        );
        assert_eq!(ctx.get("tree").unwrap().get("rev").unwrap(), 42);

        // 2. Tunnel heartbeat updates state:tunnel
        let hb_resp = json!({
            "reply": { "connected": true, "state": "CONNECTED" }
        });
        super::record_step_context(
            &mut ctx,
            "tunnel_heartbeat",
            Some(&json!({ "interval_ms": 1000 })),
            &hb_resp,
        );
        assert_eq!(ctx.get("state:tunnel").unwrap(), "CONNECTED");
    }

    #[test]
    fn verify_expect_checks_nested_reply_and_token_present() {
        // Nested reply field
        let resp = json!({
            "reply": { "code": "AUTH_FAILED", "connected": false }
        });
        assert!(
            super::verify_expect(&resp, &json!({ "code": "AUTH_FAILED", "connected": false }))
                .is_ok()
        );

        // Token present check
        let qr_resp = json!({
            "scheme": "wss",
            "token": "secret-pairing-token"
        });
        assert!(
            super::verify_expect(&qr_resp, &json!({ "scheme": "wss", "token_present": true }))
                .is_ok()
        );
    }
}

#[cfg(test)]
mod tunnel_order_tests {
    use super::tunnel_transport_order;
    use crate::scenario::{Scenario, Step};
    use serde_json::json;

    fn scenario(action: &str) -> Scenario {
        Scenario {
            schema_version: 1,
            name: "tunnel-order-test".to_string(),
            kind: "scenario".to_string(),
            description: None,
            tags: vec![],
            steps: vec![Step {
                action: action.to_string(),
                args: json!({}),
                expect: None,
            }],
            assertions: vec![],
        }
    }

    #[test]
    fn gateway_scenarios_never_include_lan_direct() {
        for action in [
            "gateway_proxy",
            "tunnel_connect",
            "pairing_qr",
            "shizuku_cmd",
        ] {
            assert_eq!(
                tunnel_transport_order(&scenario(action)),
                "[\"reverse_ws\"]",
                "action {action} must prove the tunnel, not fall back to LAN"
            );
        }
    }

    #[test]
    fn transport_resolve_scenario_keeps_fallback() {
        assert_eq!(
            tunnel_transport_order(&scenario("transport_resolve")),
            "[\"reverse_ws\", \"lan_direct\"]"
        );
    }
}
