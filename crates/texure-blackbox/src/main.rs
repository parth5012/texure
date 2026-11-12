pub mod client_http;
pub mod client_mcp;
pub mod fake_bridge;
pub mod fake_mcp;
pub mod history;
pub mod recording;
pub mod runner;
pub mod scenario;
pub mod trace;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(
    name = "texure-blackbox",
    about = "Texure blackbox test runner + workflow record/replay"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Record a human workflow on a live bridge: triggers
    /// POST /recording/start, waits for Enter or --duration-secs, then
    /// exports the trace to a scenario JSON + codemode script.
    Record {
        /// Bridge HTTP URL
        #[arg(long, default_value = "http://127.0.0.1:8765")]
        url: String,

        /// Auth token for bridge
        #[arg(long)]
        token: Option<String>,

        /// Output scenario JSON path
        #[arg(long)]
        out: PathBuf,

        /// Record for this many seconds instead of waiting for Enter
        #[arg(long)]
        duration_secs: Option<u64>,

        /// Codemode script output (default: <out>.codemode.ts)
        #[arg(long)]
        codemode_out: Option<PathBuf>,
    },

    /// Deterministically replay a recorded scenario and report pass-rate.
    Replay {
        /// Scenario JSON path (native blackbox DSL)
        #[arg(long)]
        scenario: PathBuf,

        /// Replay target: http (live bridge) | mcp-fake (built-in FakeBridge)
        #[arg(long, default_value = "mcp-fake")]
        target: String,

        /// Bridge HTTP URL (http target only)
        #[arg(long, default_value = "http://127.0.0.1:8765")]
        url: String,

        /// Auth token (http target)
        #[arg(long)]
        token: Option<String>,

        /// Optional JSON ReplayReport output
        #[arg(long)]
        report: Option<PathBuf>,
    },
}

#[derive(Parser, Debug)]
#[command(
    name = "texure-blackbox",
    about = "Texure blackbox test runner & regression tracking"
)]
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

    /// Query regression run history from SQLite
    #[arg(long)]
    history: bool,

    /// History query limit (default: 20)
    #[arg(long, default_value = "20")]
    limit: usize,

    /// History output format: table | json (default: table)
    #[arg(long, default_value = "table")]
    format: String,

    /// Path to custom history SQLite database (default: .texure/history.db)
    #[arg(long)]
    history_db: Option<PathBuf>,

    /// Render self-contained HTML trace for a run: run ID in the history DB,
    /// or a path to a history DB file (renders its newest run)
    #[arg(long)]
    trace: Option<String>,

    /// Render regression trend HTML dashboard from run history
    #[arg(long)]
    trend: bool,

    /// Output HTML path for --trace / --trend (default: trace.html | trend.html)
    #[arg(long)]
    out: Option<PathBuf>,
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
        TestDef {
            id: "BB-B12".to_string(),
            name: "Recording session lifecycle: start/stop, RECORDING_IDLE, idempotent".to_string(),
            scenario_file: "recording_lifecycle.json".to_string(),
            target: "http".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B13".to_string(),
            name: "Mutations execute cleanly while recording (no hijack)".to_string(),
            scenario_file: "record_replay.json".to_string(),
            target: "http".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B14".to_string(),
            name: "Recorded event mapping: tap/type/keypress buffered to export".to_string(),
            scenario_file: "record_replay.json".to_string(),
            target: "http".to_string(),
            is_device: false,
        },
        TestDef {
            id: "BB-B15".to_string(),
            name: "DELETE clears recording buffer; export count 0".to_string(),
            scenario_file: "recording_export.json".to_string(),
            target: "http".to_string(),
            is_device: false,
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
    // Subcommand mode (record/replay) — ticket #24. No subcommand ⇒ the
    // original flag-driven suite run below.
    let argv: Vec<String> = std::env::args().collect();
    if matches!(
        argv.get(1).map(String::as_str),
        Some("record") | Some("replay")
    ) {
        let cli = Cli::parse();
        let exit = match cli.command {
            Commands::Record {
                url,
                token,
                out,
                duration_secs,
                codemode_out,
            } => run_record(&url, token, out, duration_secs, codemode_out).await,
            Commands::Replay {
                scenario,
                target,
                url,
                token,
                report,
            } => run_replay(&scenario, &target, &url, token, report).await,
        };
        std::process::exit(exit);
    }

    let args = Args::parse();

    // HTML report modes (--trace / --trend): read-only over the history DB,
    // zero-config default .texure/history.db (issue #27).
    if args.trace.is_some() || args.trend {
        if args.trace.is_some() && args.trend && args.out.is_some() {
            eprintln!(
                "--out is ambiguous when both --trace and --trend are given; run them separately"
            );
            std::process::exit(2);
        }
        if let Some(ref trace_arg) = args.trace {
            if let Err(e) = run_trace_command(trace_arg, &args) {
                eprintln!("Error rendering trace: {e}");
                std::process::exit(1);
            }
        }
        if args.trend {
            if let Err(e) = run_trend_command(&args) {
                eprintln!("Error rendering trend: {e}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    // Check if querying history
    if args.history {
        let db_path = args
            .history_db
            .clone()
            .unwrap_or_else(|| args.root.join(history::HistoryStore::DEFAULT_DB_PATH));
        match history::HistoryStore::open_at(db_path) {
            Ok(store) => match store.render_history_cli(args.limit, &args.format) {
                Ok(output) => {
                    print!("{output}");
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Error rendering history: {e}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("Error opening history store: {e}");
                std::process::exit(1);
            }
        }
    }

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
    let mut step_traces: Vec<(String, Vec<runner::StepTrace>)> = Vec::new();

    for test in selected_tests {
        let (result, steps) = runner::run_test(
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

        step_traces.push((result.id.clone(), steps));
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
        tests: results.clone(),
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

    // Auto-record to SQLite history store
    let db_path = args
        .history_db
        .clone()
        .unwrap_or_else(|| args.root.join(history::HistoryStore::DEFAULT_DB_PATH));
    match history::HistoryStore::open_at(db_path) {
        Ok(store) => {
            let run_id = history::generate_run_id();
            // Git metadata must come from the repository the caller targeted
            // via --root, not the process's current directory.
            let git_meta = |git_args: &[&str]| -> Option<String> {
                let out = std::process::Command::new("git")
                    .args(git_args)
                    .current_dir(&args.root)
                    .output()
                    .ok()?;
                if !out.status.success() {
                    return None;
                }
                let s = String::from_utf8(out.stdout).ok()?;
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            };
            let branch = git_meta(&["rev-parse", "--abbrev-ref", "HEAD"]);
            let build_hash = git_meta(&["rev-parse", "--short", "HEAD"]);

            let run_record = history::RunRecord {
                run_id: run_id.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                app_version_name: None,
                build_hash,
                branch,
                total_scenarios: results.len(),
                passed,
                failed,
                duration_ms: total_duration_ms,
            };

            let scenario_records: Vec<history::ScenarioResultRecord> = results
                .iter()
                .map(|r| history::ScenarioResultRecord {
                    id: None,
                    run_id: run_id.clone(),
                    scenario_id: r.id.clone(),
                    name: r.name.clone(),
                    status: r.status.to_uppercase(),
                    target: Some(r.target.clone()),
                    duration_ms: r.duration_ms,
                    // Only recognized error codes; skip/scenario-load
                    // reasons are free text and must not become codes.
                    error_code: r
                        .reason
                        .as_deref()
                        .and_then(runner::extract_error_code)
                        .map(|s| s.to_string()),
                    error_message: r.reason.clone(),
                    evidence_bundle_path: r.evidence_bundle.clone(),
                })
                .collect();

            let step_records: Vec<history::StepRecord> = step_traces
                .iter()
                .flat_map(|(scenario_id, steps)| {
                    let run_id = run_id.clone();
                    steps.iter().map(move |st| history::StepRecord {
                        id: None,
                        run_id: run_id.clone(),
                        scenario_id: scenario_id.clone(),
                        step_index: st.step_index,
                        tool_name: st.tool_name.clone(),
                        params_json: st.params_json.clone(),
                        result_json: st.result_json.clone(),
                        rev_before: st.rev_before,
                        rev_after: st.rev_after,
                        timestamp_offset_ms: st.timestamp_offset_ms,
                    })
                })
                .collect();

            if let Err(e) = store.record_run(&run_record, &scenario_records, &step_records) {
                eprintln!("Warning: failed to record run history into SQLite: {e}");
            }
        }
        Err(e) => {
            eprintln!("Warning: failed to initialize SQLite history store: {e}");
        }
    }

    // Exit code: 0 iff no unskipped-or-device-skipped FAIL; nonzero otherwise.
    if failed > 0 {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

async fn run_record(
    url: &str,
    token: Option<String>,
    out: PathBuf,
    duration_secs: Option<u64>,
    codemode_out: Option<PathBuf>,
) -> i32 {
    let token = match token {
        Some(t) if !t.is_empty() => t,
        _ => {
            eprintln!("record: --token is required (auth on all bridge routes)");
            return 2;
        }
    };

    let client = client_http::HttpBridgeClient::new(url.to_string(), Some(token));

    if let Err(e) = client.recording_start().await {
        eprintln!("record: start failed: {e}");
        return 1;
    }
    println!("recording started on {url} …");

    match duration_secs {
        Some(secs) if secs > 0 => {
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
        }
        _ => {
            println!("press Enter to stop recording");
            let stdin = std::io::stdin();
            let mut line = String::new();
            let _ = stdin.lock().read_line(&mut line);
        }
    }

    if let Err(e) = client.recording_stop().await {
        eprintln!("record: stop failed: {e}");
        return 1;
    }
    let trace = match client.recording_export().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("record: export failed: {e}");
            return 1;
        }
    };

    let name = out
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("recorded_workflow");
    let scenario = match recording::export_to_scenario(&trace, name) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("record: scenario export failed: {e}");
            return 1;
        }
    };

    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(
        &out,
        serde_json::to_string_pretty(&scenario).unwrap_or_default(),
    ) {
        eprintln!("record: failed to write {}: {e}", out.display());
        return 1;
    }
    println!("scenario written to {}", out.display());

    let codemode_path = codemode_out.unwrap_or_else(|| {
        let mut p = out.clone();
        p.set_extension("codemode.ts");
        p
    });
    let script = recording::export_to_codemode(&trace, name);
    if let Err(e) = fs::write(&codemode_path, script) {
        eprintln!(
            "record: failed to write codemode script {}: {e}",
            codemode_path.display()
        );
        return 1;
    }
    println!("codemode script written to {}", codemode_path.display());

    0
}

/// Replays a scenario against `target`. `--target mcp-fake` runs the
/// built-in FakeBridge in-process (L2); `http` drives the bridge at --url.
/// Deterministic pass-rate is printed; failures emit an evidence bundle
/// under tmp/evidence/replay-<scenario>/ and exit 1.
async fn run_replay(
    scenario_path: &PathBuf,
    target: &str,
    url: &str,
    token: Option<String>,
    report: Option<PathBuf>,
) -> i32 {
    let scenario = match scenario::Scenario::load_from_file(scenario_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "replay: failed to load scenario {}: {e}",
                scenario_path.display()
            );
            return 1;
        }
    };

    let mut fake_server: Option<fake_bridge::FakeBridgeServer> = None;
    let client = match target {
        "mcp-fake" => {
            let mut server =
                match fake_bridge::FakeBridgeServer::start(fake_bridge::FakeBridgeConfig {
                    scenario_name: "static_screen".to_string(),
                    root_dir: ".".to_string(),
                    port: 0,
                    token: "test-token".to_string(),
                })
                .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("replay: failed to start FakeBridge: {e}");
                        return 1;
                    }
                };
            let base = format!("http://127.0.0.1:{}", server.addr.port());
            let _ = &mut server;
            fake_server = Some(server);
            client_http::HttpBridgeClient::new(base, Some("test-token".to_string()))
        }
        "http" => {
            let token = match token {
                Some(t) if !t.is_empty() => t,
                _ => {
                    eprintln!("replay: --token is required for --target http");
                    return 2;
                }
            };
            client_http::HttpBridgeClient::new(url.to_string(), Some(token))
        }
        other => {
            eprintln!("replay: unsupported --target '{other}' (http | mcp-fake)");
            return 2;
        }
    };

    let result = recording::replay_scenario(&scenario, &client).await;
    if let Some(mut s) = fake_server.take() {
        s.stop();
    }

    println!(
        "Replay '{}': {}/{} steps passed (pass-rate {:.0}%)",
        result.scenario,
        result.passed,
        result.total,
        (result.pass_rate * 100.0).round()
    );
    for outcome in &result.outcomes {
        if !outcome.ok {
            println!(
                "  FAIL step {} ({}) : {}",
                outcome.index,
                outcome.action,
                outcome.reason.as_deref().unwrap_or("")
            );
        }
    }

    if let Some(report_path) = report {
        if let Some(parent) = report_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json_str) = serde_json::to_string_pretty(&result) {
            let _ = fs::write(&report_path, json_str);
            println!("Replay report written to {}", report_path.display());
        }
    }

    if result.pass_rate < 1.0 {
        let bundle =
            std::path::Path::new("tmp/evidence").join(format!("replay-{}", result.scenario));
        let _ = fs::create_dir_all(&bundle);
        let _ = fs::write(
            bundle.join("replay-report.json"),
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        );
        println!("Evidence bundle written to {}", bundle.display());
        return 1;
    }

    0
}

/// `--trace <run-id|db-path>`: render the self-contained HTML trace viewer for
/// one recorded run. A filesystem path is treated as a history DB and renders
/// its newest run; anything else is a run ID resolved against the default (or
/// --history_db) database.
fn run_trace_command(trace_arg: &str, args: &Args) -> Result<(), String> {
    let arg_path = PathBuf::from(trace_arg);
    let is_db_file = arg_path.is_file()
        && arg_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("db") || e.eq_ignore_ascii_case("sqlite"))
            .unwrap_or(false);

    let (db_path, run_id) = if is_db_file {
        (arg_path, None) // newest run in the given DB
    } else {
        (
            args.history_db
                .clone()
                .unwrap_or_else(|| PathBuf::from(history::HistoryStore::DEFAULT_DB_PATH)),
            Some(trace_arg.to_string()),
        )
    };

    let store = history::HistoryStore::open_at(db_path).map_err(|e| e.to_string())?;

    let run_id = match run_id {
        Some(id) => id,
        None => store
            .list_runs(1)
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .map(|r| r.run_id)
            .ok_or_else(|| "History DB contains no runs".to_string())?,
    };

    let (run, scenarios) = store
        .get_run(&run_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Run '{run_id}' not found in history DB"))?;

    let mut steps = Vec::new();
    for s in &scenarios {
        steps.extend(
            store
                .get_scenario_steps(&run_id, &s.scenario_id)
                .map_err(|e| e.to_string())?,
        );
    }

    let html = trace::render_trace_html(&run, &scenarios, &steps);
    write_html_output(html, args.out.as_deref(), "trace.html")
}

/// `--trend`: render the regression trend HTML dashboard from the history DB.
fn run_trend_command(args: &Args) -> Result<(), String> {
    let db_path = args
        .history_db
        .clone()
        .unwrap_or_else(|| PathBuf::from(history::HistoryStore::DEFAULT_DB_PATH));
    let store = history::HistoryStore::open_at(db_path).map_err(|e| e.to_string())?;

    let runs = store.list_runs(args.limit).map_err(|e| e.to_string())?;
    let stats = store
        .get_scenario_stats(args.limit)
        .map_err(|e| e.to_string())?;

    let html = trace::render_trend_html(&runs, &stats);
    write_html_output(html, args.out.as_deref(), "trend.html")
}

fn write_html_output(html: String, out: Option<&Path>, default_name: &str) -> Result<(), String> {
    let path = out
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default_name));
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(&path, html).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    println!("HTML written to: {}", path.display());
    Ok(())
}
