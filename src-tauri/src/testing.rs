use crate::{codex_video, database::DatabaseState, project_identity};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Manager;
use uuid::Uuid;
use walkdir::WalkDir;

const CLIENT_ROOT: &str = r"F:\TB-project\scaq-client";
const LEGACY_CLIENT_ROOT: &str = r"F:\TB-project\client";
const APP_ROOT: &str = r"F:\TB-project\scaq-APP";
const LEGACY_APP_ROOT: &str = r"F:\TB-project\APP";
const TEST_RUN_COLUMNS: &str = "id,menu_id,project,project_path,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message,selected_scenarios,scenario_results,artifacts,total_count,passed_count,failed_count,skipped_count,duration_ms,exit_code,environment_summary,cleanup_status";
const COMMON_REAL_SUITE_ID: &str = "common-real";
const DEDICATED_REAL_SUITE_ID: &str = "dedicated-real";
const COMMON_REAL_SPEC_FILE: &str = "workbench-real-common.spec.js";
const COMMON_REAL_SPEC: &str = include_str!("../assets/testing/workbench-real-common.template.js");

#[derive(Default, Clone)]
pub struct TestProcessState {
    processes: Arc<Mutex<HashMap<String, TestProcessControl>>>,
}

#[derive(Default, Clone)]
pub struct TestCaseGenerationState {
    jobs: Arc<Mutex<HashMap<String, TestCaseGenerationJob>>>,
}

#[derive(Default)]
struct TestProcessControl {
    pid: Option<u32>,
    cancel_requested: bool,
}

impl TestProcessState {
    fn register(&self, run_id: &str) {
        if let Ok(mut processes) = self.processes.lock() {
            processes.insert(run_id.to_string(), TestProcessControl::default());
        }
    }

    fn set_pid(&self, run_id: &str, pid: u32) {
        if let Ok(mut processes) = self.processes.lock() {
            if let Some(process) = processes.get_mut(run_id) {
                process.pid = Some(pid);
            }
        }
    }

    fn request_cancel(&self, run_id: &str) -> Option<Option<u32>> {
        let mut processes = self.processes.lock().ok()?;
        let process = processes.get_mut(run_id)?;
        process.cancel_requested = true;
        Some(process.pid)
    }

    fn is_cancelled(&self, run_id: &str) -> bool {
        self.processes
            .lock()
            .ok()
            .and_then(|processes| {
                processes
                    .get(run_id)
                    .map(|process| process.cancel_requested)
            })
            .unwrap_or(false)
    }

    fn finish(&self, run_id: &str) {
        if let Ok(mut processes) = self.processes.lock() {
            processes.remove(run_id);
        }
    }
}

impl TestCaseGenerationState {
    fn insert(&self, job: TestCaseGenerationJob) -> Result<(), String> {
        self.jobs
            .lock()
            .map_err(|_| "无法保存专属用例生成状态。".to_string())?
            .insert(job.id.clone(), job);
        Ok(())
    }

    fn get(&self, job_id: &str) -> Result<TestCaseGenerationJob, String> {
        self.jobs
            .lock()
            .map_err(|_| "无法读取专属用例生成状态。".to_string())?
            .get(job_id)
            .cloned()
            .ok_or_else(|| "没有找到专属用例生成任务。".to_string())
    }

    fn progress(&self, job_id: &str, percent: u8, message: impl Into<String>) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.status = "running".into();
                let percent = percent.min(99);
                if percent >= job.progress_percent {
                    job.progress_percent = percent;
                    job.progress_message = message.into();
                }
            }
        }
    }

    fn complete(&self, job_id: &str, spec_path: &Path) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.status = "completed".into();
                job.progress_percent = 100;
                job.progress_message = "专属用例已生成并通过 Playwright 校验".into();
                job.generated_spec_path = Some(display_path(spec_path));
                job.finished_at = Some(Utc::now().to_rfc3339());
            }
        }
    }

    fn fail(&self, job_id: &str, error: impl Into<String>) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                let error = error.into();
                job.status = "failed".into();
                job.progress_message = "专属用例生成失败".into();
                job.error_message = error;
                job.finished_at = Some(Utc::now().to_rfc3339());
            }
        }
    }
}

pub(crate) fn client_root() -> PathBuf {
    std::env::var_os("AI_WORKBENCH_CLIENT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            [CLIENT_ROOT, LEGACY_CLIENT_ROOT]
                .into_iter()
                .map(PathBuf::from)
                .find(|path| path.is_dir())
                .unwrap_or_else(|| PathBuf::from(CLIENT_ROOT))
        })
}

pub(crate) fn app_root() -> PathBuf {
    std::env::var_os("AI_WORKBENCH_APP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            [APP_ROOT, LEGACY_APP_ROOT]
                .into_iter()
                .map(PathBuf::from)
                .find(|path| path.is_dir())
                .unwrap_or_else(|| PathBuf::from(APP_ROOT))
        })
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", rest);
        }
        if let Some(rest) = value.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    value.into_owned()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCapabilities {
    pub mock: bool,
    pub real_api: bool,
    pub source_style: bool,
    pub browser_style: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestMenu {
    pub id: String,
    pub project: String,
    pub project_path: String,
    pub project_kind: String,
    pub name: String,
    pub route: String,
    pub source_path: String,
    pub case_id: Option<String>,
    pub has_case_file: bool,
    pub case_file_path: Option<String>,
    pub can_create_case_file: bool,
    pub capabilities: TestCapabilities,
    pub tested: bool,
    pub latest_status: Option<String>,
    pub latest_time: Option<String>,
    pub latest_report_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRun {
    pub id: String,
    pub menu_id: String,
    pub project: String,
    pub project_path: String,
    pub menu_name: String,
    pub mode: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub report_markdown: String,
    pub source_report_path: Option<String>,
    pub output_excerpt: String,
    pub error_message: String,
    pub selected_scenarios: Vec<String>,
    pub scenario_results: Vec<TestScenarioResult>,
    pub artifacts: Vec<TestArtifact>,
    pub total_count: i64,
    pub passed_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub environment_summary: String,
    pub cleanup_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProject {
    pub path: String,
    pub name: String,
    pub project_kind: String,
    pub case_count: usize,
    pub page_count: usize,
    pub capabilities: TestCapabilities,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScenario {
    pub id: String,
    pub title: String,
    pub description: String,
    pub mode: String,
    pub default_selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSuite {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub read_only: bool,
    pub spec_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCaseGenerationJob {
    pub id: String,
    pub project_path: String,
    pub menu_id: String,
    pub menu_name: String,
    pub status: String,
    pub progress_percent: u8,
    pub progress_message: String,
    pub error_message: String,
    pub generated_spec_path: Option<String>,
    pub created_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestArtifact {
    pub name: String,
    pub path: String,
    pub content_type: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestScenarioResult {
    pub id: String,
    pub title: String,
    pub status: String,
    pub duration_ms: i64,
    pub purpose: String,
    pub steps: Vec<String>,
    pub checks: Vec<String>,
    pub error_message: String,
    pub artifacts: Vec<TestArtifact>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPreflight {
    pub ready: bool,
    pub status: String,
    pub checks: Vec<PreflightCheck>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRecommendation {
    menu_id: String,
    project: String,
    project_path: String,
    menu_name: String,
    changed_files: Vec<String>,
    reason: String,
    recommended_mode: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTestOptions {
    pub project_path: String,
    pub menu_id: String,
    pub mode: String,
    #[serde(default)]
    pub selected_scenarios: Vec<String>,
    pub test_suite_id: Option<String>,
    pub create_case_file: Option<bool>,
    pub confirmed_real_write: Option<bool>,
    pub account: Option<String>,
    pub token: Option<String>,
    pub use_environment_token: Option<bool>,
}

fn strip_json_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn latest_client_report(name: &str) -> Option<(String, String, String)> {
    let report_dir = client_root().join("e2e").join("reports");
    let mut reports = fs::read_dir(report_dir)
        .ok()?
        .flatten()
        .filter(|item| item.path().extension().and_then(|value| value.to_str()) == Some("md"))
        .filter(|item| item.file_name().to_string_lossy().contains(name))
        .filter_map(|item| {
            let modified = item.metadata().ok()?.modified().ok()?;
            Some((modified, item.path()))
        })
        .collect::<Vec<_>>();
    reports.sort_by_key(|item| std::cmp::Reverse(item.0));
    let (modified, path) = reports.into_iter().next()?;
    let content = fs::read_to_string(&path).unwrap_or_default();
    let status = client_report_status(&content);
    let time: DateTime<Local> = modified.into();
    Some((
        status.to_string(),
        time.to_rfc3339(),
        path.display().to_string(),
    ))
}

fn client_report_status(content: &str) -> &'static str {
    if content.contains("测试结论：通过") || content.contains("测试结论: 通过") {
        "passed"
    } else {
        "failed"
    }
}

fn latest_db_run_for_project(
    state: &DatabaseState,
    menu_id: &str,
    project_path: &Path,
) -> Result<Option<TestRun>, String> {
    let canonical_path = project_path.display().to_string();
    let friendly_path = display_path(project_path);
    state
        .connect()?
        .query_row(
            &format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE menu_id=?1 AND (project_path=?2 OR project_path=?3 OR project_path='') ORDER BY CASE WHEN project_path=?2 OR project_path=?3 THEN 0 ELSE 1 END,started_at DESC LIMIT 1"),
            params![menu_id, friendly_path, canonical_path],
            row_to_run,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn json_vec<T: for<'de> Deserialize<'de>>(value: String) -> Vec<T> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn legacy_duration_ms(value: &str) -> i64 {
    let value = value.trim();
    if let Some(milliseconds) = value.strip_suffix("ms") {
        return milliseconds.trim().parse::<f64>().unwrap_or(0.0) as i64;
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return (seconds.trim().parse::<f64>().unwrap_or(0.0) * 1000.0) as i64;
    }
    value.parse::<f64>().unwrap_or(0.0) as i64
}

fn legacy_heading_title(value: &str) -> String {
    value
        .split_once(". ")
        .map(|(_, title)| title)
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn legacy_report_scenarios(markdown: &str) -> Vec<TestScenarioResult> {
    #[derive(PartialEq)]
    enum Section {
        Other,
        Scenarios,
        Failures,
    }

    let mut section = Section::Other;
    let mut scenarios = Vec::<TestScenarioResult>::new();
    let mut current: Option<TestScenarioResult> = None;
    let mut reading_steps = false;
    let mut reading_checks = false;
    let mut failure_title = String::new();
    let mut failure_error = Vec::<String>::new();
    let mut failure_artifacts = Vec::<TestArtifact>::new();
    let mut in_code = false;
    let mut failures = HashMap::<String, (String, Vec<TestArtifact>)>::new();

    let flush_scenario = |current: &mut Option<TestScenarioResult>,
                          scenarios: &mut Vec<TestScenarioResult>| {
        if let Some(item) = current.take() {
            scenarios.push(item);
        }
    };
    let flush_failure =
        |title: &mut String,
         error: &mut Vec<String>,
         artifacts: &mut Vec<TestArtifact>,
         failures: &mut HashMap<String, (String, Vec<TestArtifact>)>| {
            if !title.is_empty() {
                failures.insert(
                    std::mem::take(title),
                    (
                        error.join("\n").trim().to_string(),
                        std::mem::take(artifacts),
                    ),
                );
                error.clear();
            }
        };

    for raw in markdown.lines() {
        let line = raw.trim();
        if let Some(heading) = line.strip_prefix("## ") {
            flush_scenario(&mut current, &mut scenarios);
            flush_failure(
                &mut failure_title,
                &mut failure_error,
                &mut failure_artifacts,
                &mut failures,
            );
            section = if heading.contains("场景明细") {
                Section::Scenarios
            } else if heading.contains("失败详情") || heading.contains("问题详情") {
                Section::Failures
            } else {
                Section::Other
            };
            reading_steps = false;
            reading_checks = false;
            in_code = false;
            continue;
        }
        if let Some(heading) = line.strip_prefix("### ") {
            match section {
                Section::Scenarios => {
                    flush_scenario(&mut current, &mut scenarios);
                    let title = legacy_heading_title(heading);
                    current = Some(TestScenarioResult {
                        id: format!("legacy-{}", scenarios.len() + 1),
                        title: title.clone(),
                        status: "failed".into(),
                        duration_ms: 0,
                        purpose: scenario_description("", &title, "当前功能"),
                        steps: Vec::new(),
                        checks: Vec::new(),
                        error_message: String::new(),
                        artifacts: Vec::new(),
                    });
                    reading_steps = false;
                    reading_checks = false;
                }
                Section::Failures => {
                    flush_failure(
                        &mut failure_title,
                        &mut failure_error,
                        &mut failure_artifacts,
                        &mut failures,
                    );
                    failure_title = legacy_heading_title(heading);
                    in_code = false;
                }
                Section::Other => {}
            }
            continue;
        }

        if section == Section::Scenarios {
            let Some(item) = current.as_mut() else {
                continue;
            };
            if let Some(value) = line.strip_prefix("- 结果：") {
                item.status = if value.contains("通过") {
                    "passed"
                } else if value.contains("跳过") {
                    "skipped"
                } else if value.contains("阻塞") {
                    "blocked"
                } else {
                    "failed"
                }
                .into();
            } else if let Some(value) = line.strip_prefix("- 测试目的：") {
                item.purpose = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("- 耗时：") {
                item.duration_ms = legacy_duration_ms(value);
            } else if line == "测试步骤：" {
                reading_steps = true;
                reading_checks = false;
            } else if line == "验证内容：" {
                reading_steps = false;
                reading_checks = true;
            } else if reading_steps {
                if let Some((number, value)) = line.split_once(". ") {
                    if number.chars().all(|ch| ch.is_ascii_digit()) {
                        item.steps.push(value.trim().to_string());
                    }
                }
            } else if reading_checks {
                if let Some(value) = line.strip_prefix("- ") {
                    item.checks.push(value.trim().to_string());
                }
            }
        } else if section == Section::Failures && !failure_title.is_empty() {
            if line.starts_with("```") {
                in_code = !in_code;
            } else if in_code {
                failure_error.push(raw.trim_end().to_string());
            } else if let Some(path) = line.strip_prefix("- screenshot:") {
                let path = path.trim();
                failure_artifacts.push(TestArtifact {
                    name: format!("{} · 失败页面", failure_title),
                    path: path.to_string(),
                    content_type: image_mime(Path::new(path))
                        .unwrap_or("image/png")
                        .to_string(),
                    kind: "screenshot".into(),
                });
            }
        }
    }
    flush_scenario(&mut current, &mut scenarios);
    flush_failure(
        &mut failure_title,
        &mut failure_error,
        &mut failure_artifacts,
        &mut failures,
    );

    for item in &mut scenarios {
        if let Some((error, artifacts)) = failures.remove(&item.title) {
            item.error_message = error;
            item.artifacts = artifacts;
        }
        if item.steps.is_empty() {
            item.steps.push("按历史报告中的场景说明执行测试。".into());
        }
        if item.checks.is_empty() {
            item.checks.push("核对历史报告记录的实际结果。".into());
        }
    }
    scenarios
}

fn hydrate_legacy_run(mut run: TestRun) -> TestRun {
    if run.project_path.trim().is_empty() {
        run.project_path = if run.project.eq_ignore_ascii_case("client") {
            display_path(&client_root())
        } else if run.project.eq_ignore_ascii_case("APP") {
            display_path(&app_root())
        } else {
            String::new()
        };
    }
    if run.scenario_results.is_empty() && !run.report_markdown.trim().is_empty() {
        let scenarios = legacy_report_scenarios(&run.report_markdown);
        if !scenarios.is_empty() {
            run.total_count = scenarios.len() as i64;
            run.passed_count = scenarios
                .iter()
                .filter(|item| item.status == "passed")
                .count() as i64;
            run.failed_count = scenarios
                .iter()
                .filter(|item| item.status == "failed" || item.status == "blocked")
                .count() as i64;
            run.skipped_count = scenarios
                .iter()
                .filter(|item| item.status == "skipped")
                .count() as i64;
            run.artifacts = scenarios
                .iter()
                .flat_map(|item| item.artifacts.clone())
                .collect();
            run.scenario_results = scenarios;
        }
    }
    run
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TestRun> {
    Ok(hydrate_legacy_run(TestRun {
        id: row.get(0)?,
        menu_id: row.get(1)?,
        project: row.get(2)?,
        project_path: row.get(3)?,
        menu_name: row.get(4)?,
        mode: row.get(5)?,
        status: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        report_markdown: row.get(9)?,
        source_report_path: row.get(10)?,
        output_excerpt: row.get(11)?,
        error_message: row.get(12)?,
        selected_scenarios: json_vec(row.get(13)?),
        scenario_results: json_vec(row.get(14)?),
        artifacts: json_vec(row.get(15)?),
        total_count: row.get(16)?,
        passed_count: row.get(17)?,
        failed_count: row.get(18)?,
        skipped_count: row.get(19)?,
        duration_ms: row.get(20)?,
        exit_code: row.get(21)?,
        environment_summary: row.get(22)?,
        cleanup_status: row.get(23)?,
    }))
}

pub(crate) fn client_menus(state: &DatabaseState) -> Result<Vec<TestMenu>, String> {
    let case_dir = client_root().join("e2e").join("menu-cases");
    if !case_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut menus = Vec::new();
    for entry in fs::read_dir(case_dir)
        .map_err(|error| format!("无法读取 client 菜单用例：{error}"))?
        .flatten()
    {
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let value: Value = serde_json::from_str(
            &fs::read_to_string(entry.path()).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let case_id = value
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let name = value
            .get("menuName")
            .and_then(Value::as_str)
            .unwrap_or(&case_id)
            .to_string();
        let menu_id = format!("client:{case_id}");
        let db = latest_db_run_for_project(state, &menu_id, &client_root())?;
        let historical = latest_client_report(&name);
        let (latest_status, latest_time, latest_report_path) = if let Some(run) = db.as_ref() {
            (
                Some(run.status.clone()),
                run.finished_at.clone().or(Some(run.started_at.clone())),
                run.source_report_path.clone(),
            )
        } else if let Some((status, time, path)) = historical {
            (Some(status), Some(time), Some(path))
        } else {
            (None, None, None)
        };
        menus.push(TestMenu {
            id: menu_id,
            project: "client".into(),
            project_path: display_path(&client_root()),
            project_kind: "vue".into(),
            name,
            route: value
                .get("route")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            source_path: value
                .get("component")
                .and_then(Value::as_str)
                .map(|value| format!("src/views/{value}.vue"))
                .unwrap_or_default(),
            case_id: Some(case_id),
            has_case_file: true,
            case_file_path: Some(entry.path().display().to_string()),
            can_create_case_file: false,
            capabilities: TestCapabilities {
                mock: true,
                real_api: true,
                source_style: true,
                browser_style: true,
            },
            tested: latest_status.is_some(),
            latest_status,
            latest_time,
            latest_report_path,
        });
    }
    menus.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(menus)
}

fn add_app_pages(items: &mut Vec<TestMenu>, root: &str, pages: Option<&Vec<Value>>) {
    for page in pages.into_iter().flatten() {
        let path = page
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim_start_matches('/');
        if path.is_empty() {
            continue;
        }
        let full_path = if root.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", root.trim_end_matches('/'), path)
        };
        let name = page
            .get("style")
            .and_then(|style| style.get("navigationBarTitleText"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
            .to_string();
        items.push(TestMenu {
            id: format!("app:{full_path}"),
            project: "APP".into(),
            project_path: display_path(&app_root()),
            project_kind: "uni-app".into(),
            name,
            route: format!("/{full_path}"),
            source_path: format!("{full_path}.vue"),
            case_id: None,
            has_case_file: false,
            case_file_path: None,
            can_create_case_file: true,
            capabilities: TestCapabilities {
                mock: false,
                real_api: false,
                source_style: true,
                browser_style: false,
            },
            tested: false,
            latest_status: None,
            latest_time: None,
            latest_report_path: None,
        });
    }
}

pub(crate) fn app_menus(state: &DatabaseState) -> Result<Vec<TestMenu>, String> {
    let pages_path = app_root().join("pages.json");
    if !pages_path.is_file() {
        return Ok(Vec::new());
    }
    let source = fs::read_to_string(pages_path)
        .map_err(|error| format!("无法读取 APP pages.json：{error}"))?;
    let value: Value = serde_json::from_str(&strip_json_comments(&source))
        .map_err(|error| format!("APP pages.json 解析失败：{error}"))?;
    let mut menus = Vec::new();
    add_app_pages(&mut menus, "", value.get("pages").and_then(Value::as_array));
    for package in value
        .get("subPackages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        add_app_pages(
            &mut menus,
            package
                .get("root")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            package.get("pages").and_then(Value::as_array),
        );
    }
    menus.sort_by(|a, b| a.route.cmp(&b.route));
    menus.dedup_by(|a, b| a.id == b.id);
    for menu in &mut menus {
        if let Some(run) = latest_db_run_for_project(state, &menu.id, &app_root())? {
            menu.tested = true;
            menu.latest_status = Some(run.status);
            menu.latest_time = run.finished_at.or(Some(run.started_at));
            menu.latest_report_path = run.source_report_path;
        }
    }
    Ok(menus)
}

fn project_kind(root: &Path) -> String {
    if root.join("pages.json").is_file() {
        "uni-app".into()
    } else if root.join("vite.config.ts").is_file()
        || root.join("vite.config.js").is_file()
        || root.join("src").join("views").is_dir()
    {
        "vue".into()
    } else if root.join("package.json").is_file() {
        "web".into()
    } else {
        "unknown".into()
    }
}

fn package_scripts(root: &Path) -> Value {
    fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .and_then(|value| value.get("scripts").cloned())
        .unwrap_or(Value::Null)
}

fn has_script(scripts: &Value, name: &str) -> bool {
    scripts
        .get(name)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

fn project_capabilities(root: &Path) -> TestCapabilities {
    let scripts = package_scripts(root);
    let playwright_cli = root
        .join("node_modules")
        .join("@playwright")
        .join("test")
        .join("cli.js")
        .is_file();
    let playwright_config = root.join("playwright.config.js").is_file()
        || root.join("playwright.config.cjs").is_file()
        || root.join("playwright.config.ts").is_file();
    TestCapabilities {
        mock: has_script(&scripts, "test:menu")
            && root
                .join("e2e")
                .join("specs")
                .join("menu-module.spec.js")
                .is_file(),
        real_api: playwright_cli
            && playwright_config
            && (has_script(&scripts, "test:menu:real") || root.join("e2e").join("specs").is_dir()),
        source_style: has_script(&scripts, "test:page-style")
            || root.join("src").is_dir()
            || root.join("pages.json").is_file(),
        browser_style: has_script(&scripts, "test:page-style:browser")
            && root
                .join("e2e")
                .join("specs")
                .join("page-style.spec.js")
                .is_file(),
    }
}

fn canonical_project_asset(
    state: &DatabaseState,
    requested: &str,
) -> Result<(PathBuf, String), String> {
    let requested = PathBuf::from(requested)
        .canonicalize()
        .map_err(|error| format!("项目目录不存在或无法读取：{error}"))?;
    let rows = {
        let connection = state.connect()?;
        let mut statement = connection
            .prepare("SELECT path,name FROM repository_assets WHERE is_hidden=0")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    rows.into_iter()
        .find_map(|(path, name)| {
            PathBuf::from(path)
                .canonicalize()
                .ok()
                .filter(|path| path == &requested)
                .map(|path| (path, name))
        })
        .ok_or_else(|| "只能选择项目资产中已扫描且未隐藏的本地项目。".to_string())
}

fn case_values(root: &Path) -> Vec<(Value, PathBuf)> {
    fs::read_dir(root.join("e2e").join("menu-cases"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| {
            let content = fs::read_to_string(entry.path()).ok()?;
            serde_json::from_str(&content)
                .ok()
                .map(|value| (value, entry.path()))
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegisteredPage {
    name: String,
    route: String,
    component: String,
    source_path: String,
}

#[derive(Clone)]
struct DynamicMenuCacheEntry {
    checked_at: Instant,
    pages: Vec<RegisteredPage>,
}

static DYNAMIC_MENU_CACHE: OnceLock<Mutex<HashMap<String, DynamicMenuCacheEntry>>> =
    OnceLock::new();

fn estimated_page_count(root: &Path, _case_count: usize) -> usize {
    if let Ok(source) = fs::read_to_string(root.join("pages.json")) {
        if let Ok(value) = serde_json::from_str::<Value>(&strip_json_comments(&source)) {
            let top = value
                .get("pages")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let sub = value
                .get("subPackages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|item| {
                    item.get("pages")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)
                })
                .sum::<usize>();
            return top + sub;
        }
    }
    visible_local_router_pages(root).len()
}

fn normalized_component(value: &str) -> String {
    value
        .trim()
        .trim_matches(|ch| ch == '\'' || ch == '"' || ch == '`')
        .replace('\\', "/")
        .trim_start_matches("@/views/")
        .trim_start_matches("src/views/")
        .trim_start_matches('/')
        .trim_end_matches(".vue")
        .to_string()
}

fn source_path_for_component(root: &Path, component: &str) -> Option<(String, String)> {
    let component = normalized_component(component);
    if component.is_empty() {
        return None;
    }
    let direct = root
        .join("src")
        .join("views")
        .join(format!("{component}.vue"));
    if direct.is_file() {
        return Some((component.clone(), format!("src/views/{component}.vue")));
    }
    let index = root
        .join("src")
        .join("views")
        .join(&component)
        .join("index.vue");
    index.is_file().then(|| {
        (
            format!("{component}/index"),
            format!("src/views/{component}/index.vue"),
        )
    })
}

fn join_route_path(parent: &str, child: &str) -> String {
    let child = child.trim();
    if child.is_empty() {
        return if parent.is_empty() {
            "/".into()
        } else {
            parent.into()
        };
    }
    if child.starts_with('/') {
        return child.replace("//", "/");
    }
    let value = format!(
        "{}/{}",
        parent.trim_end_matches('/'),
        child.trim_start_matches('/')
    )
    .trim_start_matches('/')
    .to_string();
    format!("/{value}")
}

fn collect_pages_json(root: &Path) -> Vec<RegisteredPage> {
    let Ok(source) = fs::read_to_string(root.join("pages.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&strip_json_comments(&source)) else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    let mut collect = |prefix: &str, values: Option<&Vec<Value>>| {
        for page in values.into_iter().flatten() {
            let path = page
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim_start_matches('/');
            if path.is_empty() {
                continue;
            }
            let full = if prefix.is_empty() {
                path.to_string()
            } else {
                format!("{}/{}", prefix.trim_end_matches('/'), path)
            };
            let name = page
                .get("style")
                .and_then(|item| item.get("navigationBarTitleText"))
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
                .to_string();
            pages.push(RegisteredPage {
                name,
                route: format!("/{full}"),
                component: full.clone(),
                source_path: format!("{full}.vue"),
            });
        }
    };
    collect("", value.get("pages").and_then(Value::as_array));
    for package in value
        .get("subPackages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        collect(
            package
                .get("root")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            package.get("pages").and_then(Value::as_array),
        );
    }
    pages
}

fn strip_js_comments_preserving_offsets(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = bytes.to_vec();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, b'\'' | b'"' | b'`') {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == b'/' && bytes.get(index + 1) == Some(&b'/') {
            output[index] = b' ';
            output[index + 1] = b' ';
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                output[index] = b' ';
                index += 1;
            }
            continue;
        }
        if ch == b'/' && bytes.get(index + 1) == Some(&b'*') {
            output[index] = b' ';
            output[index + 1] = b' ';
            index += 2;
            while index < bytes.len() {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    output[index] = b' ';
                    output[index + 1] = b' ';
                    index += 2;
                    break;
                }
                if bytes[index] != b'\n' {
                    output[index] = b' ';
                }
                index += 1;
            }
            continue;
        }
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| source.to_string())
}

fn js_object_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut stack = Vec::new();
    let mut ranges = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in source.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '\'' | '"' | '`') {
            quote = Some(ch);
        } else if ch == '{' {
            stack.push(index);
        } else if ch == '}' {
            if let Some(start) = stack.pop() {
                ranges.push((start, index + ch.len_utf8()));
            }
        }
    }
    ranges
}

fn js_property_start(source: &str, property: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let property = property.as_bytes();
    let mut offset = 0;
    while offset + property.len() <= bytes.len() {
        let relative = source[offset..].find(std::str::from_utf8(property).ok()?)?;
        let start = offset + relative;
        let before_ok = start == 0
            || !((bytes[start - 1] as char).is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let mut cursor = start + property.len();
        let after_ok = cursor >= bytes.len()
            || !((bytes[cursor] as char).is_ascii_alphanumeric() || bytes[cursor] == b'_');
        while cursor < bytes.len() && (bytes[cursor] as char).is_ascii_whitespace() {
            cursor += 1;
        }
        if before_ok && after_ok && bytes.get(cursor) == Some(&b':') {
            return Some(cursor + 1);
        }
        offset = start + property.len();
    }
    None
}

fn js_quoted_property(source: &str, property: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut cursor = js_property_start(source, property)?;
    while cursor < bytes.len() && (bytes[cursor] as char).is_ascii_whitespace() {
        cursor += 1;
    }
    let quote = *bytes.get(cursor)?;
    if !matches!(quote, b'\'' | b'"' | b'`') {
        return None;
    }
    cursor += 1;
    let start = cursor;
    let mut escaped = false;
    while cursor < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[cursor] == b'\\' {
            escaped = true;
        } else if bytes[cursor] == quote {
            return Some(source[start..cursor].to_string());
        }
        cursor += 1;
    }
    None
}

fn first_quoted_value(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let (mut cursor, quote) = bytes
        .iter()
        .enumerate()
        .find(|(_, value)| matches!(**value, b'\'' | b'"' | b'`'))?;
    cursor += 1;
    let start = cursor;
    let mut escaped = false;
    while cursor < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[cursor] == b'\\' {
            escaped = true;
        } else if bytes[cursor] == *quote {
            return Some(source[start..cursor].to_string());
        }
        cursor += 1;
    }
    None
}

fn object_direct_prefix(object: &str) -> &str {
    object
        .find("children")
        .map(|index| &object[..index])
        .unwrap_or(object)
}

fn object_is_hidden(object: &str) -> bool {
    let Some(mut cursor) = js_property_start(object_direct_prefix(object), "hidden") else {
        return false;
    };
    let bytes = object.as_bytes();
    while cursor < bytes.len() && (bytes[cursor] as char).is_ascii_whitespace() {
        cursor += 1;
    }
    object[cursor..].starts_with("true")
}

fn visible_local_router_pages(root: &Path) -> Vec<RegisteredPage> {
    let router_root = root.join("src").join("router");
    if !router_root.is_dir() {
        return Vec::new();
    }
    let mut pages = Vec::new();
    for entry in WalkDir::new(router_root)
        .max_depth(5)
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry.file_type().is_file()
                && matches!(
                    entry.path().extension().and_then(|value| value.to_str()),
                    Some("js" | "ts")
                )
        })
    {
        let Ok(source) = fs::read_to_string(entry.path()) else {
            continue;
        };
        let source = strip_js_comments_preserving_offsets(&source);
        let ranges = js_object_ranges(&source);
        let mut search_from = 0;
        while let Some(relative) = source[search_from..].find("component") {
            let component_position = search_from + relative;
            search_from = component_position + "component".len();
            let Some(component_expression) = source[component_position..].find("import(") else {
                continue;
            };
            if component_expression > 160
                || source[component_position..component_position + component_expression]
                    .contains(',')
            {
                continue;
            }
            let import_source =
                &source[component_position + component_expression + "import".len()..];
            let Some(component) = first_quoted_value(import_source) else {
                continue;
            };
            let Some((leaf_start, leaf_end)) = ranges
                .iter()
                .filter(|(start, end)| *start < component_position && *end > component_position)
                .max_by_key(|(start, _)| *start)
                .copied()
            else {
                continue;
            };
            let leaf = &source[leaf_start..leaf_end];
            let Some(name) =
                js_quoted_property(leaf, "title").filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            let Some((component, source_path)) = source_path_for_component(root, &component) else {
                continue;
            };
            let mut ancestors = ranges
                .iter()
                .filter(|(start, end)| *start <= leaf_start && *end >= leaf_end)
                .copied()
                .collect::<Vec<_>>();
            ancestors.sort_by_key(|(start, _)| *start);
            if ancestors
                .iter()
                .any(|(start, end)| object_is_hidden(&source[*start..*end]))
            {
                continue;
            }
            let mut route = String::new();
            for (start, end) in ancestors {
                if start == leaf_start && end == leaf_end {
                    continue;
                }
                let object = &source[start..end];
                if let Some(path) = js_quoted_property(object_direct_prefix(object), "path") {
                    route = join_route_path(&route, &path);
                }
            }
            if let Some(path) = js_quoted_property(object_direct_prefix(leaf), "path") {
                route = join_route_path(&route, &path);
            }
            if route.is_empty() || route.contains(":pathMatch") {
                continue;
            }
            pages.push(RegisteredPage {
                name,
                route,
                component,
                source_path,
            });
        }
    }
    pages.sort_by(|left, right| left.route.cmp(&right.route));
    pages.dedup_by(|left, right| left.route == right.route || left.component == right.component);
    pages
}

fn collect_dynamic_route_pages(
    root: &Path,
    routes: &[Value],
    parent_route: &str,
    parent_hidden: bool,
    pages: &mut Vec<RegisteredPage>,
) {
    for route in routes {
        let hidden = parent_hidden
            || route
                .get("hidden")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let full_route = join_route_path(
            parent_route,
            route
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        let component = route
            .get("component")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = route
            .get("meta")
            .and_then(|meta| meta.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if !hidden && !name.is_empty() {
            if let Some((component, source_path)) = source_path_for_component(root, component) {
                pages.push(RegisteredPage {
                    name: name.to_string(),
                    route: full_route.clone(),
                    component,
                    source_path,
                });
            }
        }
        if let Some(children) = route.get("children").and_then(Value::as_array) {
            collect_dynamic_route_pages(root, children, &full_route, hidden, pages);
        }
    }
}

fn dynamic_pages_from_response(root: &Path, response: &Value) -> Vec<RegisteredPage> {
    let code_ok = response
        .get("code")
        .map(|code| code.as_i64() == Some(200) || code.as_str() == Some("200"))
        .unwrap_or(true);
    if !code_ok {
        return Vec::new();
    }
    let Some(routes) = response.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    collect_dynamic_route_pages(root, routes, "", false, &mut pages);
    pages.sort_by(|left, right| left.route.cmp(&right.route));
    pages.dedup_by(|left, right| left.route == right.route || left.component == right.component);
    pages
}

fn menu_api_candidates(root: &Path) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(base_url) = std::env::var("E2E_BASE_URL") {
        if !base_url.trim().is_empty() {
            candidates.push(format!(
                "{}/dev-api/menu/getRouters",
                base_url.trim_end_matches('/')
            ));
        }
    }
    for config_name in [
        "playwright.config.js",
        "playwright.config.cjs",
        "playwright.config.ts",
    ] {
        if let Ok(source) = fs::read_to_string(root.join(config_name)) {
            let source = strip_js_comments_preserving_offsets(&source);
            if let Some(base_url) = js_quoted_property(&source, "baseURL") {
                candidates.push(format!(
                    "{}/dev-api/menu/getRouters",
                    base_url.trim_end_matches('/')
                ));
            }
        }
    }
    for config_name in ["vite.config.js", "vite.config.ts", "vite.config.mjs"] {
        let Ok(source) = fs::read_to_string(root.join(config_name)) else {
            continue;
        };
        let source = strip_js_comments_preserving_offsets(&source);
        let dev_api_start = source
            .find("\"/dev-api\"")
            .or_else(|| source.find("'/dev-api'"));
        let Some(dev_api_start) = dev_api_start else {
            continue;
        };
        let block = &source[dev_api_start..source.len().min(dev_api_start + 1200)];
        if let Some(target) = js_quoted_property(block, "target") {
            candidates.push(format!(
                "{}/api/menu/getRouters",
                target.trim_end_matches('/')
            ));
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn fetch_dynamic_menu_pages(root: &Path) -> Vec<RegisteredPage> {
    let key = display_path(root).to_lowercase();
    let cache = DYNAMIC_MENU_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.checked_at.elapsed() < Duration::from_secs(30) {
                return entry.pages.clone();
            }
        }
    }
    let token = std::env::var("HLZT_TOKEN").unwrap_or_default();
    let mut pages = Vec::new();
    if !token.trim().is_empty() {
        if let Ok(client) = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_millis(800))
            .timeout(Duration::from_secs(3))
            .build()
        {
            for url in menu_api_candidates(root) {
                let Ok(response) = client.get(url).header("hlzt-token", &token).send() else {
                    continue;
                };
                let Ok(value) = response.json::<Value>() else {
                    continue;
                };
                pages = dynamic_pages_from_response(root, &value);
                if !pages.is_empty() {
                    break;
                }
            }
        }
    }
    if let Ok(mut cache) = cache.lock() {
        if pages.is_empty() {
            if let Some(previous) = cache.get(&key).filter(|entry| !entry.pages.is_empty()) {
                return previous.pages.clone();
            }
        }
        cache.insert(
            key,
            DynamicMenuCacheEntry {
                checked_at: Instant::now(),
                pages: pages.clone(),
            },
        );
    }
    pages
}

fn registered_vue_pages(root: &Path) -> Vec<RegisteredPage> {
    let mut pages = fetch_dynamic_menu_pages(root);
    pages.extend(visible_local_router_pages(root));
    pages.sort_by(|left, right| left.route.cmp(&right.route));
    pages.dedup_by(|left, right| left.route == right.route || left.component == right.component);
    pages
}

fn same_canonical_path(left: &Path, right: &Path) -> bool {
    left.canonicalize()
        .ok()
        .zip(right.canonicalize().ok())
        .is_some_and(|(left, right)| left == right)
}

fn catalog_menu(
    state: &DatabaseState,
    root: &Path,
    project_name: &str,
    kind: &str,
    id: String,
    name: String,
    route: String,
    source_path: String,
    case: Option<(&Value, &Path)>,
) -> Result<TestMenu, String> {
    let latest = latest_db_run_for_project(state, &id, root)?;
    let available = project_capabilities(root);
    let has_case = case.is_some();
    Ok(TestMenu {
        id,
        project: project_name.into(),
        project_path: display_path(root),
        project_kind: kind.into(),
        name,
        route,
        source_path,
        case_id: case
            .and_then(|(value, _)| value.get("id").and_then(Value::as_str))
            .map(str::to_string),
        has_case_file: has_case,
        case_file_path: case.map(|(_, path)| path.display().to_string()),
        can_create_case_file: !has_case,
        capabilities: TestCapabilities {
            mock: has_case && available.mock,
            // 公共通用真实接口用例不依赖菜单专属 JSON，因此无专属用例的页面也可运行。
            real_api: available.real_api,
            source_style: available.source_style,
            browser_style: has_case && available.browser_style,
        },
        tested: latest.is_some(),
        latest_status: latest.as_ref().map(|run| run.status.clone()),
        latest_time: latest
            .as_ref()
            .and_then(|run| run.finished_at.clone().or(Some(run.started_at.clone()))),
        latest_report_path: latest.and_then(|run| run.source_report_path),
    })
}

fn project_menus(
    state: &DatabaseState,
    root: &Path,
    project_name: &str,
) -> Result<Vec<TestMenu>, String> {
    let kind = project_kind(root);
    let cases = case_values(root);
    let mut menus = Vec::new();
    let pages = if root.join("pages.json").is_file() {
        collect_pages_json(root)
    } else {
        registered_vue_pages(root)
    };
    for page in pages {
        let case = cases.iter().find(|(value, _)| {
            let case_component = value
                .get("component")
                .and_then(Value::as_str)
                .map(normalized_component)
                .unwrap_or_default();
            let case_route = value
                .get("route")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim_end_matches('/');
            (!case_component.is_empty() && case_component.eq_ignore_ascii_case(&page.component))
                || (!case_route.is_empty() && case_route == page.route.trim_end_matches('/'))
        });
        let id = case
            .and_then(|(value, _)| {
                value
                    .get("workbenchMenuId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| {
                        value.get("id").and_then(Value::as_str).map(|case_id| {
                            if same_canonical_path(root, &client_root()) {
                                format!("client:{case_id}")
                            } else {
                                format!("project:{case_id}")
                            }
                        })
                    })
            })
            .unwrap_or_else(|| {
                if same_canonical_path(root, &app_root()) {
                    format!("app:{}", page.component)
                } else {
                    format!("page:{}", page.component.replace('/', ":"))
                }
            });
        menus.push(catalog_menu(
            state,
            root,
            project_name,
            &kind,
            id,
            page.name,
            page.route,
            page.source_path,
            case.map(|(value, path)| (value, path.as_path())),
        )?);
    }
    menus.sort_by(|a, b| a.name.cmp(&b.name).then(a.route.cmp(&b.route)));
    menus.dedup_by(|a, b| a.id == b.id || (!a.route.is_empty() && a.route == b.route));
    Ok(menus)
}

#[tauri::command]
pub fn list_test_projects(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<TestProject>, String> {
    let rows = {
        let connection = state.connect()?;
        let mut statement = connection
            .prepare("SELECT path,name FROM repository_assets WHERE is_hidden=0 ORDER BY is_pinned DESC,name")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    Ok(rows
        .into_iter()
        .filter_map(|(path, name)| {
            let root = PathBuf::from(&path);
            if !root.is_dir() {
                return None;
            }
            let case_count = case_values(&root).len();
            let capabilities = project_capabilities(&root);
            let mut warnings = Vec::new();
            if !capabilities.mock {
                warnings.push("未发现模拟接口菜单运行器".into());
            }
            if !capabilities.real_api {
                warnings.push("未发现真实接口菜单运行器".into());
            }
            if !capabilities.browser_style {
                warnings.push("未发现浏览器样式运行器".into());
            }
            Some(TestProject {
                path,
                name,
                project_kind: project_kind(&root),
                case_count,
                page_count: estimated_page_count(&root, case_count),
                capabilities,
                warnings,
            })
        })
        .collect())
}

#[tauri::command]
pub fn list_test_menus(
    state: tauri::State<'_, DatabaseState>,
    project_path: Option<String>,
) -> Result<Vec<TestMenu>, String> {
    if let Some(path) = project_path.filter(|value| !value.trim().is_empty()) {
        let (root, name) = canonical_project_asset(&state, &path)?;
        project_menus(&state, &root, &name)
    } else {
        let mut menus = client_menus(&state)?;
        menus.extend(app_menus(&state)?);
        Ok(menus)
    }
}

fn git_changed_paths(root: &Path) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for args in [
        vec!["status", "--porcelain=v1", "--untracked-files=all"],
        vec!["diff", "--name-only", "HEAD~1..HEAD"],
    ] {
        let mut command = codex_video::hidden_command(Path::new("git"));
        command.current_dir(root).args(args);
        let Ok(output) = command.output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let value = if line.len() > 3 && line.as_bytes().get(2) == Some(&b' ') {
                &line[3..]
            } else {
                line
            };
            let path = value
                .rsplit(" -> ")
                .next()
                .unwrap_or(value)
                .trim()
                .replace('\\', "/");
            if !path.is_empty() {
                paths.insert(path);
            }
        }
    }
    paths
}

fn menu_changed_files(menu: &TestMenu, root: &Path, changes: &BTreeSet<String>) -> Vec<String> {
    let source = menu.source_path.replace('\\', "/").to_lowercase();
    let parent = source
        .rsplit_once('/')
        .map(|value| value.0)
        .unwrap_or(&source);
    let source_content = fs::read_to_string(root.join(&menu.source_path))
        .unwrap_or_default()
        .to_lowercase();
    changes
        .iter()
        .filter(|changed| {
            let normalized = changed.to_lowercase();
            normalized == source
                || normalized.starts_with(&format!("{parent}/"))
                || normalized
                    .rsplit('/')
                    .next()
                    .and_then(|file| file.split('.').next())
                    .is_some_and(|stem| stem.len() > 3 && source_content.contains(stem))
        })
        .cloned()
        .collect()
}

#[tauri::command]
pub fn recommend_tests_from_git(
    state: tauri::State<'_, DatabaseState>,
    project_path: Option<String>,
) -> Result<Vec<TestRecommendation>, String> {
    let (projects, menus): (Vec<PathBuf>, Vec<TestMenu>) =
        if let Some(path) = project_path.filter(|value| !value.trim().is_empty()) {
            let (root, name) = canonical_project_asset(&state, &path)?;
            (vec![root.clone()], project_menus(&state, &root, &name)?)
        } else {
            (
                vec![client_root(), app_root()],
                client_menus(&state)?
                    .into_iter()
                    .chain(app_menus(&state)?)
                    .collect(),
            )
        };
    let changes_by_root = projects
        .iter()
        .map(|root| (display_path(root), git_changed_paths(root)))
        .collect::<Vec<_>>();
    let mut recommendations = Vec::new();
    for menu in menus {
        let root = PathBuf::from(&menu.project_path);
        let changes = changes_by_root
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(&menu.project_path))
            .map(|(_, changes)| changes)
            .cloned()
            .unwrap_or_default();
        let changed_files = menu_changed_files(&menu, &root, &changes);
        if changed_files.is_empty() {
            continue;
        }
        recommendations.push(TestRecommendation {
            menu_id: menu.id,
            project: menu.project.clone(),
            project_path: menu.project_path.clone(),
            menu_name: menu.name,
            reason: format!(
                "检测到 {} 个与该页面直接相关的 Git 变更",
                changed_files.len()
            ),
            recommended_mode: if menu.capabilities.mock {
                "mock"
            } else {
                "source-style"
            }
            .into(),
            changed_files,
        });
    }
    recommendations.sort_by(|a, b| {
        b.changed_files
            .len()
            .cmp(&a.changed_files.len())
            .then(a.menu_name.cmp(&b.menu_name))
    });
    Ok(recommendations)
}

#[tauri::command]
pub fn list_test_runs(
    state: tauri::State<'_, DatabaseState>,
    menu_id: Option<String>,
    project_path: Option<String>,
) -> Result<Vec<TestRun>, String> {
    let project = project_path
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|path| canonical_project_asset(&state, path))
        .transpose()?;
    let connection = state.connect()?;
    let sql = match menu_id.is_some() {
        true => format!(
            "SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE menu_id=?1 ORDER BY started_at DESC"
        ),
        false => format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs ORDER BY started_at DESC"),
    };
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = match menu_id {
        Some(id) => statement
            .query_map([id], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        None => statement
            .query_map([], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
    };
    let mut rows = rows.map_err(|error| error.to_string())?;
    if let Some((root, name)) = project {
        let canonical = project_identity::canonical_project_name(
            &connection,
            &name,
            &root.display().to_string(),
        );
        rows.retain(|run| {
            project_identity::canonical_project_name(&connection, &run.project, &run.project_path)
                == canonical
        });
    }
    rows.truncate(300);
    Ok(rows)
}

#[tauri::command]
pub fn read_test_report(path: String) -> Result<String, String> {
    let requested = PathBuf::from(path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let allowed = client_root()
        .join("e2e")
        .join("reports")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !requested.starts_with(&allowed)
        || requested.extension().and_then(|value| value.to_str()) != Some("md")
    {
        return Err("只能读取 client 项目现有测试报告目录中的 Markdown 文件。".into());
    }
    let content = fs::read_to_string(requested).map_err(|error| error.to_string())?;
    let menu = content
        .lines()
        .find_map(|line| line.strip_prefix("- 测试菜单："))
        .unwrap_or("当前菜单");
    Ok(append_remediation(
        content.clone(),
        menu,
        "client",
        client_report_status(&content) == "passed",
    ))
}

fn append_remediation(mut report: String, menu_name: &str, project: &str, passed: bool) -> String {
    if passed || report.contains("## 整改建议") {
        return report;
    }
    let failed_steps = report
        .lines()
        .filter(|line| line.contains("| 失败 |") || line.contains("：未通过"))
        .take(5)
        .map(|line| line.trim().trim_matches('|').replace("|", " · "))
        .collect::<Vec<_>>();
    let problem = if failed_steps.is_empty() {
        format!("{menu_name} 的测试未通过，具体错误请结合上方报告和日志定位。")
    } else {
        failed_steps.join("\n- ")
    };
    let checks = if project == "APP" {
        "- `pages.json` 中的路由是否对应真实 Vue 文件\n- 页面是否包含可渲染的 `template`\n- import 路径和分包 root 是否正确\n- 样式是否复用项目现有组件与变量"
    } else if report.contains("查询") || report.contains("搜索") {
        "- 搜索/查询按钮绑定事件\n- `handleQuery` 是否重置分页并重新调用列表接口\n- 列表请求参数是否包含页面筛选值\n- 请求是否被权限、重复拦截或错误 loading 状态阻断"
    } else {
        "- 失败步骤对应的按钮、表单或 Tab 事件\n- 页面请求、权限和接口返回值\n- Element Plus 弹窗/抽屉的可见状态\n- 测试数据是否满足当前页面前置条件"
    };
    report.push_str(&format!("\n\n## 整改建议\n\n### 问题\n\n- {problem}\n\n### 可能原因\n\n页面事件、请求参数、权限、接口数据或测试前置条件与现有用例预期不一致。\n\n### 建议检查\n\n{checks}\n\n### 建议验证\n\n1. 按失败步骤在页面中复现一次。\n2. 确认点击后触发预期请求或弹窗。\n3. 核对请求参数、接口响应和页面状态。\n4. 修正后重新运行同一菜单和同一测试类型。\n\n> 本建议只用于辅助整改，不会自动修改 client 或 APP 项目代码。\n"));
    report
}

fn scenario_description(mode: &str, title: &str, menu_name: &str) -> String {
    if title.contains("登录") || title.contains("进入页面") || title.contains("基础区域")
    {
        format!("确认“{menu_name}”的页面入口、登录态和核心区域可以正常使用。")
    } else if title.contains("查询") || title.contains("列表") {
        format!("确认“{menu_name}”的查询条件、列表请求和页面结果保持一致。")
    } else if title.contains("新增") || title.contains("修改") || title.contains("删除") {
        format!("确认“{menu_name}”的表单与关键业务操作符合页面预期。")
    } else if title.contains("窄屏")
        || title.contains("桌面")
        || title.contains("样式")
        || title.contains("溢出")
    {
        format!("确认“{menu_name}”在目标视口下没有明显显示问题。")
    } else if mode == "source-style" {
        format!("检查“{menu_name}”的页面源码结构、路由注册和基础样式。")
    } else {
        format!("执行“{menu_name}”已有的自动化场景并记录真实结果。")
    }
}

fn extract_test_titles(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap_or_default();
    extract_test_titles_from_source(&content)
}

fn extract_test_titles_from_source(content: &str) -> Vec<String> {
    let mut titles = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("test(") else {
            continue;
        };
        let Some(quote) = rest
            .chars()
            .next()
            .filter(|value| matches!(value, '\'' | '"' | '`'))
        else {
            continue;
        };
        let body = &rest[quote.len_utf8()..];
        if let Some(end) = body.find(quote) {
            let title = body[..end].trim();
            if !title.is_empty()
                && !title.contains("${")
                && !titles.iter().any(|item| item == title)
            {
                titles.push(title.to_string());
            }
        }
    }
    titles
}

#[derive(Default)]
struct PageBusinessContext {
    query_fields: Vec<String>,
    list_fields: Vec<String>,
    form_fields: Vec<String>,
    actions: Vec<String>,
}

struct SourceBusinessScenario {
    title: String,
    description: String,
    passed: bool,
    checks: Vec<String>,
}

fn find_open_tag(source: &str, tag: &str, from: usize) -> Option<usize> {
    let marker = format!("<{tag}");
    let mut cursor = from;
    while let Some(relative) = source.get(cursor..)?.find(&marker) {
        let start = cursor + relative;
        let boundary = source.get(start + marker.len()..)?.chars().next();
        if boundary.is_some_and(|value| value.is_whitespace() || matches!(value, '>' | '/')) {
            return Some(start);
        }
        cursor = start + marker.len();
    }
    None
}

fn opening_tags<'a>(source: &'a str, tag: &str) -> Vec<&'a str> {
    let mut values = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = find_open_tag(source, tag, cursor) {
        let Some(relative_end) = source[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        values.push(&source[start..end]);
        cursor = end;
    }
    values
}

fn static_tag_attribute(opening_tag: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=");
    let mut cursor = 0usize;
    while let Some(relative) = opening_tag.get(cursor..)?.find(&marker) {
        let start = cursor + relative;
        let previous = opening_tag.get(..start)?.chars().next_back();
        if previous.is_some_and(|value| value == ':' || (!value.is_whitespace() && value != '<')) {
            cursor = start + marker.len();
            continue;
        }
        let body = opening_tag.get(start + marker.len()..)?;
        let quote = body
            .chars()
            .next()
            .filter(|value| matches!(value, '\'' | '"'))?;
        let value = body.get(quote.len_utf8()..)?;
        let end = value.find(quote)?;
        let value = value[..end].trim();
        return (!value.is_empty()).then(|| value.to_string());
    }
    None
}

fn unique_tag_attributes(source: &str, tag: &str, attribute: &str) -> Vec<String> {
    let mut values = Vec::new();
    for value in opening_tags(source, tag)
        .into_iter()
        .filter_map(|item| static_tag_attribute(item, attribute))
    {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

fn first_element_block<'a>(source: &'a str, tag: &str) -> Option<(usize, &'a str)> {
    let start = find_open_tag(source, tag, 0)?;
    let close = format!("</{tag}>");
    let relative_end = source.get(start..)?.find(&close)?;
    let end = start + relative_end + close.len();
    Some((start, source.get(start..end)?))
}

fn plain_markup_text(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join("")
}

fn element_texts(source: &str, tag: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = 0usize;
    let close = format!("</{tag}>");
    while let Some(start) = find_open_tag(source, tag, cursor) {
        let Some(relative_open_end) = source[start..].find('>') else {
            break;
        };
        let content_start = start + relative_open_end + 1;
        let Some(relative_close) = source[content_start..].find(&close) else {
            break;
        };
        let end = content_start + relative_close;
        let value = plain_markup_text(&source[content_start..end]);
        if !value.is_empty() && !value.contains("{{") && !values.contains(&value) {
            values.push(value);
        }
        cursor = end + close.len();
    }
    values
}

fn source_business_context(root: &Path, menu: &TestMenu) -> PageBusinessContext {
    let source = fs::read_to_string(root.join(&menu.source_path)).unwrap_or_default();
    let query_block = first_element_block(&source, "el-form")
        .map(|(_, block)| block)
        .unwrap_or_default();
    let query_actions = element_texts(query_block, "el-button");
    let query_fields = if query_actions
        .iter()
        .any(|value| value.contains("查询") || value.contains("搜索") || value.contains("重置"))
    {
        unique_tag_attributes(query_block, "el-form-item", "label")
    } else {
        Vec::new()
    };
    let list_block = first_element_block(&source, "el-table")
        .map(|(_, block)| block)
        .unwrap_or_default();
    let list_fields = unique_tag_attributes(list_block, "el-table-column", "label")
        .into_iter()
        .filter(|value| value != "操作")
        .collect();
    let editor_block = ["el-dialog", "el-drawer"]
        .into_iter()
        .filter_map(|tag| first_element_block(&source, tag))
        .min_by_key(|(start, _)| *start)
        .map(|(_, block)| block)
        .unwrap_or_default();
    PageBusinessContext {
        query_fields,
        list_fields,
        form_fields: unique_tag_attributes(editor_block, "el-form-item", "label"),
        actions: element_texts(&source, "el-button"),
    }
}

fn summarized_business_items(items: &[String], limit: usize) -> String {
    let mut value = items
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join("、");
    if items.len() > limit {
        value.push_str(&format!("等{}项", items.len()));
    }
    value
}

fn source_business_scenarios(root: &Path, menu: &TestMenu) -> Vec<SourceBusinessScenario> {
    let path = root.join(&menu.source_path);
    let content = fs::read_to_string(&path).unwrap_or_default();
    let context = source_business_context(root, menu);
    let mut scenarios = vec![SourceBusinessScenario {
        title: format!("{}页面入口、路由与基础结构", menu.name),
        description: format!("确认“{}”页面文件、路由和 Vue 基础结构完整。", menu.name),
        passed: path.is_file() && !menu.route.trim().is_empty() && content.contains("<template"),
        checks: vec![
            "页面文件存在。".into(),
            "页面路由不为空。".into(),
            "页面包含可渲染的 template。".into(),
        ],
    }];
    if !context.query_fields.is_empty() {
        let fields = summarized_business_items(&context.query_fields, 6);
        scenarios.push(SourceBusinessScenario {
            title: format!("{}查询条件：{fields}", menu.name),
            description: format!(
                "确认“{}”源码包含与业务对应的查询条件：{fields}。",
                menu.name
            ),
            passed: true,
            checks: vec![
                format!("已识别查询字段：{fields}。"),
                "查询区包含搜索、查询或重置操作。".into(),
            ],
        });
    }
    if !context.list_fields.is_empty() {
        let fields = summarized_business_items(&context.list_fields, 6);
        scenarios.push(SourceBusinessScenario {
            title: format!("{}列表字段：{fields}", menu.name),
            description: format!("确认“{}”首个业务列表包含字段：{fields}。", menu.name),
            passed: true,
            checks: vec![format!("已识别列表字段：{fields}。")],
        });
    }
    if !context.actions.is_empty() {
        let actions = summarized_business_items(&context.actions, 8);
        scenarios.push(SourceBusinessScenario {
            title: format!("{}业务操作：{actions}", menu.name),
            description: format!("确认“{}”源码提供业务操作入口：{actions}。", menu.name),
            passed: true,
            checks: vec![format!("已识别操作按钮：{actions}。")],
        });
    }
    if !context.form_fields.is_empty() {
        let fields = summarized_business_items(&context.form_fields, 6);
        scenarios.push(SourceBusinessScenario {
            title: format!("{}表单字段：{fields}", menu.name),
            description: format!("确认“{}”编辑表单包含业务字段：{fields}。", menu.name),
            passed: true,
            checks: vec![format!("已识别编辑表单字段：{fields}。")],
        });
    }
    scenarios.push(SourceBusinessScenario {
        title: format!("{}页面样式与响应式布局", menu.name),
        description: format!(
            "确认“{}”页面包含样式入口，可继续进行桌面和窄屏检查。",
            menu.name
        ),
        passed: content.contains("<style") || content.contains("style="),
        checks: vec!["页面包含样式入口或行内样式。".into()],
    });
    scenarios
}

fn menu_case_value(menu: &TestMenu) -> Option<Value> {
    let source = fs::read_to_string(menu.case_file_path.as_deref()?).ok()?;
    serde_json::from_str(&strip_json_comments(&source)).ok()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuScenarioProfile {
    General,
    Post,
    SafeResponsibility,
}

fn menu_scenario_profile(menu: &TestMenu, case: Option<&Value>) -> MenuScenarioProfile {
    let case_id = case
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .or(menu.case_id.as_deref())
        .unwrap_or_default()
        .to_lowercase();
    let component = case
        .and_then(|value| value.get("component"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace('\\', "/")
        .to_lowercase();
    let permissions = case
        .and_then(|value| value.get("permissions"))
        .and_then(Value::as_array);
    let mock_rows = case
        .and_then(|value| value.get("mockRows"))
        .and_then(Value::as_array);
    let safety_permissions = permissions.is_some_and(|items| {
        items.iter().filter_map(Value::as_str).any(|permission| {
            permission.ends_with(":publish")
                || permission.ends_with(":issue")
                || permission.ends_with(":sign")
        })
    });
    let safety_fields = mock_rows.is_some_and(|rows| {
        rows.iter().any(|row| {
            row.get("reportId").is_some()
                && (row.get("dutyStatus").is_some() || row.get("signStaus").is_some())
        })
    });
    if case_id == "safe-responsibility"
        || component.contains("saferesponsibility")
        || safety_permissions
        || safety_fields
    {
        return MenuScenarioProfile::SafeResponsibility;
    }
    let post_fields = mock_rows.is_some_and(|rows| {
        rows.iter()
            .any(|row| row.get("postCode").is_some() && row.get("postName").is_some())
    });
    if case_id == "post" || component == "system/post/index" || post_fields {
        MenuScenarioProfile::Post
    } else {
        MenuScenarioProfile::General
    }
}

fn case_supports_action(case: Option<&Value>, action: &str) -> bool {
    case.and_then(|value| value.get("permissions"))
        .and_then(Value::as_array)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .filter_map(Value::as_str)
                .any(|permission| {
                    permission == "*:*:*" || permission.ends_with(&format!(":{action}"))
                })
        })
}

fn shared_mock_title_allowed(
    title: &str,
    profile: MenuScenarioProfile,
    case_id: &str,
    can_add: bool,
) -> bool {
    match title {
        "页面基础区域正常显示" | "查询会触发列表接口" => true,
        "新增空表单会触发必填校验且不会提交接口" => can_add,
        "新增合法数据可以提交" | "编辑入口可以打开并回显数据" => {
            matches!(case_id, "dict" | "system-url" | "post")
        }
        "列表展示字段完整，分页和操作列可见"
        | "查询条件组合：编码、名称、状态可同时查询，重置后清空"
        | "新增表单逐项校验：岗位编码为空"
        | "新增表单逐项校验：岗位名称为空"
        | "新增表单逐项校验：备注为空"
        | "编辑岗位后可以提交更新接口"
        | "选择用户抽屉可以打开并加载用户列表"
        | "窄屏视口页面关键区域仍可访问且无横向溢出" => {
            profile == MenuScenarioProfile::Post
        }
        "台账列表展示责任书核心字段和操作列"
        | "台账查询条件可以触发列表接口"
        | "新增空表单会触发模板和责任书名称必填校验"
        | "责任书下发标签页可以切换并加载下发列表"
        | "责任书统计标签页可以切换并展示统计表格"
        | "窄屏视口页面关键区域仍可访问" => {
            profile == MenuScenarioProfile::SafeResponsibility
        }
        "桌面视口页面无明显横向溢出，操作按钮不换行错位" => {
            matches!(
                profile,
                MenuScenarioProfile::Post | MenuScenarioProfile::SafeResponsibility
            )
        }
        _ => false,
    }
}

fn shared_real_title_allowed(title: &str, profile: MenuScenarioProfile, can_add: bool) -> bool {
    if profile == MenuScenarioProfile::SafeResponsibility {
        return true;
    }
    match title {
        "真实登录态可以进入页面并加载后端列表"
        | "点击查询会真实调用后端列表接口"
        | "点击重置后页面仍可继续查询"
        | "桌面视口下页面主体不应横向溢出"
        | "窄屏视口下页面主体不应横向溢出" => true,
        "新增空表单只做前端校验，不写入真实后端" => can_add,
        _ => false,
    }
}

fn filter_test_titles(
    menu: &TestMenu,
    mode: &str,
    spec: &Path,
    titles: Vec<String>,
) -> Vec<String> {
    let file_name = spec
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let case = menu_case_value(menu);
    let case_id = case
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str)
        .or(menu.case_id.as_deref())
        .unwrap_or_default();
    let profile = menu_scenario_profile(menu, case.as_ref());
    let can_add = case_supports_action(case.as_ref(), "add");
    titles
        .into_iter()
        .filter(|title| match (mode, file_name) {
            ("mock", "menu-module.spec.js") => {
                shared_mock_title_allowed(title, profile, case_id, can_add)
            }
            ("real", "real-menu-module.spec.js") => {
                shared_real_title_allowed(title, profile, can_add)
            }
            _ => true,
        })
        .collect()
}

fn safe_spec_file_name(value: &str) -> Option<String> {
    let path = Path::new(value);
    let file_name = path.file_name()?.to_str()?;
    if file_name == value && file_name.ends_with(".js") {
        Some(file_name.to_string())
    } else {
        None
    }
}

fn dedicated_real_spec_file(menu: &TestMenu) -> Option<String> {
    menu_case_value(menu)
        .and_then(|value| {
            value
                .get("realSpec")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .and_then(|value| safe_spec_file_name(&value))
}

fn selected_real_suite_id(value: Option<&str>) -> &str {
    match value {
        Some(DEDICATED_REAL_SUITE_ID) => DEDICATED_REAL_SUITE_ID,
        _ => COMMON_REAL_SUITE_ID,
    }
}

fn spec_for_mode_and_suite(
    root: &Path,
    menu: &TestMenu,
    mode: &str,
    suite_id: Option<&str>,
) -> Option<PathBuf> {
    let file = match mode {
        "mock" => "menu-module.spec.js".to_string(),
        "real" if selected_real_suite_id(suite_id) == DEDICATED_REAL_SUITE_ID => {
            dedicated_real_spec_file(menu)?
        }
        "real" => COMMON_REAL_SPEC_FILE.to_string(),
        "browser-style" => "page-style.spec.js".to_string(),
        _ => return None,
    };
    Some(root.join("e2e").join("specs").join(file))
}

fn scenario_titles_for_menu_and_suite(
    root: &Path,
    menu: &TestMenu,
    mode: &str,
    suite_id: Option<&str>,
) -> Vec<String> {
    if mode == "source-style" {
        return source_business_scenarios(root, menu)
            .into_iter()
            .map(|scenario| scenario.title)
            .collect();
    }
    if mode == "real" && selected_real_suite_id(suite_id) == COMMON_REAL_SUITE_ID {
        return extract_test_titles_from_source(COMMON_REAL_SPEC);
    }
    let Some(spec) =
        spec_for_mode_and_suite(root, menu, mode, suite_id).filter(|path| path.is_file())
    else {
        return Vec::new();
    };
    let titles = extract_test_titles(&spec);
    filter_test_titles(menu, mode, &spec, titles)
}

#[tauri::command]
pub fn list_test_suites(
    state: tauri::State<'_, DatabaseState>,
    project_path: String,
    menu_id: String,
) -> Result<Vec<TestSuite>, String> {
    let (root, name) = canonical_project_asset(&state, &project_path)?;
    let menu = project_menus(&state, &root, &name)?
        .into_iter()
        .find(|menu| menu.id == menu_id)
        .ok_or_else(|| "没有找到对应功能或页面。".to_string())?;
    let common_spec = ensure_common_real_spec(&root)?;
    let mut suites = vec![TestSuite {
        id: COMMON_REAL_SUITE_ID.into(),
        name: "公共通用用例".into(),
        description: "适用于所有真实接口页面，只读取登录态、首屏接口、查询、重置、运行时错误和页面溢出，不执行增删改。".into(),
        kind: "common".into(),
        read_only: true,
        spec_path: Some(display_path(&common_spec)),
    }];
    if let Some(file_name) = dedicated_real_spec_file(&menu) {
        let path = root.join("e2e").join("specs").join(file_name);
        if path.is_file() && !extract_test_titles(&path).is_empty() {
            suites.push(TestSuite {
                id: DEDICATED_REAL_SUITE_ID.into(),
                name: format!("{} 专属用例", menu.name),
                description:
                    "由 Codex CLI 按当前页面源码和接口生成，并已通过 Playwright 场景收集校验。"
                        .into(),
                kind: "dedicated".into(),
                read_only: false,
                spec_path: Some(display_path(&path)),
            });
        }
    }
    Ok(suites)
}

#[tauri::command]
pub fn list_test_scenarios(
    state: tauri::State<'_, DatabaseState>,
    project_path: String,
    menu_id: String,
    mode: String,
    test_suite_id: Option<String>,
) -> Result<Vec<TestScenario>, String> {
    let (root, name) = canonical_project_asset(&state, &project_path)?;
    let menu = project_menus(&state, &root, &name)?
        .into_iter()
        .find(|menu| menu.id == menu_id)
        .ok_or_else(|| "没有找到对应功能或页面。".to_string())?;
    if mode == "source-style" {
        return Ok(source_business_scenarios(&root, &menu)
            .into_iter()
            .enumerate()
            .map(|(index, scenario)| TestScenario {
                id: format!("scenario-{}", index + 1),
                title: scenario.title,
                description: scenario.description,
                mode: mode.clone(),
                default_selected: true,
            })
            .collect());
    }
    let titles = scenario_titles_for_menu_and_suite(&root, &menu, &mode, test_suite_id.as_deref());
    Ok(titles
        .into_iter()
        .enumerate()
        .map(|(index, title)| TestScenario {
            id: format!("scenario-{}", index + 1),
            description: scenario_description(&mode, &title, &menu.name),
            title,
            mode: mode.clone(),
            default_selected: true,
        })
        .collect())
}

fn safe_case_id(menu: &TestMenu) -> String {
    let seed = menu
        .source_path
        .trim_end_matches(".vue")
        .replace("src/views/", "")
        .replace(['\\', '/', ':', ' '], "-")
        .trim_matches('-')
        .to_lowercase();
    if seed.is_empty() {
        "generated-page".into()
    } else {
        seed
    }
}

fn generated_case_value(root: &Path, menu: &TestMenu, scenarios: &[String]) -> Value {
    let case_id = safe_case_id(menu);
    let component = menu
        .source_path
        .trim_end_matches(".vue")
        .trim_start_matches("src/views/")
        .replace('\\', "/");
    let business_context = source_business_context(root, menu);
    let generated_scenarios = source_business_scenarios(root, menu)
        .into_iter()
        .map(|scenario| scenario.title)
        .collect::<Vec<_>>();
    json!({
        "id": case_id,
        "menuName": menu.name,
        "aliases": [],
        "route": menu.route,
        "component": component,
        "layoutFlag": "systemLayout",
        "permissions": ["*:*:*"],
        "mockRows": [],
        "scenarios": generated_scenarios,
        "selectedScenariosAtCreation": scenarios,
        "businessContext": {
            "queryFields": business_context.query_fields,
            "listFields": business_context.list_fields,
            "formFields": business_context.form_fields,
            "actions": business_context.actions
        },
        "scenarioGeneration": {
            "strategy": "page-business-source",
            "description": "根据页面查询项、列表字段、表单字段和操作按钮自动生成"
        },
        "generatedBy": "星枢 ASTRION",
        "workbenchMenuId": menu.id,
        "generatedAt": Utc::now().to_rfc3339()
    })
}

fn create_case_file(root: &Path, menu: &TestMenu, scenarios: &[String]) -> Result<PathBuf, String> {
    let case_dir = root.join("e2e").join("menu-cases");
    fs::create_dir_all(&case_dir).map_err(|error| format!("无法创建测试配置目录：{error}"))?;
    let case_id = safe_case_id(menu);
    let path = case_dir.join(format!("{case_id}.json"));
    if path.exists() {
        return Err("目标测试配置已经存在，为避免覆盖现有用例已停止。".into());
    }
    let value = generated_case_value(root, menu, scenarios);
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("无法写入测试配置：{error}"))?;
    Ok(path)
}

fn generation_case_id(menu: &TestMenu) -> String {
    let raw = menu.case_id.clone().unwrap_or_else(|| safe_case_id(menu));
    let value = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if value.is_empty() {
        "generated-page".into()
    } else {
        value
    }
}

fn dedicated_generation_prompt(root: &Path, menu: &TestMenu, candidate_relative: &Path) -> String {
    let context = source_business_context(root, menu);
    format!(
        "请为当前业务页面生成一个专属的 Playwright 真实接口测试脚本。\n\n页面名称：{}\n页面路由：{}\n页面源码：{}\n组件标识：{}\n查询字段：{}\n列表字段：{}\n表单字段：{}\n页面操作：{}\n\n必须遵守：\n1. 先阅读页面源码、它直接引用的接口文件，以及项目 e2e/specs、e2e/support 中已有的真实接口测试约定。\n2. 只允许新建这一个文件：{}。不要修改任何其他文件，不要执行 git add、commit、push、reset 或清理命令。\n3. 使用项目已有的 @playwright/test；使用 E2E_AUTH_TOKEN 或 HLZT_TOKEN；优先复用 e2e/support/menuCase 和 realMenuRoute。\n4. 至少生成 3 个只属于“{}”业务的 test(...) 场景，场景名称和断言必须来自当前页面真实字段、按钮和接口，禁止套用安全责任、岗位管理、案例分享等其他业务场景。\n5. 优先生成只读测试。只有页面的关键流程必须写入才能验证时才可生成写入场景；写入数据必须带 E2E 前缀，并使用 try/finally 清理。\n6. 不要启动开发服务器，不要运行真实测试；工作台会在生成后用 Playwright --list 做语法和场景收集校验。\n7. 完成后只用中文简短说明生成的文件和场景，不要再改其他内容。",
        menu.name,
        menu.route,
        menu.source_path,
        menu_component(menu),
        context.query_fields.join("、"),
        context.list_fields.join("、"),
        context.form_fields.join("、"),
        context.actions.join("、"),
        candidate_relative.to_string_lossy().replace('\\', "/"),
        menu.name,
    )
}

fn attach_dedicated_spec_to_case(
    root: &Path,
    menu: &TestMenu,
    spec_file: &str,
    scenarios: &[String],
) -> Result<PathBuf, String> {
    let case_dir = root.join("e2e").join("menu-cases");
    fs::create_dir_all(&case_dir).map_err(|error| format!("无法创建测试配置目录：{error}"))?;
    let path = menu
        .case_file_path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| case_dir.join(format!("{}.json", generation_case_id(menu))));
    let mut value = if path.is_file() {
        serde_json::from_str::<Value>(
            &fs::read_to_string(&path).map_err(|error| format!("无法读取现有测试配置：{error}"))?,
        )
        .map_err(|error| format!("现有测试配置不是有效 JSON：{error}"))?
    } else {
        generated_case_value(root, menu, scenarios)
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| "测试配置必须是 JSON 对象。".to_string())?;
    object.insert("realSpec".into(), Value::String(spec_file.into()));
    object.insert(
        "codexGeneration".into(),
        json!({
            "validated": true,
            "validator": "playwright-list",
            "generatedAt": Utc::now().to_rfc3339()
        }),
    );
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("无法保存专属测试配置：{error}"))?;
    Ok(path)
}

fn validate_dedicated_candidate(
    root: &Path,
    menu: &TestMenu,
    candidate: &Path,
    job_id: &str,
) -> Result<Vec<String>, String> {
    if !candidate.is_file() {
        return Err("Codex CLI 已结束，但没有生成约定的专属 Playwright 文件。".into());
    }
    let source = fs::read_to_string(candidate)
        .map_err(|error| format!("无法读取生成的 Playwright 文件：{error}"))?;
    let titles = extract_test_titles_from_source(&source);
    if titles.len() < 3 {
        return Err(format!(
            "专属用例只识别到 {} 个场景，至少需要 3 个。",
            titles.len()
        ));
    }
    if !source.contains("@playwright/test") {
        return Err("专属用例没有使用项目的 @playwright/test。".into());
    }
    let business = source_business_context(root, menu);
    let evidence = business
        .query_fields
        .iter()
        .chain(&business.list_fields)
        .chain(&business.form_fields)
        .chain(&business.actions)
        .filter(|value| value.chars().count() >= 2)
        .any(|value| source.contains(value));
    if !evidence && !source.contains(&menu.name) {
        return Err("专属用例没有包含当前页面名称、字段或操作，无法确认它属于当前业务。".into());
    }

    let case_id = generation_case_id(menu);
    let case_dir = root.join("e2e").join("menu-cases");
    fs::create_dir_all(&case_dir).map_err(|error| error.to_string())?;
    let temporary_case = (!menu.has_case_file)
        .then(|| case_dir.join(format!("workbench-candidate-{case_id}-{job_id}.json")));
    if let Some(path) = temporary_case.as_ref() {
        fs::write(
            path,
            serde_json::to_string_pretty(&generated_case_value(root, menu, &titles))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("无法准备 Playwright 校验配置：{error}"))?;
    }

    let cli = root
        .join("node_modules")
        .join("@playwright")
        .join("test")
        .join("cli.js");
    if !cli.is_file() {
        if let Some(path) = temporary_case.as_ref() {
            let _ = fs::remove_file(path);
        }
        return Err("项目尚未安装 @playwright/test，无法校验专属用例。".into());
    }
    let candidate_name = candidate
        .file_name()
        .ok_or_else(|| "专属用例文件名无效。".to_string())?;
    let output = codex_video::hidden_command(Path::new("node"))
        .current_dir(root)
        .arg(
            Path::new("node_modules")
                .join("@playwright")
                .join("test")
                .join("cli.js"),
        )
        .args(["test"])
        .arg(candidate_name)
        .args(["--list", "--project=chromium"])
        .env("E2E_MENU_NAME", &menu.name)
        .env("E2E_MENU_CASE_ID", &case_id)
        .env("E2E_MENU_ROUTE", &menu.route)
        .env("E2E_MENU_COMPONENT", menu_component(menu))
        .output()
        .map_err(|error| format!("无法启动 Playwright 场景收集校验：{error}"));
    if let Some(path) = temporary_case.as_ref() {
        let _ = fs::remove_file(path);
    }
    let output = output?;
    if !output.status.success() {
        let detail = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(format!(
            "Playwright 无法收集生成的专属场景：{}",
            detail.trim()
        ));
    }
    Ok(titles)
}

fn run_test_case_generation(
    database: DatabaseState,
    generation_state: TestCaseGenerationState,
    job_id: String,
    root: PathBuf,
    menu: TestMenu,
) -> Result<PathBuf, String> {
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    generation_state.progress(&job_id, 8, "正在读取页面源码和现有测试约定");
    let case_id = generation_case_id(&menu);
    let final_file = format!("real-{case_id}.spec.js");
    let final_path = root.join("e2e").join("specs").join(&final_file);
    if final_path.exists() {
        return Err("目标专属 Playwright 文件已经存在，为避免覆盖已停止生成。".into());
    }
    let candidate_file = format!("workbench-candidate-{case_id}-{job_id}.spec.js");
    let candidate_relative = Path::new("e2e").join("specs").join(&candidate_file);
    let candidate_path = root.join(&candidate_relative);
    fs::create_dir_all(candidate_path.parent().unwrap_or(&root))
        .map_err(|error| format!("无法创建专属用例目录：{error}"))?;

    let job_dir = database
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("testing-codex-jobs")
        .join(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| error.to_string())?;
    let jsonl_path = job_dir.join("codex-run.jsonl");
    let stderr_path = job_dir.join("codex-stderr.log");
    let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let prompt = dedicated_generation_prompt(&root, &menu, &candidate_relative);

    generation_state.progress(&job_id, 15, "Codex CLI 已启动，正在分析当前业务");
    let mut command = codex_video::hidden_command(&cli_path);
    command
        .args(["--sandbox", "workspace-write", "--cd"])
        .arg(&root)
        .args([
            "exec",
            "--ephemeral",
            "--json",
            "--skip-git-repo-check",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file));
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Codex CLI 失败：{error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|error| format!("发送专属用例任务给 Codex 失败：{error}"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Codex CLI 生成进度。".to_string())?;
    let mut log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&jsonl_path)
        .map_err(|error| error.to_string())?;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        writeln!(log, "{line}").map_err(|error| error.to_string())?;
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            match value.get("type").and_then(Value::as_str) {
                Some("thread.started") => {
                    generation_state.progress(&job_id, 22, "Codex 已建立生成任务")
                }
                Some("item.started") => {
                    generation_state.progress(&job_id, 38, "正在检查页面源码和接口定义")
                }
                Some("item.completed")
                    if value.pointer("/item/type").and_then(Value::as_str)
                        == Some("file_change") =>
                {
                    generation_state.progress(&job_id, 72, "专属 Playwright 脚本已写入，正在整理")
                }
                Some("item.completed") => {
                    generation_state.progress(&job_id, 58, "正在生成当前业务测试场景")
                }
                Some("turn.completed") => {
                    generation_state.progress(&job_id, 82, "Codex 生成完成，准备校验")
                }
                _ => {}
            }
        }
    }
    let status = child
        .wait()
        .map_err(|error| format!("等待 Codex CLI 结束时出错：{error}"))?;
    if !status.success() {
        let detail = fs::read_to_string(&stderr_path).unwrap_or_default();
        let _ = fs::remove_file(&candidate_path);
        return Err(format!("Codex CLI 生成失败：{}", detail.trim()));
    }

    generation_state.progress(&job_id, 88, "正在校验业务归属和 Playwright 语法");
    let titles = validate_dedicated_candidate(&root, &menu, &candidate_path, &job_id)?;
    if final_path.exists() {
        let _ = fs::remove_file(&candidate_path);
        return Err("校验期间目标专属用例已存在，为避免覆盖已停止。".into());
    }
    fs::rename(&candidate_path, &final_path)
        .map_err(|error| format!("无法保存已校验的专属用例：{error}"))?;
    if let Err(error) = attach_dedicated_spec_to_case(&root, &menu, &final_file, &titles) {
        let _ = fs::remove_file(&final_path);
        return Err(error);
    }
    generation_state.progress(&job_id, 96, "校验通过，正在更新测试中心用例列表");
    Ok(final_path)
}

#[tauri::command]
pub fn start_test_case_generation(
    state: tauri::State<'_, DatabaseState>,
    generation_state: tauri::State<'_, TestCaseGenerationState>,
    project_path: String,
    menu_id: String,
) -> Result<TestCaseGenerationJob, String> {
    let (root, project_name) = canonical_project_asset(&state, &project_path)?;
    let menu = project_menus(&state, &root, &project_name)?
        .into_iter()
        .find(|menu| menu.id == menu_id)
        .ok_or_else(|| "没有找到对应功能或页面。".to_string())?;
    if dedicated_real_spec_file(&menu)
        .map(|file| root.join("e2e").join("specs").join(file).is_file())
        .unwrap_or(false)
    {
        return Err("当前业务已经有专属真实接口用例，无需重复生成。".into());
    }
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    let login = codex_video::hidden_command(&cli_path)
        .args(["login", "status"])
        .output();
    if !login.as_ref().is_ok_and(|output| output.status.success()) {
        return Err("Codex CLI 尚未登录，请先在终端运行 codex login。".into());
    }
    let id = Uuid::new_v4().to_string();
    let job = TestCaseGenerationJob {
        id: id.clone(),
        project_path: display_path(&root),
        menu_id: menu.id.clone(),
        menu_name: menu.name.clone(),
        status: "queued".into(),
        progress_percent: 3,
        progress_message: "正在准备 Codex CLI 生成任务".into(),
        error_message: String::new(),
        generated_spec_path: None,
        created_at: Utc::now().to_rfc3339(),
        finished_at: None,
    };
    generation_state.insert(job.clone())?;
    let database = state.inner().clone();
    let jobs = generation_state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        match run_test_case_generation(database, jobs.clone(), id.clone(), root, menu) {
            Ok(path) => jobs.complete(&id, &path),
            Err(error) => jobs.fail(&id, error),
        }
    });
    Ok(job)
}

#[tauri::command]
pub fn get_test_case_generation(
    generation_state: tauri::State<'_, TestCaseGenerationState>,
    job_id: String,
) -> Result<TestCaseGenerationJob, String> {
    generation_state.get(&job_id)
}

fn preflight_for(
    state: &DatabaseState,
    options: &StartTestOptions,
) -> Result<(PathBuf, String, TestMenu, TestPreflight), String> {
    let (root, name) = canonical_project_asset(state, &options.project_path)?;
    let menu = project_menus(state, &root, &name)?
        .into_iter()
        .find(|menu| menu.id == options.menu_id)
        .ok_or_else(|| "没有找到对应功能或页面。".to_string())?;
    let capabilities = project_capabilities(&root);
    let mut checks = Vec::new();
    checks.push(PreflightCheck {
        name: "项目目录".into(),
        passed: root.is_dir(),
        detail: root.display().to_string(),
    });
    let source = root.join(&menu.source_path);
    checks.push(PreflightCheck {
        name: "页面源码".into(),
        passed: source.is_file(),
        detail: source.display().to_string(),
    });
    let real_suite_id = selected_real_suite_id(options.test_suite_id.as_deref());
    let uses_common_real = options.mode == "real" && real_suite_id == COMMON_REAL_SUITE_ID;
    let has_case_or_create = menu.has_case_file || options.create_case_file == Some(true);
    if options.mode != "source-style" && !uses_common_real {
        checks.push(PreflightCheck {
            name: "测试配置".into(),
            passed: has_case_or_create,
            detail: if menu.has_case_file {
                menu.case_file_path.clone().unwrap_or_default()
            } else if options.create_case_file == Some(true) {
                "将在执行前创建新配置，不覆盖已有文件".into()
            } else {
                "缺少 e2e/menu-cases 配置".into()
            },
        });
    }
    let supported = match options.mode.as_str() {
        "mock" => capabilities.mock,
        "real" => capabilities.real_api,
        "source-style" => capabilities.source_style,
        "browser-style" => capabilities.browser_style,
        _ => false,
    };
    checks.push(PreflightCheck {
        name: "测试运行器".into(),
        passed: supported,
        detail: if supported {
            "项目已有对应测试脚本".into()
        } else {
            "项目资产中未发现对应测试脚本或规格文件".into()
        },
    });
    if options.mode == "real" {
        if uses_common_real {
            checks.push(PreflightCheck {
                name: "测试用例".into(),
                passed: true,
                detail: "公共通用用例仅执行只读检查，不提交增删改请求".into(),
            });
        } else {
            let dedicated_ready = dedicated_real_spec_file(&menu)
                .map(|file| root.join("e2e").join("specs").join(file).is_file())
                .unwrap_or(false);
            checks.push(PreflightCheck {
                name: "专属测试用例".into(),
                passed: dedicated_ready,
                detail: if dedicated_ready {
                    "专属 Playwright 脚本已生成并通过场景收集校验".into()
                } else {
                    "尚未生成可用的专属 Playwright 脚本".into()
                },
            });
            checks.push(PreflightCheck {
                name: "真实写入确认".into(),
                passed: options.confirmed_real_write == Some(true),
                detail: "专属真实用例可能创建、修改并清理 E2E 前缀数据".into(),
            });
        }
        let token_ready = options
            .token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || (options.use_environment_token != Some(false)
                && (std::env::var_os("HLZT_TOKEN").is_some()
                    || std::env::var_os("E2E_AUTH_TOKEN").is_some()));
        checks.push(PreflightCheck {
            name: "测试凭据".into(),
            passed: token_ready,
            detail: if token_ready {
                "仅在本次子进程使用，不写入数据库".into()
            } else {
                "缺少临时 Token 或 Windows 用户 HLZT_TOKEN".into()
            },
        });
    }
    let allowed_scenarios = scenario_titles_for_menu_and_suite(
        &root,
        &menu,
        &options.mode,
        options.test_suite_id.as_deref(),
    );
    let invalid_scenarios = options
        .selected_scenarios
        .iter()
        .filter(|title| !allowed_scenarios.contains(title))
        .count();
    checks.push(PreflightCheck {
        name: "测试场景".into(),
        passed: !options.selected_scenarios.is_empty() && invalid_scenarios == 0,
        detail: if invalid_scenarios == 0 {
            format!(
                "已按当前业务配置选择 {} 个场景",
                options.selected_scenarios.len()
            )
        } else {
            format!("有 {invalid_scenarios} 个场景不属于当前业务，请重新选择")
        },
    });
    let ready = checks.iter().all(|check| check.passed);
    Ok((
        root,
        name,
        menu,
        TestPreflight {
            ready,
            status: if ready {
                "ready".into()
            } else {
                "blocked".into()
            },
            checks,
            warnings: Vec::new(),
        },
    ))
}

#[tauri::command]
pub fn preflight_test(
    state: tauri::State<'_, DatabaseState>,
    options: StartTestOptions,
) -> Result<TestPreflight, String> {
    preflight_for(&state, &options).map(|(_, _, _, preflight)| preflight)
}

#[cfg(test)]
fn app_static_report(menu: &TestMenu) -> (bool, String) {
    let source = app_root().join(&menu.source_path);
    let content = fs::read_to_string(&source).unwrap_or_default();
    let exists = source.is_file();
    let template = content.contains("<template");
    let style = content.contains("<style") || content.contains("style=");
    let script = content.contains("<script");
    let passed = exists && template;
    let mark = |ok: bool| if ok { "通过" } else { "未通过" };
    let report = format!("# APP 页面静态检查报告\n\n- 页面：{}\n- 路由：{}\n- 源码：{}\n- 生成时间：{}\n- 结论：{}\n\n## 检查结果\n\n- 页面文件存在：{}\n- 包含 template：{}\n- 包含 script：{}\n- 包含 style 或行内样式：{}\n\n## 验证边界\n\n本报告只检查 pages.json 注册信息和本地 Vue 源码结构，不代表真实接口、浏览器交互或视觉效果已经通过。APP 当前没有可复用的菜单级自动化用例。\n",
        menu.name, menu.route, source.display(), Local::now().to_rfc3339(), mark(passed), mark(exists), mark(template), mark(script), mark(style));
    (passed, report)
}

fn regex_escape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for ch in value.chars() {
        if matches!(
            ch,
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
        ) {
            result.push('\\');
        }
        result.push(ch);
    }
    result
}

fn ensure_common_real_spec(root: &Path) -> Result<PathBuf, String> {
    let directory = root.join("e2e").join("specs");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建公共测试用例目录：{error}"))?;
    let path = directory.join(COMMON_REAL_SPEC_FILE);
    let current = fs::read_to_string(&path).unwrap_or_default();
    if current != COMMON_REAL_SPEC {
        fs::write(&path, COMMON_REAL_SPEC)
            .map_err(|error| format!("无法写入公共通用测试用例：{error}"))?;
    }
    Ok(path)
}

fn menu_component(menu: &TestMenu) -> String {
    menu.source_path
        .trim_end_matches(".vue")
        .trim_start_matches("src/views/")
        .replace('\\', "/")
}

fn menu_layout_flag(menu: &TestMenu) -> String {
    menu_case_value(menu)
        .and_then(|value| {
            value
                .get("layoutFlag")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "businessLayout".into())
}

fn artifact_kind(name: &str, content_type: &str) -> String {
    let value = format!("{name} {content_type}").to_lowercase();
    if value.contains("screenshot") || value.contains("image/") {
        "screenshot".into()
    } else if value.contains("trace") || value.ends_with(".zip") {
        "trace".into()
    } else if value.contains("log") || value.contains("text/") {
        "log".into()
    } else {
        "attachment".into()
    }
}

fn normalize_artifact_path(root: &Path, value: &str) -> String {
    let path = PathBuf::from(value);
    let resolved = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    display_path(&resolved)
}

fn collect_playwright_specs(value: &Value, root: &Path, output: &mut Vec<TestScenarioResult>) {
    if let Some(specs) = value.get("specs").and_then(Value::as_array) {
        for spec in specs {
            let title = spec
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("未命名场景")
                .to_string();
            let tests = spec
                .get("tests")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for test in tests {
                let results = test
                    .get("results")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let last = results.last().cloned().unwrap_or(Value::Null);
                let outcome = test
                    .get("outcome")
                    .and_then(Value::as_str)
                    .or_else(|| last.get("status").and_then(Value::as_str))
                    .unwrap_or("failed");
                let status = match outcome {
                    "expected" | "passed" => "passed",
                    "skipped" => "skipped",
                    _ => "failed",
                }
                .to_string();
                let duration_ms = results
                    .iter()
                    .filter_map(|item| item.get("duration").and_then(Value::as_i64))
                    .sum();
                let error_message = last
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let artifacts = last
                    .get("attachments")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|item| {
                        let path = item.get("path").and_then(Value::as_str)?;
                        let name = item
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("附件")
                            .to_string();
                        let content_type = item
                            .get("contentType")
                            .and_then(Value::as_str)
                            .unwrap_or("application/octet-stream")
                            .to_string();
                        Some(TestArtifact {
                            kind: artifact_kind(&name, &content_type),
                            name,
                            path: normalize_artifact_path(root, path),
                            content_type,
                        })
                    })
                    .collect::<Vec<_>>();
                output.push(TestScenarioResult {
                    id: format!("scenario-result-{}", output.len() + 1),
                    purpose: scenario_description("", &title, "当前功能"),
                    steps: vec![
                        "进入目标功能或页面。".into(),
                        "执行该场景对应的页面操作。".into(),
                        "核对页面、接口和断言结果。".into(),
                    ],
                    checks: vec!["项目现有自动化断言全部达到预期。".into()],
                    title: title.clone(),
                    status,
                    duration_ms,
                    error_message,
                    artifacts,
                });
            }
        }
    }
    if let Some(suites) = value.get("suites").and_then(Value::as_array) {
        for suite in suites {
            collect_playwright_specs(suite, root, output);
        }
    }
}

fn persist_screenshot_artifacts(root: &Path, run_id: &str, scenarios: &mut [TestScenarioResult]) {
    let target_dir = root
        .join("e2e")
        .join("reports")
        .join("artifacts")
        .join(run_id);
    let mut index = 0usize;
    for scenario in scenarios {
        for artifact in &mut scenario.artifacts {
            if artifact.kind != "screenshot" {
                continue;
            }
            let source = PathBuf::from(&artifact.path);
            if !source.is_file() || image_mime(&source).is_none() {
                continue;
            }
            index += 1;
            let extension = source
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("png");
            let target = target_dir.join(format!("screenshot-{index:03}.{extension}"));
            if fs::create_dir_all(&target_dir).is_ok() && fs::copy(&source, &target).is_ok() {
                artifact.path = display_path(&target);
            }
        }
    }
}

fn static_scenarios(root: &Path, menu: &TestMenu, selected: &[String]) -> Vec<TestScenarioResult> {
    source_business_scenarios(root, menu)
        .into_iter()
        .filter(|scenario| selected.iter().any(|item| item == &scenario.title))
        .enumerate()
        .map(|(index, scenario)| TestScenarioResult {
            id: format!("static-{}", index + 1),
            title: scenario.title.clone(),
            status: if scenario.passed {
                "passed".into()
            } else {
                "failed".into()
            },
            duration_ms: 0,
            purpose: scenario.description,
            steps: vec![
                format!("读取 {}。", menu.source_path),
                "按页面业务内容检查源码结构。".into(),
            ],
            checks: scenario.checks,
            error_message: if scenario.passed {
                String::new()
            } else {
                format!("{} 未达到静态检查要求。", scenario.title)
            },
            artifacts: Vec::new(),
        })
        .collect()
}

struct ExecutionResult {
    status: String,
    output_excerpt: String,
    error_message: String,
    scenario_results: Vec<TestScenarioResult>,
    exit_code: Option<i32>,
    report_path: Option<String>,
    environment_summary: String,
    cleanup_status: String,
}

fn execute_project(
    root: &Path,
    menu: &TestMenu,
    options: &StartTestOptions,
    run_id: &str,
    process_state: &TestProcessState,
) -> Result<ExecutionResult, String> {
    if process_state.is_cancelled(run_id) {
        return Ok(cancelled_execution());
    }
    if options.mode == "source-style" {
        let results = static_scenarios(root, menu, &options.selected_scenarios);
        let failed = results.iter().any(|item| item.status == "failed");
        return Ok(ExecutionResult {
            status: if failed {
                "failed".into()
            } else {
                "passed".into()
            },
            output_excerpt: format!("已完成 {} 个静态检查场景。", results.len()),
            error_message: results
                .iter()
                .find(|item| item.status == "failed")
                .map(|item| item.error_message.clone())
                .unwrap_or_default(),
            scenario_results: results,
            exit_code: Some(if failed { 1 } else { 0 }),
            report_path: None,
            environment_summary: "工作台内置源码检查；未启动浏览器或真实接口。".into(),
            cleanup_status: "not-applicable".into(),
        });
    }
    if options.mode == "real"
        && selected_real_suite_id(options.test_suite_id.as_deref()) == COMMON_REAL_SUITE_ID
    {
        ensure_common_real_spec(root)?;
    }
    let spec = spec_for_mode_and_suite(root, menu, &options.mode, options.test_suite_id.as_deref())
        .ok_or_else(|| "没有找到对应测试规格文件。".to_string())?;
    if !spec.is_file() {
        return Err(format!("测试规格文件不存在：{}", spec.display()));
    }
    let raw_dir = root.join("e2e").join("reports").join("raw");
    fs::create_dir_all(&raw_dir).map_err(|error| error.to_string())?;
    let raw_path = raw_dir.join(format!("workbench-{run_id}.json"));
    let cli = root
        .join("node_modules")
        .join("@playwright")
        .join("test")
        .join("cli.js");
    if !cli.is_file() {
        return Err("项目尚未安装 @playwright/test，无法运行浏览器场景。".into());
    }
    let grep = options
        .selected_scenarios
        .iter()
        .map(|item| regex_escape(item))
        .collect::<Vec<_>>()
        .join("|");
    let cli_argument = Path::new("node_modules")
        .join("@playwright")
        .join("test")
        .join("cli.js");
    let spec_argument = spec
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| "测试规格文件名无效。".to_string())?;
    let child_root = PathBuf::from(display_path(root));
    let mut command = codex_video::hidden_command(Path::new("node"));
    command
        .current_dir(&child_root)
        .arg(cli_argument)
        .args(["test"])
        .arg(spec_argument)
        .args(["--project=chromium", "--grep"])
        .arg(grep);
    command
        .env("E2E_MENU_NAME", &menu.name)
        .env("E2E_MENU_ROUTE", &menu.route)
        .env("E2E_MENU_COMPONENT", menu_component(menu))
        .env("E2E_MENU_LAYOUT_FLAG", menu_layout_flag(menu))
        .env(
            "E2E_MENU_CASE_ID",
            menu.case_id.clone().unwrap_or_else(|| safe_case_id(menu)),
        )
        .env("E2E_JSON_REPORT", display_path(&raw_path));
    if let Some(account) = options
        .account
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        command.env("E2E_TEST_ACCOUNT", account);
    }
    if let Some(token) = options
        .token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        command.env("E2E_AUTH_TOKEN", token);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|error| format!("无法启动项目现有测试脚本：{error}"))?;
    process_state.set_pid(run_id, child.id());
    let output = child
        .wait_with_output()
        .map_err(|error| format!("等待项目测试脚本结束时出错：{error}"))?;
    if process_state.is_cancelled(run_id) {
        return Ok(cancelled_execution());
    }
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut scenarios = Vec::new();
    if let Ok(value) = fs::read_to_string(&raw_path)
        .and_then(|content| serde_json::from_str::<Value>(&content).map_err(std::io::Error::other))
    {
        collect_playwright_specs(&value, root, &mut scenarios);
    }
    persist_screenshot_artifacts(root, run_id, &mut scenarios);
    let exit_code = output.status.code();
    if scenarios.is_empty() {
        let detail = if combined.trim().is_empty() {
            "测试进程没有生成场景结果，请检查项目 Playwright 配置和测试规格文件。".to_string()
        } else {
            combined.chars().take(1200).collect()
        };
        scenarios.push(TestScenarioResult {
            id: "executor-output".into(),
            title: "测试执行器未生成有效结果".into(),
            status: "failed".into(),
            duration_ms: 0,
            purpose: "确认项目现有 Playwright 测试可以启动并输出结构化结果。".into(),
            steps: vec![
                "启动项目现有测试执行器。".into(),
                "等待 Playwright 输出测试场景结果。".into(),
            ],
            checks: vec!["至少生成一个可识别的测试场景结果。".into()],
            error_message: detail,
            artifacts: Vec::new(),
        });
    }
    let status = if output.status.success() && scenarios.iter().any(|item| item.status == "passed")
    {
        "passed"
    } else {
        "failed"
    }
    .to_string();
    let error_message = scenarios
        .iter()
        .find(|item| item.status == "failed" && !item.error_message.is_empty())
        .map(|item| item.error_message.clone())
        .unwrap_or_else(|| {
            if output.status.success() {
                String::new()
            } else {
                combined.chars().take(1200).collect()
            }
        });
    let cleanup_status = if options.mode == "real"
        && options
            .selected_scenarios
            .iter()
            .any(|title| title.contains("新增") || title.contains("删除") || title.contains("修改"))
    {
        if status == "passed" {
            "completed"
        } else {
            "unknown"
        }
    } else {
        "not-applicable"
    }
    .to_string();
    Ok(ExecutionResult {
        status,
        output_excerpt: combined.chars().take(12000).collect(),
        error_message,
        scenario_results: scenarios,
        exit_code,
        report_path: Some(display_path(&raw_path)),
        environment_summary: format!("Node + Playwright；规格文件 {}", display_path(&spec)),
        cleanup_status,
    })
}

fn cancelled_execution() -> ExecutionResult {
    ExecutionResult {
        status: "cancelled".into(),
        output_excerpt: "用户已取消本次测试。".into(),
        error_message: String::new(),
        scenario_results: vec![TestScenarioResult {
            id: "cancelled".into(),
            title: "测试执行已取消".into(),
            status: "skipped".into(),
            duration_ms: 0,
            purpose: "记录人工终止的测试执行。".into(),
            steps: vec!["启动测试执行器。".into(), "收到人工取消指令。".into()],
            checks: vec!["测试子进程已经终止。".into()],
            error_message: String::new(),
            artifacts: Vec::new(),
        }],
        exit_code: None,
        report_path: None,
        environment_summary: "测试由用户主动取消。".into(),
        cleanup_status: "unknown".into(),
    }
}

fn save_run(state: &DatabaseState, run: &TestRun) -> Result<(), String> {
    state.connect()?.execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message,selected_scenarios,scenario_results,artifacts,total_count,passed_count,failed_count,skipped_count,duration_ms,exit_code,environment_summary,cleanup_status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24) ON CONFLICT(id) DO UPDATE SET status=excluded.status,finished_at=excluded.finished_at,report_markdown=excluded.report_markdown,source_report_path=excluded.source_report_path,output_excerpt=excluded.output_excerpt,error_message=excluded.error_message,scenario_results=excluded.scenario_results,artifacts=excluded.artifacts,total_count=excluded.total_count,passed_count=excluded.passed_count,failed_count=excluded.failed_count,skipped_count=excluded.skipped_count,duration_ms=excluded.duration_ms,exit_code=excluded.exit_code,environment_summary=excluded.environment_summary,cleanup_status=excluded.cleanup_status",
        params![run.id,run.menu_id,run.project,run.project_path,run.menu_name,run.mode,run.status,run.started_at,run.finished_at,run.report_markdown,run.source_report_path,run.output_excerpt,run.error_message,serde_json::to_string(&run.selected_scenarios).unwrap_or_else(|_| "[]".into()),serde_json::to_string(&run.scenario_results).unwrap_or_else(|_| "[]".into()),serde_json::to_string(&run.artifacts).unwrap_or_else(|_| "[]".into()),run.total_count,run.passed_count,run.failed_count,run.skipped_count,run.duration_ms,run.exit_code,run.environment_summary,run.cleanup_status]).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn recover_incomplete_test_runs(state: &DatabaseState) -> Result<usize, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(&format!(
            "SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE status IN ('queued','running')"
        ))
        .map_err(|error| error.to_string())?;
    let runs = statement
        .query_map([], row_to_run)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    drop(statement);
    drop(connection);
    for mut run in runs.iter().cloned() {
        run.status = "error".into();
        run.finished_at = Some(Utc::now().to_rfc3339());
        run.error_message = "桌面程序在测试结束前关闭，本次执行状态无法继续确认。".into();
        run.environment_summary = "检测到上次未正常结束的测试记录；请重新执行。".into();
        run.cleanup_status = "unknown".into();
        run.scenario_results = vec![TestScenarioResult {
            id: "interrupted".into(),
            title: "测试执行被中断".into(),
            status: "blocked".into(),
            duration_ms: 0,
            purpose: "标记未能正常收尾的历史测试。".into(),
            steps: vec!["测试开始执行。".into(), "桌面程序在完成前退出。".into()],
            checks: vec!["重新运行相同测试场景。".into()],
            error_message: run.error_message.clone(),
            artifacts: Vec::new(),
        }];
        run.total_count = 1;
        run.passed_count = 0;
        run.failed_count = 0;
        run.skipped_count = 0;
        run.report_markdown = build_structured_report(&run);
        save_run(state, &run)?;
    }
    Ok(runs.len())
}

fn build_structured_report(run: &TestRun) -> String {
    let mut lines = vec![
        "# 测试执行报告".to_string(),
        String::new(),
        format!("- 项目：{}", run.project),
        format!("- 项目目录：{}", run.project_path),
        format!("- 功能或页面：{}", run.menu_name),
        format!("- 测试类型：{}", run.mode),
        format!(
            "- 测试结论：{}",
            match run.status.as_str() {
                "passed" => "通过",
                "failed" => "不通过",
                "blocked" => "环境阻塞",
                "error" => "执行异常",
                "cancelled" => "已取消",
                _ => "执行中",
            }
        ),
        format!("- 开始时间：{}", run.started_at),
        format!(
            "- 结束时间：{}",
            run.finished_at.clone().unwrap_or_default()
        ),
        format!("- 总耗时：{} ms", run.duration_ms),
        format!("- 执行环境：{}", run.environment_summary),
        String::new(),
        "## 汇总".into(),
        String::new(),
        "| 场景总数 | 通过 | 失败 | 跳过 |".into(),
        "| ---: | ---: | ---: | ---: |".into(),
        format!(
            "| {} | {} | {} | {} |",
            run.total_count, run.passed_count, run.failed_count, run.skipped_count
        ),
    ];
    let failures = run
        .scenario_results
        .iter()
        .filter(|item| item.status == "failed" || item.status == "blocked")
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        lines.extend([String::new(), "## 问题场景详情".into(), String::new()]);
        for (index, item) in failures.iter().enumerate() {
            lines.push(format!("### {}. {}", index + 1, item.title));
            lines.push(String::new());
            lines.push(format!(
                "- 结果：{}",
                if item.status == "blocked" {
                    "环境阻塞"
                } else {
                    "失败"
                }
            ));
            lines.push(format!("- 测试目的：{}", item.purpose));
            lines.push(format!("- 耗时：{} ms", item.duration_ms));
            if !item.error_message.is_empty() {
                lines.extend([
                    String::new(),
                    "错误信息：".into(),
                    String::new(),
                    "```text".into(),
                    item.error_message.clone(),
                    "```".into(),
                ]);
            }
            if !item.steps.is_empty() {
                lines.extend([String::new(), "测试步骤：".into()]);
                for (step_index, step) in item.steps.iter().enumerate() {
                    lines.push(format!("{}. {}", step_index + 1, step));
                }
            }
            if !item.checks.is_empty() {
                lines.extend([String::new(), "验证内容：".into()]);
                for check in &item.checks {
                    lines.push(format!("- {check}"));
                }
            }
            if !item.artifacts.is_empty() {
                lines.extend([String::new(), "附件：".into()]);
                for artifact in &item.artifacts {
                    lines.push(format!("- {}: {}", artifact.name, artifact.path));
                }
            }
            lines.push(String::new());
        }
        lines.extend([
            "## 整改建议".into(),
            String::new(),
            "1. 先按失败场景的步骤在相同环境复现。".into(),
            "2. 优先处理最早出现的错误，再检查后续连带失败。".into(),
            "3. 修正后使用相同测试类型和相同场景复测。".into(),
            "4. 复测通过后由人工确认是否关闭整改任务。".into(),
        ]);
    }
    lines.extend([String::new(), "## 全部场景".into(), String::new()]);
    for (index, item) in run.scenario_results.iter().enumerate() {
        lines.push(format!("### {}. {}", index + 1, item.title));
        lines.push(String::new());
        lines.push(format!(
            "- 结果：{}",
            match item.status.as_str() {
                "passed" => "通过",
                "skipped" => "跳过",
                "blocked" => "环境阻塞",
                _ => "失败",
            }
        ));
        lines.push(format!("- 测试目的：{}", item.purpose));
        lines.push(format!("- 耗时：{} ms", item.duration_ms));
        lines.push(String::new());
    }
    lines.join("\n")
}

#[tauri::command]
pub async fn start_test_run(
    state: tauri::State<'_, DatabaseState>,
    process_state: tauri::State<'_, TestProcessState>,
    options: StartTestOptions,
) -> Result<TestRun, String> {
    let database = state.inner().clone();
    let process_state = process_state.inner().clone();
    let (root, project_name, mut menu, preflight) = preflight_for(&database, &options)?;
    let started_at = Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();
    if !preflight.ready {
        let details = preflight
            .checks
            .iter()
            .filter(|item| !item.passed)
            .map(|item| format!("{}：{}", item.name, item.detail))
            .collect::<Vec<_>>();
        let blocked = TestScenarioResult {
            id: "preflight".into(),
            title: "执行前环境检查".into(),
            status: "blocked".into(),
            duration_ms: 0,
            purpose: "确认测试运行所需项目、脚本、场景和凭据已经准备完成。".into(),
            steps: vec![
                "读取项目资产和测试配置。".into(),
                "核对测试运行器、源码和凭据。".into(),
            ],
            checks: preflight
                .checks
                .iter()
                .map(|item| {
                    format!(
                        "{}：{}",
                        item.name,
                        if item.passed { "通过" } else { "未通过" }
                    )
                })
                .collect(),
            error_message: details.join("；"),
            artifacts: Vec::new(),
        };
        let mut run = TestRun {
            id: run_id.clone(),
            menu_id: menu.id,
            project: project_name,
            project_path: display_path(&root),
            menu_name: menu.name,
            mode: options.mode,
            status: "blocked".into(),
            started_at,
            finished_at: Some(Utc::now().to_rfc3339()),
            report_markdown: String::new(),
            source_report_path: None,
            output_excerpt: String::new(),
            error_message: details.join("；"),
            selected_scenarios: options.selected_scenarios,
            scenario_results: vec![blocked],
            artifacts: Vec::new(),
            total_count: 1,
            passed_count: 0,
            failed_count: 0,
            skipped_count: 0,
            duration_ms: 0,
            exit_code: None,
            environment_summary: "执行前检查未通过，测试未启动。".into(),
            cleanup_status: "not-applicable".into(),
        };
        run.report_markdown = build_structured_report(&run);
        save_run(&database, &run)?;
        return Ok(run);
    }
    if options.create_case_file == Some(true) && !menu.has_case_file {
        let path = create_case_file(&root, &menu, &options.selected_scenarios)?;
        menu.case_file_path = Some(path.display().to_string());
        menu.case_id = Some(safe_case_id(&menu));
        menu.has_case_file = true;
    }
    let running = TestRun {
        id: run_id.clone(),
        menu_id: menu.id.clone(),
        project: project_name.clone(),
        project_path: display_path(&root),
        menu_name: menu.name.clone(),
        mode: options.mode.clone(),
        status: "running".into(),
        started_at: started_at.clone(),
        finished_at: None,
        report_markdown: String::new(),
        source_report_path: None,
        output_excerpt: "执行前检查已通过，正在运行所选场景。".into(),
        error_message: String::new(),
        selected_scenarios: options.selected_scenarios.clone(),
        scenario_results: Vec::new(),
        artifacts: Vec::new(),
        total_count: options.selected_scenarios.len() as i64,
        passed_count: 0,
        failed_count: 0,
        skipped_count: 0,
        duration_ms: 0,
        exit_code: None,
        environment_summary: "执行前检查通过；测试进程正在运行。".into(),
        cleanup_status: "pending".into(),
    };
    save_run(&database, &running)?;
    process_state.register(&run_id);
    let returned_run = running.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let execution = match execute_project(&root, &menu, &options, &run_id, &process_state) {
            Ok(result) => result,
            Err(error) => ExecutionResult {
                status: "error".into(),
                output_excerpt: String::new(),
                error_message: error.clone(),
                scenario_results: vec![TestScenarioResult {
                    id: "executor".into(),
                    title: "启动测试执行器".into(),
                    status: "failed".into(),
                    duration_ms: 0,
                    purpose: "启动项目已有测试工具。".into(),
                    steps: vec!["读取测试命令。".into(), "启动子进程。".into()],
                    checks: vec!["测试工具可以正常启动。".into()],
                    error_message: error,
                    artifacts: Vec::new(),
                }],
                exit_code: None,
                report_path: None,
                environment_summary: "测试执行器启动失败。".into(),
                cleanup_status: "not-applicable".into(),
            },
        };
        let artifacts = execution
            .scenario_results
            .iter()
            .flat_map(|item| item.artifacts.clone())
            .collect::<Vec<_>>();
        let total = execution.scenario_results.len() as i64;
        let passed = execution
            .scenario_results
            .iter()
            .filter(|item| item.status == "passed")
            .count() as i64;
        let failed = execution
            .scenario_results
            .iter()
            .filter(|item| item.status == "failed")
            .count() as i64;
        let skipped = execution
            .scenario_results
            .iter()
            .filter(|item| item.status == "skipped")
            .count() as i64;
        let mut run = TestRun {
            id: run_id.clone(),
            menu_id: menu.id,
            project: project_name,
            project_path: display_path(&root),
            menu_name: menu.name,
            mode: options.mode,
            status: execution.status,
            started_at,
            finished_at: Some(Utc::now().to_rfc3339()),
            report_markdown: String::new(),
            source_report_path: execution.report_path,
            output_excerpt: execution.output_excerpt,
            error_message: execution.error_message,
            selected_scenarios: options.selected_scenarios,
            scenario_results: execution.scenario_results,
            artifacts,
            total_count: total,
            passed_count: passed,
            failed_count: failed,
            skipped_count: skipped,
            duration_ms: started.elapsed().as_millis() as i64,
            exit_code: execution.exit_code,
            environment_summary: execution.environment_summary,
            cleanup_status: execution.cleanup_status,
        };
        run.report_markdown = build_structured_report(&run);
        let _ = save_run(&database, &run);
        let _ = crate::suggestions::sync_task_suggestions_for_state(&database);
        process_state.finish(&run_id);
    });
    Ok(returned_run)
}

fn run_by_id(state: &DatabaseState, id: &str) -> Result<TestRun, String> {
    state
        .connect()?
        .query_row(
            &format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE id=?1"),
            [id],
            row_to_run,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "没有找到对应测试记录。".to_string())
}

#[tauri::command]
pub fn get_test_run(
    state: tauri::State<'_, DatabaseState>,
    run_id: String,
) -> Result<TestRun, String> {
    run_by_id(&state, &run_id)
}

#[tauri::command]
pub fn cancel_test_run(
    state: tauri::State<'_, DatabaseState>,
    process_state: tauri::State<'_, TestProcessState>,
    run_id: String,
) -> Result<TestRun, String> {
    let run = run_by_id(&state, &run_id)?;
    if !matches!(run.status.as_str(), "queued" | "running") {
        return Ok(run);
    }
    let pid = process_state
        .request_cancel(&run_id)
        .ok_or_else(|| "测试进程已经结束，请刷新结果。".to_string())?;
    if let Some(pid) = pid {
        #[cfg(target_os = "windows")]
        {
            let _ = codex_video::hidden_command(Path::new("taskkill"))
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = codex_video::hidden_command(Path::new("kill"))
                .args(["-TERM", &pid.to_string()])
                .output();
        }
    }
    Ok(run)
}

fn image_mime(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg") | Some("jpeg") => Some("image/jpeg"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}

fn artifact_data_uri(run: &TestRun, requested: &str) -> Result<String, String> {
    if !run
        .artifacts
        .iter()
        .any(|item| item.path == requested && item.kind == "screenshot")
    {
        return Err("该文件不是本次测试记录中的页面截图。".into());
    }
    let root = PathBuf::from(&run.project_path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path = PathBuf::from(requested)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.starts_with(&root) {
        return Err("只能读取目标项目目录中的测试截图。".into());
    }
    let mime = image_mime(&path).ok_or_else(|| "仅支持 PNG、JPG 和 WebP 测试截图。".to_string())?;
    let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
    if metadata.len() > 20 * 1024 * 1024 {
        return Err("测试截图超过 20MB，未在工作台中加载。".into());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

#[tauri::command]
pub fn read_test_artifact(
    state: tauri::State<'_, DatabaseState>,
    run_id: String,
    path: String,
) -> Result<String, String> {
    let run = run_by_id(&state, &run_id)?;
    artifact_data_uri(&run, &path)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn pdf_html(run: &TestRun) -> String {
    let mut scenarios = String::new();
    let mut ordered = run.scenario_results.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|item| {
        if item.status == "failed" || item.status == "blocked" {
            0
        } else {
            1
        }
    });
    for (index, item) in ordered.into_iter().enumerate() {
        let status_label = match item.status.as_str() {
            "passed" => "通过",
            "skipped" => "跳过",
            "blocked" => "环境阻塞",
            _ => "失败",
        };
        let screenshots = item
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == "screenshot")
            .filter_map(|artifact| {
                artifact_data_uri(run, &artifact.path).ok().map(|uri| {
                    format!(
                        "<figure><img src=\"{}\"><figcaption>{}</figcaption></figure>",
                        uri,
                        html_escape(&artifact.name)
                    )
                })
            })
            .collect::<String>();
        let steps = item
            .steps
            .iter()
            .map(|value| format!("<li>{}</li>", html_escape(value)))
            .collect::<String>();
        let checks = item
            .checks
            .iter()
            .map(|value| format!("<li>{}</li>", html_escape(value)))
            .collect::<String>();
        scenarios.push_str(&format!("<section class=\"scenario {}\"><header><span>{:02}</span><h2>{}</h2><b>{}</b></header><p class=\"purpose\">{}</p>{}<div class=\"columns\"><div><h3>测试步骤</h3><ol>{}</ol></div><div><h3>验证内容</h3><ul>{}</ul></div></div>{}</section>",item.status,index+1,html_escape(&item.title),status_label,html_escape(&item.purpose),if item.error_message.is_empty(){String::new()}else{format!("<pre>{}</pre>",html_escape(&item.error_message))},steps,checks,screenshots));
    }
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><style>
@page{{size:A4;margin:16mm 14mm}}*{{box-sizing:border-box}}body{{font-family:"Microsoft YaHei","Segoe UI",sans-serif;color:#202535;font-size:10.5pt;line-height:1.65;margin:0}}.cover{{padding:18px 20px;border:1px solid #dce1eb;border-radius:12px;background:#f7f8fc;margin-bottom:15px}}.eyebrow{{color:#6757d9;letter-spacing:2px;font-size:8pt}}h1{{margin:6px 0 3px;font-size:22pt}}.meta{{color:#697084}}.summary{{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin:14px 0}}.summary div{{padding:10px;border:1px solid #dce1eb;border-radius:8px;text-align:center}}.summary b{{display:block;font-size:17pt}}.scenario{{break-inside:avoid;margin:0 0 12px;border:1px solid #dce1eb;border-radius:10px;overflow:hidden}}.scenario.failed,.scenario.blocked{{border-color:#ef9298}}.scenario header{{display:flex;align-items:center;gap:9px;padding:10px 12px;background:#f6f7fb}}.scenario header span{{width:26px;height:26px;border-radius:7px;background:#ece9ff;color:#6757d9;text-align:center;line-height:26px}}.scenario h2{{font-size:12.5pt;margin:0;flex:1}}.scenario header b{{color:#4ba778}}.scenario.failed header b,.scenario.blocked header b{{color:#d94b56}}.purpose{{margin:10px 12px;color:#596174}}.columns{{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:0 12px 12px}}h3{{font-size:10pt;margin:3px 0}}ol,ul{{margin:4px 0;padding-left:20px}}pre{{margin:0 12px 10px;padding:9px;border-radius:7px;background:#fff0f1;color:#8c2530;white-space:pre-wrap;word-break:break-word}}figure{{margin:8px 12px 12px;break-inside:avoid}}img{{display:block;max-width:100%;max-height:220mm;border:1px solid #dce1eb;border-radius:7px}}figcaption{{color:#697084;font-size:8pt;margin-top:4px}}footer{{margin-top:15px;border-top:1px solid #dce1eb;padding-top:8px;color:#697084;font-size:8pt}}</style></head><body>
<section class="cover"><div class="eyebrow">TEST REPORT</div><h1>{}</h1><div class="meta">{} · {} · {}</div><div class="summary"><div><span>结果</span><b>{}</b></div><div><span>场景</span><b>{}</b></div><div><span>通过</span><b>{}</b></div><div><span>失败</span><b>{}</b></div></div><div class="meta">环境：{}<br>项目目录：{}</div></section>{}<footer>由星枢 ASTRION 导出 · {}</footer></body></html>"#,
        html_escape(&run.menu_name),
        html_escape(&run.project),
        html_escape(&run.mode),
        html_escape(&run.started_at),
        if run.status == "passed" {
            "通过"
        } else if run.status == "blocked" {
            "环境阻塞"
        } else {
            "未通过"
        },
        run.total_count,
        run.passed_count,
        run.failed_count,
        html_escape(&run.environment_summary),
        html_escape(&run.project_path),
        scenarios,
        html_escape(&Utc::now().to_rfc3339())
    )
}

fn safe_pdf_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|ch| {
            if matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                ch
            }
        })
        .collect::<String>();
    name.trim().chars().take(70).collect()
}

fn edge_path() -> Option<PathBuf> {
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
    ];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Google")
                .join("Chrome")
                .join("Application")
                .join("chrome.exe"),
        );
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn complete_pdf_size(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    if size < 12 {
        return None;
    }
    let mut header = [0_u8; 5];
    file.read_exact(&mut header).ok()?;
    if &header != b"%PDF-" {
        return None;
    }
    let tail_size = size.min(2048) as usize;
    file.seek(SeekFrom::End(-(tail_size as i64))).ok()?;
    let mut tail = vec![0_u8; tail_size];
    file.read_exact(&mut tail).ok()?;
    tail.windows(5)
        .any(|window| window == b"%%EOF")
        .then_some(size)
}

fn wait_for_complete_pdf(path: &Path, timeout: std::time::Duration) -> Result<u64, String> {
    let started = std::time::Instant::now();
    let mut previous_size = None;
    loop {
        if let Some(size) = complete_pdf_size(path) {
            if previous_size == Some(size) {
                return Ok(size);
            }
            previous_size = Some(size);
        } else {
            previous_size = None;
        }
        if started.elapsed() >= timeout {
            return Err("浏览器已结束，但 PDF 文件没有完整落盘。".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

#[tauri::command]
pub async fn export_test_report_pdf(
    app: tauri::AppHandle,
    state: tauri::State<'_, DatabaseState>,
    run_id: String,
) -> Result<String, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let run = run_by_id(&database, &run_id)?;
        let output_dir = app
            .path()
            .document_dir()
            .map_err(|error| error.to_string())?
            .join("AI个人工作台")
            .join("测试报告");
        fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
        let output_path = exported_pdf_path(&output_dir, &run);
        let attempt_id = Uuid::new_v4();
        let staging_path = output_dir.join(format!(
            ".{}-{attempt_id}.part.pdf",
            exported_report_stem(&run)
        ));
        let html_path = std::env::temp_dir().join(format!(
            "workbench-test-report-{}-{attempt_id}.html",
            run.id
        ));
        let profile_path = std::env::temp_dir().join(format!(
            "workbench-test-pdf-profile-{}-{attempt_id}",
            run.id
        ));
        fs::write(&html_path, pdf_html(&run)).map_err(|error| error.to_string())?;
        let browser = edge_path().ok_or_else(|| {
            "没有找到 Microsoft Edge 或 Google Chrome，无法导出 PDF。".to_string()
        })?;
        let mut command = codex_video::hidden_command(&browser);
        let output_arg = format!("--print-to-pdf={}", staging_path.display());
        let profile_arg = format!("--user-data-dir={}", profile_path.display());
        let result = command
            .args([
                "--headless",
                "--disable-gpu",
                "--allow-file-access-from-files",
                "--no-pdf-header-footer",
            ])
            .arg(profile_arg)
            .arg(output_arg)
            .arg(&html_path)
            .output()
            .map_err(|error| format!("无法启动浏览器导出 PDF：{error}"))?;
        let pdf_result = wait_for_complete_pdf(&staging_path, std::time::Duration::from_secs(15));
        let _ = fs::remove_file(&html_path);
        let _ = fs::remove_dir_all(&profile_path);
        if let Err(error) = pdf_result {
            let _ = fs::remove_file(&staging_path);
            return Err(format!(
                "PDF 导出失败：{error} {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        if output_path.is_file() {
            fs::remove_file(&output_path)
                .map_err(|error| format!("无法替换旧 PDF 报告：{error}"))?;
        }
        fs::rename(&staging_path, &output_path)
            .map_err(|error| format!("无法保存 PDF 报告：{error}"))?;
        Ok(output_path.display().to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn exported_report_stem(run: &TestRun) -> String {
    format!(
        "{}-{}-{}-{}",
        safe_pdf_name(&run.project),
        safe_pdf_name(&run.menu_name),
        safe_pdf_name(&run.started_at.chars().take(19).collect::<String>()).replace('T', "-"),
        run.id.chars().take(8).collect::<String>()
    )
}

fn exported_pdf_path(output_dir: &Path, run: &TestRun) -> PathBuf {
    output_dir.join(format!("{}.pdf", exported_report_stem(run)))
}

fn exported_markdown_path(output_dir: &Path, run: &TestRun) -> PathBuf {
    output_dir.join(format!("{}.md", exported_report_stem(run)))
}

fn write_exported_markdown(output_dir: &Path, run: &TestRun) -> Result<PathBuf, String> {
    fs::create_dir_all(output_dir).map_err(|error| error.to_string())?;
    let output_path = exported_markdown_path(output_dir, run);
    let content = if run.report_markdown.trim().is_empty() {
        build_structured_report(run)
    } else {
        run.report_markdown.clone()
    };
    fs::write(&output_path, format!("{}\n", content.trim_end()))
        .map_err(|error| format!("MD 导出失败：{error}"))?;
    Ok(output_path)
}

#[tauri::command]
pub fn export_test_report_markdown(
    app: tauri::AppHandle,
    state: tauri::State<'_, DatabaseState>,
    run_id: String,
) -> Result<String, String> {
    let run = run_by_id(state.inner(), &run_id)?;
    let output_dir = app
        .path()
        .document_dir()
        .map_err(|error| error.to_string())?
        .join("AI个人工作台")
        .join("测试报告");
    write_exported_markdown(&output_dir, &run).map(|path| path.display().to_string())
}

fn existing_exported_pdf(output_dir: &Path, run: &TestRun) -> Option<PathBuf> {
    let path = exported_pdf_path(output_dir, run);
    complete_pdf_size(&path).map(|_| path)
}

#[tauri::command]
pub fn get_existing_test_report_pdf(
    app: tauri::AppHandle,
    state: tauri::State<'_, DatabaseState>,
    run_id: String,
) -> Result<Option<String>, String> {
    let run = run_by_id(state.inner(), &run_id)?;
    let output_dir = app
        .path()
        .document_dir()
        .map_err(|error| error.to_string())?
        .join("AI个人工作台")
        .join("测试报告");
    Ok(existing_exported_pdf(&output_dir, &run).map(|path| path.display().to_string()))
}

fn canonical_exported_pdf(output_dir: &Path, requested: &str) -> Result<PathBuf, String> {
    let allowed = output_dir
        .canonicalize()
        .map_err(|error| format!("测试报告目录无法读取：{error}"))?;
    let path = PathBuf::from(requested)
        .canonicalize()
        .map_err(|error| format!("PDF 文件不存在或无法读取：{error}"))?;
    if !path.starts_with(&allowed)
        || path.extension().and_then(|value| value.to_str()) != Some("pdf")
        || complete_pdf_size(&path).is_none()
    {
        return Err("只能打开测试中心刚导出的 PDF 报告。".into());
    }
    Ok(path)
}

#[tauri::command]
pub fn open_test_report_pdf(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let output_dir = app
        .path()
        .document_dir()
        .map_err(|error| error.to_string())?
        .join("AI个人工作台")
        .join("测试报告");
    let path = canonical_exported_pdf(&output_dir, &path)?;
    let browser = edge_path()
        .ok_or_else(|| "没有找到 Microsoft Edge 或 Google Chrome，无法打开 PDF。".to_string())?;
    let file_url = tauri::Url::from_file_path(&path)
        .map_err(|_| "无法把 PDF 路径转换为本地文件地址。".to_string())?;
    codex_video::hidden_command(&browser)
        .arg("--new-window")
        .arg(file_url.as_str())
        .spawn()
        .map_err(|error| format!("无法打开 PDF：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        app_menus, app_static_report, append_remediation, canonical_exported_pdf, client_menus,
        client_report_status, complete_pdf_size, create_case_file, display_path,
        dynamic_pages_from_response, existing_exported_pdf, filter_test_titles,
        legacy_report_scenarios, pdf_html, persist_screenshot_artifacts, project_menus,
        recover_incomplete_test_runs, run_by_id, save_run, scenario_titles_for_menu_and_suite,
        source_business_scenarios, static_scenarios, strip_json_comments,
        visible_local_router_pages, write_exported_markdown, TestArtifact, TestCapabilities,
        TestMenu, TestProcessState, TestRun, TestScenarioResult, COMMON_REAL_SUITE_ID,
    };
    use crate::database::DatabaseState;
    use std::path::Path;
    use uuid::Uuid;

    fn sample_menu(root: &Path) -> TestMenu {
        TestMenu {
            id: "project:example-page".into(),
            project: "fixture".into(),
            project_path: root.display().to_string(),
            project_kind: "vue".into(),
            name: "示例页面".into(),
            route: "/example".into(),
            source_path: "src/views/example/index.vue".into(),
            case_id: None,
            has_case_file: false,
            case_file_path: None,
            can_create_case_file: true,
            capabilities: TestCapabilities {
                mock: true,
                real_api: true,
                source_style: true,
                browser_style: true,
            },
            tested: false,
            latest_status: None,
            latest_time: None,
            latest_report_path: None,
        }
    }

    fn business_page_source() -> &'static str {
        r#"<template>
  <el-form>
    <el-form-item label="岗位编码"><el-input /></el-form-item>
    <el-form-item label="岗位名称"><el-input /></el-form-item>
    <el-form-item label="状态"><el-select /></el-form-item>
    <el-form-item><el-button>搜索</el-button><el-button>重置</el-button></el-form-item>
  </el-form>
  <el-button>新增</el-button><el-button>修改</el-button><el-button>删除</el-button>
  <el-table>
    <el-table-column label="岗位编码" />
    <el-table-column label="岗位名称" />
    <el-table-column label="状态" />
    <el-table-column label="操作" />
  </el-table>
  <el-dialog><el-form><el-form-item label="岗位编码" /><el-form-item label="岗位名称" /></el-form></el-dialog>
</template>
<style scoped></style>"#
    }
    #[test]
    fn comments_are_removed_without_touching_urls() {
        let value = strip_json_comments("{\"url\":\"http://localhost\",// note\n\"ok\":true}");
        assert!(value.contains("http://localhost"));
        assert!(!value.contains("note"));
    }

    #[test]
    fn vue_catalog_only_uses_visible_router_pages_with_menu_titles() {
        let root = std::env::temp_dir().join(format!("workbench-routes-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src/router")).unwrap();
        for component in ["system/post/index", "system/secret/index", "orphan/index"] {
            let path = root.join("src/views").join(format!("{component}.vue"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "<template><div /></template>").unwrap();
        }
        std::fs::write(
            root.join("src/router/index.js"),
            r#"export const routes = [{
  path: '/system',
  component: Layout,
  children: [{
    path: 'post',
    component: () => import('@/views/system/post/index'),
    meta: { title: '岗位管理' }
  }, {
    path: 'secret',
    hidden: true,
    component: () => import('@/views/system/secret/index'),
    meta: { title: '隐藏详情' }
  }]
}]"#,
        )
        .unwrap();

        let pages = visible_local_router_pages(&root);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].name, "岗位管理");
        assert_eq!(pages[0].route, "/system/post");
        assert_eq!(pages[0].source_path, "src/views/system/post/index.vue");
        assert!(pages.iter().all(|page| !page.component.contains("orphan")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn dynamic_menu_catalog_excludes_hidden_and_unregistered_source_pages() {
        let root = std::env::temp_dir().join(format!("workbench-menu-api-{}", Uuid::new_v4()));
        for component in [
            "system/post/index",
            "system/dict/index",
            "safe/safetyManagement/caseShare/index",
        ] {
            let source = root.join("src/views").join(format!("{component}.vue"));
            std::fs::create_dir_all(source.parent().unwrap()).unwrap();
            std::fs::write(source, "<template><div /></template>").unwrap();
        }
        let response = serde_json::json!({
            "code": 200,
            "data": [
                {
                    "path": "/system",
                    "component": "Layout",
                    "meta": { "title": "系统管理" },
                    "children": [
                        { "path": "post", "component": "system/post/index", "meta": { "title": "岗位管理" } },
                        { "path": "dict", "component": "system/dict/index", "meta": { "title": "字典管理" } },
                        { "path": "secret", "component": "system/secret/index", "hidden": true, "meta": { "title": "隐藏详情" } },
                        { "path": "missing", "component": "system/missing/index", "meta": { "title": "缺失页面" } }
                    ]
                },
                {
                    "path": "/safetyManagement",
                    "component": "Layout",
                    "children": [
                        { "path": "caseShare", "component": "safe/safetyManagement/caseShare/index", "meta": { "title": "案例分享" } }
                    ]
                }
            ]
        });

        let pages = dynamic_pages_from_response(&root, &response);
        assert_eq!(pages.len(), 3);
        assert!(pages
            .iter()
            .any(|page| page.name == "岗位管理" && page.route == "/system/post"));
        assert!(pages
            .iter()
            .any(|page| page.name == "字典管理" && page.route == "/system/dict"));
        assert!(pages.iter().any(|page| {
            page.name == "案例分享" && page.route == "/safetyManagement/caseShare"
        }));
        assert!(pages
            .iter()
            .all(|page| page.name != "隐藏详情" && page.name != "缺失页面"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn case_files_only_attach_to_registered_pages() {
        let root = std::env::temp_dir().join(format!("workbench-page-cases-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("e2e/menu-cases")).unwrap();
        std::fs::write(
            root.join("pages.json"),
            r#"{"pages":[{"path":"pages/post/index","style":{"navigationBarTitleText":"岗位管理"}}]}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("e2e/menu-cases/post.json"),
            r#"{"id":"post","menuName":"岗位管理","route":"/pages/post/index","component":"pages/post/index"}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("e2e/menu-cases/orphan.json"),
            r#"{"id":"orphan","menuName":"孤立页面","route":"/pages/orphan/index","component":"pages/orphan/index"}"#,
        )
        .unwrap();
        let database_path = root.join("testing.sqlite3");
        let state = DatabaseState::new(database_path).unwrap();

        let menus = project_menus(&state, &root, "fixture").unwrap();
        assert_eq!(menus.len(), 1);
        assert_eq!(menus[0].name, "岗位管理");
        assert!(menus[0].has_case_file);
        assert_eq!(menus[0].case_id.as_deref(), Some("post"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn passed_report_is_not_failed_by_summary_column_name() {
        let report = "- 测试结论：通过\n\n| 用例总数 | 通过 | 失败 | 跳过 |";
        assert_eq!(client_report_status(report), "passed");
        assert_eq!(client_report_status("- 测试结论：不通过"), "failed");
    }

    #[test]
    fn failed_report_gets_actionable_advice_without_modifying_project() {
        let report = append_remediation(
            "# 报告\n\n| 失败 | 点击搜索没有请求 |".into(),
            "案例分享",
            "client",
            false,
        );
        assert!(report.contains("## 整改建议"));
        assert!(report.contains("handleQuery"));
        assert!(report.contains("不会自动修改"));
        assert!(
            !append_remediation("# 通过".into(), "案例分享", "client", true).contains("整改建议")
        );
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_windows_path_is_converted_for_node_and_saved_reports() {
        assert_eq!(
            display_path(Path::new(r"\\?\F:\TB-project\client")),
            r"F:\TB-project\client"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\client")),
            r"\\server\share\client"
        );
    }

    #[test]
    fn legacy_markdown_report_recovers_scenarios_errors_and_screenshots() {
        let report = r#"# 旧报告

## 场景明细

### 1. 点击搜索

- 结果：失败
- 测试目的：确认搜索可用。
- 耗时：1.5s

测试步骤：
1. 输入关键词。
2. 点击搜索。

验证内容：
- 列表刷新。

## 失败详情

### 1. 点击搜索

```text
等待列表刷新超时
```

附件：
- screenshot: F:\project\test-results\failed.png
"#;
        let scenarios = legacy_report_scenarios(report);
        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].status, "failed");
        assert_eq!(scenarios[0].duration_ms, 1500);
        assert_eq!(scenarios[0].steps.len(), 2);
        assert!(scenarios[0].error_message.contains("等待列表刷新超时"));
        assert_eq!(scenarios[0].artifacts[0].kind, "screenshot");
    }

    #[test]
    fn screenshots_are_copied_to_a_run_specific_report_directory() {
        let root = std::env::temp_dir().join(format!("workbench-artifact-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("test-results")).unwrap();
        let source = root.join("test-results/failure.png");
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/Square310x310Logo.png"),
            &source,
        )
        .unwrap();
        let mut scenarios = vec![TestScenarioResult {
            id: "failed".into(),
            title: "失败场景".into(),
            status: "failed".into(),
            duration_ms: 1,
            purpose: "确认页面。".into(),
            steps: vec!["打开页面。".into()],
            checks: vec!["页面正常。".into()],
            error_message: "失败".into(),
            artifacts: vec![TestArtifact {
                name: "失败截图".into(),
                path: source.display().to_string(),
                content_type: "image/png".into(),
                kind: "screenshot".into(),
            }],
        }];
        persist_screenshot_artifacts(&root, "run-1", &mut scenarios);
        let copied = Path::new(&scenarios[0].artifacts[0].path);
        assert!(copied.is_file());
        assert!(copied.to_string_lossy().contains("reports"));
        assert!(copied.to_string_lossy().contains("run-1"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn real_project_catalogs_can_be_loaded() {
        let path =
            std::env::temp_dir().join(format!("workbench-testing-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let client = client_menus(&state).unwrap();
        let app = app_menus(&state).unwrap();
        let visible_client_routes = visible_local_router_pages(&super::client_root());
        assert!(!client.is_empty());
        assert!(!app.is_empty());
        assert!(visible_client_routes
            .iter()
            .any(|page| page.name == "工作台"));
        assert!(visible_client_routes
            .iter()
            .any(|page| page.name == "我的任务"));
        assert!(visible_client_routes
            .iter()
            .all(|page| page.name != "ConfigExam" && !page.source_path.contains("ConfigExam")));
        assert!(visible_client_routes.len() < 20);
        let post = client
            .iter()
            .find(|menu| menu.case_id.as_deref() == Some("post"))
            .expect("client 项目应包含岗位管理测试配置");
        let real_scenarios = scenario_titles_for_menu_and_suite(
            Path::new(&post.project_path),
            post,
            "real",
            Some(COMMON_REAL_SUITE_ID),
        );
        assert!(real_scenarios.len() >= 6);
        assert!(real_scenarios
            .iter()
            .all(|title| !title.contains("台账") && !title.contains("下发")));
        assert!(!real_scenarios
            .iter()
            .any(|title| title.contains("三个业务 Tab") || title.contains("签发弹窗")));
        let mock_scenarios =
            scenario_titles_for_menu_and_suite(Path::new(&post.project_path), post, "mock", None);
        assert!(mock_scenarios
            .iter()
            .any(|title| title.contains("岗位编码")));
        assert!(mock_scenarios.iter().all(|title| !title.contains("责任书")));
        let generated_scenarios = scenario_titles_for_menu_and_suite(
            Path::new(&post.project_path),
            post,
            "source-style",
            None,
        );
        assert!(generated_scenarios
            .iter()
            .any(|title| title.contains("岗位管理查询条件：岗位编码、岗位名称、状态")));
        assert!(generated_scenarios
            .iter()
            .any(|title| title.contains("岗位管理业务操作：搜索、重置、新增、修改、删除、导出")));
        assert!(generated_scenarios
            .iter()
            .all(|title| !title.contains("责任书")));
        assert!(app.iter().all(|menu| menu.source_path.ends_with(".vue")));
        let (passed, report) = app_static_report(&app[0]);
        assert!(passed, "{report}");
        assert!(report.contains("验证边界"));
        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn common_real_template_supports_all_dev_proxies_and_native_fetch_status() {
        assert!(super::COMMON_REAL_SPEC.contains("pathname.startsWith('/dev-')"));
        assert!(super::COMMON_REAL_SPEC.contains("expect(response.status,"));
        assert!(!super::COMMON_REAL_SPEC.contains("expect(response.status(),"));
    }

    #[test]
    fn generated_case_file_keeps_selected_scenarios_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!("workbench-case-{}", Uuid::new_v4()));
        let source = root.join("src/views/example/index.vue");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, business_page_source()).unwrap();
        let menu = sample_menu(&root);
        let selected = vec!["场景一".to_string(), "场景二".to_string()];
        let path = create_case_file(&root, &menu, &selected).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        assert!(original.contains("场景一"));
        assert!(original.contains("workbenchMenuId"));
        let value: serde_json::Value = serde_json::from_str(&original).unwrap();
        assert!(value["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|title| title.contains("示例页面查询条件：岗位编码、岗位名称、状态")));
        assert_eq!(
            value["businessContext"]["queryFields"],
            serde_json::json!(["岗位编码", "岗位名称", "状态"])
        );
        assert!(value["businessContext"]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|action| action == "新增"));
        assert_eq!(
            value["selectedScenariosAtCreation"],
            serde_json::json!(["场景一", "场景二"])
        );
        assert!(create_case_file(&root, &menu, &selected).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn post_scenarios_do_not_reuse_safe_responsibility_business_steps() {
        let root = std::env::temp_dir().join(format!("workbench-post-case-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let case_path = root.join("post.json");
        std::fs::write(
            &case_path,
            serde_json::to_string(&serde_json::json!({
                "id": "post",
                "component": "system/post/index",
                "permissions": ["system:post:list", "system:post:add", "system:post:edit"],
                "mockRows": [{ "postCode": "E2E", "postName": "测试岗位" }]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut menu = sample_menu(&root);
        menu.case_id = Some("post".into());
        menu.case_file_path = Some(case_path.display().to_string());

        let real_titles = vec![
            "真实登录态可以进入页面并加载后端列表".into(),
            "新增空表单只做前端校验，不写入真实后端".into(),
            "三个业务 Tab 切换后对应列表接口和页面结构正常".into(),
            "台账表格行内预览、修改、提交、签发弹窗、撤销、删除操作正常".into(),
            "桌面视口下页面主体不应横向溢出".into(),
            "真实新增、修改、删除流程可以完整执行".into(),
        ];
        let filtered_real = filter_test_titles(
            &menu,
            "real",
            Path::new("real-menu-module.spec.js"),
            real_titles,
        );
        assert_eq!(
            filtered_real,
            vec![
                "真实登录态可以进入页面并加载后端列表",
                "新增空表单只做前端校验，不写入真实后端",
                "桌面视口下页面主体不应横向溢出"
            ]
        );

        let mock_titles = vec![
            "页面基础区域正常显示".into(),
            "列表展示字段完整，分页和操作列可见".into(),
            "台账列表展示责任书核心字段和操作列".into(),
            "责任书统计标签页可以切换并展示统计表格".into(),
        ];
        let filtered_mock =
            filter_test_titles(&menu, "mock", Path::new("menu-module.spec.js"), mock_titles);
        assert_eq!(
            filtered_mock,
            vec!["页面基础区域正常显示", "列表展示字段完整，分页和操作列可见"]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn safe_responsibility_keeps_its_business_specific_scenarios() {
        let root = std::env::temp_dir().join(format!("workbench-safe-case-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let case_path = root.join("safe-responsibility.json");
        std::fs::write(
            &case_path,
            serde_json::to_string(&serde_json::json!({
                "id": "safe-responsibility",
                "component": "safe/safetyManagement/safeResponsibility/index",
                "permissions": ["safe:safeResponsibility:list", "safe:safeResponsibility:issue"]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut menu = sample_menu(&root);
        menu.case_id = Some("safe-responsibility".into());
        menu.case_file_path = Some(case_path.display().to_string());

        let mock_titles = vec![
            "页面基础区域正常显示".into(),
            "列表展示字段完整，分页和操作列可见".into(),
            "台账列表展示责任书核心字段和操作列".into(),
            "责任书统计标签页可以切换并展示统计表格".into(),
        ];
        let filtered =
            filter_test_titles(&menu, "mock", Path::new("menu-module.spec.js"), mock_titles);
        assert_eq!(
            filtered,
            vec![
                "页面基础区域正常显示",
                "台账列表展示责任书核心字段和操作列",
                "责任书统计标签页可以切换并展示统计表格"
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn page_without_case_file_can_run_source_checks() {
        let root = std::env::temp_dir().join(format!("workbench-static-{}", Uuid::new_v4()));
        let source = root.join("src/views/example/index.vue");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "<template><main>示例</main></template><script setup></script><style scoped></style>",
        )
        .unwrap();
        let menu = sample_menu(&root);
        let selected = source_business_scenarios(&root, &menu)
            .into_iter()
            .map(|scenario| scenario.title)
            .collect::<Vec<_>>();
        let scenarios = static_scenarios(&root, &menu, &selected);
        assert_eq!(scenarios.len(), 2);
        assert!(scenarios.iter().all(|item| item.status == "passed"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pdf_report_places_failure_first_and_embeds_recorded_screenshot() {
        let root = std::env::temp_dir().join(format!("workbench-pdf-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let screenshot = root.join("failure.png");
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/Square310x310Logo.png"),
            &screenshot,
        )
        .unwrap();
        let artifact = TestArtifact {
            name: "失败截图".into(),
            path: screenshot.display().to_string(),
            content_type: "image/png".into(),
            kind: "screenshot".into(),
        };
        let scenario =
            |title: &str, status: &str, artifacts: Vec<TestArtifact>| TestScenarioResult {
                id: title.into(),
                title: title.into(),
                status: status.into(),
                duration_ms: 10,
                purpose: "确认页面行为。".into(),
                steps: vec!["打开页面。".into()],
                checks: vec!["页面结果符合预期。".into()],
                error_message: if status == "failed" {
                    "按钮没有响应".into()
                } else {
                    String::new()
                },
                artifacts,
            };
        let run = TestRun {
            id: "pdf-run".into(),
            menu_id: "project:example-page".into(),
            project: "fixture".into(),
            project_path: root.display().to_string(),
            menu_name: "示例页面".into(),
            mode: "browser-style".into(),
            status: "failed".into(),
            started_at: "2026-08-24T10:00:00+08:00".into(),
            finished_at: Some("2026-08-24T10:00:01+08:00".into()),
            report_markdown: String::new(),
            source_report_path: None,
            output_excerpt: String::new(),
            error_message: "按钮没有响应".into(),
            selected_scenarios: vec!["通过场景".into(), "失败场景".into()],
            scenario_results: vec![
                scenario("通过场景", "passed", Vec::new()),
                scenario("失败场景", "failed", vec![artifact.clone()]),
            ],
            artifacts: vec![artifact],
            total_count: 2,
            passed_count: 1,
            failed_count: 1,
            skipped_count: 0,
            duration_ms: 20,
            exit_code: Some(1),
            environment_summary: "测试环境".into(),
            cleanup_status: "not-applicable".into(),
        };
        let html = pdf_html(&run);
        assert!(html.find("失败场景").unwrap() < html.find("通过场景").unwrap());
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("按钮没有响应"));
        let reports = root.join("reports");
        let markdown = write_exported_markdown(&reports, &run).unwrap();
        let markdown_content = std::fs::read_to_string(&markdown).unwrap();
        assert_eq!(
            markdown.extension().and_then(|value| value.to_str()),
            Some("md")
        );
        assert!(markdown_content.contains("测试执行报告"));
        assert!(markdown_content.contains("按钮没有响应"));
        assert!(markdown_content.contains("failure.png"));
        assert!(existing_exported_pdf(&reports, &run).is_none());
        let pdf = super::exported_pdf_path(&reports, &run);
        std::fs::create_dir_all(&reports).unwrap();
        std::fs::write(&pdf, b"%PDF-incomplete-report").unwrap();
        assert!(complete_pdf_size(&pdf).is_none());
        assert!(existing_exported_pdf(&reports, &run).is_none());
        std::fs::write(&pdf, b"%PDF-1.4\n%%EOF\n").unwrap();
        assert_eq!(complete_pdf_size(&pdf), Some(15));
        assert_eq!(existing_exported_pdf(&reports, &run), Some(pdf));
        if let Some(output) = std::env::var_os("WORKBENCH_PDF_QA_HTML") {
            let output = std::path::PathBuf::from(output);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(output, &html).unwrap();
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn exported_pdf_opener_only_accepts_pdf_inside_report_directory() {
        let root = std::env::temp_dir().join(format!("workbench-open-pdf-{}", Uuid::new_v4()));
        let reports = root.join("reports");
        std::fs::create_dir_all(&reports).unwrap();
        let pdf = reports.join("report.pdf");
        let text = reports.join("report.txt");
        let outside = root.join("outside.pdf");
        std::fs::write(&pdf, b"%PDF-1.4\n%%EOF\n").unwrap();
        std::fs::write(&text, b"text").unwrap();
        std::fs::write(&outside, b"%PDF-1.4\n%%EOF\n").unwrap();
        assert_eq!(
            canonical_exported_pdf(&reports, pdf.to_str().unwrap()).unwrap(),
            pdf.canonicalize().unwrap()
        );
        assert!(canonical_exported_pdf(&reports, text.to_str().unwrap()).is_err());
        assert!(canonical_exported_pdf(&reports, outside.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn process_state_records_cancel_request() {
        let state = TestProcessState::default();
        state.register("run-1");
        assert!(!state.is_cancelled("run-1"));
        assert_eq!(state.request_cancel("run-1"), Some(None));
        assert!(state.is_cancelled("run-1"));
        state.finish("run-1");
        assert_eq!(state.request_cancel("run-1"), None);
    }

    #[test]
    fn startup_recovery_marks_unfinished_run_as_execution_error() {
        let path =
            std::env::temp_dir().join(format!("workbench-recovery-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let run = TestRun {
            id: "unfinished".into(),
            menu_id: "project:example".into(),
            project: "fixture".into(),
            project_path: "C:/fixture".into(),
            menu_name: "示例页面".into(),
            mode: "mock".into(),
            status: "running".into(),
            started_at: "2026-08-24T10:00:00+08:00".into(),
            finished_at: None,
            report_markdown: String::new(),
            source_report_path: None,
            output_excerpt: String::new(),
            error_message: String::new(),
            selected_scenarios: vec!["页面显示".into()],
            scenario_results: Vec::new(),
            artifacts: Vec::new(),
            total_count: 1,
            passed_count: 0,
            failed_count: 0,
            skipped_count: 0,
            duration_ms: 0,
            exit_code: None,
            environment_summary: "执行中".into(),
            cleanup_status: "pending".into(),
        };
        save_run(&state, &run).unwrap();
        assert_eq!(recover_incomplete_test_runs(&state).unwrap(), 1);
        let recovered = run_by_id(&state, "unfinished").unwrap();
        assert_eq!(recovered.status, "error");
        assert!(recovered.error_message.contains("关闭"));
        assert!(recovered.report_markdown.contains("执行异常"));
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
