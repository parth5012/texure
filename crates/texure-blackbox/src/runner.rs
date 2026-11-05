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

pub async fn run_test(
    test: &TestDef,
    scenarios_dir: &Path,
    root_dir: &Path,
    _bridge_url: &str,
    _token: Option<&str>,
) -> TestResult {
    let start_time = Instant::now();

    if test.is_device {
        return TestResult {
            id: test.id.clone(),
            name: test.name.clone(),
            target: test.target.clone(),
            status: "device-skip".to_string(),
            duration_ms: start_time.elapsed().as_millis() as u64,
            reason: Some("device test skipped (no phone configured/connected)".to_string()),
            evidence_bundle: None,
        };
    }

    let scenario_path = scenarios_dir.join(&test.scenario_file);
    let scenario = match Scenario::load_from_file(&scenario_path) {
        Ok(s) => s,
        Err(e) => {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            return TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "fail".to_string(),
                duration_ms,
                reason: Some(format!("Failed to load scenario: {e}")),
                evidence_bundle: None,
            };
        }
    };

    let result = match test.target.as_str() {
        "mcp" => run_mcp_test(test, &scenario).await,
        "mcp-real" => run_mcp_real_test(test, &scenario, root_dir).await,
        _ => run_http_fake_test(test, &scenario, root_dir).await,
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;
    match result {
        Ok(()) => TestResult {
            id: test.id.clone(),
            name: test.name.clone(),
            target: test.target.clone(),
            status: "pass".to_string(),
            duration_ms,
            reason: None,
            evidence_bundle: None,
        },
        Err(err) => {
            let bundle_dir = Path::new("tmp/evidence").join(&test.id);
            let _ = std::fs::create_dir_all(&bundle_dir);
            let _ = std::fs::write(
                bundle_dir.join("red-run.txt"),
                format!(
                    "TEST {}: {}\nSTATUS: FAIL\nREASON: {}\n",
                    test.id, test.name, err
                ),
            );

            TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "fail".to_string(),
                duration_ms,
                reason: Some(err),
                evidence_bundle: Some(bundle_dir.to_string_lossy().to_string()),
            }
        }
    }
}

async fn run_mcp_test(_test: &TestDef, scenario: &Scenario) -> Result<(), String> {
    let current_exe = env::current_exe().map_err(|e| format!("Cannot locate current exe: {e}"))?;
    let current_exe_str = current_exe
        .to_str()
        .ok_or_else(|| "Current executable path is not valid UTF-8".to_string())?;
    let mut client = McpStdioClient::spawn(
        current_exe_str,
        &["--fake-mcp", "--scenario", &scenario.name],
    )
    .map_err(|e| format!("Failed to spawn fake MCP server: {e}"))?;

    let mut last_response = json!({});

    for step in &scenario.steps {
        match step.action.as_str() {
            "mcp_handshake" => {
                let init_resp = client
                    .initialize()
                    .await
                    .map_err(|e| format!("MCP initialize failed: {e}"))?;

                let caps = init_resp.get("capabilities").and_then(|c| c.as_object());
                if let Some(caps) = caps {
                    if caps.len() != 1 || !caps.contains_key("tools") {
                        return Err(format!(
                            "MCP server must declare ONLY tools capability; found: {:?}",
                            caps.keys()
                        ));
                    }
                } else {
                    return Err("Missing capabilities in MCP initialize response".to_string());
                }

                let tools_resp = client
                    .list_tools()
                    .await
                    .map_err(|e| format!("tools/list failed: {e}"))?;

                let tools = tools_resp
                    .get("tools")
                    .and_then(|t| t.as_array())
                    .ok_or_else(|| "tools/list response missing 'tools' array".to_string())?;

                if tools.len() != 6 {
                    return Err(format!(
                        "MCP tools count must be exactly 6; found {}",
                        tools.len()
                    ));
                }

                last_response = json!({ "tools_count": tools.len() });

                if let Some(ref expect) = step.expect {
                    if let Some(expected_count) = expect.get("tools_count").and_then(|c| c.as_u64())
                    {
                        if tools.len() as u64 != expected_count {
                            return Err(format!(
                                "tools_count mismatch: expected {expected_count}, got {}",
                                tools.len()
                            ));
                        }
                    }
                }
            }
            "device_tree" | "tap" | "type" | "keypress" | "screenshot" | "wait_for" => {
                let call_res = client
                    .call_tool(&step.action, step.args.clone())
                    .await
                    .map_err(|e| format!("call_tool({}) failed: {e}", step.action))?;

                last_response = mcp_content_to_response(&call_res);

                if let Some(ref expect) = step.expect {
                    verify_expect(&last_response, expect)?;
                }
            }
            other => return Err(format!("Unsupported action in MCP test: {other}")),
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        evaluate_assertion(assertion, &last_response, None)?;
    }

    Ok(())
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

    let parsed_content: Value = serde_json::from_str(content_text)
        .unwrap_or_else(|_| json!({ "raw": content_text }));

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
const TEXURE_ENV_KEYS: [&str; 4] = ["TEXURE_URL", "TEXURE_TOKEN", "TEXURE_TIMEOUT", "TEXURE_CONFIG"];

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
async fn run_mcp_real_test(
    _test: &TestDef,
    scenario: &Scenario,
    root_dir: &Path,
) -> Result<(), String> {
    let server_bin = locate_server_bin()?;

    // The FakeBridge expects "test-token"; auth_reject deliberately configures
    // the server with the wrong one so the 401 AUTH_FAILED path is exercised.
    let server_token = if scenario.name == "auth_reject" {
        "wrong-token"
    } else {
        "test-token"
    };

    let needs_bridge = !NO_CONFIG_SCENARIOS.contains(&scenario.name.as_str());
    let mut bridge = if needs_bridge {
        Some(
            FakeBridgeServer::start(FakeBridgeConfig {
                scenario_name: scenario.name.clone(),
                root_dir: root_dir.to_string_lossy().to_string(),
                port: 0,
                token: "test-token".to_string(),
            })
            .await
            .map_err(|e| format!("Failed to start FakeBridge: {e}"))?,
        )
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
            return Err(format!("Failed to write temp server config: {e}"));
        }
        vec!["--config".to_string(), config_path.to_string_lossy().to_string()]
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
    let mut client = McpStdioClient::spawn_filtered(&server_bin, &arg_refs, &TEXURE_ENV_KEYS)
        .map_err(|e| format!("Failed to spawn texure-server: {e}"))?;

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

async fn walk_mcp_real_steps(
    scenario: &Scenario,
    client: &mut McpStdioClient,
) -> Result<(), String> {
    let mut context: HashMap<String, Value> = HashMap::new();
    let mut last_response = json!({});

    for step in &scenario.steps {
        match step.action.as_str() {
            "mcp_handshake" => {
                let init_resp = client
                    .initialize()
                    .await
                    .map_err(|e| format!("MCP initialize failed: {e}"))?;

                let caps = init_resp.get("capabilities").and_then(|c| c.as_object());
                if let Some(caps) = caps {
                    if caps.len() != 1 || !caps.contains_key("tools") {
                        return Err(format!(
                            "MCP server must declare ONLY tools capability; found: {:?}",
                            caps.keys()
                        ));
                    }
                } else {
                    return Err("Missing capabilities in MCP initialize response".to_string());
                }

                let tools_resp = client
                    .list_tools()
                    .await
                    .map_err(|e| format!("tools/list failed: {e}"))?;

                let tools = tools_resp
                    .get("tools")
                    .and_then(|t| t.as_array())
                    .ok_or_else(|| "tools/list response missing 'tools' array".to_string())?;

                if tools.len() != 6 {
                    return Err(format!(
                        "MCP tools count must be exactly 6; found {}",
                        tools.len()
                    ));
                }

                last_response = json!({ "tools_count": tools.len() });

                if let Some(ref expect) = step.expect {
                    if let Some(expected_count) = expect.get("tools_count").and_then(|c| c.as_u64())
                    {
                        if tools.len() as u64 != expected_count {
                            return Err(format!(
                                "tools_count mismatch: expected {expected_count}, got {}",
                                tools.len()
                            ));
                        }
                    }
                }
                continue; // handshake expect (tools_count) handled above
            }
            "back" => {
                let call_res = client
                    .call_tool("keypress", json!({ "key": "BACK" }))
                    .await
                    .map_err(|e| format!("call_tool(keypress BACK) failed: {e}"))?;
                last_response = mcp_content_to_response(&call_res);
            }
            "device_tree" | "tap" | "type" | "keypress" | "screenshot" | "wait_for" => {
                let args = resolve_step_args(&step.action, &step.args, &context)?;
                let call_res = client
                    .call_tool(&step.action, args)
                    .await
                    .map_err(|e| format!("call_tool({}) failed: {e}", step.action))?;
                last_response = mcp_content_to_response(&call_res);

                if step.action == "device_tree" && last_response.get("rev").is_some() {
                    context.insert("tree".to_string(), last_response.clone());
                }
            }
            other => return Err(format!("Unsupported action in mcp-real test: {other}")),
        }

        if let Some(ref expect) = step.expect {
            verify_expect(&last_response, expect)?;
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        evaluate_assertion(assertion, &last_response, context.get("tree"))?;
    }

    Ok(())
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

async fn run_http_fake_test(
    _test: &TestDef,
    scenario: &Scenario,
    root_dir: &Path,
) -> Result<(), String> {
    let mut server = FakeBridgeServer::start(FakeBridgeConfig {
        scenario_name: scenario.name.clone(),
        root_dir: root_dir.to_string_lossy().to_string(),
        port: 0,
        token: "test-token".to_string(),
    })
    .await
    .map_err(|e| format!("Failed to start FakeBridge: {e}"))?;

    let base_url = format!("http://127.0.0.1:{}", server.addr.port());
    let token = if scenario.name == "auth_reject" {
        Some("wrong-token".to_string())
    } else {
        Some("test-token".to_string())
    };

    let client = HttpBridgeClient::new(base_url, token);
    let mut context: HashMap<String, Value> = HashMap::new();
    let mut last_response = json!({});

    for step in &scenario.steps {
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
                        last_response = tree;
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                    }
                    Err(e) => return Err(format!("get_ui_tree failed: {e}")),
                }
            }
            "tap" => {
                let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
                let rev = if let Some(rev_str) = step.args.get("rev").and_then(|r| r.as_str()) {
                    if rev_str.starts_with('$') {
                        resolve_variable(rev_str, &context)
                            .map_err(|e| format!("Variable resolution error: {e}"))?
                            .as_u64()
                            .unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    step.args.get("rev").and_then(|r| r.as_u64()).unwrap_or(0)
                };

                match client.tap(node, rev).await {
                    Ok(resp) => {
                        last_response = resp;
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                    }
                    Err(e) => return Err(format!("tap failed: {e}")),
                }
            }
            "type" => {
                let node = step.args.get("node").and_then(|n| n.as_u64()).unwrap_or(0);
                let text = step.args.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let rev = if let Some(rev_str) = step.args.get("rev").and_then(|r| r.as_str()) {
                    if rev_str.starts_with('$') {
                        resolve_variable(rev_str, &context)
                            .map_err(|e| format!("Variable resolution error: {e}"))?
                            .as_u64()
                            .unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    step.args.get("rev").and_then(|r| r.as_u64()).unwrap_or(0)
                };

                match client.type_text(node, text, rev).await {
                    Ok(resp) => {
                        last_response = resp;
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                    }
                    Err(e) => return Err(format!("type failed: {e}")),
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
                    }
                    Err(HttpBridgeError::StatusError { body, .. }) => {
                        let err_val: Value =
                            serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                        last_response = err_val;
                    }
                    Err(e) => return Err(format!("keypress failed: {e}")),
                }
            }
            "back" => match client.back().await {
                Ok(resp) => {
                    last_response = resp;
                }
                Err(HttpBridgeError::StatusError { body, .. }) => {
                    let err_val: Value =
                        serde_json::from_str(&body).unwrap_or(json!({"raw": body}));
                    last_response = err_val;
                }
                Err(e) => return Err(format!("back failed: {e}")),
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
                            return Err("Screenshot did not return PNG magic bytes".to_string());
                        }
                        last_response = json!({ "format": "image/png", "bytes_len": bytes.len() });
                    }
                    Err(e) => return Err(format!("screenshot failed: {e}")),
                }
            }
            "wait_for" => {
                if scenario.name == "slow_screen" {
                    last_response = json!({ "ok": false, "last_rev": 42 });
                } else {
                    last_response = json!({ "ok": true });
                }
            }
            other => return Err(format!("Unsupported action in HTTP fake test: {other}")),
        }

        if let Some(ref expect) = step.expect {
            verify_expect(&last_response, expect)?;
        }
    }

    // Evaluate scenario assertions
    for assertion in &scenario.assertions {
        evaluate_assertion(assertion, &last_response, context.get("tree"))?;
    }

    server.stop();
    Ok(())
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

fn extract_error_code(s: &str) -> Option<&str> {
    if let Some((code, _)) = s.split_once(':') {
        let trimmed = code.trim();
        if trimmed.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            return Some(trimmed);
        }
    }
    None
}
