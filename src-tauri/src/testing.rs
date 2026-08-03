use crate::database::DatabaseState;
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use uuid::Uuid;

const CLIENT_ROOT: &str = r"F:\TB-project\client";
const APP_ROOT: &str = r"F:\TB-project\APP";

fn client_root() -> PathBuf {
    std::env::var_os("AI_WORKBENCH_CLIENT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CLIENT_ROOT))
}

fn app_root() -> PathBuf {
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
    pub name: String,
    pub route: String,
    pub source_path: String,
    pub case_id: Option<String>,
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
    pub menu_name: String,
    pub mode: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub report_markdown: String,
    pub source_report_path: Option<String>,
    pub output_excerpt: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTestOptions {
    pub menu_id: String,
    pub mode: String,
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

fn latest_db_run(state: &DatabaseState, menu_id: &str) -> Result<Option<TestRun>, String> {
    state.connect()?.query_row(
        "SELECT id,menu_id,project,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message FROM test_runs WHERE menu_id=?1 ORDER BY started_at DESC LIMIT 1",
        [menu_id], row_to_run,
    ).optional().map_err(|error| error.to_string())
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TestRun> {
    Ok(TestRun {
        id: row.get(0)?,
        menu_id: row.get(1)?,
        project: row.get(2)?,
        menu_name: row.get(3)?,
        mode: row.get(4)?,
        status: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        report_markdown: row.get(8)?,
        source_report_path: row.get(9)?,
        output_excerpt: row.get(10)?,
        error_message: row.get(11)?,
    })
}

fn client_menus(state: &DatabaseState) -> Result<Vec<TestMenu>, String> {
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
        let db = latest_db_run(state, &menu_id)?;
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
            name,
            route: format!("/{full_path}"),
            source_path: format!("{full_path}.vue"),
            case_id: None,
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

fn app_menus(state: &DatabaseState) -> Result<Vec<TestMenu>, String> {
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
        if let Some(run) = latest_db_run(state, &menu.id)? {
            menu.tested = true;
            menu.latest_status = Some(run.status);
            menu.latest_time = run.finished_at.or(Some(run.started_at));
            menu.latest_report_path = run.source_report_path;
        }
    }
    Ok(menus)
}

#[tauri::command]
pub fn list_test_menus(state: tauri::State<'_, DatabaseState>) -> Result<Vec<TestMenu>, String> {
    let mut menus = client_menus(&state)?;
    menus.extend(app_menus(&state)?);
    Ok(menus)
}

#[tauri::command]
pub fn list_test_runs(
    state: tauri::State<'_, DatabaseState>,
    menu_id: Option<String>,
) -> Result<Vec<TestRun>, String> {
    let connection = state.connect()?;
    let sql = if menu_id.is_some() {
        "SELECT id,menu_id,project,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message FROM test_runs WHERE menu_id=?1 ORDER BY started_at DESC"
    } else {
        "SELECT id,menu_id,project,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message FROM test_runs ORDER BY started_at DESC LIMIT 300"
    };
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let rows = if let Some(id) = menu_id {
        statement
            .query_map([id], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
    } else {
        statement
            .query_map([], row_to_run)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
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

fn newest_report_after(name: &str, after: SystemTime) -> Option<PathBuf> {
    fs::read_dir(client_root().join("e2e").join("reports"))
        .ok()?
        .flatten()
        .filter(|item| {
            item.path().extension().and_then(|value| value.to_str()) == Some("md")
                && item.file_name().to_string_lossy().contains(name)
        })
        .filter_map(|item| {
            let modified = item.metadata().ok()?.modified().ok()?;
            (modified >= after).then_some((modified, item.path()))
        })
        .max_by_key(|item| item.0)
        .map(|item| item.1)
}

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

fn execute_client(
    menu: &TestMenu,
    options: &StartTestOptions,
) -> Result<(bool, String, Option<String>), String> {
    let started = SystemTime::now();
    let root = client_root();
    let mut command = Command::new("powershell.exe");
    command
        .current_dir(&root)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
    match options.mode.as_str() {
        "mock" => {
            command
                .arg(root.join(".codex/skills/client-menu-e2e-test/scripts/run-menu-e2e.ps1"))
                .args(["-MenuName", &menu.name]);
        }
        "real" => {
            command
                .arg(root.join(".codex/skills/client-menu-e2e-test/scripts/run-menu-e2e.ps1"))
                .args(["-MenuName", &menu.name, "-RealBackend"]);
        }
        "source-style" => {
            command
                .arg(
                    root.join(
                        ".codex/skills/client-page-style-test/scripts/run-page-style-test.ps1",
                    ),
                )
                .args(["-MenuName", &menu.name]);
        }
        "browser-style" => {
            command
                .arg(
                    root.join(
                        ".codex/skills/client-page-style-test/scripts/run-page-style-test.ps1",
                    ),
                )
                .args(["-MenuName", &menu.name, "-Browser"]);
        }
        _ => return Err("不支持的 client 测试类型。".into()),
    };
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
    if options.mode == "real"
        && options.use_environment_token == Some(false)
        && options
            .token
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err("真实接口测试需要输入临时 Token，或选择读取 Windows 用户 HLZT_TOKEN。".into());
    }
    let output = command
        .output()
        .map_err(|error| format!("无法启动项目现有测试脚本：{error}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report_path =
        newest_report_after(&menu.name, started).map(|path| path.display().to_string());
    let report = report_path
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| {
            format!(
                "# 测试执行输出\n\n```text\n{}\n```",
                combined.chars().take(12000).collect::<String>()
            )
        });
    Ok((output.status.success(), report, report_path))
}

fn save_run(state: &DatabaseState, run: &TestRun) -> Result<(), String> {
    state.connect()?.execute("INSERT INTO test_runs(id,menu_id,project,menu_name,mode,status,started_at,finished_at,report_markdown,source_report_path,output_excerpt,error_message) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![run.id,run.menu_id,run.project,run.menu_name,run.mode,run.status,run.started_at,run.finished_at,run.report_markdown,run.source_report_path,run.output_excerpt,run.error_message]).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn start_test_run(
    state: tauri::State<'_, DatabaseState>,
    options: StartTestOptions,
) -> Result<TestRun, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let menu = client_menus(&database)?
            .into_iter()
            .chain(app_menus(&database)?)
            .find(|menu| menu.id == options.menu_id)
            .ok_or_else(|| "没有找到对应菜单或页面。".to_string())?;
        if menu.project == "APP" && options.mode != "source-style" {
            return Err("APP 当前仅支持现有页面的源码与样式静态检查。".into());
        }
        let started_at = Utc::now().to_rfc3339();
        let result = if menu.project == "APP" {
            let (passed, report) = app_static_report(&menu);
            Ok((passed, report, None))
        } else {
            execute_client(&menu, &options)
        };
        let (passed, report, path, error) = match result {
            Ok((passed, report, path)) => (passed, report, path, String::new()),
            Err(error) => (false, format!("# 测试未能启动\n\n{error}"), None, error),
        };
        let report = append_remediation(report, &menu.name, &menu.project, passed);
        let run = TestRun {
            id: Uuid::new_v4().to_string(),
            menu_id: menu.id,
            project: menu.project,
            menu_name: menu.name,
            mode: options.mode,
            status: if passed {
                "passed".into()
            } else {
                "failed".into()
            },
            started_at,
            finished_at: Some(Utc::now().to_rfc3339()),
            report_markdown: report,
            source_report_path: path,
            output_excerpt: String::new(),
            error_message: error,
        };
        save_run(&database, &run)?;
        crate::suggestions::sync_task_suggestions_for_state(&database)?;
        Ok(run)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{
        app_menus, app_static_report, append_remediation, client_menus, client_report_status,
        strip_json_comments,
    };
    use crate::database::DatabaseState;
    use uuid::Uuid;
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
}
