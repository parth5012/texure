use super::docs_gate::Finding;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub fn check_ast_grep(root_dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    let rules_dir = root_dir.join("lint/ast-grep");
    if !rules_dir.exists() {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "ast-grep/rules-dir-missing".to_string(),
            file: "lint/ast-grep".to_string(),
            line: 1,
            message: "ast-grep rules directory lint/ast-grep does not exist".to_string(),
        });
        return findings;
    }

    // Verify presence of required guardrail rule files
    let expected_rules = [
        ("g1-release-leak.yml", "G1"),
        ("g4-no-adb-system-exec.yml", "G4"),
        ("g5-g6-forbidden-injection.yml", "G5/G6"),
    ];

    for (file_name, guardrail) in expected_rules {
        let rule_path = rules_dir.join(file_name);
        if !rule_path.exists() {
            findings.push(Finding {
                severity: "FAIL".to_string(),
                rule: "ast-grep/rule-file-missing".to_string(),
                file: format!("lint/ast-grep/{file_name}"),
                line: 1,
                message: format!("Missing required rule file for Guardrail {guardrail}"),
            });
        }
    }

    // Execute Class C scan (perfgym dogfood attempted with sg scan fallback)
    match run_sg_scan(root_dir) {
        Ok(json_str) => {
            match serde_json::from_str::<Value>(&json_str) {
                Ok(Value::Array(items)) => {
                    for item in items {
                        let rule = item
                            .get("ruleId")
                            .and_then(|r| r.as_str())
                            .unwrap_or("guardrail-ast-grep")
                            .to_string();
                        let raw_file = item
                            .get("file")
                            .and_then(|f| f.as_str())
                            .unwrap_or("unknown");
                        let file = if let Ok(rel) = Path::new(raw_file).strip_prefix(root_dir) {
                            rel.to_string_lossy().to_string()
                        } else {
                            raw_file.trim_start_matches("./").to_string()
                        };
                        let line = item
                            .get("range")
                            .and_then(|r| r.get("start"))
                            .and_then(|s| s.get("line"))
                            .and_then(|l| l.as_u64())
                            .map(|l| (l + 1) as usize)
                            .unwrap_or(1);
                        let message = item
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("ast-grep guardrail rule match")
                            .to_string();

                        // All Class C guardrail violations are FAIL severity per docs/linting-plan.md
                        findings.push(Finding {
                            severity: "FAIL".to_string(),
                            rule: format!("ast-grep/{rule}"),
                            file,
                            line,
                            message,
                        });
                    }
                }
                Ok(_) => {
                    findings.push(Finding {
                        severity: "WARN".to_string(),
                        rule: "ast-grep/invalid-json-output".to_string(),
                        file: "lint/ast-grep".to_string(),
                        line: 1,
                        message: "ast-grep returned non-array JSON".to_string(),
                    });
                }
                Err(e) => {
                    findings.push(Finding {
                        severity: "WARN".to_string(),
                        rule: "ast-grep/parse-error".to_string(),
                        file: "lint/ast-grep".to_string(),
                        line: 1,
                        message: format!("Failed to parse ast-grep JSON output: {e}"),
                    });
                }
            }
        }
        Err(e) => {
            findings.push(Finding {
                severity: "WARN".to_string(),
                rule: "ast-grep/executor-unavailable".to_string(),
                file: "lint/ast-grep".to_string(),
                line: 1,
                message: format!("ast-grep execution could not complete: {e}"),
            });
        }
    }

    findings
}

fn run_sg_scan(root_dir: &Path) -> Result<String, String> {
    // 1. Try `ast-grep scan --json`
    if let Ok(out) = Command::new("ast-grep")
        .args(["scan", "--json"])
        .current_dir(root_dir)
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if let Some(json) = extract_json_array(&stdout) {
            return Ok(json);
        }
    }

    // 2. Try `sg scan --json` (only if sg is ast-grep, not util-linux sg)
    let is_ast_grep_sg = Command::new("sg")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("ast-grep"))
        .unwrap_or(false);

    if is_ast_grep_sg {
        if let Ok(out) = Command::new("sg")
            .args(["scan", "--json"])
            .current_dir(root_dir)
            .output()
        {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            if let Some(json) = extract_json_array(&stdout) {
                return Ok(json);
            }
        }
    }

    // 3. Fall back to `npx -y -p @ast-grep/cli ast-grep scan --json`
    let npx_res = Command::new("npx")
        .args(["-y", "-p", "@ast-grep/cli", "ast-grep", "scan", "--json"])
        .current_dir(root_dir)
        .output()
        .map_err(|e| format!("Failed to spawn npx @ast-grep/cli: {e}"))?;

    let stdout = String::from_utf8_lossy(&npx_res.stdout).to_string();
    let stderr = String::from_utf8_lossy(&npx_res.stderr).to_string();
    if let Some(json) = extract_json_array(&stdout) {
        return Ok(json);
    }

    Err(format!("npx ast-grep produced non-JSON output. stdout: {stdout}, stderr: {stderr}"))
}

fn extract_json_array(output: &str) -> Option<String> {
    let start = output.find('[')?;
    let end = output.rfind(']')?;
    if end >= start {
        Some(output[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_grep_rule_files_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let rules_dir = root.join("lint/ast-grep");
        assert!(rules_dir.exists(), "lint/ast-grep must exist");
        assert!(rules_dir.join("g1-release-leak.yml").exists());
        assert!(rules_dir.join("g4-no-adb-system-exec.yml").exists());
        assert!(rules_dir.join("g5-g6-forbidden-injection.yml").exists());
    }

    #[test]
    fn test_extract_json_array() {
        let raw = "npm warn foo\n[\n  {\"ruleId\": \"test\"}\n]\nError: 1 error found";
        let extracted = extract_json_array(raw).expect("extract json");
        assert!(extracted.starts_with('['));
        assert!(extracted.ends_with(']'));
    }
}
