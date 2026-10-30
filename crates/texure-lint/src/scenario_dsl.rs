use super::docs_gate::Finding;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn check_scenarios(root_dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    let scenarios_dir = root_dir.join("tests/blackbox/scenarios");
    if !scenarios_dir.exists() {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "scenario-dsl/dir-missing".to_string(),
            file: "tests/blackbox/scenarios".to_string(),
            line: 1,
            message: "Scenario directory tests/blackbox/scenarios does not exist".to_string(),
        });
        return findings;
    }

    let entries = match fs::read_dir(&scenarios_dir) {
        Ok(e) => e,
        Err(err) => {
            findings.push(Finding {
                severity: "FAIL".to_string(),
                rule: "scenario-dsl/read-error".to_string(),
                file: "tests/blackbox/scenarios".to_string(),
                line: 1,
                message: format!("Failed to read scenarios directory: {err}"),
            });
            return findings;
        }
    };

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
    let allowed_preds = ["exists", "missing", "count_lt", "state", "bounds_of"];

    let mut count = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        count += 1;

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let rel_file = format!("tests/blackbox/scenarios/{filename}");

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/io-error".to_string(),
                    file: rel_file,
                    line: 1,
                    message: format!("Failed to read file: {e}"),
                });
                continue;
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/invalid-json".to_string(),
                    file: rel_file,
                    line: 1,
                    message: format!("Invalid JSON: {e}"),
                });
                continue;
            }
        };

        // 1. Check schema_version: 1
        match json.get("schema_version").and_then(|v| v.as_u64()) {
            Some(1) => {}
            Some(other) => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/schema-version".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: format!("schema_version must be 1, found {other}"),
                });
            }
            None => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/missing-schema-version".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: "Missing or invalid schema_version (expected integer 1)".to_string(),
                });
            }
        }

        // 2. Check name
        match json.get("name").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => {}
            _ => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/invalid-name".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: "Scenario 'name' must be a non-empty string".to_string(),
                });
            }
        }

        // 3. Check kind
        match json.get("kind").and_then(|v| v.as_str()) {
            Some("scenario") | Some("fixture") => {}
            Some(other) => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/invalid-kind".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: format!("'kind' must be 'scenario' or 'fixture', found '{other}'"),
                });
            }
            None => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/missing-kind".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: "Missing 'kind' field in scenario".to_string(),
                });
            }
        }

        // 4. Check steps
        match json.get("steps").and_then(|v| v.as_array()) {
            Some(steps) => {
                for (step_idx, step) in steps.iter().enumerate() {
                    if let Some(action) = step.get("action").and_then(|v| v.as_str()) {
                        if action == "sleep" {
                            findings.push(Finding {
                                severity: "FAIL".to_string(),
                                rule: "scenario-dsl/forbidden-sleep".to_string(),
                                file: rel_file.clone(),
                                line: 1,
                                message: format!("Step {step_idx} uses forbidden action 'sleep'; use wait_for with predicates"),
                            });
                        } else if !allowed_actions.contains(&action) {
                            findings.push(Finding {
                                severity: "FAIL".to_string(),
                                rule: "scenario-dsl/unknown-action".to_string(),
                                file: rel_file.clone(),
                                line: 1,
                                message: format!("Step {step_idx} has unknown action '{action}'"),
                            });
                        }
                    } else {
                        findings.push(Finding {
                            severity: "FAIL".to_string(),
                            rule: "scenario-dsl/missing-action".to_string(),
                            file: rel_file.clone(),
                            line: 1,
                            message: format!("Step {step_idx} is missing 'action' string"),
                        });
                    }

                    // Check $tree.* references in args
                    if let Some(args) = step.get("args") {
                        check_tree_refs(args, &rel_file, step_idx, &mut findings);
                    }
                }
            }
            None => {
                findings.push(Finding {
                    severity: "FAIL".to_string(),
                    rule: "scenario-dsl/missing-steps".to_string(),
                    file: rel_file.clone(),
                    line: 1,
                    message: "Scenario must contain a 'steps' array".to_string(),
                });
            }
        }

        // 5. Check assertions
        if let Some(assertions) = json.get("assertions").and_then(|v| v.as_array()) {
            for (assert_idx, assertion) in assertions.iter().enumerate() {
                if let Some(pred) = assertion.get("pred").and_then(|v| v.as_str()) {
                    if !allowed_preds.contains(&pred) {
                        findings.push(Finding {
                            severity: "FAIL".to_string(),
                            rule: "scenario-dsl/unknown-predicate".to_string(),
                            file: rel_file.clone(),
                            line: 1,
                            message: format!("Assertion {assert_idx} has unknown predicate '{pred}'"),
                        });
                    }
                } else {
                    findings.push(Finding {
                        severity: "FAIL".to_string(),
                        rule: "scenario-dsl/missing-predicate".to_string(),
                        file: rel_file.clone(),
                        line: 1,
                        message: format!("Assertion {assert_idx} is missing 'pred'"),
                    });
                }
            }
        }
    }

    if count == 0 {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "scenario-dsl/no-scenarios".to_string(),
            file: "tests/blackbox/scenarios".to_string(),
            line: 1,
            message: "No scenario JSON files found in tests/blackbox/scenarios".to_string(),
        });
    }

    findings
}

fn check_tree_refs(val: &Value, file: &str, step_idx: usize, findings: &mut Vec<Finding>) {
    match val {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix("$tree.") {
                if rest != "rev" && !rest.starts_with("nodes[") {
                    findings.push(Finding {
                        severity: "FAIL".to_string(),
                        rule: "scenario-dsl/invalid-tree-ref".to_string(),
                        file: file.to_string(),
                        line: 1,
                        message: format!("Step {step_idx} has malformed tree ref '{s}' (expected $tree.rev or $tree.nodes[<idx>]...)"),
                    });
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                check_tree_refs(item, file, step_idx, findings);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                check_tree_refs(v, file, step_idx, findings);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_dsl_checks_on_repo() {
        let root = Path::new("../../");
        let root = if root.join("tests/blackbox/scenarios").exists() {
            root
        } else {
            Path::new(".")
        };
        let findings = check_scenarios(root);
        let fails: Vec<_> = findings.iter().filter(|f| f.severity == "FAIL").collect();
        assert!(fails.is_empty(), "Expected 0 FAIL findings, found: {:?}", fails);
    }
}

