pub mod docs_gate;
pub mod hygiene;
pub mod scenario_dsl;

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "texure-lint",
    about = "Custom linter for texure documentation, contracts, scenarios, and hygiene (P0)"
)]
struct Args {
    /// Path to emit machine-readable JSON report
    #[arg(long)]
    report: Option<PathBuf>,

    /// Repository root directory (defaults to current dir)
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LintReport {
    pub summary: LintSummary,
    pub findings: Vec<FindingJson>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LintSummary {
    pub total: usize,
    pub fails: usize,
    pub warns: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FindingJson {
    pub severity: String,
    pub rule: String,
    pub file: String,
    pub line: usize,
    pub message: String,
}

fn main() {
    let args = Args::parse();
    let root = &args.root;

    let mut findings = Vec::new();

    // 1. Docs-gate checks
    findings.extend(docs_gate::check_docs_gate(root));

    // 2. Scenario DSL checks
    findings.extend(scenario_dsl::check_scenarios(root));

    // 3. Hygiene checks
    findings.extend(hygiene::check_hygiene(root));

    let fails = findings.iter().filter(|f| f.severity == "FAIL").count();
    let warns = findings.iter().filter(|f| f.severity == "WARN").count();

    println!("================================================================================");
    println!("Texure Custom Linter (texure-lint P0)");
    println!("================================================================================");

    if findings.is_empty() {
        println!("All checks passed! Zero lint findings.");
    } else {
        println!(
            "{:<8} {:<32} {:<30} MESSAGE",
            "SEVERITY", "RULE", "LOCATION"
        );
        println!(
            "--------------------------------------------------------------------------------"
        );
        for f in &findings {
            let loc = format!("{}:{}", f.file, f.line);
            println!("{:<8} {:<32} {:<30} {}", f.severity, f.rule, loc, f.message);
        }
    }

    println!("--------------------------------------------------------------------------------");
    println!(
        "Summary: {} total finding(s) | {} FAIL | {} WARN",
        findings.len(),
        fails,
        warns
    );
    println!("================================================================================");

    let report_findings: Vec<FindingJson> = findings
        .into_iter()
        .map(|f| FindingJson {
            severity: f.severity,
            rule: f.rule,
            file: f.file,
            line: f.line,
            message: f.message,
        })
        .collect();

    let report = LintReport {
        summary: LintSummary {
            total: report_findings.len(),
            fails,
            warns,
        },
        findings: report_findings,
    };

    if let Some(ref report_path) = args.report {
        if let Some(parent) = report_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!(
                    "Warning: failed to create directory {}: {e}",
                    parent.display()
                );
            }
        }
        match serde_json::to_string_pretty(&report) {
            Ok(json_str) => {
                if let Err(e) = fs::write(report_path, json_str) {
                    eprintln!("Error writing report to {}: {e}", report_path.display());
                } else {
                    println!("Lint report written to: {}", report_path.display());
                }
            }
            Err(e) => eprintln!("Error serializing report to JSON: {e}"),
        }
    }

    // Exit code: 0 iff 0 FAIL findings (WARN does not fail the build)
    if fails > 0 {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}
