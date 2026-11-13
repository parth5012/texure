use crate::client_http::{HttpBridgeClient, HttpBridgeError};
use crate::client_mcp::McpStdioClient;
use crate::fake_bridge::{FakeBridgeConfig, FakeBridgeServer};
use crate::scenario::{resolve_variable, Scenario};
use crate::{TestDef, TestResult};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::time::Instant;

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
            other => return Err((steps, format!("Unsupported action in MCP test: {other}"))),
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        if let Err(e) = evaluate_assertion(assertion, &last_response, None) {
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
const TEXURE_ENV_KEYS: [&str; 4] = [
    "TEXURE_URL",
    "TEXURE_TOKEN",
    "TEXURE_TIMEOUT",
    "TEXURE_CONFIG",
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
            "recording_start" | "recording_stop" | "recording_export" | "recording_clear"
        )
    }) {
        return Err((
            steps,
            "SKIP: recording_* actions are bridge-HTTP routes, not MCP tools; covered by http/mcp-fake targets"
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

                let rev_after = if step.action == "device_tree" {
                    last_response.get("rev").and_then(|r| r.as_i64())
                } else {
                    None
                };
                if step.action == "device_tree" && last_response.get("rev").is_some() {
                    context.insert("tree".to_string(), last_response.clone());
                }
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
        if let Err(e) = evaluate_assertion(assertion, &last_response, context.get("tree")) {
            return Err((steps, e));
        }
    }

    Ok((steps, ()))
}

fn resolve_step_args(
    action: &str,
    args: &Value,
    context: &HashMap<String, Value>,
) -> Result<Value, String> {
    if !matches!(action, "tap" | "type") {
        return Ok(args.clone());
    }

    let mut resolved = args.clone();
    if let Some(rev_str) = args.get("rev").and_then(|r| r.as_str()) {
        if rev_str.starts_with('$') {
            let value = resolve_variable(rev_str, context)
                .map_err(|e| format!("Variable resolution error: {e}"))?;
            resolved["rev"] = value;
        }
    }
    Ok(resolved)
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
                let params = json!({ "node": node, "rev": rev });
                match client.tap(node, rev).await {
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
                let params = json!({ "node": node, "text": text, "rev": rev });
                match client.type_text(node, text, rev).await {
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
            "screenshot" => {
                let scale = step
                    .args
                    .get("scale")
                    .and_then(|s| s.as_f64())
                    .map(|s| s as f32);
                match client.screenshot(scale).await {
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
        if let Err(e) = evaluate_assertion(assertion, &last_response, context.get("tree")) {
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

            if k == "error.code" {
                let code = actual
                    .get("error")
                    .and_then(|e| e.get("code"))
                    .and_then(|c| c.as_str())
                    .or_else(|| actual.get("code").and_then(|c| c.as_str()))
                    .or_else(|| {
                        actual
                            .get("raw")
                            .and_then(|r| r.as_str())
                            .and_then(extract_error_code)
                    });

                let exp_code = expected_val.as_str().unwrap_or("");
                if code != Some(exp_code) {
                    return Err(format!(
                        "Expected error.code == '{exp_code}', got '{:?}' in {:?}",
                        code, actual
                    ));
                }
                continue;
            }

            if k == "ok" {
                let actual_ok = actual.get("ok").and_then(|o| o.as_bool());
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

            return Err(format!("Unsupported expect key: {k}"));
        }
    }
    Ok(())
}

fn evaluate_assertion(
    assertion: &crate::scenario::Assertion,
    last_response: &Value,
    tree: Option<&Value>,
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
        "state" if assertion.target == "bridge" => {
            let expected_state = assertion
                .value
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("IDLE");
            if expected_state != "IDLE" {
                return Err(format!("Bridge state mismatch: expected {expected_state}"));
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
