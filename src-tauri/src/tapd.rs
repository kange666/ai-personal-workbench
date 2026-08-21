use crate::{codex_video, database::DatabaseState};
use chrono::Utc;
use keyring::Entry;
use reqwest::{Client, RequestBuilder};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Stdio,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const CREDENTIAL_SERVICE: &str = "AI Personal Workbench";
const CREDENTIAL_USER: &str = "tapd-openapi";
const TAPD_API_ROOT: &str = "https://api.tapd.cn";
const WORKSPACE_ID: &str = "37583308";
const WORKSPACE_NAME: &str = "安全生产管理";
const DEFAULT_OWNER: &str = "刘子世康";
const PAGE_LIMIT: usize = 200;
const AUTO_FIX_ENABLED_KEY: &str = "tapd_auto_fix_enabled";
const AUTO_FIX_REPOSITORY_KEY: &str = "tapd_auto_fix_repository_path";

// 自动缺陷处理必须串行，避免多个 Codex 任务同时修改同一个工作区。
static AUTO_FIX_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TapdCredential {
    #[serde(default)]
    auth_mode: String,
    #[serde(default)]
    api_user: String,
    #[serde(default)]
    api_password: String,
    #[serde(default)]
    access_token: String,
}

impl TapdCredential {
    fn mode(&self) -> &str {
        if self.auth_mode == "token"
            || (!self.access_token.trim().is_empty() && self.auth_mode.is_empty())
        {
            "token"
        } else {
            "basic"
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdStatus {
    configured: bool,
    source: String,
    auth_mode: String,
    workspace_id: String,
    workspace_name: String,
    owner: String,
    last_synced_at: Option<String>,
    item_count: i64,
    warnings: Vec<String>,
    auto_fix_enabled: bool,
    auto_fix_repository_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdWorkItem {
    id: String,
    workspace_id: String,
    item_type: String,
    title: String,
    description: String,
    status: String,
    status_label: String,
    priority: String,
    owner: String,
    creator: String,
    iteration_id: String,
    begin_date: String,
    due_date: String,
    created_at: String,
    modified_at: String,
    source_url: String,
    synced_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdSyncSummary {
    bugs: usize,
    tasks: usize,
    stories: usize,
    total: usize,
    notifications_created: usize,
    warnings: Vec<String>,
    synced_at: String,
    auto_jobs_started: usize,
    auto_jobs_skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdCodexJob {
    id: String,
    item_id: String,
    repository_path: String,
    status: String,
    thread_id: Option<String>,
    output: String,
    error_message: String,
    baseline_head: String,
    baseline_worktree: String,
    result_head: String,
    changed_files: Vec<String>,
    test_summary: String,
    review_status: String,
    review_note: String,
    reviewed_at: Option<String>,
    trigger_source: String,
    process_report_path: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdAutoFixSettings {
    enabled: bool,
    repository_path: String,
}

#[derive(Default)]
struct SaveItemsOutcome {
    notifications_created: usize,
    new_bug_ids: Vec<String>,
}

#[derive(Default)]
struct RepositoryCommands {
    start: String,
    test: String,
    build: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapdJobReview {
    id: String,
    decision: String,
    note: String,
}

fn credential_entry() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|error| error.to_string())
}

fn credentials() -> Result<(TapdCredential, String), String> {
    let access_token = std::env::var("TAPD_ACCESS_TOKEN").unwrap_or_default();
    if !access_token.trim().is_empty() {
        return Ok((
            TapdCredential {
                auth_mode: "token".into(),
                api_user: String::new(),
                api_password: String::new(),
                access_token,
            },
            "环境变量".into(),
        ));
    }
    let user = std::env::var("TAPD_API_USER").unwrap_or_default();
    let password = std::env::var("TAPD_API_PASSWORD").unwrap_or_default();
    if !user.trim().is_empty() && !password.trim().is_empty() {
        return Ok((
            TapdCredential {
                auth_mode: "basic".into(),
                api_user: user,
                api_password: password,
                access_token: String::new(),
            },
            "环境变量".into(),
        ));
    }
    let raw = credential_entry()?
        .get_password()
        .map_err(|_| "尚未配置 TAPD OpenAPI 凭据，请在设置中保存。".to_string())?;
    let credential = serde_json::from_str::<TapdCredential>(&raw)
        .map_err(|_| "Windows 凭据库中的 TAPD 配置无效，请重新保存。".to_string())?;
    if credential.mode() == "token" {
        if credential.access_token.trim().is_empty() {
            return Err("TAPD 个人访问令牌为空。".into());
        }
    } else if credential.api_user.trim().is_empty() || credential.api_password.trim().is_empty() {
        return Err("TAPD OpenAPI 用户名或密码为空。".into());
    }
    Ok((credential, "Windows 凭据库".into()))
}

fn authenticated_request(
    client: &Client,
    credential: &TapdCredential,
    url: String,
) -> RequestBuilder {
    let request = client.get(url);
    if credential.mode() == "token" {
        request.bearer_auth(credential.access_token.trim())
    } else {
        request.basic_auth(&credential.api_user, Some(&credential.api_password))
    }
}

fn authenticated_post_request(
    client: &Client,
    credential: &TapdCredential,
    url: String,
) -> RequestBuilder {
    let request = client.post(url);
    if credential.mode() == "token" {
        request.bearer_auth(credential.access_token.trim())
    } else {
        request.basic_auth(&credential.api_user, Some(&credential.api_password))
    }
}

fn app_meta(state: &DatabaseState, key: &str) -> Option<String> {
    state
        .connect()
        .ok()?
        .query_row("SELECT value FROM app_meta WHERE key=?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .ok()?
        .flatten()
}

fn set_app_meta(state: &DatabaseState, key: &str, value: &str) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn auto_fix_settings(state: &DatabaseState) -> TapdAutoFixSettings {
    TapdAutoFixSettings {
        enabled: app_meta(state, AUTO_FIX_ENABLED_KEY).as_deref() == Some("true"),
        repository_path: app_meta(state, AUTO_FIX_REPOSITORY_KEY).unwrap_or_default(),
    }
}

fn repository_commands(
    state: &DatabaseState,
    repository_path: &str,
) -> Result<RepositoryCommands, String> {
    state
        .connect()?
        .query_row(
            "SELECT start_command,test_command,build_command FROM repository_assets WHERE path=?1",
            [repository_path],
            |row| {
                Ok(RepositoryCommands {
                    start: row.get(0)?,
                    test: row.get(1)?,
                    build: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "请选择工作台“项目资产”中已扫描的本地项目。".to_string())
}

fn redact_sensitive_text(value: &str) -> String {
    let mut redacted = value.to_string();
    if let Ok(token) = std::env::var("HLZT_TOKEN") {
        let token = token.trim();
        if token.len() >= 6 {
            redacted = redacted.replace(token, "[HLZT_TOKEN 已隐藏]");
        }
    }
    redacted
}

fn owner(state: &DatabaseState) -> String {
    app_meta(state, "tapd_owner")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_OWNER.to_string())
}

fn permission_warnings(state: &DatabaseState) -> Vec<String> {
    app_meta(state, "tapd_last_warnings")
        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .unwrap_or_default()
}

fn save_permission_warnings(state: &DatabaseState, warnings: &[String]) -> Result<(), String> {
    let value = serde_json::to_string(warnings).map_err(|error| error.to_string())?;
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES('tapd_last_warnings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(|item| match item {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn html_to_text(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                if !output.ends_with(' ') && !output.ends_with('\n') {
                    output.push(' ');
                }
            }
            _ if !in_tag => output.push(character),
            _ => {}
        }
    }
    output
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn status_label(item_type: &str, status: &str) -> String {
    let label = match (item_type, status) {
        ("bug", "new") => "待处理",
        ("bug", "in_progress") | ("bug", "progressing") => "处理中",
        ("bug", "resolved") => "已解决",
        ("bug", "verified") => "已验证",
        ("bug", "closed") => "已关闭",
        ("bug", "reopened") => "重新打开",
        ("task", "open") => "未开始",
        ("task", "progressing") => "进行中",
        ("task", "done") => "已完成",
        ("story", "planning") => "规划中",
        ("story", "developing") => "实现中",
        ("story", "resolved") => "已实现",
        ("story", "released") => "已发布",
        (_, value) if value.is_empty() => "未设置",
        (_, value) => value,
    };
    label.to_string()
}

fn item_url(item_type: &str, id: &str) -> String {
    format!("https://www.tapd.cn/tapd_fe/{WORKSPACE_ID}/{item_type}/detail/{id}")
}

fn normalize_item(item_type: &str, wrapper: &Value, synced_at: &str) -> Option<TapdWorkItem> {
    let key = match item_type {
        "bug" => "Bug",
        "task" => "Task",
        "story" => "Story",
        _ => return None,
    };
    let item = wrapper.get(key).unwrap_or(wrapper);
    let id = text(item.get("id"));
    if id.is_empty() {
        return None;
    }
    let title = if item_type == "task" {
        text(item.get("name"))
    } else {
        text(item.get("title"))
    };
    let status = text(item.get("status"));
    let priority = text(item.get("priority_label"));
    Some(TapdWorkItem {
        id: id.clone(),
        workspace_id: text(item.get("workspace_id")),
        item_type: item_type.to_string(),
        title,
        description: html_to_text(&text(item.get("description"))),
        status_label: status_label(item_type, &status),
        status,
        priority: if priority.is_empty() {
            text(item.get("priority"))
        } else {
            priority
        },
        owner: if item_type == "bug" {
            text(item.get("current_owner"))
        } else {
            text(item.get("owner"))
        },
        creator: if item_type == "bug" {
            text(item.get("reporter"))
        } else {
            text(item.get("creator"))
        },
        iteration_id: text(item.get("iteration_id")),
        begin_date: text(item.get("begin")),
        due_date: text(item.get("due")),
        created_at: text(item.get("created")),
        modified_at: text(item.get("modified")),
        source_url: item_url(item_type, &id),
        synced_at: synced_at.to_string(),
    })
}

async fn fetch_type(
    client: &Client,
    credential: &TapdCredential,
    item_type: &str,
    owner: &str,
    synced_at: &str,
) -> Result<Vec<TapdWorkItem>, String> {
    let endpoint = match item_type {
        "bug" => "bugs",
        "task" => "tasks",
        "story" => "stories",
        _ => return Err("不支持的 TAPD 工作项类型。".into()),
    };
    let owner_field = if item_type == "bug" {
        "current_owner"
    } else {
        "owner"
    };
    let mut result = Vec::new();
    for page in 1..=20 {
        let mut query = vec![
            ("workspace_id", WORKSPACE_ID.to_string()),
            ("limit", PAGE_LIMIT.to_string()),
            ("page", page.to_string()),
            ("order", "modified desc".to_string()),
        ];
        if !owner.trim().is_empty() {
            query.push((owner_field, owner.to_string()));
        }
        let response =
            authenticated_request(client, credential, format!("{TAPD_API_ROOT}/{endpoint}"))
                .query(&query)
                .send()
                .await
                .map_err(|error| format!("读取 TAPD {endpoint} 失败：{error}"))?;
        let http_status = response.status();
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| format!("解析 TAPD {endpoint} 返回失败：{error}"))?;
        if !http_status.is_success() || body.get("status").and_then(Value::as_i64) != Some(1) {
            let info = text(body.get("info"));
            return Err(if info.is_empty() {
                format!("TAPD {endpoint} 返回 {http_status}")
            } else {
                format!("TAPD {endpoint}：{info}")
            });
        }
        let items = body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let count = items.len();
        result.extend(
            items
                .iter()
                .filter_map(|item| normalize_item(item_type, item, synced_at)),
        );
        if count < PAGE_LIMIT {
            break;
        }
    }
    Ok(result)
}

fn readable_permission_error(item_type: &str, error: &str) -> String {
    let (label, scope) = match item_type {
        "bug" => ("缺陷", "bug#read"),
        "task" => ("任务", "task#read"),
        "story" => ("需求", "story#read"),
        _ => (item_type, item_type),
    };
    if error.to_ascii_lowercase().contains("no permission") {
        format!("{label}接口无权限：个人访问令牌缺少 {scope} 权限")
    } else {
        format!("{label}接口读取失败：{error}")
    }
}

#[derive(Debug, Clone)]
struct TapdItemSnapshot {
    status_label: String,
    priority: String,
    owner: String,
    due_date: String,
}

fn item_type_label(item_type: &str) -> &str {
    match item_type {
        "bug" => "缺陷",
        "story" => "需求",
        _ => "任务",
    }
}

fn display_value(value: &str) -> &str {
    if value.trim().is_empty() {
        "未设置"
    } else {
        value
    }
}

fn is_actionable_new_bug(item: &TapdWorkItem) -> bool {
    if item.item_type != "bug" {
        return false;
    }
    let status = format!("{} {}", item.status, item.status_label).to_ascii_lowercase();
    ![
        "已解决",
        "已关闭",
        "已完成",
        "已发布",
        "resolved",
        "closed",
        "done",
        "released",
    ]
    .iter()
    .any(|terminal| status.contains(terminal))
}

fn tapd_change_summary(previous: Option<&TapdItemSnapshot>, item: &TapdWorkItem) -> Option<String> {
    let label = item_type_label(&item.item_type);
    let Some(previous) = previous else {
        return Some(format!("新{label}已分配给你"));
    };
    let mut changes = Vec::new();
    if previous.status_label != item.status_label {
        changes.push(format!(
            "状态：{} → {}",
            display_value(&previous.status_label),
            display_value(&item.status_label)
        ));
    }
    if previous.priority != item.priority {
        changes.push(format!(
            "优先级：{} → {}",
            display_value(&previous.priority),
            display_value(&item.priority)
        ));
    }
    if previous.owner != item.owner {
        changes.push(format!(
            "负责人：{} → {}",
            display_value(&previous.owner),
            display_value(&item.owner)
        ));
    }
    if previous.due_date != item.due_date {
        changes.push(format!(
            "截止时间：{} → {}",
            display_value(&previous.due_date),
            display_value(&item.due_date)
        ));
    }
    (!changes.is_empty()).then(|| changes.join("；"))
}

fn tapd_notification_output(item: &TapdWorkItem, change: &str) -> String {
    format!(
        "变更：{change}\n类型：{}\n标题：{}\n状态：{}\n优先级：{}\n负责人：{}\n截止时间：{}\n创建人：{}\n\n详细描述：\n{}",
        item_type_label(&item.item_type),
        item.title,
        display_value(&item.status_label),
        display_value(&item.priority),
        display_value(&item.owner),
        display_value(&item.due_date),
        display_value(&item.creator),
        if item.description.trim().is_empty() { "TAPD 未填写详细描述。" } else { &item.description },
    )
}

fn save_items(
    state: &DatabaseState,
    items: &[TapdWorkItem],
    create_notifications: bool,
) -> Result<SaveItemsOutcome, String> {
    let mut connection = state.connect()?;
    let existing = {
        let mut statement = connection
            .prepare("SELECT id,status_label,priority,owner,due_date FROM tapd_work_items WHERE workspace_id=?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([WORKSPACE_ID], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    TapdItemSnapshot {
                        status_label: row.get(1)?,
                        priority: row.get(2)?,
                        owner: row.get(3)?,
                        due_date: row.get(4)?,
                    },
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<BTreeMap<_, _>, _>>()
            .map_err(|error| error.to_string())?
    };
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut outcome = SaveItemsOutcome::default();
    for item in items {
        if create_notifications && !existing.contains_key(&item.id) && is_actionable_new_bug(item) {
            outcome.new_bug_ids.push(item.id.clone());
        }
        let change = create_notifications
            .then(|| tapd_change_summary(existing.get(&item.id), item))
            .flatten();
        transaction
            .execute(
                "INSERT INTO tapd_work_items(id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
                 ON CONFLICT(id) DO UPDATE SET workspace_id=excluded.workspace_id,item_type=excluded.item_type,title=excluded.title,description=excluded.description,status=excluded.status,status_label=excluded.status_label,priority=excluded.priority,owner=excluded.owner,creator=excluded.creator,iteration_id=excluded.iteration_id,begin_date=excluded.begin_date,due_date=excluded.due_date,created_at=excluded.created_at,modified_at=excluded.modified_at,source_url=excluded.source_url,synced_at=excluded.synced_at",
                params![item.id,item.workspace_id,item.item_type,item.title,item.description,item.status,item.status_label,item.priority,item.owner,item.creator,item.iteration_id,item.begin_date,item.due_date,item.created_at,item.modified_at,item.source_url,item.synced_at],
            )
            .map_err(|error| error.to_string())?;
        if let Some(change) = change {
            let version = if item.modified_at.trim().is_empty() {
                &item.synced_at
            } else {
                &item.modified_at
            };
            let body = format!(
                "{} · {} · 状态：{} · 截止：{}",
                WORKSPACE_NAME,
                change,
                display_value(&item.status_label),
                display_value(&item.due_date)
            );
            let inserted = transaction
                .execute(
                    "INSERT OR IGNORE INTO notifications(id,kind,title,body,output,source_id,route,is_read,review_status,created_at)
                     VALUES(?1,'tapd_item',?2,?3,?4,?5,?6,0,'accepted',?7)",
                    params![
                        format!("tapd-item:{}:{}:{}", item.item_type, item.id, version),
                        format!("TAPD {}：{}", item_type_label(&item.item_type), item.title),
                        body,
                        tapd_notification_output(item, &change),
                        item.id,
                        format!("/tapd?item={}", item.id),
                        item.synced_at,
                    ],
                )
                .map_err(|error| error.to_string())?;
            outcome.notifications_created += inserted;
        }
    }
    transaction
        .commit()
        .map_err(|error| error.to_string())
        .map(|_| outcome)
}

fn read_item(state: &DatabaseState, id: &str) -> Result<TapdWorkItem, String> {
    state.connect()?.query_row(
        "SELECT id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at FROM tapd_work_items WHERE id=?1",
        [id],
        |row| Ok(TapdWorkItem { id:row.get(0)?,workspace_id:row.get(1)?,item_type:row.get(2)?,title:row.get(3)?,description:row.get(4)?,status:row.get(5)?,status_label:row.get(6)?,priority:row.get(7)?,owner:row.get(8)?,creator:row.get(9)?,iteration_id:row.get(10)?,begin_date:row.get(11)?,due_date:row.get(12)?,created_at:row.get(13)?,modified_at:row.get(14)?,source_url:row.get(15)?,synced_at:row.get(16)? })
    ).optional().map_err(|error| error.to_string())?.ok_or_else(|| "没有找到该 TAPD 工作项，请先同步。".into())
}

fn git_output(repository: &Path, args: &[&str]) -> String {
    let mut command = codex_video::hidden_command(Path::new("git"));
    command.current_dir(repository).args(args);
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default()
}

fn worktree_paths(status: &str) -> BTreeSet<String> {
    status
        .lines()
        .filter_map(|line| {
            let value = line.get(3..)?.trim();
            let path = value.rsplit(" -> ").next().unwrap_or(value).trim();
            (!path.is_empty()).then(|| path.replace('\\', "/"))
        })
        .collect()
}

fn git_evidence(repository: &Path, baseline_head: &str) -> (String, Vec<String>) {
    let result_head = git_output(repository, &["rev-parse", "HEAD"]);
    let status = git_output(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let mut paths = worktree_paths(&status);
    if !baseline_head.is_empty() && !result_head.is_empty() && baseline_head != result_head {
        paths.extend(
            git_output(
                repository,
                &[
                    "diff",
                    "--name-only",
                    &format!("{baseline_head}..{result_head}"),
                ],
            )
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.replace('\\', "/")),
        );
    }
    (result_head, paths.into_iter().collect())
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<TapdCodexJob> {
    let changed_files: String = row.get(12)?;
    Ok(TapdCodexJob {
        id: row.get(0)?,
        item_id: row.get(1)?,
        repository_path: row.get(2)?,
        status: row.get(3)?,
        thread_id: row.get(4)?,
        output: row.get(5)?,
        error_message: row.get(6)?,
        baseline_head: row.get(7)?,
        baseline_worktree: row.get(8)?,
        result_head: row.get(9)?,
        test_summary: row.get(10)?,
        review_status: row.get(11)?,
        changed_files: serde_json::from_str(&changed_files).unwrap_or_default(),
        review_note: row.get(13)?,
        reviewed_at: row.get(14)?,
        trigger_source: row.get(15)?,
        process_report_path: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn read_job(state: &DatabaseState, id: &str) -> Result<TapdCodexJob, String> {
    state.connect()?.query_row(
        "SELECT id,item_id,repository_path,status,thread_id,output,error_message,baseline_head,baseline_worktree,result_head,test_summary,review_status,changed_files,review_note,reviewed_at,trigger_source,process_report_path,created_at,updated_at FROM tapd_codex_jobs WHERE id=?1",
        [id],
        row_to_job,
    ).map_err(|error| error.to_string())
}

fn process_report_file(state: &DatabaseState, job_id: &str) -> std::path::PathBuf {
    state
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tapd-codex-jobs")
        .join(job_id)
        .join("PROCESS.md")
}

fn initialize_process_report(
    state: &DatabaseState,
    job_id: &str,
    item: &TapdWorkItem,
    repository_path: &str,
    trigger_source: &str,
) -> Result<String, String> {
    let path = process_report_file(state, job_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let trigger = if trigger_source == "auto" {
        "新缺陷自动处理"
    } else {
        "人工发送"
    };
    let content = format!(
        "# TAPD 工作项处理报告\n\n- 工作项：{} #{}\n- 标题：{}\n- TAPD 项目：{}（{}）\n- 本地项目：{}\n- 触发方式：{}\n- 开始时间：{}\n\n## 固定处理流程\n\n1. 修改前静态验证\n2. 修改前动态真实接口或页面验证\n3. 确认问题后规划最小改动方案\n4. 实施最小范围修改\n5. 修改后静态复测\n6. 修改后动态真实接口或页面复测\n\n> 安全说明：实际接口认证只允许从当前 Windows 用户环境变量 `HLZT_TOKEN` 读取，且只作为 `hlzt-token` 请求头使用；报告不得记录令牌值。\n",
        item_type_label(&item.item_type),
        item.id,
        item.title,
        WORKSPACE_NAME,
        WORKSPACE_ID,
        repository_path,
        trigger,
        Utc::now().to_rfc3339(),
    );
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn append_process_report(
    state: &DatabaseState,
    job_id: &str,
    heading: &str,
    content: &str,
) -> Result<(), String> {
    let job = read_job(state, job_id)?;
    let path = if job.process_report_path.trim().is_empty() {
        process_report_file(state, job_id)
    } else {
        std::path::PathBuf::from(&job.process_report_path)
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    writeln!(
        file,
        "\n## {} · {}\n\n{}\n",
        heading,
        Utc::now().to_rfc3339(),
        redact_sensitive_text(content).trim()
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn read_tapd_process_report(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<String, String> {
    let job = read_job(&state, &id)?;
    let path = if job.process_report_path.trim().is_empty() {
        process_report_file(&state, &id)
    } else {
        std::path::PathBuf::from(job.process_report_path)
    };
    if !path.is_file() {
        return Err("该历史任务没有可查看的处理报告。".into());
    }
    fs::read_to_string(path).map_err(|error| format!("读取处理报告失败：{error}"))
}

fn build_codex_prompt(
    item: &TapdWorkItem,
    repository_path: &Path,
    additional_note: &str,
    commands: &RepositoryCommands,
) -> String {
    let additional_note = additional_note.trim();
    let supplement = if additional_note.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n用户补充说明（来自工作台）：\n{additional_note}\n\n请同时结合 TAPD 内容和用户补充说明执行。补充说明用于明确背景、范围和验收要求；如果两者存在冲突，不要自行扩大范围，请在最终说明中明确指出。"
        )
    };
    format!(
        "请严格按下面的分阶段流程处理这条 TAPD 工作项，并在指定本地项目中完成验证或最小范围修复。不得跳过阶段，也不得先改代码再补写修改前验证。\n\n项目目录：{}\nTAPD 项目：{}（{}）\n工作项类型：{}\n标题：{}\n状态：{}\n优先级：{}\n处理人：{}\n预计结束：{}\n来源：{}\n\n详细描述：\n{}{}\n\n项目资产中记录的命令（需先核对当前项目实际脚本再使用）：\n- 启动：{}\n- 测试：{}\n- 构建：{}\n\n必须执行的流程门槛：\n1. 修改前验证：先做静态验证，再通过项目已有功能、用例和真实接口或真实页面做动态验证。分别记录命令、关键证据和结论。\n2. 只有在问题被证明确实存在后，才规划最小改动方案；方案必须说明根因、修改边界和回归风险。若问题未复现，不得为了产生改动而修改代码。\n3. 执行方案：只修改与该缺陷直接相关的文件，遵循项目现有规范，不扩大功能范围。\n4. 修改后复测：再次完成静态测试和动态真实接口或页面测试，覆盖与修改前相同的复现路径，并记录结果。只有两类复测都通过，才可以判定问题已解决。\n5. 如果缺少运行环境、账号、页面入口或必要权限，必须如实标记“流程未完成”，说明缺失项；不得伪造动态测试或把静态检查冒充真实页面测试。\n\n安全生产管理项目的真实接口规则：\n- 认证只允许在执行时读取当前 Windows 用户环境变量 `HLZT_TOKEN`。\n- 仅将其作为 HTTP 请求头 `hlzt-token` 发送；禁止把实际令牌写入命令参数、源码、测试文件、Markdown、日志或最终回复，也不要回显该环境变量。\n- 不得把真实接口测试变成写入、删除或批量修改业务数据；优先使用项目已有只读查询和既有测试用例。\n\n通用安全边界：\n- 保留用户现有未提交改动，不要提交、推送、重置、清理或删除。\n- 不要修改与本工作项无关的配置、路由或业务逻辑。\n\n最终回复必须使用下面这些 Markdown 标题，写入每个阶段的真实过程和结果，不能只给笼统总结：\n## 1. 修改前验证\n### 静态验证\n### 动态真实接口或页面验证\n## 2. 最小改动方案\n## 3. 实施记录\n## 4. 修改后复测\n### 静态复测\n### 动态真实接口或页面复测\n## 5. 最终结论与人工确认项",
        repository_path.display(), WORKSPACE_NAME, WORKSPACE_ID, item.item_type, item.title,
        item.status_label, if item.priority.is_empty(){"未设置"}else{&item.priority},
        if item.owner.is_empty(){DEFAULT_OWNER}else{&item.owner},
        if item.due_date.is_empty(){"未设置"}else{&item.due_date}, item.source_url,
        if item.description.is_empty(){"TAPD 未填写详细描述，请结合标题和代码现状判断。"}else{&item.description},
        supplement,
        display_value(&commands.start),
        display_value(&commands.test),
        display_value(&commands.build),
    )
}

fn finish_job(
    state: &DatabaseState,
    job_id: &str,
    item: &TapdWorkItem,
    status: &str,
    thread_id: Option<&str>,
    output: &str,
    error: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let job = read_job(state, job_id)?;
    let output = redact_sensitive_text(output);
    let error = redact_sensitive_text(error);
    let repository = Path::new(&job.repository_path);
    let (result_head, changed_files) = git_evidence(repository, &job.baseline_head);
    let changed_files_json =
        serde_json::to_string(&changed_files).map_err(|cause| cause.to_string())?;
    state.connect()?.execute(
        "UPDATE tapd_codex_jobs SET status=?1,thread_id=COALESCE(?2,thread_id),output=?3,error_message=?4,result_head=?5,changed_files=?6,review_status='pending',updated_at=?7 WHERE id=?8",
        params![status,thread_id,output,error,result_head,changed_files_json,now,job_id],
    ).map_err(|cause| cause.to_string())?;
    let files = if changed_files.is_empty() {
        "- 未检测到文件变化。".to_string()
    } else {
        changed_files
            .iter()
            .map(|file| format!("- `{file}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let report_result = format!(
        "执行状态：{}\n\n### Codex 分阶段记录\n\n{}\n\n### 工作区证据\n\n- 起始 Git HEAD：`{}`\n- 结束 Git HEAD：`{}`\n- 启动前已有未提交改动：{}\n\n{}\n\n### 错误信息\n\n{}",
        if status == "completed" { "已完成，等待人工确认" } else { "未完成" },
        if output.trim().is_empty() { "Codex 没有返回最终说明。" } else { output.trim() },
        display_value(&job.baseline_head),
        display_value(&result_head),
        if job.baseline_worktree.trim().is_empty() { "否" } else { "是" },
        files,
        if error.trim().is_empty() { "无" } else { error.trim() },
    );
    append_process_report(state, job_id, "本轮处理结果", &report_result)?;
    let (title, body) = if status == "completed" {
        (
            format!("TAPD 任务已完成：{}", item.title),
            format!("{} · Codex 已完成，等待你确认结果", WORKSPACE_NAME),
        )
    } else {
        (
            format!("TAPD 任务需要处理：{}", item.title),
            format!("{} · Codex 执行失败", WORKSPACE_NAME),
        )
    };
    state.connect()?.execute(
        "INSERT INTO notifications(id,kind,title,body,output,source_id,route,is_read,created_at,read_at)
         VALUES(?1,'codex_task',?2,?3,?4,?5,?6,0,?7,NULL)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,output=excluded.output,is_read=0,created_at=excluded.created_at,read_at=NULL",
        params![format!("tapd-codex:{job_id}"),title,body,if output.is_empty(){&error}else{&output},job_id,format!("/tapd?item={}",item.id),now],
    ).map_err(|cause| cause.to_string())?;
    let _ = crate::codex::scan_codex_sessions_for_state(state);
    let _ = crate::git::scan_git_repositories_for_state(state);
    Ok(())
}

fn has_complete_process_report(output: &str) -> bool {
    [
        "## 1. 修改前验证",
        "### 静态验证",
        "### 动态真实接口或页面验证",
        "## 2. 最小改动方案",
        "## 3. 实施记录",
        "## 4. 修改后复测",
        "### 静态复测",
        "### 动态真实接口或页面复测",
        "## 5. 最终结论与人工确认项",
    ]
    .iter()
    .all(|heading| output.contains(heading))
}

fn execute_codex_job(
    state: DatabaseState,
    job_id: String,
    item: TapdWorkItem,
    repository_path: String,
    prompt: String,
    resume_thread_id: Option<String>,
) -> Result<(), String> {
    let repository = Path::new(&repository_path);
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    let job_dir = state
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tapd-codex-jobs")
        .join(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| error.to_string())?;
    let jsonl_path = job_dir.join("codex-run.jsonl");
    let stderr_path = job_dir.join("codex-stderr.log");
    let last_message_path = job_dir.join("codex-last-message.md");
    let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let mut command = codex_video::hidden_command(&cli_path);
    command
        .args(["--sandbox", "workspace-write", "--cd"])
        .arg(repository);
    if let Some(thread_id) = resume_thread_id.as_deref() {
        command
            .args(["exec", "resume", "--json", "--skip-git-repo-check"])
            .arg("--output-last-message")
            .arg(&last_message_path)
            .arg(thread_id)
            .arg("-");
    } else {
        command
            .args(["exec", "--json", "--skip-git-repo-check"])
            .arg("--output-last-message")
            .arg(&last_message_path)
            .arg("-");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file));
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Codex CLI 失败：{error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|error| format!("发送 TAPD 任务给 Codex 失败：{error}"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Codex 输出。".to_string())?;
    let mut log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&jsonl_path)
        .map_err(|error| error.to_string())?;
    let mut thread_id = None;
    let mut streamed_output = String::new();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        writeln!(log, "{}", redact_sensitive_text(&line)).map_err(|error| error.to_string())?;
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("type").and_then(Value::as_str) == Some("thread.started") {
                thread_id = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if value.get("type").and_then(Value::as_str) == Some("item.completed")
                && value.pointer("/item/type").and_then(Value::as_str) == Some("agent_message")
            {
                streamed_output = text(value.pointer("/item/text"));
            }
        }
    }
    let process_status = child.wait().map_err(|error| error.to_string())?;
    let final_output = redact_sensitive_text(
        &fs::read_to_string(&last_message_path)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(streamed_output),
    );
    let _ = fs::write(&last_message_path, &final_output);
    let sanitized_stderr =
        redact_sensitive_text(&fs::read_to_string(&stderr_path).unwrap_or_else(|_| String::new()));
    let _ = fs::write(&stderr_path, &sanitized_stderr);
    if process_status.success() && has_complete_process_report(&final_output) {
        finish_job(
            &state,
            &job_id,
            &item,
            "completed",
            thread_id.as_deref(),
            &final_output,
            "",
        )
    } else {
        let error = if process_status.success() {
            "Codex 已返回，但处理报告缺少固定流程章节，不能判定本次缺陷处理完整。".to_string()
        } else if sanitized_stderr.trim().is_empty() {
            "Codex CLI 执行失败。".into()
        } else {
            sanitized_stderr
        };
        let _ = fs::write(&stderr_path, &error);
        finish_job(
            &state,
            &job_id,
            &item,
            "failed",
            thread_id.as_deref(),
            &final_output,
            error.trim(),
        )
    }
}

fn run_codex_job(
    state: DatabaseState,
    job_id: String,
    item: TapdWorkItem,
    repository_path: String,
    additional_note: String,
) -> Result<(), String> {
    let commands = repository_commands(&state, &repository_path)?;
    let prompt = build_codex_prompt(
        &item,
        Path::new(&repository_path),
        &additional_note,
        &commands,
    );
    execute_codex_job(state, job_id, item, repository_path, prompt, None)
}

fn build_codex_follow_up_prompt(note: &str) -> String {
    format!(
        "上一轮 TAPD 修改尚未通过人工确认，请继续处理下面的补充要求：\n\n{}\n\n请基于当前项目里上一轮已经产生的修改继续完善，只修改与本次反馈直接相关的内容。修改前先复现本轮反馈；修改后必须同时完成静态复测和动态真实接口或页面复测。真实接口认证只允许读取当前 Windows 用户环境变量 `HLZT_TOKEN` 并仅作为 `hlzt-token` 请求头，禁止回显或留存实际令牌。不要提交、推送、重置、清理或删除用户现有改动。\n\n最终回复仍必须使用以下完整 Markdown 标题：\n## 1. 修改前验证\n### 静态验证\n### 动态真实接口或页面验证\n## 2. 最小改动方案\n## 3. 实施记录\n## 4. 修改后复测\n### 静态复测\n### 动态真实接口或页面复测\n## 5. 最终结论与人工确认项",
        note.trim()
    )
}

fn run_codex_follow_up_job(
    state: DatabaseState,
    job_id: String,
    item: TapdWorkItem,
    repository_path: String,
    thread_id: String,
    note: String,
) -> Result<(), String> {
    execute_codex_job(
        state,
        job_id,
        item,
        repository_path,
        build_codex_follow_up_prompt(&note),
        Some(thread_id),
    )
}

fn validate_repository(state: &DatabaseState, repository_path: &str) -> Result<(), String> {
    let repository = Path::new(repository_path);
    if !repository.is_dir() {
        return Err("选择的本地项目目录不存在。".into());
    }
    repository_commands(state, repository_path).map(|_| ())
}

fn ensure_codex_ready() -> Result<(), String> {
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    let login = codex_video::hidden_command(&cli_path)
        .args(["login", "status"])
        .output()
        .map_err(|error| error.to_string())?;
    if login.status.success() {
        Ok(())
    } else {
        Err("Codex CLI 尚未登录。".into())
    }
}

fn create_job_record(
    state: &DatabaseState,
    item: &TapdWorkItem,
    repository_path: &str,
    trigger_source: &str,
) -> Result<TapdCodexJob, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let repository = Path::new(repository_path);
    let baseline_head = git_output(repository, &["rev-parse", "HEAD"]);
    let baseline_worktree = git_output(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let report_path = initialize_process_report(state, &id, item, repository_path, trigger_source)?;
    state.connect()?.execute(
        "INSERT INTO tapd_codex_jobs(id,item_id,repository_path,status,thread_id,output,error_message,baseline_head,baseline_worktree,result_head,changed_files,test_summary,review_status,review_note,reviewed_at,trigger_source,process_report_path,created_at,updated_at) VALUES(?1,?2,?3,'queued',NULL,'','',?4,?5,'','[]','','pending','',NULL,?6,?7,?8,?8)",
        params![id,item.id,repository_path,baseline_head,baseline_worktree,trigger_source,report_path,now],
    ).map_err(|error| error.to_string())?;
    read_job(state, &id)
}

fn set_job_running(state: &DatabaseState, job_id: &str) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "UPDATE tapd_codex_jobs SET status='running',updated_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), job_id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn run_prepared_job(
    state: DatabaseState,
    job: TapdCodexJob,
    item: TapdWorkItem,
    additional_note: String,
) {
    let lock = AUTO_FIX_LOCK.get_or_init(|| Mutex::new(()));
    let Ok(_guard) = lock.lock() else {
        let _ = finish_job(
            &state,
            &job.id,
            &item,
            "failed",
            None,
            "",
            "TAPD Codex 串行执行锁不可用。",
        );
        return;
    };
    if let Err(error) = set_job_running(&state, &job.id) {
        let _ = finish_job(&state, &job.id, &item, "failed", None, "", &error);
        return;
    }
    if let Err(error) = run_codex_job(
        state.clone(),
        job.id.clone(),
        item.clone(),
        job.repository_path.clone(),
        additional_note,
    ) {
        let _ = finish_job(&state, &job.id, &item, "failed", None, "", &error);
    }
}

fn queue_auto_fix_jobs(
    state: &DatabaseState,
    bug_ids: &[String],
    repository_path: &str,
) -> Result<(usize, usize), String> {
    if bug_ids.is_empty() {
        return Ok((0, 0));
    }
    validate_repository(state, repository_path)?;
    ensure_codex_ready()?;
    let mut queued = Vec::new();
    let mut skipped = 0;
    for item_id in bug_ids {
        let already_exists: bool = state
            .connect()?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tapd_codex_jobs WHERE item_id=?1)",
                [item_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if already_exists {
            skipped += 1;
            continue;
        }
        let item = read_item(state, item_id)?;
        let job = create_job_record(state, &item, repository_path, "auto")?;
        queued.push((job, item));
    }
    let started = queued.len();
    if !queued.is_empty() {
        let task_state = state.clone();
        tauri::async_runtime::spawn_blocking(move || {
            for (job, item) in queued {
                run_prepared_job(task_state.clone(), job, item, String::new());
            }
        });
    }
    Ok((started, skipped))
}

#[tauri::command]
pub fn tapd_status(state: tauri::State<'_, DatabaseState>) -> TapdStatus {
    let configured = credentials();
    let auth_mode = configured
        .as_ref()
        .map(|(credential, _)| credential.mode().to_string())
        .unwrap_or_else(|_| "token".into());
    let connection = state.connect().ok();
    let item_count = connection
        .as_ref()
        .and_then(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM tapd_work_items WHERE workspace_id=?1",
                    [WORKSPACE_ID],
                    |row| row.get(0),
                )
                .ok()
        })
        .unwrap_or(0);
    let auto_fix = auto_fix_settings(&state);
    TapdStatus {
        configured: configured.is_ok(),
        source: configured
            .map(|(_, source)| source)
            .unwrap_or_else(|_| "未配置".into()),
        auth_mode,
        workspace_id: WORKSPACE_ID.into(),
        workspace_name: WORKSPACE_NAME.into(),
        owner: owner(&state),
        last_synced_at: app_meta(&state, "tapd_last_synced_at"),
        item_count,
        warnings: permission_warnings(&state),
        auto_fix_enabled: auto_fix.enabled,
        auto_fix_repository_path: auto_fix.repository_path,
    }
}

#[tauri::command]
pub fn save_tapd_auto_fix_settings(
    state: tauri::State<'_, DatabaseState>,
    enabled: bool,
    repository_path: String,
) -> Result<TapdAutoFixSettings, String> {
    let repository_path = repository_path.trim().to_string();
    if enabled {
        if repository_path.is_empty() {
            return Err("开启自动修改前，请先选择要处理缺陷的本地项目。".into());
        }
        validate_repository(&state, &repository_path)?;
        ensure_codex_ready()?;
    } else if !repository_path.is_empty() {
        validate_repository(&state, &repository_path)?;
    }
    set_app_meta(
        &state,
        AUTO_FIX_ENABLED_KEY,
        if enabled { "true" } else { "false" },
    )?;
    set_app_meta(&state, AUTO_FIX_REPOSITORY_KEY, &repository_path)?;
    Ok(TapdAutoFixSettings {
        enabled,
        repository_path,
    })
}

#[tauri::command]
pub fn save_tapd_credentials(
    state: tauri::State<'_, DatabaseState>,
    auth_mode: String,
    api_user: String,
    api_password: String,
    access_token: String,
    owner: String,
) -> Result<(), String> {
    let auth_mode = auth_mode.trim();
    let api_user = api_user.trim();
    let api_password = api_password.trim();
    let access_token = access_token.trim();
    if auth_mode == "token" && access_token.is_empty() {
        return Err("请输入有效的 TAPD 个人访问令牌。".into());
    }
    if auth_mode != "token" && (api_user.is_empty() || api_password.len() < 4) {
        return Err("请输入有效的 TAPD API 用户名和密码。".into());
    }
    let raw = serde_json::to_string(&TapdCredential {
        auth_mode: if auth_mode == "token" {
            "token"
        } else {
            "basic"
        }
        .into(),
        api_user: api_user.into(),
        api_password: api_password.into(),
        access_token: access_token.into(),
    })
    .map_err(|error| error.to_string())?;
    credential_entry()?
        .set_password(&raw)
        .map_err(|error| error.to_string())?;
    state.connect()?.execute(
        "INSERT INTO app_meta(key,value) VALUES('tapd_owner',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [if owner.trim().is_empty(){DEFAULT_OWNER}else{owner.trim()}],
    ).map_err(|error| error.to_string())?;
    save_permission_warnings(&state, &[])?;
    Ok(())
}

#[tauri::command]
pub fn clear_tapd_credentials(state: tauri::State<'_, DatabaseState>) -> Result<(), String> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            save_permission_warnings(&state, &[])?;
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn test_tapd_connection(
    state: tauri::State<'_, DatabaseState>,
) -> Result<String, String> {
    let (credential, _) = credentials()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let owner = owner(&state);
    let checked_at = Utc::now().to_rfc3339();
    let mut readable = Vec::new();
    let mut warnings = Vec::new();
    for (item_type, label) in [("bug", "缺陷"), ("task", "任务"), ("story", "需求")] {
        match fetch_type(&client, &credential, item_type, &owner, &checked_at).await {
            Ok(items) => readable.push(format!("{label} {} 条", items.len())),
            Err(error) => warnings.push(readable_permission_error(item_type, &error)),
        }
    }
    save_permission_warnings(&state, &warnings)?;
    if readable.is_empty() {
        return Err(format!("TAPD 连接失败：{}", warnings.join("；")));
    }
    if warnings.is_empty() {
        Ok(format!(
            "连接成功，已验证项目 {}：{}。",
            WORKSPACE_NAME,
            readable.join("、")
        ))
    } else {
        Ok(format!(
            "连接成功，但令牌权限不完整。当前可读取：{}；{}。已有权限的数据仍可正常同步。",
            readable.join("、"),
            warnings.join("；")
        ))
    }
}

#[tauri::command]
pub async fn sync_tapd_items(
    state: tauri::State<'_, DatabaseState>,
) -> Result<TapdSyncSummary, String> {
    let database = state.inner().clone();
    let (credential, _) = credentials()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|error| error.to_string())?;
    let synced_at = Utc::now().to_rfc3339();
    let owner = owner(&database);
    let mut warnings = Vec::new();
    let bugs = match fetch_type(&client, &credential, "bug", &owner, &synced_at).await {
        Ok(items) => items,
        Err(error) => {
            warnings.push(readable_permission_error("bug", &error));
            Vec::new()
        }
    };
    let tasks = match fetch_type(&client, &credential, "task", &owner, &synced_at).await {
        Ok(items) => items,
        Err(error) => {
            warnings.push(readable_permission_error("task", &error));
            Vec::new()
        }
    };
    let stories = match fetch_type(&client, &credential, "story", &owner, &synced_at).await {
        Ok(items) => items,
        Err(error) => {
            warnings.push(readable_permission_error("story", &error));
            Vec::new()
        }
    };
    let mut all = Vec::new();
    all.extend(bugs.iter().cloned());
    all.extend(tasks.iter().cloned());
    all.extend(stories.iter().cloned());
    if all.is_empty() && warnings.len() == 3 {
        save_permission_warnings(&database, &warnings)?;
        return Err(format!(
            "TAPD 三类工作项均无法读取：{}",
            warnings.join("；")
        ));
    }
    let create_notifications = app_meta(&database, "tapd_last_synced_at").is_some();
    let saved = save_items(&database, &all, create_notifications)?;
    save_permission_warnings(&database, &warnings)?;
    database.connect()?.execute("INSERT INTO app_meta(key,value) VALUES('tapd_last_synced_at',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&synced_at]).map_err(|error| error.to_string())?;
    let auto_fix = auto_fix_settings(&database);
    let (auto_jobs_started, auto_jobs_skipped) = if auto_fix.enabled {
        match queue_auto_fix_jobs(&database, &saved.new_bug_ids, &auto_fix.repository_path) {
            Ok(result) => result,
            Err(error) => {
                warnings.push(format!("新缺陷自动处理未启动：{error}"));
                (0, saved.new_bug_ids.len())
            }
        }
    } else {
        (0, 0)
    };
    Ok(TapdSyncSummary {
        bugs: bugs.len(),
        tasks: tasks.len(),
        stories: stories.len(),
        total: all.len(),
        notifications_created: saved.notifications_created,
        warnings,
        synced_at,
        auto_jobs_started,
        auto_jobs_skipped,
    })
}

#[tauri::command]
pub fn list_tapd_items(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<TapdWorkItem>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare("SELECT id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at FROM tapd_work_items WHERE workspace_id=?1 ORDER BY modified_at DESC,created_at DESC").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([WORKSPACE_ID], |row| {
            Ok(TapdWorkItem {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                item_type: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                status: row.get(5)?,
                status_label: row.get(6)?,
                priority: row.get(7)?,
                owner: row.get(8)?,
                creator: row.get(9)?,
                iteration_id: row.get(10)?,
                begin_date: row.get(11)?,
                due_date: row.get(12)?,
                created_at: row.get(13)?,
                modified_at: row.get(14)?,
                source_url: row.get(15)?,
                synced_at: row.get(16)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_tapd_codex_jobs(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<TapdCodexJob>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare("SELECT id,item_id,repository_path,status,thread_id,output,error_message,baseline_head,baseline_worktree,result_head,test_summary,review_status,changed_files,review_note,reviewed_at,trigger_source,process_report_path,created_at,updated_at FROM tapd_codex_jobs ORDER BY created_at DESC LIMIT 100").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row_to_job(row))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_tapd_codex_job(
    state: tauri::State<'_, DatabaseState>,
    item_id: String,
    repository_path: String,
    additional_note: String,
) -> Result<TapdCodexJob, String> {
    let database = state.inner().clone();
    let item = read_item(&database, &item_id)?;
    validate_repository(&database, &repository_path)?;
    if additional_note.chars().count() > 4_000 {
        return Err("补充备注不能超过 4000 个字符。".into());
    }
    ensure_codex_ready()?;
    let job = create_job_record(&database, &item, &repository_path, "manual")?;
    let task_state = database.clone();
    let task_job = job.clone();
    let task_item = item.clone();
    let task_note = additional_note.trim().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        run_prepared_job(task_state, task_job, task_item, task_note);
    });
    Ok(job)
}

#[tauri::command]
pub fn continue_tapd_codex_job(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    note: String,
) -> Result<TapdCodexJob, String> {
    let note = note.trim().to_string();
    if note.is_empty() {
        return Err("请先填写需要继续修改的说明。".into());
    }
    if note.chars().count() > 4_000 {
        return Err("继续修改说明不能超过 4000 个字符。".into());
    }

    let database = state.inner().clone();
    let job = read_job(&database, &id)?;
    if job.status != "completed" {
        return Err("只有已完成的 Codex 结果可以继续修改。".into());
    }
    let thread_id = job
        .thread_id
        .clone()
        .ok_or_else(|| "这次执行没有可恢复的 Codex 会话，请重新发送工作项。".to_string())?;
    let item = read_item(&database, &job.item_id)?;
    validate_repository(&database, &job.repository_path)?;
    ensure_codex_ready()?;
    append_process_report(&database, &id, "人工反馈：继续修改", &note)?;

    let now = Utc::now().to_rfc3339();
    database
        .connect()?
        .execute(
            "UPDATE tapd_codex_jobs SET status='queued',output='',error_message='',test_summary='',review_status='pending',review_note=?1,reviewed_at=NULL,updated_at=?2 WHERE id=?3",
            params![note, now, id],
        )
        .map_err(|error| error.to_string())?;

    let task_state = database.clone();
    let task_id = id.clone();
    let task_repo = job.repository_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lock = AUTO_FIX_LOCK.get_or_init(|| Mutex::new(()));
        let Ok(_guard) = lock.lock() else {
            let _ = finish_job(
                &task_state,
                &task_id,
                &item,
                "failed",
                None,
                "",
                "TAPD Codex 串行执行锁不可用。",
            );
            return;
        };
        if let Err(error) = set_job_running(&task_state, &task_id) {
            let _ = finish_job(&task_state, &task_id, &item, "failed", None, "", &error);
            return;
        }
        if let Err(error) = run_codex_follow_up_job(
            task_state.clone(),
            task_id.clone(),
            item.clone(),
            task_repo,
            thread_id,
            note,
        ) {
            let _ = finish_job(&task_state, &task_id, &item, "failed", None, "", &error);
        }
    });
    read_job(&database, &id)
}

#[tauri::command]
pub fn run_tapd_codex_job_tests(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<TapdCodexJob, String> {
    let database = state.inner().clone();
    let job = read_job(&database, &id)?;
    if job.status != "completed" {
        return Err("Codex 尚未完成，暂时不能运行项目测试。".into());
    }
    let test_command = database
        .connect()?
        .query_row(
            "SELECT test_command FROM repository_assets WHERE path=?1",
            [&job.repository_path],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| error.to_string())?;
    if test_command.trim().is_empty() {
        return Err("该项目资产尚未配置测试命令，请先在项目资产中确认 testCommand。".into());
    }
    let job_dir = database
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("tapd-codex-jobs")
        .join(&id);
    fs::create_dir_all(&job_dir).map_err(|error| error.to_string())?;
    let stdout_path = job_dir.join("project-test-stdout.log");
    let stderr_path = job_dir.join("project-test-stderr.log");
    let stdout = File::create(&stdout_path).map_err(|error| error.to_string())?;
    let stderr = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let mut command = codex_video::hidden_command(Path::new("cmd"));
    command
        .current_dir(&job.repository_path)
        .args(["/d", "/s", "/c", &test_command])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动项目测试失败：{error}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(15 * 60) {
            let _ = child.kill();
            return Err(
                "项目测试超过 15 分钟，已停止。请在项目资产中改用更聚焦的测试命令。".into(),
            );
        }
        thread::sleep(Duration::from_millis(250));
    };
    let stdout = fs::read_to_string(stdout_path).unwrap_or_default();
    let stderr = fs::read_to_string(stderr_path).unwrap_or_default();
    let excerpt = format!("{}\n{}", stdout, stderr)
        .chars()
        .rev()
        .take(5_000)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    let summary = format!(
        "{}\n命令：{}\n{}",
        if status.success() {
            "项目测试通过"
        } else {
            "项目测试失败"
        },
        test_command,
        excerpt.trim()
    );
    database
        .connect()?
        .execute(
            "UPDATE tapd_codex_jobs SET test_summary=?1,updated_at=?2 WHERE id=?3",
            params![summary, Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
    append_process_report(&database, &id, "工作台补充项目测试", &summary)?;
    read_job(&database, &id)
}

async fn mark_tapd_bug_resolved(state: &DatabaseState, item: &TapdWorkItem) -> Result<(), String> {
    if item.item_type != "bug" {
        return Err(
            "当前仅支持把 TAPD 缺陷更新为“已解决”；任务和需求需要按各自工作流单独配置完成状态。"
                .into(),
        );
    }
    if item.status == "resolved" || item.status_label == "已解决" {
        return Ok(());
    }

    let (credential, _) = credentials()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let response =
        authenticated_post_request(&client, &credential, format!("{TAPD_API_ROOT}/bugs"))
            .form(&[
                ("workspace_id", WORKSPACE_ID),
                ("id", item.id.as_str()),
                ("v_status", "已解决"),
            ])
            .send()
            .await
            .map_err(|error| format!("更新 TAPD 缺陷状态失败：{error}"))?;
    let http_status = response.status();
    let body = response
        .json::<Value>()
        .await
        .map_err(|error| format!("解析 TAPD 更新结果失败：{error}"))?;
    if !http_status.is_success() || body.get("status").and_then(Value::as_i64) != Some(1) {
        let info = text(body.get("info"));
        let detail = if info.is_empty() {
            format!("HTTP {http_status}")
        } else {
            info
        };
        if detail.to_ascii_lowercase().contains("no permission") {
            return Err("TAPD 更新失败：个人访问令牌缺少 bug#write 权限，未执行本地归档。".into());
        }
        return Err(format!(
            "TAPD 缺陷无法流转为“已解决”：{detail}；未执行本地归档。"
        ));
    }

    let now = Utc::now().to_rfc3339();
    state
        .connect()?
        .execute(
            "UPDATE tapd_work_items SET status='resolved',status_label='已解决',modified_at=?1,synced_at=?1 WHERE id=?2",
            params![now, item.id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn review_tapd_codex_job(
    state: tauri::State<'_, DatabaseState>,
    review: TapdJobReview,
) -> Result<TapdCodexJob, String> {
    if !["accepted", "changes_requested"].contains(&review.decision.as_str()) {
        return Err("无效的确认结论。".into());
    }
    let database = state.inner().clone();
    let job = read_job(&database, &review.id)?;
    if job.status != "completed" {
        return Err("只有已完成的 Codex 结果可以确认。".into());
    }
    if review.decision == "accepted" {
        let item = read_item(&database, &job.item_id)?;
        mark_tapd_bug_resolved(&database, &item).await?;
        crate::codex::scan_codex_sessions_for_state(&database)?;
        crate::git::scan_git_repositories_for_state(&database)?;
        crate::reports::refresh_today_daily_for_state(&database)?;
        crate::knowledge::sync_knowledge_for_state(&database)?;
    }
    let now = Utc::now().to_rfc3339();
    database
        .connect()?
        .execute(
            "UPDATE tapd_codex_jobs SET review_status=?1,review_note=?2,reviewed_at=?3,updated_at=?3 WHERE id=?4",
            params![review.decision, review.note.trim(), now, review.id],
        )
        .map_err(|error| error.to_string())?;
    database
        .connect()?
        .execute(
            "UPDATE notifications SET is_read=1,read_at=?1 WHERE id=?2",
            params![now, format!("tapd-codex:{}", review.id)],
        )
        .map_err(|error| error.to_string())?;
    read_job(&database, &review.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_supports_personal_token_and_legacy_basic_json() {
        let token: TapdCredential = serde_json::from_str(
            r#"{"authMode":"token","accessToken":"example-token","apiUser":"","apiPassword":""}"#,
        )
        .unwrap();
        assert_eq!(token.mode(), "token");
        let legacy: TapdCredential =
            serde_json::from_str(r#"{"apiUser":"user","apiPassword":"password"}"#).unwrap();
        assert_eq!(legacy.mode(), "basic");
    }

    #[test]
    fn permission_error_is_presented_in_readable_chinese() {
        let message = readable_permission_error("task", "TAPD tasks: no permission");
        assert!(message.contains("任务接口无权限"));
        assert!(message.contains("task#read"));
    }

    #[test]
    fn bug_payload_is_normalized_for_the_workbench() {
        let value = serde_json::json!({"Bug":{"id":"1","workspace_id":WORKSPACE_ID,"title":"安全责任书统计","description":"<p>详情只显示一条</p>","status":"new","current_owner":DEFAULT_OWNER}});
        let item = normalize_item("bug", &value, "now").unwrap();
        assert_eq!(item.title, "安全责任书统计");
        assert_eq!(item.status_label, "待处理");
        assert_eq!(item.description, "详情只显示一条");
    }

    #[test]
    fn tapd_notification_only_reports_new_or_meaningful_changes() {
        let item = TapdWorkItem {
            id: "1".into(),
            workspace_id: WORKSPACE_ID.into(),
            item_type: "task".into(),
            title: "整改安全标识".into(),
            description: "修复页面显示问题".into(),
            status: "doing".into(),
            status_label: "进行中".into(),
            priority: "高".into(),
            owner: DEFAULT_OWNER.into(),
            creator: "管理员".into(),
            iteration_id: String::new(),
            begin_date: String::new(),
            due_date: "2026-08-12".into(),
            created_at: String::new(),
            modified_at: "2026-08-10T14:00:00Z".into(),
            source_url: "https://www.tapd.cn".into(),
            synced_at: "2026-08-10T14:01:00Z".into(),
        };
        assert_eq!(
            tapd_change_summary(None, &item).as_deref(),
            Some("新任务已分配给你")
        );
        let unchanged = TapdItemSnapshot {
            status_label: item.status_label.clone(),
            priority: item.priority.clone(),
            owner: item.owner.clone(),
            due_date: item.due_date.clone(),
        };
        assert!(tapd_change_summary(Some(&unchanged), &item).is_none());
        let changed = TapdItemSnapshot {
            status_label: "待处理".into(),
            priority: item.priority.clone(),
            owner: item.owner.clone(),
            due_date: "2026-08-11".into(),
        };
        let summary = tapd_change_summary(Some(&changed), &item).unwrap();
        assert!(summary.contains("状态：待处理 → 进行中"));
        assert!(summary.contains("截止时间：2026-08-11 → 2026-08-12"));
    }

    #[test]
    fn tapd_sync_baseline_does_not_create_history_noise_and_changes_are_deduplicated() {
        let directory = std::env::temp_dir().join(format!("tapd-notifications-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let mut item = TapdWorkItem {
            id: "task-1".into(),
            workspace_id: WORKSPACE_ID.into(),
            item_type: "task".into(),
            title: "整改安全标识".into(),
            description: "修复页面显示问题".into(),
            status: "open".into(),
            status_label: "待处理".into(),
            priority: "高".into(),
            owner: DEFAULT_OWNER.into(),
            creator: "管理员".into(),
            iteration_id: String::new(),
            begin_date: String::new(),
            due_date: "2026-08-12".into(),
            created_at: "2026-08-10T12:00:00Z".into(),
            modified_at: "2026-08-10T12:00:00Z".into(),
            source_url: "https://www.tapd.cn".into(),
            synced_at: "2026-08-10T12:01:00Z".into(),
        };
        let baseline = save_items(&state, &[item.clone()], false).unwrap();
        assert_eq!(baseline.notifications_created, 0);
        assert!(baseline.new_bug_ids.is_empty());
        item.status = "doing".into();
        item.status_label = "进行中".into();
        item.modified_at = "2026-08-10T13:00:00Z".into();
        item.synced_at = "2026-08-10T13:01:00Z".into();
        assert_eq!(
            save_items(&state, &[item.clone()], true)
                .unwrap()
                .notifications_created,
            1
        );
        assert_eq!(
            save_items(&state, &[item], true)
                .unwrap()
                .notifications_created,
            0
        );
        let notification: (String, String, i64) = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT kind,title,is_read FROM notifications LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(notification.0, "tapd_item");
        assert_eq!(notification.1, "TAPD 任务：整改安全标识");
        assert_eq!(notification.2, 0);
        drop(state);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn only_new_actionable_bugs_become_auto_fix_candidates() {
        let directory = std::env::temp_dir().join(format!("tapd-auto-fix-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let baseline = TapdWorkItem {
            id: "bug-baseline".into(),
            workspace_id: WORKSPACE_ID.into(),
            item_type: "bug".into(),
            title: "历史缺陷".into(),
            description: String::new(),
            status: "new".into(),
            status_label: "待处理".into(),
            priority: String::new(),
            owner: DEFAULT_OWNER.into(),
            creator: String::new(),
            iteration_id: String::new(),
            begin_date: String::new(),
            due_date: String::new(),
            created_at: "2026-08-20T12:00:00Z".into(),
            modified_at: "2026-08-20T12:00:00Z".into(),
            source_url: "https://www.tapd.cn".into(),
            synced_at: "2026-08-20T12:01:00Z".into(),
        };
        let first = save_items(&state, &[baseline.clone()], false).unwrap();
        assert!(first.new_bug_ids.is_empty());

        let mut new_bug = baseline.clone();
        new_bug.id = "bug-new".into();
        new_bug.title = "新缺陷".into();
        new_bug.modified_at = "2026-08-21T12:00:00Z".into();
        new_bug.synced_at = "2026-08-21T12:01:00Z".into();
        let second = save_items(&state, &[baseline, new_bug.clone()], true).unwrap();
        assert_eq!(second.new_bug_ids, vec!["bug-new"]);
        let repeated = save_items(&state, &[new_bug], true).unwrap();
        assert!(repeated.new_bug_ids.is_empty());
        drop(state);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn job_record_creates_a_clickable_markdown_process_report() {
        let directory = std::env::temp_dir().join(format!("tapd-report-{}", Uuid::new_v4()));
        let repository = directory.join("client");
        std::fs::create_dir_all(&repository).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let item = TapdWorkItem {
            id: "bug-report".into(),
            workspace_id: WORKSPACE_ID.into(),
            item_type: "bug".into(),
            title: "详情按钮溢出".into(),
            description: "按钮在窄屏下超出卡片".into(),
            status: "new".into(),
            status_label: "待处理".into(),
            priority: "高".into(),
            owner: DEFAULT_OWNER.into(),
            creator: String::new(),
            iteration_id: String::new(),
            begin_date: String::new(),
            due_date: String::new(),
            created_at: "2026-08-21T12:00:00Z".into(),
            modified_at: "2026-08-21T12:00:00Z".into(),
            source_url: "https://www.tapd.cn".into(),
            synced_at: "2026-08-21T12:01:00Z".into(),
        };
        save_items(&state, &[item.clone()], false).unwrap();
        let job = create_job_record(
            &state,
            &item,
            repository.to_string_lossy().as_ref(),
            "manual",
        )
        .unwrap();
        assert_eq!(job.status, "queued");
        assert_eq!(job.trigger_source, "manual");
        let report = std::fs::read_to_string(&job.process_report_path).unwrap();
        assert!(report.contains("# TAPD 工作项处理报告"));
        assert!(report.contains("修改前动态真实接口或页面验证"));
        assert!(report.contains("HLZT_TOKEN"));
        drop(state);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn codex_prompt_forbids_unrequested_git_actions() {
        let item = TapdWorkItem {
            id: "1".into(),
            workspace_id: WORKSPACE_ID.into(),
            item_type: "bug".into(),
            title: "测试".into(),
            description: "描述".into(),
            status: "new".into(),
            status_label: "待处理".into(),
            priority: "高".into(),
            owner: DEFAULT_OWNER.into(),
            creator: String::new(),
            iteration_id: String::new(),
            begin_date: String::new(),
            due_date: String::new(),
            created_at: String::new(),
            modified_at: String::new(),
            source_url: "https://www.tapd.cn".into(),
            synced_at: String::new(),
        };
        let prompt = build_codex_prompt(
            &item,
            Path::new(r"F:\TB-project\client"),
            "附件弹窗需要支持图片预览，并保持现有权限控制。",
            &RepositoryCommands::default(),
        );
        assert!(prompt.contains("不要提交、推送、重置、清理或删除"));
        assert!(prompt.contains("安全生产管理"));
        assert!(prompt.contains("用户补充说明（来自工作台）"));
        assert!(prompt.contains("附件弹窗需要支持图片预览"));
        assert!(prompt.contains("修改前验证"));
        assert!(prompt.contains("静态验证"));
        assert!(prompt.contains("动态真实接口或页面复测"));
        assert!(prompt.contains("HLZT_TOKEN"));
        assert!(prompt.contains("hlzt-token"));
        let prompt_without_note = build_codex_prompt(
            &item,
            Path::new(r"F:\TB-project\client"),
            "   ",
            &RepositoryCommands::default(),
        );
        assert!(!prompt_without_note.contains("用户补充说明（来自工作台）"));
    }

    #[test]
    fn complete_process_report_requires_every_workflow_heading() {
        let report = "## 1. 修改前验证\n### 静态验证\n### 动态真实接口或页面验证\n## 2. 最小改动方案\n## 3. 实施记录\n## 4. 修改后复测\n### 静态复测\n### 动态真实接口或页面复测\n## 5. 最终结论与人工确认项";
        assert!(has_complete_process_report(report));
        assert!(!has_complete_process_report(
            "## 1. 修改前验证\n### 静态验证"
        ));
    }

    #[test]
    fn codex_follow_up_prompt_contains_review_note_and_safety_boundary() {
        let prompt = build_codex_follow_up_prompt("按钮仍然会溢出，请限制标题宽度。");
        assert!(prompt.contains("按钮仍然会溢出"));
        assert!(prompt.contains("基于当前项目里上一轮已经产生的修改继续完善"));
        assert!(prompt.contains("不要提交、推送、重置、清理或删除"));
    }
}
