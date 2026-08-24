use crate::{codex_video, database::DatabaseState};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use uuid::Uuid;
use walkdir::WalkDir;

const CLIENT_ROOT: &str = r"F:\TB-project\client";
const APP_ROOT: &str = r"F:\TB-project\APP";
const TEST_RUN_COLUMNS: &str = "id,menu_id,project,project_path,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message,selected_scenarios,scenario_results,artifacts,total_count,passed_count,failed_count,skipped_count,duration_ms,exit_code,environment_summary,cleanup_status";

#[derive(Default, Clone)]
pub struct TestProcessState {
    processes: Arc<Mutex<HashMap<String, TestProcessControl>>>,
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

pub(crate) fn client_root() -> PathBuf {
    std::env::var_os("AI_WORKBENCH_CLIENT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CLIENT_ROOT))
}

pub(crate) fn app_root() -> PathBuf {
    std::env::var_os("AI_WORKBENCH_APP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(APP_ROOT))
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
    state
        .connect()?
        .query_row(
            &format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE menu_id=?1 AND (project_path=?2 OR project_path='') ORDER BY CASE WHEN project_path=?2 THEN 0 ELSE 1 END,started_at DESC LIMIT 1"),
            params![menu_id, project_path.display().to_string()],
            row_to_run,
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn json_vec<T: for<'de> Deserialize<'de>>(value: String) -> Vec<T> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TestRun> {
    Ok(TestRun {
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
    })
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
            project_path: client_root().display().to_string(),
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
            project_path: app_root().display().to_string(),
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
    TestCapabilities {
        mock: has_script(&scripts, "test:menu")
            && root
                .join("e2e")
                .join("specs")
                .join("menu-module.spec.js")
                .is_file(),
        real_api: has_script(&scripts, "test:menu:real")
            && root
                .join("e2e")
                .join("specs")
                .join("real-menu-module.spec.js")
                .is_file(),
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

fn estimated_page_count(root: &Path, case_count: usize) -> usize {
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
    let views = root.join("src").join("views");
    if views.is_dir() {
        return WalkDir::new(views)
            .max_depth(8)
            .into_iter()
            .flatten()
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("vue")
            })
            .take(600)
            .count()
            .max(case_count);
    }
    case_count
}

fn route_from_component(component: &str) -> String {
    let value = component.trim_matches('/').trim_end_matches("/index");
    format!("/{value}")
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
        project_path: root.display().to_string(),
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
            real_api: has_case && available.real_api,
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
    let mut case_components = HashSet::new();
    for (value, path) in &cases {
        let case_id = value.get("id").and_then(Value::as_str).unwrap_or_default();
        if case_id.is_empty() {
            continue;
        }
        let component = value
            .get("component")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace('\\', "/");
        case_components.insert(component.to_lowercase());
        let source_path = if root
            .join("src")
            .join("views")
            .join(format!("{component}.vue"))
            .is_file()
        {
            format!("src/views/{component}.vue")
        } else {
            format!("{component}.vue")
        };
        let stable_menu_id = value
            .get("workbenchMenuId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                if same_canonical_path(root, &client_root()) {
                    format!("client:{case_id}")
                } else {
                    format!("project:{case_id}")
                }
            });
        menus.push(catalog_menu(
            state,
            root,
            project_name,
            &kind,
            stable_menu_id,
            value
                .get("menuName")
                .and_then(Value::as_str)
                .unwrap_or(case_id)
                .to_string(),
            value
                .get("route")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            source_path,
            Some((value, path.as_path())),
        )?);
    }

    if let Ok(source) = fs::read_to_string(root.join("pages.json")) {
        if let Ok(value) = serde_json::from_str::<Value>(&strip_json_comments(&source)) {
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
                    pages.push((full, name));
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
            for (path, name) in pages {
                if menus
                    .iter()
                    .any(|menu| menu.route.trim_start_matches('/') == path)
                {
                    continue;
                }
                let component = path.trim_end_matches(".vue");
                let id = if same_canonical_path(root, &app_root()) {
                    format!("app:{component}")
                } else {
                    format!("page:{component}")
                };
                menus.push(catalog_menu(
                    state,
                    root,
                    project_name,
                    &kind,
                    id,
                    name,
                    format!("/{component}"),
                    format!("{component}.vue"),
                    None,
                )?);
            }
        }
    } else {
        let views = root.join("src").join("views");
        if views.is_dir() {
            for entry in WalkDir::new(&views)
                .max_depth(8)
                .into_iter()
                .flatten()
                .filter(|entry| {
                    entry.file_type().is_file()
                        && entry.path().extension().and_then(|value| value.to_str()) == Some("vue")
                })
                .take(600)
            {
                let relative = entry
                    .path()
                    .strip_prefix(&views)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .replace('\\', "/");
                let component = relative.trim_end_matches(".vue").to_string();
                if component.to_lowercase().contains("/components/")
                    || case_components.contains(&component.to_lowercase())
                {
                    continue;
                }
                let name = if component.ends_with("/index") {
                    component
                        .trim_end_matches("/index")
                        .rsplit('/')
                        .next()
                        .unwrap_or(&component)
                } else {
                    component.rsplit('/').next().unwrap_or(&component)
                }
                .to_string();
                menus.push(catalog_menu(
                    state,
                    root,
                    project_name,
                    &kind,
                    format!("page:{}", component.replace('/', ":")),
                    name,
                    route_from_component(&component),
                    format!("src/views/{relative}"),
                    None,
                )?);
            }
        }
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
        .map(|root| (root.display().to_string(), git_changed_paths(root)))
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
    let connection = state.connect()?;
    let sql = match (menu_id.is_some(), project_path.is_some()) {
        (true, true) => format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE menu_id=?1 AND (project_path=?2 OR project_path='') ORDER BY started_at DESC"),
        (true, false) => format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE menu_id=?1 ORDER BY started_at DESC"),
        (false, true) => format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE project_path=?1 ORDER BY started_at DESC LIMIT 300"),
        (false, false) => format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs ORDER BY started_at DESC LIMIT 300"),
    };
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = match (menu_id, project_path) {
        (Some(id), Some(path)) => statement
            .query_map(params![id, path], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        (Some(id), None) => statement
            .query_map([id], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        (None, Some(path)) => statement
            .query_map([path], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
        (None, None) => statement
            .query_map([], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>(),
    };
    rows.map_err(|error| error.to_string())
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

fn scenario_description(mode: &str, title: &str) -> String {
    if title.contains("登录") || title.contains("进入页面") || title.contains("基础区域")
    {
        "确认页面入口、登录态和核心区域可以正常使用。".into()
    } else if title.contains("查询") || title.contains("列表") {
        "确认查询条件、列表请求和页面结果保持一致。".into()
    } else if title.contains("新增") || title.contains("修改") || title.contains("删除") {
        "确认表单与关键业务操作符合页面预期。".into()
    } else if title.contains("窄屏")
        || title.contains("桌面")
        || title.contains("样式")
        || title.contains("溢出")
    {
        "确认页面在目标视口下没有明显显示问题。".into()
    } else if mode == "source-style" {
        "检查页面源码结构、路由注册和基础样式。".into()
    } else {
        "执行项目已有自动化场景并记录真实结果。".into()
    }
}

fn extract_test_titles(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap_or_default();
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

fn spec_for_mode(root: &Path, menu: &TestMenu, mode: &str) -> Option<PathBuf> {
    let file = match mode {
        "mock" => "menu-module.spec.js".to_string(),
        "real" => menu
            .case_file_path
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            .and_then(|value| {
                value
                    .get("realSpec")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "real-menu-module.spec.js".into()),
        "browser-style" => "page-style.spec.js".to_string(),
        _ => return None,
    };
    Some(root.join("e2e").join("specs").join(file))
}

#[tauri::command]
pub fn list_test_scenarios(
    state: tauri::State<'_, DatabaseState>,
    project_path: String,
    menu_id: String,
    mode: String,
) -> Result<Vec<TestScenario>, String> {
    let (root, name) = canonical_project_asset(&state, &project_path)?;
    let menu = project_menus(&state, &root, &name)?
        .into_iter()
        .find(|menu| menu.id == menu_id)
        .ok_or_else(|| "没有找到对应功能或页面。".to_string())?;
    let titles = if mode == "source-style" {
        vec![
            "页面文件与路由注册".into(),
            "Vue 基础结构".into(),
            "页面样式结构".into(),
        ]
    } else {
        spec_for_mode(&root, &menu, &mode)
            .filter(|path| path.is_file())
            .map(|path| extract_test_titles(&path))
            .unwrap_or_default()
    };
    Ok(titles
        .into_iter()
        .enumerate()
        .map(|(index, title)| TestScenario {
            id: format!("scenario-{}", index + 1),
            description: scenario_description(&mode, &title),
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

fn create_case_file(root: &Path, menu: &TestMenu, scenarios: &[String]) -> Result<PathBuf, String> {
    let case_dir = root.join("e2e").join("menu-cases");
    fs::create_dir_all(&case_dir).map_err(|error| format!("无法创建测试配置目录：{error}"))?;
    let case_id = safe_case_id(menu);
    let path = case_dir.join(format!("{case_id}.json"));
    if path.exists() {
        return Err("目标测试配置已经存在，为避免覆盖现有用例已停止。".into());
    }
    let component = menu
        .source_path
        .trim_end_matches(".vue")
        .trim_start_matches("src/views/")
        .replace('\\', "/");
    let value = json!({
        "id": case_id,
        "menuName": menu.name,
        "aliases": [],
        "route": menu.route,
        "component": component,
        "layoutFlag": "systemLayout",
        "permissions": ["*:*:*"],
        "mockRows": [],
        "scenarios": scenarios,
        "generatedBy": "AI个人工作台",
        "workbenchMenuId": menu.id,
        "generatedAt": Utc::now().to_rfc3339()
    });
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("无法写入测试配置：{error}"))?;
    Ok(path)
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
    let has_case_or_create = menu.has_case_file || options.create_case_file == Some(true);
    if options.mode != "source-style" {
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
        checks.push(PreflightCheck {
            name: "真实写入确认".into(),
            passed: options.confirmed_real_write == Some(true),
            detail: "真实用例可能创建、修改并清理 E2E 前缀数据".into(),
        });
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
    checks.push(PreflightCheck {
        name: "测试场景".into(),
        passed: !options.selected_scenarios.is_empty(),
        detail: format!("已选择 {} 个场景", options.selected_scenarios.len()),
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
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
    .display()
    .to_string()
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
                    purpose: scenario_description("", &title),
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

fn static_scenarios(root: &Path, menu: &TestMenu, selected: &[String]) -> Vec<TestScenarioResult> {
    let path = root.join(&menu.source_path);
    let content = fs::read_to_string(&path).unwrap_or_default();
    let cases = [
        (
            "页面文件与路由注册",
            path.is_file() && !menu.route.trim().is_empty(),
            vec!["页面文件存在。", "页面路由不为空。"],
        ),
        (
            "Vue 基础结构",
            content.contains("<template"),
            vec!["页面包含可渲染的 template。"],
        ),
        (
            "页面样式结构",
            content.contains("<style") || content.contains("style="),
            vec!["页面包含样式入口或行内样式。"],
        ),
    ];
    cases
        .into_iter()
        .filter(|(title, _, _)| selected.iter().any(|item| item == title))
        .enumerate()
        .map(|(index, (title, passed, checks))| TestScenarioResult {
            id: format!("static-{}", index + 1),
            title: title.into(),
            status: if passed {
                "passed".into()
            } else {
                "failed".into()
            },
            duration_ms: 0,
            purpose: scenario_description("source-style", title),
            steps: vec![
                format!("读取 {}。", menu.source_path),
                "检查源码和页面注册信息。".into(),
            ],
            checks: checks.into_iter().map(str::to_string).collect(),
            error_message: if passed {
                String::new()
            } else {
                format!("{} 未达到静态检查要求。", title)
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
    let spec = spec_for_mode(root, menu, &options.mode)
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
    let mut command = codex_video::hidden_command(Path::new("node"));
    command
        .current_dir(root)
        .arg(cli)
        .args(["test"])
        .arg(&spec)
        .args(["--project=chromium", "--grep"])
        .arg(grep);
    command
        .env("E2E_MENU_NAME", &menu.name)
        .env(
            "E2E_MENU_CASE_ID",
            menu.case_id.clone().unwrap_or_else(|| safe_case_id(menu)),
        )
        .env("E2E_JSON_REPORT", &raw_path);
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
    let exit_code = output.status.code();
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
        report_path: Some(raw_path.display().to_string()),
        environment_summary: format!("Node + Playwright；规格文件 {}", spec.display()),
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
            project_path: root.display().to_string(),
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
        project_path: root.display().to_string(),
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
            project_path: root.display().to_string(),
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
<section class="cover"><div class="eyebrow">TEST REPORT</div><h1>{}</h1><div class="meta">{} · {} · {}</div><div class="summary"><div><span>结果</span><b>{}</b></div><div><span>场景</span><b>{}</b></div><div><span>通过</span><b>{}</b></div><div><span>失败</span><b>{}</b></div></div><div class="meta">环境：{}<br>项目目录：{}</div></section>{}<footer>由 AI 个人工作台导出 · {}</footer></body></html>"#,
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
        let file_name = format!(
            "{}-{}-{}-{}.pdf",
            safe_pdf_name(&run.project),
            safe_pdf_name(&run.menu_name),
            safe_pdf_name(&run.started_at.chars().take(19).collect::<String>()).replace('T', "-"),
            run.id.chars().take(8).collect::<String>()
        );
        let output_path = output_dir.join(file_name);
        let html_path = std::env::temp_dir().join(format!("workbench-test-report-{}.html", run.id));
        let profile_path =
            std::env::temp_dir().join(format!("workbench-test-pdf-profile-{}", run.id));
        fs::write(&html_path, pdf_html(&run)).map_err(|error| error.to_string())?;
        let browser = edge_path().ok_or_else(|| {
            "没有找到 Microsoft Edge 或 Google Chrome，无法导出 PDF。".to_string()
        })?;
        let mut command = codex_video::hidden_command(&browser);
        let output_arg = format!("--print-to-pdf={}", output_path.display());
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
        let _ = fs::remove_file(&html_path);
        let _ = fs::remove_dir_all(&profile_path);
        if !result.status.success()
            || !output_path.is_file()
            || fs::metadata(&output_path)
                .map(|item| item.len())
                .unwrap_or(0)
                == 0
        {
            return Err(format!(
                "PDF 导出失败：{}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        Ok(output_path.display().to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{
        app_menus, app_static_report, append_remediation, client_menus, client_report_status,
        create_case_file, pdf_html, recover_incomplete_test_runs, run_by_id, save_run,
        strip_json_comments, TestArtifact, TestCapabilities, TestMenu, TestProcessState, TestRun,
        TestScenarioResult,
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
    #[test]
    fn comments_are_removed_without_touching_urls() {
        let value = strip_json_comments("{\"url\":\"http://localhost\",// note\n\"ok\":true}");
        assert!(value.contains("http://localhost"));
        assert!(!value.contains("note"));
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

    #[test]
    fn real_project_catalogs_can_be_loaded() {
        let path =
            std::env::temp_dir().join(format!("workbench-testing-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let client = client_menus(&state).unwrap();
        let app = app_menus(&state).unwrap();
        assert_eq!(client.len(), 15);
        assert!(!app.is_empty());
        assert!(app.iter().all(|menu| menu.source_path.ends_with(".vue")));
        let (passed, report) = app_static_report(&app[0]);
        assert!(passed, "{report}");
        assert!(report.contains("验证边界"));
        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generated_case_file_keeps_selected_scenarios_and_never_overwrites() {
        let root = std::env::temp_dir().join(format!("workbench-case-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("src/views/example")).unwrap();
        let menu = sample_menu(&root);
        let selected = vec!["场景一".to_string(), "场景二".to_string()];
        let path = create_case_file(&root, &menu, &selected).unwrap();
        let original = std::fs::read_to_string(&path).unwrap();
        assert!(original.contains("场景一"));
        assert!(original.contains("workbenchMenuId"));
        assert!(create_case_file(&root, &menu, &selected).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
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
