use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use texure_contract::tools::FROZEN_TOOL_NAMES;

pub struct FakeMcpServer {
    scenario: String,
    _bridge_url: Option<String>,
}

impl FakeMcpServer {
    pub fn new(scenario: String, bridge_url: Option<String>) -> Self {
        Self {
            scenario,
            _bridge_url: bridge_url,
        }
    }

    pub fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let req: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    let parse_err = json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {
                            "code": -32700,
                            "message": "Parse error"
                        }
                    });
                    let mut out = serde_json::to_string(&parse_err)?;
                    out.push('\n');
                    stdout.write_all(out.as_bytes())?;
                    stdout.flush()?;
                    continue;
                }
            };

            let id = req.get("id").cloned();
            let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
            let params = req.get("params").cloned().unwrap_or(json!({}));

            if let Some(resp) = self.handle_request(id, method, params) {
                let mut out = serde_json::to_string(&resp)?;
                out.push('\n');
                stdout.write_all(out.as_bytes())?;
                stdout.flush()?;
            }
        }

        Ok(())
    }

    fn handle_request(&self, id: Option<Value>, method: &str, params: Value) -> Option<Value> {
        // Notifications have no id member and must not receive a response
        let id = id?;

        let resp = match method {
            "initialize" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": &id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "texure-server",
                            "version": "0.1.0"
                        }
                    }
                })
            }
            "tools/list" => {
                json!({
                    "jsonrpc": "2.0",
                    "id": &id,
                    "result": {
                        "tools": [
                            {
                                "name": "device_tree",
                                "description": "Fetches accessibility/view hierarchy tree of the active Android window",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "max_depth": { "type": "integer", "description": "Maximum tree traversal depth" },
                                        "subtree": { "type": "string", "description": "Optional subtree node id" },
                                        "verbose": { "type": "boolean", "description": "Return full error payload on error" }
                                    }
                                }
                            },
                            {
                                "name": "tap",
                                "description": "Tap an Android UI node by id with rev check",
                                "inputSchema": {
                                    "type": "object",
                                    "required": ["node", "rev"],
                                    "properties": {
                                        "node": { "type": "integer" },
                                        "rev": { "type": "integer" },
                                        "verbose": { "type": "boolean" }
                                    }
                                }
                            },
                            {
                                "name": "type",
                                "description": "Type text into an editable Android node",
                                "inputSchema": {
                                    "type": "object",
                                    "required": ["node", "text", "rev"],
                                    "properties": {
                                        "node": { "type": "integer" },
                                        "text": { "type": "string" },
                                        "rev": { "type": "integer" },
                                        "verbose": { "type": "boolean" }
                                    }
                                }
                            },
                            {
                                "name": "keypress",
                                "description": "Send a key event (ENTER, BACK, DEL, TAB)",
                                "inputSchema": {
                                    "type": "object",
                                    "required": ["key"],
                                    "properties": {
                                        "key": { "type": "string", "enum": ["ENTER", "BACK", "DEL", "TAB"] },
                                        "verbose": { "type": "boolean" }
                                    }
                                }
                            },
                            {
                                "name": "screenshot",
                                "description": "Capture screenshot of the active window",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "scale": { "type": "number", "enum": [0.25, 0.5, 1.0] },
                                        "verbose": { "type": "boolean" }
                                    }
                                }
                            },
                            {
                                "name": "wait_for",
                                "description": "Wait for a UI condition to be satisfied",
                                "inputSchema": {
                                    "type": "object",
                                    "required": ["where", "value"],
                                    "properties": {
                                        "where": { "type": "string", "enum": ["text", "tag", "visible", "hidden"] },
                                        "value": { "type": "string" },
                                        "timeout": { "type": "integer", "maximum": 30 },
                                        "verbose": { "type": "boolean" }
                                    }
                                }
                            }
                        ]
                    }
                })
            }
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let verbose = args
                    .get("verbose")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // Frozen 6 tool list check
                if !FROZEN_TOOL_NAMES.contains(&name) {
                    return Some(json!({
                        "jsonrpc": "2.0",
                        "id": &id,
                        "error": {
                            "code": -32601,
                            "message": format!("Unknown tool: {name}")
                        }
                    }));
                }

                // Handle scenarios that trigger server-level state/errors
                if self.scenario == "no_config" {
                    return Some(self.make_error_response(
                        &id,
                        "NO_CONFIG",
                        "no config file/env",
                        "create ~/.config/texure/config.toml or set TEXURE_URL/TEXURE_TOKEN",
                        verbose,
                    ));
                }

                if self.scenario == "device_offline" {
                    return Some(self.make_error_response(
                        &id,
                        "OFFLINE",
                        "device unreachable",
                        "same LAN? app open? fg service alive?",
                        verbose,
                    ));
                }

                if self.scenario == "busy_second_client" {
                    return Some(self.make_error_response(
                        &id,
                        "BUSY",
                        "second client during single-flight (owner: client-1)",
                        "wait for owner release / ask owner",
                        verbose,
                    ));
                }

                if self.scenario == "verbose_errors" {
                    return Some(self.make_error_response(
                        &id,
                        "NO_CONFIG",
                        "no config file/env",
                        "create ~/.config/texure/config.toml or set TEXURE_URL/TEXURE_TOKEN",
                        verbose,
                    ));
                }

                if name == "wait_for" {
                    let val = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    if val == "NonexistentView" || self.scenario == "slow_screen" {
                        let timeout_payload = json!({
                            "ok": false,
                            "last_rev": 42,
                            "hint": "re-fetch device_tree and re-plan"
                        });
                        return Some(json!({
                            "jsonrpc": "2.0",
                            "id": &id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": serde_json::to_string(&timeout_payload).unwrap()
                                    }
                                ]
                            }
                        }));
                    } else {
                        return Some(json!({
                            "jsonrpc": "2.0",
                            "id": &id,
                            "result": {
                                "content": [
                                    {
                                        "type": "text",
                                        "text": "{\"ok\": true}"
                                    }
                                ]
                            }
                        }));
                    }
                }

                if name == "device_tree" {
                    let tree_resp = json!({
                        "rev": 42,
                        "window": "com.velavoice/.MainActivity",
                        "nodes": [
                            {
                                "id": "btn_send",
                                "text": "Send",
                                "cls": "android.widget.Button",
                                "bounds": [12, 340, 108, 56],
                                "clickable": true,
                                "editable": false
                            }
                        ]
                    });
                    return Some(json!({
                        "jsonrpc": "2.0",
                        "id": &id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string(&tree_resp).unwrap()
                                }
                            ]
                        }
                    }));
                }

                // Default fallback response
                json!({
                    "jsonrpc": "2.0",
                    "id": &id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": "{\"ok\": true, \"rev\": 42}"
                            }
                        ]
                    }
                })
            }
            _ => {
                json!({
                    "jsonrpc": "2.0",
                    "id": &id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {method}")
                    }
                })
            }
        };

        Some(resp)
    }

    fn make_error_response(
        &self,
        id: &Value,
        code: &str,
        message: &str,
        hint: &str,
        verbose: bool,
    ) -> Value {
        let err_json = json!({
            "error": {
                "code": code,
                "message": message,
                "hint": hint
            }
        });

        let text = if verbose {
            serde_json::to_string(&err_json).unwrap()
        } else {
            format!("{code}: {message} (hint: {hint})")
        };

        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": text
                    }
                ],
                "isError": true
            }
        })
    }
}
