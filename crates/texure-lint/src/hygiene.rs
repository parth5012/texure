use super::docs_gate::Finding;
use std::fs;
use std::path::Path;

pub fn check_hygiene(root_dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    // 1. AGENTS.md <= 60 lines
    let agents_path = root_dir.join("AGENTS.md");
    if agents_path.exists() {
        if let Ok(content) = fs::read_to_string(&agents_path) {
            let line_count = content.lines().count();
            if line_count > 60 {
                findings.push(Finding {
                    severity: "WARN".to_string(),
                    rule: "hygiene/agents-md-line-budget".to_string(),
                    file: "AGENTS.md".to_string(),
                    line: line_count,
                    message: format!("AGENTS.md line count ({line_count}) exceeds budget of 60 lines"),
                });
            }
        }
    } else {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "hygiene/agents-md-missing".to_string(),
            file: "AGENTS.md".to_string(),
            line: 1,
            message: "AGENTS.md not found in repository root".to_string(),
        });
    }

    // 2. LOG.md session freshness
    let log_path = root_dir.join("LOG.md");
    if log_path.exists() {
        if let Ok(content) = fs::read_to_string(&log_path) {
            // Check for a heading with a date format ## YYYY-MM-DD
            let has_recent_entry = content.lines().any(|l| {
                let trimmed = l.trim();
                trimmed.starts_with("## 2026-")
            });
            if !has_recent_entry {
                findings.push(Finding {
                    severity: "WARN".to_string(),
                    rule: "hygiene/log-freshness".to_string(),
                    file: "LOG.md".to_string(),
                    line: 1,
                    message: "LOG.md does not contain a recent session entry (## 2026-MM-DD)".to_string(),
                });
            }
        }
    } else {
        findings.push(Finding {
            severity: "WARN".to_string(),
            rule: "hygiene/log-missing".to_string(),
            file: "LOG.md".to_string(),
            line: 1,
            message: "LOG.md not found in repository root".to_string(),
        });
    }

    // 3. FakeBridge scenario references resolve
    let fakebridge_scenarios = [
        "static_screen",
        "stale_screen",
        "slow_screen",
        "auth_reject",
        "busy_second_client",
        "human_win",
        "lock_screen",
        "backgrounded",
    ];

    for sc in fakebridge_scenarios {
        let sc_file = root_dir.join(format!("tests/blackbox/scenarios/{sc}.json"));
        if !sc_file.exists() {
            findings.push(Finding {
                severity: "FAIL".to_string(),
                rule: "hygiene/fakebridge-scenario-missing".to_string(),
                file: format!("tests/blackbox/scenarios/{sc}.json"),
                line: 1,
                message: format!("FakeBridge scenario fixture file '{sc}.json' is referenced in docs but does not exist"),
            });
        }
    }

    // 4. Contract fixtures check
    let fixtures_dir = root_dir.join("contract/fixtures");
    if fixtures_dir.exists() {
        if let Ok(entries) = fs::read_dir(&fixtures_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    let filename = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if let Ok(c) = fs::read_to_string(&p) {
                        if serde_json::from_str::<serde_json::Value>(&c).is_err() {
                            findings.push(Finding {
                                severity: "FAIL".to_string(),
                                rule: "hygiene/invalid-contract-fixture".to_string(),
                                file: format!("contract/fixtures/{filename}"),
                                line: 1,
                                message: format!("Fixture 'contract/fixtures/{filename}' is not valid JSON"),
                            });
                        }
                    }
                }
            }
        }
    }

    findings
}
