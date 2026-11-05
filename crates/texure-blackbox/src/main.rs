pub mod client_http;
pub mod client_mcp;
pub mod fake_bridge;
pub mod fake_mcp;
pub mod runner;
pub mod scenario;

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "texure-blackbox", about = "Texure blackbox test runner")]
struct Args {
    /// Target mode: all | mcp | http | mcp-fake | mcp-real
    #[arg(long, default_value = "all")]
    target: String,

    /// Filter by scenario name or test ID
    #[arg(long)]
    scenario: Option<String>,

    /// Path to emit machine-readable JSON report
    #[arg(long)]
    report: Option<PathBuf>,

    /// Bridge HTTP URL for device/live testing
    #[arg(long, default_value = "http://127.0.0.1:8765")]
    url: String,

    /// Auth token for bridge
    #[arg(long)]
    token: Option<String>,

    /// Scenarios directory
    #[arg(long, default_value = "tests/blackbox/scenarios")]
    scenarios_dir: PathBuf,

    /// Repository root directory
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Run in fake MCP stdio mode (used for MCP testing)
    #[arg(long)]
    fake_mcp: bool,
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

pub fn get_all_tests() -> Vec<TestDef> {
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
            target: "mcp".to_string(),
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

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Check if running as fake MCP stdio server
    if args.fake_mcp {
        let sc = args.scenario.unwrap_or_else(|| "static_screen".to_string());
        let server = fake_mcp::FakeMcpServer::new(sc, Some(args.url));
        if let Err(e) = server.run_stdio() {
            eprintln!("fake-mcp stdio error: {e}");
            std::process::exit(1);
        }
        return;
    }

    let start_time = Instant::now();
    let all_tests = get_all_tests();

    // Filter tests by target and scenario
    let mut selected_tests: Vec<TestDef> = all_tests
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
                "mcp-fake" | "fake" => {
                    t.target == "mcp-fake" || t.target == "fake" || t.target == "mcp"
                }
                "mcp-real" => t.target == "mcp" || t.target == "mcp-fake" || t.target == "http",
                other => t.target == other,
            }
        })
        .collect();

    // mcp-real runs the same scenario suite through the real texure-server
    // binary; `--target all` appends those runs alongside the fake ones.
    match args.target.as_str() {
        "mcp-real" => {
            for test in &mut selected_tests {
                test.target = "mcp-real".to_string();
            }
        }
        "all" => {
            let real_runs: Vec<TestDef> = selected_tests
                .iter()
                .filter(|t| t.target == "mcp" || t.target == "mcp-fake" || t.target == "http")
                .map(|t| TestDef {
                    target: "mcp-real".to_string(),
                    ..t.clone()
                })
                .collect();
            selected_tests.extend(real_runs);
        }
        _ => {}
    }

    println!("================================================================================");
    println!("Texure Blackbox Test Suite");
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
        let result = runner::run_test(
            &test,
            &args.scenarios_dir,
            &args.root,
            &args.url,
            args.token.as_deref(),
        )
        .await;

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
