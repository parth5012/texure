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
    pub step_index: usize,
    pub tool_name: String,
    pub params_json: Option<String>,
    pub result_json: Option<String>,
    pub rev_before: Option<i64>,
    pub rev_after: Option<i64>,
    pub timestamp_offset_ms: u64,
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
            "INSERT OR REPLACE INTO runs (
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
                    run_id, scenario_id, name, status, duration_ms,
                    error_code, error_message, evidence_bundle_path
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
                params![
                    s.run_id,
                    s.scenario_id,
                    s.name,
                    s.status,
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
                    run_id, scenario_id, step_index, tool_name,
                    params_json, result_json, rev_before, rev_after,
                    timestamp_offset_ms
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
                params![
                    st.run_id,
                    st.scenario_id,
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
            "SELECT id, run_id, scenario_id, name, status, duration_ms,
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
                duration_ms: row.get::<_, i64>(5)? as u64,
                error_code: row.get(6)?,
                error_message: row.get(7)?,
                evidence_bundle_path: row.get(8)?,
            })
        })?;

        let mut scenarios = Vec::new();
        for s in scen_rows {
            scenarios.push(s?);
        }

        Ok(Some((run_record, scenarios)))
    }

    pub fn get_scenario_steps(
        &self,
        run_id: &str,
        scenario_id: &str,
    ) -> Result<Vec<StepRecord>, HistoryError> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, scenario_id, step_index, tool_name,
                    params_json, result_json, rev_before, rev_after, timestamp_offset_ms
             FROM step_records
             WHERE run_id = ?1 AND scenario_id = ?2
             ORDER BY step_index ASC;",
        )?;

        let rows = stmt.query_map(params![run_id, scenario_id], |row| {
            Ok(StepRecord {
                id: Some(row.get(0)?),
                run_id: row.get(1)?,
                scenario_id: row.get(2)?,
                step_index: row.get::<_, i64>(3)? as usize,
                tool_name: row.get(4)?,
                params_json: row.get(5)?,
                result_json: row.get(6)?,
                rev_before: row.get(7)?,
                rev_after: row.get(8)?,
                timestamp_offset_ms: row.get::<_, i64>(9)? as u64,
            })
        })?;

        let mut steps = Vec::new();
        for r in rows {
            steps.push(r?);
        }
        Ok(steps)
    }

    pub fn render_history_cli(&self, limit: usize, format: &str) -> Result<String, HistoryError> {
        let runs = self.list_runs(limit)?;
        if format.eq_ignore_ascii_case("json") {
            return Ok(serde_json::to_string_pretty(&runs)?);
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
    format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S"))
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
        assert_eq!(
            detail.1[1].error_code.as_deref(),
            Some("AUTH_FAILED")
        );

        let bb_a1_steps = store
            .get_scenario_steps(&run.run_id, "BB-A1")
            .expect("Failed to query steps");
        assert_eq!(bb_a1_steps.len(), 1);
        assert_eq!(bb_a1_steps[0].tool_name, "device_tree");
        assert_eq!(bb_a1_steps[0].rev_after, Some(1));
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
