use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HistoryError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Unsupported history format: {0} (expected table or json)")]
    UnsupportedFormat(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRecord {
    pub run_id: String,
    pub timestamp: String,
    pub app_version_name: Option<String>,
    pub build_hash: Option<String>,
    pub branch: Option<String>,
    pub total_scenarios: usize,
    pub passed: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioResultRecord {
    pub id: Option<i64>,
    pub run_id: String,
    pub scenario_id: String,
    pub name: String,
    pub status: String, // PASS, FAIL, SKIP, DEVICE-SKIP
    /// Runner target this execution ran against (mcp, mcp-real, http, ...).
    /// `--target all` executes the same scenario_id on several targets, so
    /// scenario identity alone is ambiguous (issue review: target column).
    pub target: Option<String>,
    pub duration_ms: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub evidence_bundle_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepRecord {
    pub id: Option<i64>,
    pub run_id: String,
    pub scenario_id: String,
    /// Target this step executed on (matches scenario_results.target).
    pub target: Option<String>,
    pub step_index: usize,
    pub tool_name: String,
    pub params_json: Option<String>,
    pub result_json: Option<String>,
    pub rev_before: Option<i64>,
    pub rev_after: Option<i64>,
    pub timestamp_offset_ms: u64,
}

/// Per-scenario pass-rate aggregation over the most recent N runs,
/// powering the regression trend report's flaky detection (issue #27).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioTrend {
    pub scenario_id: String,
    pub name: String,
    pub runs: usize,
    pub passes: usize,
    pub fails: usize,
    pub skips: usize,
}

impl ScenarioTrend {
    /// Pass rate in [0.0, 1.0] across the aggregated runs.
    pub fn pass_rate(&self) -> f64 {
        if self.runs == 0 {
            0.0
        } else {
            self.passes as f64 / self.runs as f64
        }
    }

    /// Flaky = sometimes passes, sometimes fails: strictly between 0% and 100%.
    pub fn is_flaky(&self) -> bool {
        self.runs >= 2 && self.pass_rate() > 0.0 && self.pass_rate() < 1.0
    }
}

pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub const DEFAULT_DB_PATH: &'static str = ".texure/history.db";

    pub fn open_default() -> Result<Self, HistoryError> {
        Self::open_at(PathBuf::from(Self::DEFAULT_DB_PATH))
    }

    pub fn open_at(db_path: PathBuf) -> Result<Self, HistoryError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let store = Self { db_path };
        store.init_schema()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, HistoryError> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(conn)
    }

    pub fn init_schema(&self) -> Result<(), HistoryError> {
        let conn = self.connect()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                app_version_name TEXT,
                build_hash TEXT,
                branch TEXT,
                total_scenarios INTEGER NOT NULL,
                passed INTEGER NOT NULL,
                failed INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS scenario_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                scenario_id TEXT NOT NULL,
                name TEXT NOT NULL,
                status TEXT NOT NULL,
                target TEXT,
                duration_ms INTEGER NOT NULL,
                error_code TEXT,
                error_message TEXT,
                evidence_bundle_path TEXT,
                FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS step_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                scenario_id TEXT NOT NULL,
                target TEXT,
                step_index INTEGER NOT NULL,
                tool_name TEXT NOT NULL,
                params_json TEXT,
                result_json TEXT,
                rev_before INTEGER,
                rev_after INTEGER,
                timestamp_offset_ms INTEGER NOT NULL,
                FOREIGN KEY (run_id) REFERENCES runs(run_id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_runs_timestamp ON runs(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_scenario_results_run ON scenario_results(run_id);
            CREATE INDEX IF NOT EXISTS idx_step_records_run_scenario ON step_records(run_id, scenario_id);",
        )?;
        // Migration for DBs created before the target column existed
        // (--target all runs were ambiguous across mcp / mcp-real).
        Self::ensure_column(&conn, "scenario_results", "target", "TEXT")?;
        Self::ensure_column(&conn, "step_records", "target", "TEXT")?;
        Ok(())
    }

    /// Add `column` to `table` if it is missing (internal constants only).
    fn ensure_column(
        conn: &Connection,
        table: &str,
        column: &str,
        decl: &str,
    ) -> Result<(), HistoryError> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == column {
                return Ok(());
            }
        }
        conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl};"))?;
        Ok(())
    }

    pub fn record_run(
        &self,
        run: &RunRecord,
        scenarios: &[ScenarioResultRecord],
        steps: &[StepRecord],
    ) -> Result<(), HistoryError> {
        let mut conn = self.connect()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO runs (
                run_id, timestamp, app_version_name, build_hash, branch,
                total_scenarios, passed, failed, duration_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![
                run.run_id,
                run.timestamp,
                run.app_version_name,
                run.build_hash,
                run.branch,
                run.total_scenarios as i64,
                run.passed as i64,
                run.failed as i64,
                run.duration_ms as i64,
            ],
        )?;

        for s in scenarios {
            tx.execute(
                "INSERT INTO scenario_results (
                    run_id, scenario_id, name, status, target, duration_ms,
                    error_code, error_message, evidence_bundle_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
                params![
                    s.run_id,
                    s.scenario_id,
                    s.name,
                    s.status,
                    s.target,
                    s.duration_ms as i64,
                    s.error_code,
                    s.error_message,
                    s.evidence_bundle_path,
                ],
            )?;
        }

        for st in steps {
            tx.execute(
                "INSERT INTO step_records (
                    run_id, scenario_id, target, step_index, tool_name,
                    params_json, result_json, rev_before, rev_after,
                    timestamp_offset_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
                params![
                    st.run_id,
                    st.scenario_id,
                    st.target,
                    st.step_index as i64,
                    st.tool_name,
                    st.params_json,
                    st.result_json,
                    st.rev_before,
                    st.rev_after,
                    st.timestamp_offset_ms as i64,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn list_runs(&self, limit: usize) -> Result<Vec<RunRecord>, HistoryError> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, timestamp, app_version_name, build_hash, branch,
                    total_scenarios, passed, failed, duration_ms
             FROM runs
             ORDER BY timestamp DESC
             LIMIT ?1;",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(RunRecord {
                run_id: row.get(0)?,
                timestamp: row.get(1)?,
                app_version_name: row.get(2)?,
                build_hash: row.get(3)?,
                branch: row.get(4)?,
                total_scenarios: row.get::<_, i64>(5)? as usize,
                passed: row.get::<_, i64>(6)? as usize,
                failed: row.get::<_, i64>(7)? as usize,
                duration_ms: row.get::<_, i64>(8)? as u64,
            })
        })?;

        let mut runs = Vec::new();
        for r in rows {
            runs.push(r?);
        }
        Ok(runs)
    }

    pub fn get_run(
        &self,
        run_id: &str,
    ) -> Result<Option<(RunRecord, Vec<ScenarioResultRecord>)>, HistoryError> {
        let conn = self.connect()?;
        let mut run_stmt = conn.prepare(
            "SELECT run_id, timestamp, app_version_name, build_hash, branch,
                    total_scenarios, passed, failed, duration_ms
             FROM runs
             WHERE run_id = ?1;",
        )?;

        let run = run_stmt
            .query_row(params![run_id], |row| {
                Ok(RunRecord {
                    run_id: row.get(0)?,
                    timestamp: row.get(1)?,
                    app_version_name: row.get(2)?,
                    build_hash: row.get(3)?,
                    branch: row.get(4)?,
                    total_scenarios: row.get::<_, i64>(5)? as usize,
                    passed: row.get::<_, i64>(6)? as usize,
                    failed: row.get::<_, i64>(7)? as usize,
                    duration_ms: row.get::<_, i64>(8)? as u64,
                })
            })
            .optional()?;

        let Some(run_record) = run else {
            return Ok(None);
        };

        let mut scen_stmt = conn.prepare(
            "SELECT id, run_id, scenario_id, name, status, target, duration_ms,
                    error_code, error_message, evidence_bundle_path
             FROM scenario_results
             WHERE run_id = ?1
             ORDER BY id ASC;",
        )?;

        let scen_rows = scen_stmt.query_map(params![run_id], |row| {
            Ok(ScenarioResultRecord {
                id: Some(row.get(0)?),
                run_id: row.get(1)?,
                scenario_id: row.get(2)?,
                name: row.get(3)?,
                status: row.get(4)?,
                target: row.get(5)?,
                duration_ms: row.get::<_, i64>(6)? as u64,
                error_code: row.get(7)?,
                error_message: row.get(8)?,
                evidence_bundle_path: row.get(9)?,
            })
        })?;

        let mut scenarios = Vec::new();
        for s in scen_rows {
            scenarios.push(s?);
        }

        Ok(Some((run_record, scenarios)))
    }

    /// Steps for one scenario execution. `target` disambiguates `--target all`
    /// runs where the same scenario_id executed on several targets; `None`
    /// matches legacy rows recorded before the target column existed.
    pub fn get_scenario_steps(
        &self,
        run_id: &str,
        scenario_id: &str,
        target: Option<&str>,
    ) -> Result<Vec<StepRecord>, HistoryError> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, scenario_id, target, step_index, tool_name,
                    params_json, result_json, rev_before, rev_after, timestamp_offset_ms
             FROM step_records
             WHERE run_id = ?1 AND scenario_id = ?2 AND (target IS NULL OR target = ?3)
             ORDER BY step_index ASC;",
        )?;

        let rows = stmt.query_map(params![run_id, scenario_id, target], |row| {
            Ok(StepRecord {
                id: Some(row.get(0)?),
                run_id: row.get(1)?,
                scenario_id: row.get(2)?,
                target: row.get(3)?,
                step_index: row.get::<_, i64>(4)? as usize,
                tool_name: row.get(5)?,
                params_json: row.get(6)?,
                result_json: row.get(7)?,
                rev_before: row.get(8)?,
                rev_after: row.get(9)?,
                timestamp_offset_ms: row.get::<_, i64>(10)? as u64,
            })
        })?;

        let mut steps = Vec::new();
        for r in rows {
            steps.push(r?);
        }
        Ok(steps)
    }

    pub fn get_scenario_stats(&self, limit: usize) -> Result<Vec<ScenarioTrend>, HistoryError> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT sr.scenario_id, MAX(sr.name) AS name, COUNT(*) AS runs,
                    SUM(CASE WHEN sr.status = 'PASS' THEN 1 ELSE 0 END) AS passes,
                    SUM(CASE WHEN sr.status = 'FAIL' THEN 1 ELSE 0 END) AS fails,
                    SUM(CASE WHEN sr.status IN ('SKIP', 'DEVICE-SKIP') THEN 1 ELSE 0 END) AS skips
             FROM scenario_results AS sr
             WHERE sr.run_id IN (
                 SELECT run_id FROM runs ORDER BY timestamp DESC, run_id DESC LIMIT ?1
             )
             GROUP BY sr.scenario_id
             ORDER BY sr.scenario_id ASC;",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ScenarioTrend {
                scenario_id: row.get(0)?,
                name: row.get(1)?,
                runs: row.get::<_, i64>(2)? as usize,
                passes: row.get::<_, i64>(3)? as usize,
                fails: row.get::<_, i64>(4)? as usize,
                skips: row.get::<_, i64>(5)? as usize,
            })
        })?;

        let mut stats = Vec::new();
        for r in rows {
            stats.push(r?);
        }
        Ok(stats)
    }

    pub fn render_history_cli(&self, limit: usize, format: &str) -> Result<String, HistoryError> {
        let runs = self.list_runs(limit)?;
        if format.eq_ignore_ascii_case("json") {
            return Ok(serde_json::to_string_pretty(&runs)?);
        }
        if !format.eq_ignore_ascii_case("table") {
            // Unknown formats must fail loudly: a caller expecting JSON must
            // never silently receive table text.
            return Err(HistoryError::UnsupportedFormat(format.to_string()));
        }

        let mut out = String::new();
        out.push_str("========================================================================================\n");
        out.push_str(&format!(
            "{:<24} {:<20} {:>6} {:>6} {:>6} {:>8} {:<10}\n",
            "RUN ID", "TIMESTAMP", "TOTAL", "PASS", "FAIL", "DURATION", "BRANCH"
        ));
        out.push_str("----------------------------------------------------------------------------------------\n");

        if runs.is_empty() {
            out.push_str("No test run history found in database.\n");
        } else {
            for r in runs {
                out.push_str(&format!(
                    "{:<24} {:<20} {:>6} {:>6} {:>6} {:>6}ms {:<10}\n",
                    r.run_id,
                    r.timestamp.chars().take(19).collect::<String>(),
                    r.total_scenarios,
                    r.passed,
                    r.failed,
                    r.duration_ms,
                    r.branch.as_deref().unwrap_or("unknown")
                ));
            }
        }
        out.push_str("========================================================================================\n");
        Ok(out)
    }
}

pub fn generate_run_id() -> String {
    // Sub-second precision keeps IDs unique across rapid successive runs.
    format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S%.3f"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_schema_idempotent() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("sub/dir/history.db");

        let store = HistoryStore::open_at(db_path.clone()).expect("Failed to open store");
        assert!(db_path.exists());

        // Re-initializing should be clean and idempotent
        assert!(store.init_schema().is_ok());
    }

    #[test]
    fn test_record_and_query_runs() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");

        let run = RunRecord {
            run_id: "run_20261102_120000".to_string(),
            timestamp: "2026-11-02T12:00:00Z".to_string(),
            app_version_name: Some("1.0.0-debug".to_string()),
            build_hash: Some("abc1234".to_string()),
            branch: Some("main".to_string()),
            total_scenarios: 2,
            passed: 1,
            failed: 1,
            duration_ms: 1500,
        };

        let scenarios = vec![
            ScenarioResultRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-A1".to_string(),
                name: "Static UI tree".to_string(),
                status: "PASS".to_string(),
                target: Some("mcp-fake".to_string()),
                duration_ms: 400,
                error_code: None,
                error_message: None,
                evidence_bundle_path: None,
            },
            ScenarioResultRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-B3".to_string(),
                name: "Auth rejection".to_string(),
                status: "FAIL".to_string(),
                target: Some("mcp-fake".to_string()),
                duration_ms: 1100,
                error_code: Some("AUTH_FAILED".to_string()),
                error_message: Some("Token mismatch".to_string()),
                evidence_bundle_path: Some("tmp/evidence/BB-B3".to_string()),
            },
        ];

        let steps = vec![
            StepRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-A1".to_string(),
                target: Some("mcp-fake".to_string()),
                step_index: 0,
                tool_name: "device_tree".to_string(),
                params_json: Some("{}".to_string()),
                result_json: Some(r#"{"rev":1}"#.to_string()),
                rev_before: None,
                rev_after: Some(1),
                timestamp_offset_ms: 100,
            },
            StepRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-B3".to_string(),
                target: Some("mcp-fake".to_string()),
                step_index: 0,
                tool_name: "tap".to_string(),
                params_json: Some(r#"{"node_id":"login_btn"}"#.to_string()),
                result_json: Some(r#"{"error":"AUTH_FAILED"}"#.to_string()),
                rev_before: Some(1),
                rev_after: Some(1),
                timestamp_offset_ms: 250,
            },
        ];

        store
            .record_run(&run, &scenarios, &steps)
            .expect("Failed to record run");

        let runs = store.list_runs(10).expect("Failed to list runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0], run);

        let detail = store
            .get_run(&run.run_id)
            .expect("Failed to get run detail")
            .expect("Run should exist");

        assert_eq!(detail.0, run);
        assert_eq!(detail.1.len(), 2);
        assert_eq!(detail.1[0].scenario_id, "BB-A1");
        assert_eq!(detail.1[0].status, "PASS");
        assert_eq!(detail.1[1].scenario_id, "BB-B3");
        assert_eq!(detail.1[1].status, "FAIL");
        assert_eq!(detail.1[1].error_code.as_deref(), Some("AUTH_FAILED"));

        let bb_a1_steps = store
            .get_scenario_steps(&run.run_id, "BB-A1", Some("mcp-fake"))
            .expect("Failed to query steps");
        assert_eq!(bb_a1_steps.len(), 1);
        assert_eq!(bb_a1_steps[0].tool_name, "device_tree");
        assert_eq!(bb_a1_steps[0].rev_after, Some(1));
    }

    #[test]
    fn test_scenario_stats_flaky_detection() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");

        // Three runs: BB-A1 = pass, pass, fail (flaky); BB-A2 = pass x3 (stable).
        for (i, (a1_status, a2_status)) in [("PASS", "PASS"), ("PASS", "PASS"), ("FAIL", "PASS")]
            .iter()
            .enumerate()
        {
            let run = RunRecord {
                run_id: format!("run_2026111{i}_000000"),
                timestamp: format!("2026-11-1{i}T10:00:00Z"),
                app_version_name: None,
                build_hash: None,
                branch: None,
                total_scenarios: 2,
                passed: if *a1_status == "PASS" { 2 } else { 1 },
                failed: if *a1_status == "PASS" { 0 } else { 1 },
                duration_ms: 1000,
            };
            let scenarios = vec![
                ScenarioResultRecord {
                    id: None,
                    run_id: run.run_id.clone(),
                    scenario_id: "BB-A1".to_string(),
                    name: "Static UI tree".to_string(),
                    status: a1_status.to_string(),
                    target: None,
                    duration_ms: 300,
                    error_code: None,
                    error_message: None,
                    evidence_bundle_path: None,
                },
                ScenarioResultRecord {
                    id: None,
                    run_id: run.run_id.clone(),
                    scenario_id: "BB-A2".to_string(),
                    name: "Mutating actions".to_string(),
                    status: a2_status.to_string(),
                    target: None,
                    duration_ms: 500,
                    error_code: None,
                    error_message: None,
                    evidence_bundle_path: None,
                },
            ];
            store
                .record_run(&run, &scenarios, &[])
                .expect("record failed");
        }

        let stats = store.get_scenario_stats(10).expect("stats failed");
        assert_eq!(stats.len(), 2, "one row per distinct scenario");

        let a1 = stats.iter().find(|s| s.scenario_id == "BB-A1").unwrap();
        assert_eq!(a1.runs, 3);
        assert_eq!(a1.passes, 2);
        assert_eq!(a1.fails, 1);
        assert!((a1.pass_rate() - 2.0 / 3.0).abs() < 1e-9);
        assert!(a1.is_flaky(), "2/3 pass rate must be flagged flaky");

        let a2 = stats.iter().find(|s| s.scenario_id == "BB-A2").unwrap();
        assert_eq!(a2.passes, 3);
        assert!(!a2.is_flaky(), "100% pass rate is not flaky");
    }

    #[test]
    fn test_scenario_stats_respects_limit() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");

        for i in 0..5 {
            let run = RunRecord {
                run_id: format!("run_2026111{i}_000000"),
                timestamp: format!("2026-11-1{i}T10:00:00Z"),
                app_version_name: None,
                build_hash: None,
                branch: None,
                total_scenarios: 1,
                passed: 1,
                failed: 0,
                duration_ms: 1000,
            };
            let scenarios = vec![ScenarioResultRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-A1".to_string(),
                name: "Static UI tree".to_string(),
                status: "PASS".to_string(),
                target: None,
                duration_ms: 300,
                error_code: None,
                error_message: None,
                evidence_bundle_path: None,
            }];
            store
                .record_run(&run, &scenarios, &[])
                .expect("record failed");
        }

        let stats = store.get_scenario_stats(3).expect("stats failed");
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].runs, 3, "stats must only span the newest N runs");
    }

    #[test]
    fn test_scenario_steps_filtered_by_target() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");

        // Same scenario_id executed on two targets (mirrors `--target all`).
        let run = RunRecord {
            run_id: "run_20261112_120000.000".to_string(),
            timestamp: "2026-11-12T12:00:00+00:00".to_string(),
            app_version_name: None,
            build_hash: None,
            branch: None,
            total_scenarios: 2,
            passed: 2,
            failed: 0,
            duration_ms: 1000,
        };
        let scenarios = ["mcp", "mcp-real"]
            .iter()
            .map(|t| ScenarioResultRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-A1".to_string(),
                name: "Static UI tree".to_string(),
                status: "PASS".to_string(),
                target: Some((*t).to_string()),
                duration_ms: 300,
                error_code: None,
                error_message: None,
                evidence_bundle_path: None,
            })
            .collect::<Vec<_>>();
        let steps = ["mcp", "mcp-real"]
            .iter()
            .map(|t| StepRecord {
                id: None,
                run_id: run.run_id.clone(),
                scenario_id: "BB-A1".to_string(),
                target: Some((*t).to_string()),
                step_index: 0,
                tool_name: "device_tree".to_string(),
                params_json: None,
                result_json: Some(r#"{"rev":1}"#.to_string()),
                rev_before: None,
                rev_after: Some(1),
                timestamp_offset_ms: 50,
            })
            .collect::<Vec<_>>();
        store
            .record_run(&run, &scenarios, &steps)
            .expect("Failed to record run");

        // Each target sees only its own steps, never the other target's.
        let mcp = store
            .get_scenario_steps(&run.run_id, "BB-A1", Some("mcp"))
            .expect("Failed to query mcp steps");
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0].target.as_deref(), Some("mcp"));

        let real = store
            .get_scenario_steps(&run.run_id, "BB-A1", Some("mcp-real"))
            .expect("Failed to query mcp-real steps");
        assert_eq!(real.len(), 1);
        assert_eq!(real[0].target.as_deref(), Some("mcp-real"));
    }

    #[test]
    fn test_unsupported_format_is_rejected() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let store = HistoryStore::open_at(temp_dir.path().join("history.db"))
            .expect("Failed to open store");

        let err = store
            .render_history_cli(10, "jsno")
            .expect_err("unknown format must be rejected");
        assert!(
            err.to_string().contains("Unsupported history format"),
            "wrong error: {err}"
        );

        // Documented formats still work.
        assert!(store.render_history_cli(10, "table").is_ok());
        assert!(store.render_history_cli(10, "json").is_ok());
    }

    #[test]
    fn test_legacy_db_without_target_column_migrates() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");

        // Create a pre-target-schema DB by hand.
        let conn = rusqlite::Connection::open(&db_path).expect("Failed to open raw db");
        conn.execute_batch(
            "CREATE TABLE runs (
                run_id TEXT PRIMARY KEY, timestamp TEXT NOT NULL,
                app_version_name TEXT, build_hash TEXT, branch TEXT,
                total_scenarios INTEGER NOT NULL, passed INTEGER NOT NULL,
                failed INTEGER NOT NULL, duration_ms INTEGER NOT NULL
            );
            CREATE TABLE scenario_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL, scenario_id TEXT NOT NULL,
                name TEXT NOT NULL, status TEXT NOT NULL,
                duration_ms INTEGER NOT NULL, error_code TEXT,
                error_message TEXT, evidence_bundle_path TEXT
            );
            CREATE TABLE step_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL, scenario_id TEXT NOT NULL,
                step_index INTEGER NOT NULL, tool_name TEXT NOT NULL,
                params_json TEXT, result_json TEXT,
                rev_before INTEGER, rev_after INTEGER,
                timestamp_offset_ms INTEGER NOT NULL
            );",
        )
        .expect("Failed to create legacy schema");
        drop(conn);

        // Opening through HistoryStore must migrate without dropping data.
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");
        let run = RunRecord {
            run_id: "run_legacy_001".to_string(),
            timestamp: "2026-11-12T12:00:00+00:00".to_string(),
            app_version_name: None,
            build_hash: None,
            branch: None,
            total_scenarios: 1,
            passed: 1,
            failed: 0,
            duration_ms: 100,
        };
        store
            .record_run(
                &run,
                &[ScenarioResultRecord {
                    id: None,
                    run_id: run.run_id.clone(),
                    scenario_id: "BB-A1".to_string(),
                    name: "Legacy row".to_string(),
                    status: "PASS".to_string(),
                    target: None,
                    duration_ms: 100,
                    error_code: None,
                    error_message: None,
                    evidence_bundle_path: None,
                }],
                &[],
            )
            .expect("record on migrated DB must work");

        // Legacy rows (NULL target) stay queryable via the None filter.
        let steps = store
            .get_scenario_steps(&run.run_id, "BB-A1", None)
            .expect("Failed to query legacy steps");
        assert!(steps.is_empty(), "no steps were recorded for this run");
        let detail = store
            .get_run(&run.run_id)
            .expect("get_run on migrated DB")
            .expect("run should exist");
        assert_eq!(detail.1[0].target, None);
    }

    #[test]
    fn test_empty_database_and_cli_output() {
        let temp_dir = tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("history.db");
        let store = HistoryStore::open_at(db_path).expect("Failed to open store");

        let runs = store.list_runs(5).expect("Failed to query");
        assert!(runs.is_empty());

        let table_out = store
            .render_history_cli(5, "table")
            .expect("Table rendering failed");
        assert!(table_out.contains("No test run history found"));

        let json_out = store
            .render_history_cli(5, "json")
            .expect("Json rendering failed");
        assert_eq!(json_out.trim(), "[]");
    }
}
