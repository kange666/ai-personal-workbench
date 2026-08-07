use crate::{codex_video, database::DatabaseState};
use chrono::Utc;
use keyring::Entry;
use reqwest::{Client, RequestBuilder};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Stdio,
    time::Duration,
};
use uuid::Uuid;

const CREDENTIAL_SERVICE: &str = "AI Personal Workbench";
const CREDENTIAL_USER: &str = "tapd-openapi";
const TAPD_API_ROOT: &str = "https://api.tapd.cn";
const WORKSPACE_ID: &str = "37583308";
const WORKSPACE_NAME: &str = "安全生产管理";
const DEFAULT_OWNER: &str = "刘子世康";
const PAGE_LIMIT: usize = 200;

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
    warnings: Vec<String>,
    synced_at: String,
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
    created_at: String,
    updated_at: String,
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

fn save_items(state: &DatabaseState, items: &[TapdWorkItem]) -> Result<(), String> {
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for item in items {
        transaction
            .execute(
                "INSERT INTO tapd_work_items(id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
                 ON CONFLICT(id) DO UPDATE SET workspace_id=excluded.workspace_id,item_type=excluded.item_type,title=excluded.title,description=excluded.description,status=excluded.status,status_label=excluded.status_label,priority=excluded.priority,owner=excluded.owner,creator=excluded.creator,iteration_id=excluded.iteration_id,begin_date=excluded.begin_date,due_date=excluded.due_date,created_at=excluded.created_at,modified_at=excluded.modified_at,source_url=excluded.source_url,synced_at=excluded.synced_at",
                params![item.id,item.workspace_id,item.item_type,item.title,item.description,item.status,item.status_label,item.priority,item.owner,item.creator,item.iteration_id,item.begin_date,item.due_date,item.created_at,item.modified_at,item.source_url,item.synced_at],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn read_item(state: &DatabaseState, id: &str) -> Result<TapdWorkItem, String> {
    state.connect()?.query_row(
        "SELECT id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at FROM tapd_work_items WHERE id=?1",
        [id],
        |row| Ok(TapdWorkItem { id:row.get(0)?,workspace_id:row.get(1)?,item_type:row.get(2)?,title:row.get(3)?,description:row.get(4)?,status:row.get(5)?,status_label:row.get(6)?,priority:row.get(7)?,owner:row.get(8)?,creator:row.get(9)?,iteration_id:row.get(10)?,begin_date:row.get(11)?,due_date:row.get(12)?,created_at:row.get(13)?,modified_at:row.get(14)?,source_url:row.get(15)?,synced_at:row.get(16)? })
    ).optional().map_err(|error| error.to_string())?.ok_or_else(|| "没有找到该 TAPD 工作项，请先同步。".into())
}

fn read_job(state: &DatabaseState, id: &str) -> Result<TapdCodexJob, String> {
    state.connect()?.query_row(
        "SELECT id,item_id,repository_path,status,thread_id,output,error_message,created_at,updated_at FROM tapd_codex_jobs WHERE id=?1",
        [id],
        |row| Ok(TapdCodexJob{id:row.get(0)?,item_id:row.get(1)?,repository_path:row.get(2)?,status:row.get(3)?,thread_id:row.get(4)?,output:row.get(5)?,error_message:row.get(6)?,created_at:row.get(7)?,updated_at:row.get(8)?})
    ).map_err(|error| error.to_string())
}

fn build_codex_prompt(item: &TapdWorkItem, repository_path: &Path) -> String {
    format!(
        "请处理下面这条 TAPD 工作项，并在指定本地项目中完成实现或修复。\n\n项目目录：{}\nTAPD 项目：{}（{}）\n工作项类型：{}\n标题：{}\n状态：{}\n优先级：{}\n处理人：{}\n预计结束：{}\n来源：{}\n\n详细描述：\n{}\n\n执行要求：\n1. 先检查项目现状和相关代码，确认问题根因。\n2. 只修改与该工作项直接相关的文件，遵循项目现有规范。\n3. 完成后运行风险相称的构建或测试。\n4. 不要提交、推送、重置、清理或删除用户现有改动。\n5. 最终用中文说明：做了什么、修改文件、验证结果、仍需人工确认的内容。",
        repository_path.display(), WORKSPACE_NAME, WORKSPACE_ID, item.item_type, item.title,
        item.status_label, if item.priority.is_empty(){"未设置"}else{&item.priority},
        if item.owner.is_empty(){DEFAULT_OWNER}else{&item.owner},
        if item.due_date.is_empty(){"未设置"}else{&item.due_date}, item.source_url,
        if item.description.is_empty(){"TAPD 未填写详细描述，请结合标题和代码现状判断。"}else{&item.description},
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
    state.connect()?.execute(
        "UPDATE tapd_codex_jobs SET status=?1,thread_id=COALESCE(?2,thread_id),output=?3,error_message=?4,updated_at=?5 WHERE id=?6",
        params![status,thread_id,output,error,now,job_id],
    ).map_err(|cause| cause.to_string())?;
    let (title, body) = if status == "completed" {
        (
            format!("TAPD 任务已完成：{}", item.title),
            format!("{} · Codex 已完成处理", WORKSPACE_NAME),
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
        params![format!("tapd-codex:{job_id}"),title,body,if output.is_empty(){error}else{output},job_id,format!("/tapd?item={}",item.id),now],
    ).map_err(|cause| cause.to_string())?;
    Ok(())
}

fn run_codex_job(
    state: DatabaseState,
    job_id: String,
    item: TapdWorkItem,
    repository_path: String,
) -> Result<(), String> {
    let repository = Path::new(&repository_path);
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    let prompt = build_codex_prompt(&item, repository);
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
        .arg(repository)
        .args(["exec", "--json", "--skip-git-repo-check"])
        .arg("--output-last-message")
        .arg(&last_message_path)
        .arg("-")
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
        writeln!(log, "{line}").map_err(|error| error.to_string())?;
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
    let final_output = fs::read_to_string(&last_message_path)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(streamed_output);
    if process_status.success() {
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
        let error =
            fs::read_to_string(&stderr_path).unwrap_or_else(|_| "Codex CLI 执行失败。".into());
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
    }
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
    save_items(&database, &all)?;
    save_permission_warnings(&database, &warnings)?;
    database.connect()?.execute("INSERT INTO app_meta(key,value) VALUES('tapd_last_synced_at',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&synced_at]).map_err(|error| error.to_string())?;
    Ok(TapdSyncSummary {
        bugs: bugs.len(),
        tasks: tasks.len(),
        stories: stories.len(),
        total: all.len(),
        warnings,
        synced_at,
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
    let mut statement = connection.prepare("SELECT id,item_id,repository_path,status,thread_id,output,error_message,created_at,updated_at FROM tapd_codex_jobs ORDER BY created_at DESC LIMIT 100").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(TapdCodexJob {
                id: row.get(0)?,
                item_id: row.get(1)?,
                repository_path: row.get(2)?,
                status: row.get(3)?,
                thread_id: row.get(4)?,
                output: row.get(5)?,
                error_message: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_tapd_codex_job(
    state: tauri::State<'_, DatabaseState>,
    item_id: String,
    repository_path: String,
) -> Result<TapdCodexJob, String> {
    let database = state.inner().clone();
    let item = read_item(&database, &item_id)?;
    let repository = Path::new(&repository_path);
    if !repository.is_dir() {
        return Err("选择的本地项目目录不存在。".into());
    }
    let known = database
        .connect()?
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM repository_assets WHERE path=?1)",
            [&repository_path],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| error.to_string())?;
    if !known {
        return Err("请选择工作台“项目资产”中已扫描的本地项目。".into());
    }
    let (cli_path, _) = codex_video::resolve_codex_cli()?;
    let login = codex_video::hidden_command(&cli_path)
        .args(["login", "status"])
        .output()
        .map_err(|error| error.to_string())?;
    if !login.status.success() {
        return Err("Codex CLI 尚未登录。".into());
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    database.connect()?.execute("INSERT INTO tapd_codex_jobs(id,item_id,repository_path,status,thread_id,output,error_message,created_at,updated_at) VALUES(?1,?2,?3,'running',NULL,'','',?4,?4)", params![id,item_id,repository_path,now]).map_err(|error| error.to_string())?;
    let task_state = database.clone();
    let task_id = id.clone();
    let task_repo = repository_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) =
            run_codex_job(task_state.clone(), task_id.clone(), item.clone(), task_repo)
        {
            let _ = finish_job(&task_state, &task_id, &item, "failed", None, "", &error);
        }
    });
    read_job(&database, &id)
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
        let prompt = build_codex_prompt(&item, Path::new(r"F:\TB-project\client"));
        assert!(prompt.contains("不要提交、推送、重置、清理或删除"));
        assert!(prompt.contains("安全生产管理"));
    }
}
