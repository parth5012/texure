use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: String, // "FAIL" | "WARN"
    pub rule: String,
    pub file: String,
    pub line: usize,
    pub message: String,
}

pub fn check_docs_gate(root_dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();

    let behavior_spec_path = root_dir.join("docs/behavior-spec.md");
    if !behavior_spec_path.exists() {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "docs-gate/file-missing".to_string(),
            file: "docs/behavior-spec.md".to_string(),
            line: 1,
            message: "docs/behavior-spec.md not found".to_string(),
        });
        return findings;
    }

    let content = match fs::read_to_string(&behavior_spec_path) {
        Ok(c) => c,
        Err(e) => {
            findings.push(Finding {
                severity: "FAIL".to_string(),
                rule: "docs-gate/read-error".to_string(),
                file: "docs/behavior-spec.md".to_string(),
                line: 1,
                message: format!("Failed to read docs/behavior-spec.md: {e}"),
            });
            return findings;
        }
    };

    // Expected full BB suite IDs
    let all_expected_bb_ids = [
        "BB-A0", "BB-A1", "BB-A2", "BB-A3", "BB-A4", "BB-A5", "BB-A6",
        "BB-B1", "BB-B2", "BB-B3", "BB-B4", "BB-B5", "BB-B6", "BB-B7", "BB-B8", "BB-B9", "BB-B10", "BB-B11",
        "BB-C1", "BB-C2", "BB-C3",
    ];

    let mut mapped_bb_ids = HashSet::new();
    let mut behavior_rows = Vec::new();

    static BB_REGEX: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"BB-[A-C][0-9]+").unwrap());

    let lines: Vec<&str> = content.lines().collect();
    let mut in_table = false;
    let mut has_test_id_column = false;
    let mut test_id_col_idx = None;
    let mut behavior_col_idx = None;

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let inner = trimmed
                .strip_prefix('|')
                .and_then(|s| s.strip_suffix('|'))
                .unwrap_or(trimmed);
            let cells: Vec<String> = inner
                .split('|')
                .map(|s| s.trim().to_string())
                .collect();

            if cells.is_empty() {
                continue;
            }

            if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
                // Separator line
                in_table = true;
                continue;
            }

            if !in_table {
                // Header row
                behavior_col_idx = cells.iter().position(|c| c.eq_ignore_ascii_case("behavior") || c.eq_ignore_ascii_case("tool"));
                test_id_col_idx = cells.iter().position(|c| c.eq_ignore_ascii_case("test id") || c.eq_ignore_ascii_case("test"));
                has_test_id_column = test_id_col_idx.is_some();
            } else if test_id_col_idx.is_some() {
                // Data row
                let behavior_name = behavior_col_idx.and_then(|idx| cells.get(idx)).cloned().unwrap_or_default();
                if behavior_name.is_empty() || behavior_name.starts_with("---") {
                    continue;
                }

                let test_id_str = test_id_col_idx.and_then(|idx| cells.get(idx)).cloned().unwrap_or_default();
                let found_ids: Vec<String> = BB_REGEX
                    .find_iter(&test_id_str)
                    .map(|m| m.as_str().to_string())
                    .collect();

                behavior_rows.push((line_num + 1, behavior_name.clone(), found_ids.clone()));

                if found_ids.is_empty() {
                    findings.push(Finding {
                        severity: "FAIL".to_string(),
                        rule: "docs-gate/behavior-unmapped".to_string(),
                        file: "docs/behavior-spec.md".to_string(),
                        line: line_num + 1,
                        message: format!("Behavior row '{}' missing blackbox Test ID mapping (expected BB-<group><n>)", behavior_name),
                    });
                } else {
                    for id in found_ids {
                        mapped_bb_ids.insert(id);
                    }
                }
            }
        } else {
            in_table = false;
            has_test_id_column = false;
            test_id_col_idx = None;
            behavior_col_idx = None;
        }
    }

    if !has_test_id_column && behavior_rows.is_empty() {
        findings.push(Finding {
            severity: "FAIL".to_string(),
            rule: "docs-gate/no-tables".to_string(),
            file: "docs/behavior-spec.md".to_string(),
            line: 1,
            message: "No behavior tables with Test ID column found in docs/behavior-spec.md".to_string(),
        });
    }

    // Check bidirectional: Every expected BB test ID must be covered by behavior-spec
    for expected_id in all_expected_bb_ids {
        if !mapped_bb_ids.contains(expected_id) {
            findings.push(Finding {
                severity: "FAIL".to_string(),
                rule: "docs-gate/unmapped-blackbox-id".to_string(),
                file: "docs/behavior-spec.md".to_string(),
                line: 1,
                message: format!("Blackbox test ID '{}' has no corresponding behavior row in docs/behavior-spec.md", expected_id),
            });
        }
    }

    // Check docs/bridge-contract.md for frozen tool list and contract routes
    let contract_path = root_dir.join("docs/bridge-contract.md");
    if contract_path.exists() {
        if let Ok(c_content) = fs::read_to_string(&contract_path) {
            let frozen_tools = ["device_tree", "tap", "type", "keypress", "screenshot", "wait_for"];
            for tool in frozen_tools {
                if !c_content.contains(tool) {
                    findings.push(Finding {
                        severity: "FAIL".to_string(),
                        rule: "docs-gate/frozen-tool-missing".to_string(),
                        file: "docs/bridge-contract.md".to_string(),
                        line: 1,
                        message: format!("Frozen tool '{}' missing from docs/bridge-contract.md", tool),
                    });
                }
            }

            let required_routes = ["/ui-tree", "/tap", "/type", "/keypress", "/screenshot", "/ws"];
            for route in required_routes {
                if !c_content.contains(route) {
                    findings.push(Finding {
                        severity: "FAIL".to_string(),
                        rule: "docs-gate/route-missing".to_string(),
                        file: "docs/bridge-contract.md".to_string(),
                        line: 1,
                        message: format!("Required contract route '{}' missing from docs/bridge-contract.md", route),
                    });
                }
            }
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docs_gate_on_repo() {
        let root = Path::new("../../");
        let root = if root.join("docs/behavior-spec.md").exists() {
            root
        } else {
            Path::new(".")
        };
        let findings = check_docs_gate(root);
        let fails: Vec<_> = findings.iter().filter(|f| f.severity == "FAIL").collect();
        assert!(fails.is_empty(), "Expected 0 FAIL findings, found: {:?}", fails);
    }
}

