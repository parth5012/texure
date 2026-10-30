pub mod client_http;
pub mod client_mcp;
pub mod scenario;

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "texure-blackbox",
    about = "Texure blackbox test runner (P0 skeleton)"
)]
struct Args {
    /// Target mode: all | mcp | http | mcp-fake | fake
    #[arg(long, default_value = "all")]
    target: String,

    /// Filter by scenario name or test ID
    #[arg(long)]
    scenario: Option<String>,

    /// Path to emit machine-readable JSON report
    #[arg(long)]
    report: Option<PathBuf>,

    /// Bridge HTTP URL for device/live testing
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    url: String,

    /// Auth token for bridge
    #[arg(long)]
    token: Option<String>,

    /// Scenarios directory
    #[arg(long, default_value = "tests/blackbox/scenarios")]
    scenarios_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDef {
    pub id: String,
    pub name: String,
    pub scenario_file: String,
    pub target: String,
    pub is_device: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub name: String,
    pub target: String,
    pub status: String, // "pass" | "fail" | "skip" | "device-skip"
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_bundle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub summary: ReportSummary,
    pub tests: Vec<TestResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub device_skipped: usize,
    pub duration_ms: u64,
}

fn get_all_tests() -> Vec<TestDef> {
    vec![
        // Group A: Contract shape
        TestDef {
            id: "BB-A0".to_string(),
            name: "MCP handshake: tool list and capabilities".to_string(),
            scenario_file: "mcp_handshake.json".to_string(),
            target: "mcp".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A1".to_string(),
            name: "/ui-tree shape: rev, nodes, depth cap".to_string(),
            scenario_file: "static_screen.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A2".to_string(),
            name: "/tap, /type, /keypress, /back return {ok: true}".to_string(),
            scenario_file: "mutating_actions.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A3".to_string(),
            name: "Error envelope conforms to taxonomy".to_string(),
            scenario_file: "error_envelope.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A4".to_string(),
            name: "/screenshot PNG magic bytes & scale".to_string(),
            scenario_file: "screenshot_contract.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A5".to_string(),
            name: "/ws emits tree_changed {rev} strictly increasing".to_string(),
            scenario_file: "ws_events.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-A6".to_string(),
            name: "Token budget on deep gauntlet screen".to_string(),
            scenario_file: "token_budget.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        // Group B: State machine behaviors
        TestDef {
            id: "BB-B1".to_string(),
            name: "Unconfigured server -> NO_CONFIG + fix hint".to_string(),
            scenario_file: "no_config.json".to_string(),
            target: "mcp".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B2".to_string(),
            name: "Unreachable device -> OFFLINE + WS reconnect".to_string(),
            scenario_file: "device_offline.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B3".to_string(),
            name: "Token mismatch -> AUTH_FAILED".to_string(),
            scenario_file: "auth_reject.json".to_string(),
            target: "http".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B4".to_string(),
            name: "Stale rev tap -> NODE_OUTDATED".to_string(),
            scenario_file: "stale_screen.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B5".to_string(),
            name: "Backgrounded app mutations -> APP_NOT_FOREGROUND".to_string(),
            scenario_file: "backgrounded.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
        TestDef {
            id: "BB-B6".to_string(),
            name: "Screen off / locked -> SCREEN_OFF / DEVICE_LOCKED".to_string(),
            scenario_file: "lock_screen.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
        TestDef {
            id: "BB-B7".to_string(),
            name: "human_win digitizer touch cancels injection".to_string(),
            scenario_file: "human_win.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
        TestDef {
            id: "BB-B8".to_string(),
            name: "Single-flight lock: second client gets BUSY".to_string(),
            scenario_file: "busy_second_client.json".to_string(),
            target: "mcp".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B9".to_string(),
            name: "Lock release on disconnect and idle timeout".to_string(),
            scenario_file: "lock_release.json".to_string(),
            target: "mcp".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B10".to_string(),
            name: "verbose:true full JSON error vs one-liner".to_string(),
            scenario_file: "verbose_errors.json".to_string(),
            target: "mcp".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B11".to_string(),
            name: "Optional device flow: rotate token via overlay".to_string(),
            scenario_file: "token_rotation.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
        // Group C: Goal flows
        TestDef {
            id: "BB-C1".to_string(),
            name: "login_flow completes unattended on VelaVoice".to_string(),
            scenario_file: "login_flow.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
        TestDef {
            id: "BB-C2".to_string(),
            name: "slow_screen wait_for timeout <= 30s + HTTP fallback".to_string(),
            scenario_file: "slow_screen.json".to_string(),
            target: "mcp-fake".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-C3".to_string(),
            name: "steer_flow cooperative steering".to_string(),
            scenario_file: "steer_flow.json".to_string(),
            target: "device".to_string(),
            is_device: true,
        },
    ]
}

fn write_evidence_bundle(test_id: &str, test_name: &str, reason: &str) -> String {
    let bundle_dir = PathBuf::from("tmp/evidence").join(test_id);
    let _ = fs::create_dir_all(&bundle_dir);

    let summary = serde_json::json!({
        "test_id": test_id,
        "name": test_name,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "reason": reason,
        "trace": [
            format!("Starting blackbox test {test_id}"),
            format!("Evaluating prerequisites for {test_id}"),
            format!("FAILED: {reason}")
        ]
    });

    let _ = fs::write(
        bundle_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap_or_default(),
    );
    let _ = fs::write(
        bundle_dir.join("red-run.txt"),
        format!("TEST {test_id}: {test_name}\nSTATUS: FAIL\nREASON: {reason}\n"),
    );

    bundle_dir.to_string_lossy().to_string()
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let start_time = Instant::now();

    let all_tests = get_all_tests();

    // Filter tests by target and scenario
    let selected_tests: Vec<TestDef> = all_tests
        .into_iter()
        .filter(|t| {
            if let Some(ref sc) = args.scenario {
                if !t.id.eq_ignore_ascii_case(sc)
                    && !t.name.to_lowercase().contains(&sc.to_lowercase())
                    && !t.scenario_file.contains(sc)
                {
                    return false;
                }
            }

            match args.target.as_str() {
                "all" => true,
                "mcp" => t.target == "mcp",
                "http" => t.target == "http",
                "mcp-fake" | "fake" => t.target == "mcp-fake" || t.target == "fake",
                other => t.target == other,
            }
        })
        .collect();

    println!("================================================================================");
    println!("Texure Blackbox Test Suite (Runner P0)");
    println!(
        "Target filter: {} | Selected tests: {}",
        args.target,
        selected_tests.len()
    );
    println!("================================================================================");
    println!(
        "{:<8} {:<10} {:<12} {:>8}   REASON",
        "ID", "TARGET", "STATUS", "DURATION"
    );
    println!("--------------------------------------------------------------------------------");

    let mut results = Vec::new();

    for test in selected_tests {
        let test_start = Instant::now();

        // Check if scenario file exists and is valid DSL
        let scenario_path = args.scenarios_dir.join(&test.scenario_file);
        let scenario_check = if scenario_path.exists() {
            scenario::Scenario::load_from_file(&scenario_path)
        } else {
            Err(scenario::ScenarioError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "scenario file {} not found in {}",
                    test.scenario_file,
                    args.scenarios_dir.display()
                ),
            )))
        };

        let result = if test.is_device {
            // Device tests are tagged device and skip cleanly when no phone is configured
            let duration_ms = test_start.elapsed().as_millis() as u64;
            TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "device-skip".to_string(),
                duration_ms,
                reason: Some("device test skipped (no phone configured/connected)".to_string()),
                evidence_bundle: None,
            }
        } else {
            // In P0, no server or bridge implementation exists yet.
            // All non-device tests fail with a clean reason and generate evidence bundles.
            let duration_ms = test_start.elapsed().as_millis() as u64;
            let reason = if let Err(e) = scenario_check {
                format!("Scenario validation failed: {e}")
            } else {
                match test.target.as_str() {
                    "mcp" => {
                        "MCP client error: texure-server binary not found; server not implemented yet"
                            .to_string()
                    }
                    "http" => {
                        format!("HTTP bridge connection failed: {} offline (not implemented yet)", args.url)
                    }
                    _ => "FakeBridge target not implemented yet (pending #1)".to_string(),
                }
            };

            let bundle_path = write_evidence_bundle(&test.id, &test.name, &reason);

            TestResult {
                id: test.id.clone(),
                name: test.name.clone(),
                target: test.target.clone(),
                status: "fail".to_string(),
                duration_ms,
                reason: Some(reason),
                evidence_bundle: Some(bundle_path),
            }
        };

        println!(
            "{:<8} {:<10} {:<12} {:>6}ms   {}",
            result.id,
            result.target,
            result.status.to_uppercase(),
            result.duration_ms,
            result.reason.as_deref().unwrap_or("")
        );

        results.push(result);
    }

    let total_duration_ms = start_time.elapsed().as_millis() as u64;
    let passed = results.iter().filter(|r| r.status == "pass").count();
    let failed = results.iter().filter(|r| r.status == "fail").count();
    let skipped = results.iter().filter(|r| r.status == "skip").count();
    let device_skipped = results.iter().filter(|r| r.status == "device-skip").count();

    println!("--------------------------------------------------------------------------------");
    println!(
        "Summary: {} total | {} passed | {} failed | {} skipped | {} device-skipped | Duration: {}ms",
        results.len(),
        passed,
        failed,
        skipped,
        device_skipped,
        total_duration_ms
    );
    println!("================================================================================");

    let report = Report {
        summary: ReportSummary {
            total: results.len(),
            passed,
            failed,
            skipped,
            device_skipped,
            duration_ms: total_duration_ms,
        },
        tests: results,
    };

    if let Some(ref report_path) = args.report {
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json_str) = serde_json::to_string_pretty(&report) {
            let _ = fs::write(report_path, json_str);
            println!("Report written to: {}", report_path.display());
        }
    }

    // Exit code: 0 iff no unskipped-or-device-skipped FAIL; nonzero otherwise.
    if failed > 0 {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}
