use crate::database::DatabaseState;
use chrono::Utc;
use keyring::Entry;
use reqwest::blocking::{Client, Response};
use reqwest::header::LOCATION;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const CREDENTIAL_SERVICE: &str = "ai-personal-workbench";
const CREDENTIAL_ACCOUNT: &str = "jenkins-api-token";
const META_BASE_URL: &str = "jenkins_base_url";
const META_USERNAME: &str = "jenkins_username";
const META_VERSION: &str = "jenkins_version";
const META_VERIFIED_AT: &str = "jenkins_verified_at";
static JENKINS_TRIGGER_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug)]
struct JenkinsConnection {
    base_url: String,
    username: String,
    api_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsConnectionStatus {
    pub configured: bool,
    pub base_url: String,
    pub username: String,
    pub version: String,
    pub last_verified_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsJob {
    pub name: String,
    pub display_name: String,
    pub full_name: String,
    pub url: String,
    pub class_name: String,
    pub favorite: bool,
    pub view_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsBranchOptions {
    pub job_full_name: String,
    pub parameter_name: String,
    pub branches: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsPipelineStage {
    pub id: String,
    pub name: String,
    pub status: String,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsChange {
    pub commit_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JenkinsPublishRecord {
    pub id: String,
    pub job_name: String,
    pub job_full_name: String,
    pub job_url: String,
    pub branch_parameter: String,
    pub branch: String,
    pub queue_id: Option<i64>,
    pub queue_url: String,
    pub build_number: Option<i64>,
    pub build_url: String,
    pub status: String,
    pub sync_state: String,
    pub queue_reason: String,
    pub current_stage: String,
    pub stages: Vec<JenkinsPipelineStage>,
    pub changes: Vec<JenkinsChange>,
    pub started_at: String,
    pub build_started_at: Option<String>,
    pub finished_at: Option<String>,
    pub updated_at: String,
    pub result: String,
    pub error_message: String,
}

#[derive(Clone, Debug)]
struct ParameterDefinition {
    name: String,
    choices: Vec<String>,
    default_value: Option<String>,
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn normalize_base_url(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(value).map_err(|_| "Jenkins 地址格式无效。".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Jenkins 地址必须是有效的 http:// 或 https:// 地址。".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Jenkins 地址中不能包含用户名或密码。".to_string());
    }
    Ok(value.to_string())
}

fn credential_entry() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_ACCOUNT).map_err(|error| error.to_string())
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

fn saved_connection(state: &DatabaseState) -> Result<JenkinsConnection, String> {
    let base_url = app_meta(state, META_BASE_URL).unwrap_or_default();
    let username = app_meta(state, META_USERNAME).unwrap_or_default();
    let api_token = credential_entry()?
        .get_password()
        .map_err(|_| "尚未配置 Jenkins API Token，请先在发布中心保存连接。".to_string())?;
    if base_url.is_empty() || username.trim().is_empty() || api_token.is_empty() {
        return Err("尚未完整配置 Jenkins 连接。".to_string());
    }
    Ok(JenkinsConnection {
        base_url,
        username,
        api_token,
    })
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| format!("创建 Jenkins 请求失败：{error}"))
}

fn authenticated_get(
    client: &Client,
    connection: &JenkinsConnection,
    url: &str,
) -> Result<Response, String> {
    client
        .get(url)
        .basic_auth(&connection.username, Some(&connection.api_token))
        .send()
        .map_err(|error| format!("无法连接 Jenkins：{error}"))
}

fn response_error(response: Response, action: &str) -> String {
    match response.status().as_u16() {
        401 => "Jenkins 用户名或 API Token 无效。".to_string(),
        403 => format!("当前 Jenkins 账号没有{action}权限。"),
        404 => format!("Jenkins 中未找到要{action}的项目。"),
        status => format!("Jenkins {action}请求返回 HTTP {status}。"),
    }
}

fn test_connection_value(
    connection: &JenkinsConnection,
) -> Result<JenkinsConnectionStatus, String> {
    let client = http_client()?;
    let response = authenticated_get(
        &client,
        connection,
        &format!("{}/api/json?tree=nodeName,mode", connection.base_url),
    )?;
    if !response.status().is_success() {
        return Err(response_error(response, "读取"));
    }
    let version = response
        .headers()
        .get("X-Jenkins")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("未知版本")
        .to_string();
    response
        .json::<Value>()
        .map_err(|error| format!("Jenkins 返回的数据无法识别：{error}"))?;
    Ok(JenkinsConnectionStatus {
        configured: true,
        base_url: connection.base_url.clone(),
        username: connection.username.clone(),
        version,
        last_verified_at: now(),
    })
}

#[tauri::command]
pub fn jenkins_connection_status(
    state: tauri::State<'_, DatabaseState>,
) -> JenkinsConnectionStatus {
    JenkinsConnectionStatus {
        configured: saved_connection(&state).is_ok(),
        base_url: app_meta(&state, META_BASE_URL).unwrap_or_default(),
        username: app_meta(&state, META_USERNAME).unwrap_or_default(),
        version: app_meta(&state, META_VERSION).unwrap_or_default(),
        last_verified_at: app_meta(&state, META_VERIFIED_AT).unwrap_or_default(),
    }
}

#[tauri::command]
pub async fn test_jenkins_connection(
    state: tauri::State<'_, DatabaseState>,
    base_url: String,
    username: String,
    api_token: String,
) -> Result<JenkinsConnectionStatus, String> {
    let base_url = normalize_base_url(&base_url)?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("请输入 Jenkins 用户名。".to_string());
    }
    let token = if api_token.is_empty() {
        credential_entry()?
            .get_password()
            .map_err(|_| "请输入 Jenkins API Token。".to_string())?
    } else {
        api_token
    };
    let connection = JenkinsConnection {
        base_url,
        username,
        api_token: token,
    };
    let _ = state;
    tauri::async_runtime::spawn_blocking(move || test_connection_value(&connection))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn save_jenkins_connection(
    state: tauri::State<'_, DatabaseState>,
    base_url: String,
    username: String,
    api_token: String,
) -> Result<JenkinsConnectionStatus, String> {
    let database = state.inner().clone();
    let base_url = normalize_base_url(&base_url)?;
    let username = username.trim().to_string();
    if username.is_empty() {
        return Err("请输入 Jenkins 用户名。".to_string());
    }
    let token = if api_token.is_empty() {
        credential_entry()?
            .get_password()
            .map_err(|_| "请输入 Jenkins API Token。".to_string())?
    } else {
        api_token
    };
    let connection = JenkinsConnection {
        base_url,
        username,
        api_token: token.clone(),
    };
    let status = tauri::async_runtime::spawn_blocking(move || test_connection_value(&connection))
        .await
        .map_err(|error| error.to_string())??;
    credential_entry()?
        .set_password(&token)
        .map_err(|error| format!("保存 Jenkins API Token 失败：{error}"))?;
    set_app_meta(&database, META_BASE_URL, &status.base_url)?;
    set_app_meta(&database, META_USERNAME, &status.username)?;
    set_app_meta(&database, META_VERSION, &status.version)?;
    set_app_meta(&database, META_VERIFIED_AT, &status.last_verified_at)?;
    Ok(status)
}

fn favorite_names(state: &DatabaseState, base_url: &str) -> Result<HashSet<String>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT job_full_name FROM jenkins_favorite_jobs WHERE jenkins_base_url=?1")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([base_url], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(|error| error.to_string())
}

fn job_aliases(state: &DatabaseState, base_url: &str) -> Result<HashMap<String, String>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT job_full_name,display_name FROM jenkins_job_aliases WHERE jenkins_base_url=?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([base_url], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(|error| error.to_string())
}

fn view_memberships(value: &Value) -> HashMap<String, HashSet<String>> {
    let mut memberships = HashMap::<String, HashSet<String>>::new();
    for view in value
        .get("views")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let class_name = view
            .get("_class")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if class_name.contains("allview") {
            continue;
        }
        let view_name = view.get("name").and_then(Value::as_str).unwrap_or_default();
        if view_name.is_empty() {
            continue;
        }
        for job in view
            .get("jobs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(full_name) = job.get("fullName").and_then(Value::as_str) {
                memberships
                    .entry(format!("name:{full_name}"))
                    .or_default()
                    .insert(view_name.to_string());
            }
            if let Some(url) = job.get("url").and_then(Value::as_str) {
                memberships
                    .entry(format!("url:{}", url.trim_end_matches('/')))
                    .or_default()
                    .insert(view_name.to_string());
            }
        }
    }
    memberships
}

fn job_view_names(
    memberships: &HashMap<String, HashSet<String>>,
    inherited: &[String],
    full_name: &str,
    url: &str,
) -> Vec<String> {
    let mut names = inherited.iter().cloned().collect::<HashSet<_>>();
    for key in [
        format!("name:{full_name}"),
        format!("url:{}", url.trim_end_matches('/')),
    ] {
        if let Some(direct) = memberships.get(&key) {
            names.extend(direct.iter().cloned());
        }
    }
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    names
}

fn list_jobs_for_state(state: &DatabaseState) -> Result<Vec<JenkinsJob>, String> {
    let connection = saved_connection(state)?;
    let client = http_client()?;
    let favorites = favorite_names(state, &connection.base_url)?;
    let aliases = job_aliases(state, &connection.base_url)?;
    let mut jobs = Vec::new();
    let mut pending = vec![(connection.base_url.clone(), Vec::<String>::new())];
    let mut visited = HashSet::new();
    while let Some((container_url, inherited_views)) = pending.pop() {
        if !visited.insert(container_url.clone()) {
            continue;
        }
        if visited.len() > 1_000 {
            return Err("Jenkins 项目层级超过 1000 个容器，已停止读取。".to_string());
        }
        let response = authenticated_get(
            &client,
            &connection,
            &format!(
                "{}api/json?tree=jobs[name,fullName,url,buildable,_class],views[name,_class,jobs[fullName,url]]",
                container_url.trim_end_matches('/').to_string() + "/"
            ),
        )?;
        if !response.status().is_success() {
            return Err(response_error(response, "读取项目"));
        }
        let value = response
            .json::<Value>()
            .map_err(|error| format!("无法解析 Jenkins 项目列表：{error}"))?;
        let memberships = view_memberships(&value);
        for item in value
            .get("jobs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
            let full_name = item.get("fullName").and_then(Value::as_str).unwrap_or(name);
            let url = item.get("url").and_then(Value::as_str).unwrap_or_default();
            let class_name = item
                .get("_class")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if name.is_empty() || full_name.is_empty() || url.is_empty() {
                continue;
            }
            let view_names = job_view_names(&memberships, &inherited_views, full_name, url);
            let lower_class = class_name.to_ascii_lowercase();
            if lower_class.contains("folder") || lower_class.contains("multibranch") {
                pending.push((url.to_string(), view_names));
            } else if item.get("buildable").and_then(Value::as_bool) == Some(true) {
                jobs.push(JenkinsJob {
                    name: name.to_string(),
                    display_name: aliases
                        .get(full_name)
                        .cloned()
                        .unwrap_or_else(|| name.to_string()),
                    full_name: full_name.to_string(),
                    url: url.to_string(),
                    class_name: class_name.to_string(),
                    favorite: favorites.contains(full_name),
                    view_names,
                });
            }
        }
    }
    jobs.sort_by(|left, right| {
        right.favorite.cmp(&left.favorite).then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| {
                    left.full_name
                        .to_lowercase()
                        .cmp(&right.full_name.to_lowercase())
                })
        })
    });
    Ok(jobs)
}

#[tauri::command]
pub async fn list_jenkins_jobs(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<JenkinsJob>, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_jobs_for_state(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn set_jenkins_job_favorite(
    state: tauri::State<'_, DatabaseState>,
    job_full_name: String,
    favorite: bool,
) -> Result<(), String> {
    let connection = saved_connection(&state)?;
    let job_full_name = job_full_name.trim();
    if job_full_name.is_empty() {
        return Err("Jenkins 项目名称不能为空。".to_string());
    }
    let database = state.connect()?;
    if favorite {
        database
            .execute(
                "INSERT INTO jenkins_favorite_jobs(jenkins_base_url,job_full_name,created_at) VALUES(?1,?2,?3) ON CONFLICT(jenkins_base_url,job_full_name) DO NOTHING",
                params![connection.base_url, job_full_name, now()],
            )
            .map_err(|error| error.to_string())?;
    } else {
        database
            .execute(
                "DELETE FROM jenkins_favorite_jobs WHERE jenkins_base_url=?1 AND job_full_name=?2",
                params![connection.base_url, job_full_name],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn normalize_display_name(value: &str) -> Result<Option<String>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 80 {
        return Err("自定义名称不能超过 80 个字符。".to_string());
    }
    if value.chars().any(char::is_control) {
        return Err("自定义名称不能包含控制字符。".to_string());
    }
    Ok(Some(value.to_string()))
}

#[tauri::command]
pub fn set_jenkins_job_display_name(
    state: tauri::State<'_, DatabaseState>,
    job_full_name: String,
    display_name: String,
) -> Result<(), String> {
    let connection = saved_connection(&state)?;
    let job_full_name = job_full_name.trim();
    if job_full_name.is_empty() {
        return Err("Jenkins 项目名称不能为空。".to_string());
    }
    let display_name = normalize_display_name(&display_name)?;
    let original_name = job_full_name.rsplit('/').next().unwrap_or(job_full_name);
    let database = state.connect()?;
    if let Some(display_name) = display_name {
        database
            .execute(
                "INSERT INTO jenkins_job_aliases(jenkins_base_url,job_full_name,display_name,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(jenkins_base_url,job_full_name) DO UPDATE SET display_name=excluded.display_name,updated_at=excluded.updated_at",
                params![connection.base_url, job_full_name, display_name, now()],
            )
            .map_err(|error| error.to_string())?;
        database
            .execute(
                "UPDATE jenkins_publish_records SET job_name=?1 WHERE jenkins_base_url=?2 AND job_full_name=?3",
                params![display_name, connection.base_url, job_full_name],
            )
            .map_err(|error| error.to_string())?;
    } else {
        database
            .execute(
                "DELETE FROM jenkins_job_aliases WHERE jenkins_base_url=?1 AND job_full_name=?2",
                params![connection.base_url, job_full_name],
            )
            .map_err(|error| error.to_string())?;
        database
            .execute(
                "UPDATE jenkins_publish_records SET job_name=?1 WHERE jenkins_base_url=?2 AND job_full_name=?3",
                params![original_name, connection.base_url, job_full_name],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn load_job_json(connection: &JenkinsConnection, job: &JenkinsJob) -> Result<Value, String> {
    let client = http_client()?;
    let response = authenticated_get(
        &client,
        connection,
        &format!(
            "{}api/json?tree=name,fullName,buildable,property[parameterDefinitions[name,type,_class,defaultParameterValue[value],choices]]",
            job.url.trim_end_matches('/').to_string() + "/"
        ),
    )?;
    if !response.status().is_success() {
        return Err(response_error(response, "读取分支"));
    }
    response
        .json::<Value>()
        .map_err(|error| format!("无法解析 Jenkins 项目参数：{error}"))
}

fn parameter_definitions(value: &Value) -> Vec<ParameterDefinition> {
    let mut definitions = Vec::new();
    for property in value
        .get("property")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for parameter in property
            .get("parameterDefinitions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(name) = parameter.get("name").and_then(Value::as_str) else {
                continue;
            };
            let choices = parameter
                .get("choices")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let default_value = parameter
                .get("defaultParameterValue")
                .and_then(|item| item.get("value"))
                .and_then(|item| match item {
                    Value::String(value) => Some(value.clone()),
                    Value::Bool(value) => Some(value.to_string()),
                    Value::Number(value) => Some(value.to_string()),
                    _ => None,
                });
            definitions.push(ParameterDefinition {
                name: name.to_string(),
                choices,
                default_value,
            });
        }
    }
    definitions
}

fn detect_branch_parameter(
    definitions: &[ParameterDefinition],
) -> Result<&ParameterDefinition, String> {
    for expected in ["BRANCH", "BRANCH_NAME", "GIT_BRANCH", "branchName"] {
        if let Some(parameter) = definitions
            .iter()
            .find(|item| item.name.eq_ignore_ascii_case(expected))
        {
            return Ok(parameter);
        }
    }
    let candidates = definitions
        .iter()
        .filter(|item| {
            item.name.to_ascii_lowercase().contains("branch")
                && (!item.choices.is_empty() || item.default_value.is_some())
        })
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        Ok(candidates[0])
    } else {
        Err("该 Jenkins Job 未暴露可识别的分支参数。".to_string())
    }
}

fn branch_options_from_value(
    job_full_name: &str,
    value: &Value,
) -> Result<JenkinsBranchOptions, String> {
    if value.get("buildable").and_then(Value::as_bool) == Some(false) {
        return Err("该 Jenkins Job 当前已禁用，无法发布。".to_string());
    }
    let definitions = parameter_definitions(value);
    let parameter = detect_branch_parameter(&definitions)?;
    let mut branches = parameter.choices.clone();
    if branches.is_empty() {
        if let Some(default_value) = &parameter.default_value {
            if !default_value.trim().is_empty() {
                branches.push(default_value.clone());
            }
        }
    }
    branches.retain(|branch| !branch.trim().is_empty());
    let mut seen = HashSet::new();
    branches.retain(|branch| seen.insert(branch.clone()));
    if branches.is_empty() {
        return Err("Jenkins 分支参数没有提供可选择的分支。".to_string());
    }
    Ok(JenkinsBranchOptions {
        job_full_name: job_full_name.to_string(),
        parameter_name: parameter.name.clone(),
        branches,
    })
}

fn job_and_branches(
    state: &DatabaseState,
    job_full_name: &str,
) -> Result<(JenkinsConnection, JenkinsJob, JenkinsBranchOptions), String> {
    let connection = saved_connection(state)?;
    let job = list_jobs_for_state(state)?
        .into_iter()
        .find(|job| job.full_name == job_full_name)
        .ok_or_else(|| "Jenkins 中未找到所选项目，可能已删除或无权访问。".to_string())?;
    let value = load_job_json(&connection, &job)?;
    let options = branch_options_from_value(&job.full_name, &value)?;
    Ok((connection, job, options))
}

#[tauri::command]
pub async fn list_jenkins_job_branches(
    state: tauri::State<'_, DatabaseState>,
    job_full_name: String,
) -> Result<JenkinsBranchOptions, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        job_and_branches(&database, &job_full_name).map(|(_, _, options)| options)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn queue_id_from_url(url: &str) -> Option<i64> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let parts = parsed
        .path_segments()?
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    parts
        .windows(2)
        .find(|pair| pair[0] == "item")
        .and_then(|pair| pair[1].parse().ok())
}

fn absolute_url(base_url: &str, value: &str) -> Result<String, String> {
    if let Ok(url) = reqwest::Url::parse(value) {
        return Ok(url.to_string());
    }
    reqwest::Url::parse(&(base_url.trim_end_matches('/').to_string() + "/"))
        .and_then(|base| base.join(value))
        .map(|url| url.to_string())
        .map_err(|_| "Jenkins 返回了无效地址。".to_string())
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JenkinsPublishRecord> {
    let stages_json: String = row.get(15)?;
    let changes_json: String = row.get(16)?;
    Ok(JenkinsPublishRecord {
        id: row.get(0)?,
        job_name: row.get(1)?,
        job_full_name: row.get(2)?,
        job_url: row.get(3)?,
        branch_parameter: row.get(4)?,
        branch: row.get(5)?,
        queue_id: row.get(6)?,
        queue_url: row.get(7)?,
        build_number: row.get(8)?,
        build_url: row.get(9)?,
        status: row.get(10)?,
        sync_state: row.get(11)?,
        queue_reason: row.get(12)?,
        current_stage: row.get(13)?,
        stages: serde_json::from_str(&stages_json).unwrap_or_default(),
        changes: serde_json::from_str(&changes_json).unwrap_or_default(),
        started_at: row.get(17)?,
        build_started_at: row.get(18)?,
        finished_at: row.get(19)?,
        updated_at: row.get(20)?,
        result: row.get(21)?,
        error_message: row.get(22)?,
    })
}

const RECORD_SELECT: &str = "SELECT id,job_name,job_full_name,job_url,branch_parameter,branch,queue_id,queue_url,build_number,build_url,status,sync_state,queue_reason,current_stage,progress_percent,stages_json,changes_json,started_at,build_started_at,finished_at,updated_at,result,error_message FROM jenkins_publish_records";

fn get_record(state: &DatabaseState, id: &str) -> Result<JenkinsPublishRecord, String> {
    state
        .connect()?
        .query_row(
            &format!("{RECORD_SELECT} WHERE id=?1"),
            [id],
            record_from_row,
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "发布记录不存在。".to_string())
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "success" | "failed" | "aborted")
}

fn has_active_publish(
    connection: &Connection,
    base_url: &str,
    job_full_name: &str,
    branch: &str,
) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM jenkins_publish_records WHERE jenkins_base_url=?1 AND job_full_name=?2 AND branch=?3 AND status IN ('queued','running'))",
            params![base_url, job_full_name, branch],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

fn create_completion_notification(
    state: &DatabaseState,
    record: &JenkinsPublishRecord,
) -> Result<(), String> {
    if !is_terminal(&record.status) {
        return Ok(());
    }
    let connection = state.connect()?;
    let sent: i64 = connection
        .query_row(
            "SELECT notification_sent FROM jenkins_publish_records WHERE id=?1",
            [&record.id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if sent != 0 {
        return Ok(());
    }
    let success = record.status == "success";
    let outcome = if success {
        "成功"
    } else if record.status == "aborted" {
        "已中止"
    } else {
        "失败"
    };
    let title = format!("{} · {}发布{}", record.job_name, record.branch, outcome);
    let body = if success {
        format!(
            "Jenkins 构建 #{} 已完成。",
            record.build_number.unwrap_or_default()
        )
    } else {
        record
            .error_message
            .trim()
            .to_string()
            .if_empty_then("请打开 Jenkins 查看失败详情。")
    };
    connection
        .execute(
            "INSERT OR IGNORE INTO notifications(id,kind,title,body,output,source_id,route,is_read,created_at) VALUES(?1,'jenkins_publish',?2,?3,?4,?5,?6,0,?7)",
            params![format!("jenkins-publish-{}", record.id), title, body, record.error_message, record.id, format!("/deployments?run={}", record.id), now()],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE jenkins_publish_records SET notification_sent=1 WHERE id=?1",
            [&record.id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

trait EmptyFallback {
    fn if_empty_then(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

fn stage_snapshot(value: &Value) -> (String, Vec<JenkinsPipelineStage>) {
    let stages = value
        .get("stages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|stage| JenkinsPipelineStage {
            id: stage
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            name: stage
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("未命名阶段")
                .to_string(),
            status: stage
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("UNKNOWN")
                .to_string(),
            duration_ms: stage
                .get("durationMillis")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    let current = stages
        .iter()
        .find(|stage| {
            matches!(
                stage.status.as_str(),
                "IN_PROGRESS" | "PAUSED_PENDING_INPUT"
            )
        })
        .map(|stage| stage.name.clone())
        .or_else(|| stages.last().map(|stage| stage.name.clone()))
        .unwrap_or_default();
    (current, stages)
}

fn append_change_items(
    change_set: &Value,
    changes: &mut Vec<JenkinsChange>,
    seen: &mut HashSet<String>,
) {
    for item in change_set
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if changes.len() >= 100 {
            return;
        }
        let commit_id = item
            .get("commitId")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let message = item
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let author = item
            .get("author")
            .and_then(|author| author.get("fullName"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let timestamp = item.get("timestamp").and_then(Value::as_i64);
        if commit_id.is_empty() && message.is_empty() && author.is_empty() {
            continue;
        }
        let key = format!("{commit_id}\n{message}\n{author}");
        if seen.insert(key) {
            changes.push(JenkinsChange {
                commit_id,
                message,
                author,
                timestamp,
            });
        }
    }
}

fn changes_from_value(value: &Value) -> Vec<JenkinsChange> {
    let mut changes = Vec::new();
    let mut seen = HashSet::new();
    if let Some(change_set) = value.get("changeSet") {
        append_change_items(change_set, &mut changes, &mut seen);
    }
    for change_set in value
        .get("changeSets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        append_change_items(change_set, &mut changes, &mut seen);
    }
    changes
}

fn merge_changes(existing: &[JenkinsChange], current: Vec<JenkinsChange>) -> Vec<JenkinsChange> {
    let mut changes = existing.to_vec();
    let mut seen = changes
        .iter()
        .map(|change| {
            format!(
                "{}\n{}\n{}",
                change.commit_id, change.message, change.author
            )
        })
        .collect::<HashSet<_>>();
    for change in current {
        if changes.len() >= 100 {
            break;
        }
        let key = format!(
            "{}\n{}\n{}",
            change.commit_id, change.message, change.author
        );
        if seen.insert(key) {
            changes.push(change);
        }
    }
    changes
}

fn build_result_status(result: &str) -> &'static str {
    match result {
        "SUCCESS" => "success",
        "ABORTED" | "NOT_BUILT" => "aborted",
        _ => "failed",
    }
}

fn recover_build_by_queue_id(
    client: &Client,
    connection: &JenkinsConnection,
    record: &JenkinsPublishRecord,
) -> Result<Option<(i64, String)>, String> {
    let Some(queue_id) = record.queue_id else {
        return Ok(None);
    };
    let response = authenticated_get(
        client,
        connection,
        &format!(
            "{}api/json?tree=builds[number,url,queueId]{{0,100}}",
            record.job_url.trim_end_matches('/').to_string() + "/"
        ),
    )?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let value = response
        .json::<Value>()
        .map_err(|error| error.to_string())?;
    Ok(value
        .get("builds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|build| build.get("queueId").and_then(Value::as_i64) == Some(queue_id))
        .and_then(|build| {
            Some((
                build.get("number")?.as_i64()?,
                build.get("url")?.as_str()?.to_string(),
            ))
        }))
}

fn sync_publish_once(state: &DatabaseState, id: &str) -> Result<JenkinsPublishRecord, String> {
    let record = get_record(state, id)?;
    if is_terminal(&record.status) {
        create_completion_notification(state, &record)?;
        return Ok(record);
    }
    let connection = saved_connection(state)?;
    let client = http_client()?;
    let timestamp = now();
    if record.status == "queued" {
        let queue_api = format!(
            "{}api/json",
            record.queue_url.trim_end_matches('/').to_string() + "/"
        );
        let response = authenticated_get(&client, &connection, &queue_api)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            if let Some((number, url)) = recover_build_by_queue_id(&client, &connection, &record)? {
                state.connect()?.execute(
                    "UPDATE jenkins_publish_records SET build_number=?1,build_url=?2,status='running',sync_state='synced',build_started_at=COALESCE(build_started_at,?3),updated_at=?3,error_message='' WHERE id=?4",
                    params![number,url,timestamp,id],
                ).map_err(|error| error.to_string())?;
            } else {
                state.connect()?.execute(
                    "UPDATE jenkins_publish_records SET sync_state='reconnecting',updated_at=?1 WHERE id=?2",
                    params![timestamp,id],
                ).map_err(|error| error.to_string())?;
            }
            return get_record(state, id);
        }
        if !response.status().is_success() {
            return Err(response_error(response, "读取队列"));
        }
        let value = response
            .json::<Value>()
            .map_err(|error| error.to_string())?;
        if value.get("cancelled").and_then(Value::as_bool) == Some(true) {
            state.connect()?.execute(
                "UPDATE jenkins_publish_records SET status='aborted',sync_state='synced',result='ABORTED',finished_at=?1,updated_at=?1 WHERE id=?2",
                params![timestamp,id],
            ).map_err(|error| error.to_string())?;
        } else if let Some(executable) = value.get("executable") {
            let number = executable.get("number").and_then(Value::as_i64);
            let url = executable
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let (Some(number), false) = (number, url.is_empty()) {
                state.connect()?.execute(
                    "UPDATE jenkins_publish_records SET build_number=?1,build_url=?2,status='running',sync_state='synced',build_started_at=COALESCE(build_started_at,?3),updated_at=?3,queue_reason='',error_message='' WHERE id=?4",
                    params![number,url,timestamp,id],
                ).map_err(|error| error.to_string())?;
            }
        } else {
            let reason = value
                .get("why")
                .and_then(Value::as_str)
                .unwrap_or("等待 Jenkins 分配执行节点");
            state.connect()?.execute(
                "UPDATE jenkins_publish_records SET queue_reason=?1,sync_state='synced',updated_at=?2,error_message='' WHERE id=?3",
                params![reason,timestamp,id],
            ).map_err(|error| error.to_string())?;
        }
    } else if record.status == "running" {
        let build_api = format!(
            "{}api/json?tree=number,url,building,result,timestamp,duration,queueId,changeSet[items[commitId,id,msg,author[fullName],timestamp]],changeSets[items[commitId,id,msg,author[fullName],timestamp]]",
            record.build_url.trim_end_matches('/').to_string() + "/"
        );
        let response = authenticated_get(&client, &connection, &build_api)?;
        if !response.status().is_success() {
            return Err(response_error(response, "读取构建"));
        }
        let value = response
            .json::<Value>()
            .map_err(|error| error.to_string())?;
        let stages_response = authenticated_get(
            &client,
            &connection,
            &format!(
                "{}wfapi/describe",
                record.build_url.trim_end_matches('/').to_string() + "/"
            ),
        );
        let (current_stage, stages) = match stages_response {
            Ok(response) if response.status().is_success() => response
                .json::<Value>()
                .map(|value| stage_snapshot(&value))
                .unwrap_or_default(),
            _ => (String::new(), Vec::new()),
        };
        let stages_json = serde_json::to_string(&stages).map_err(|error| error.to_string())?;
        let changes_json =
            serde_json::to_string(&merge_changes(&record.changes, changes_from_value(&value)))
                .map_err(|error| error.to_string())?;
        let building = value
            .get("building")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if building {
            state.connect()?.execute(
                "UPDATE jenkins_publish_records SET current_stage=?1,stages_json=?2,changes_json=?3,sync_state='synced',updated_at=?4,error_message='' WHERE id=?5",
                params![current_stage,stages_json,changes_json,timestamp,id],
            ).map_err(|error| error.to_string())?;
        } else {
            let result = value
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or("FAILURE");
            let status = build_result_status(result);
            let error_message = if status == "failed" {
                format!("Jenkins 构建结果：{result}")
            } else {
                String::new()
            };
            state.connect()?.execute(
                "UPDATE jenkins_publish_records SET status=?1,current_stage=?2,stages_json=?3,changes_json=?4,sync_state='synced',result=?5,error_message=?6,finished_at=?7,updated_at=?7 WHERE id=?8",
                params![status,current_stage,stages_json,changes_json,result,error_message,timestamp,id],
            ).map_err(|error| error.to_string())?;
        }
    }
    let updated = get_record(state, id)?;
    create_completion_notification(state, &updated)?;
    Ok(updated)
}

fn spawn_monitor(app: AppHandle, state: DatabaseState, id: String) {
    std::thread::spawn(move || loop {
        match sync_publish_once(&state, &id) {
            Ok(record) => {
                let terminal = is_terminal(&record.status);
                let _ = app.emit("jenkins-publish-updated", &record);
                if terminal {
                    break;
                }
                std::thread::sleep(Duration::from_secs(if record.status == "queued" {
                    1
                } else {
                    2
                }));
            }
            Err(error) => {
                let timestamp = now();
                if let Ok(connection) = state.connect() {
                    let _ = connection.execute(
                        "UPDATE jenkins_publish_records SET sync_state='reconnecting',error_message=?1,updated_at=?2 WHERE id=?3 AND status IN ('queued','running')",
                        params![error,timestamp,id],
                    );
                }
                let _ = app.emit("jenkins-publish-updated", id.clone());
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    });
}

#[tauri::command]
pub async fn trigger_jenkins_publish(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
    job_full_name: String,
    branch: String,
) -> Result<JenkinsPublishRecord, String> {
    let database = state.inner().clone();
    let app_for_monitor = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (connection, job, options) = job_and_branches(&database, &job_full_name)?;
        if !options.branches.iter().any(|item| item == &branch) {
            return Err("所选分支已不在 Jenkins Job 的可选范围内，请重新选择。".to_string());
        }
        let _trigger_guard = JENKINS_TRIGGER_LOCK
            .lock()
            .map_err(|_| "Jenkins 发布锁异常，请稍后重试。".to_string())?;
        if has_active_publish(
            &database.connect()?,
            &connection.base_url,
            &job.full_name,
            &branch,
        )? {
            return Err(format!(
                "{} · {} 已有正在执行的发布，请等待完成后再试。",
                job.full_name, branch
            ));
        }
        let client = http_client()?;
        let response = client
            .post(format!("{}buildWithParameters", job.url.trim_end_matches('/').to_string() + "/"))
            .basic_auth(&connection.username, Some(&connection.api_token))
            .form(&[(options.parameter_name.as_str(), branch.as_str())])
            .send()
            .map_err(|error| format!("触发 Jenkins 发布失败：{error}"))?;
        if !response.status().is_success() {
            return Err(response_error(response, "触发构建"));
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| "Jenkins 已接受请求，但没有返回队列地址。".to_string())?;
        let queue_url = absolute_url(&connection.base_url, location)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        database.connect()?.execute(
            "INSERT INTO jenkins_publish_records(id,jenkins_base_url,job_name,job_full_name,job_url,branch_parameter,branch,queue_id,queue_url,status,sync_state,started_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'queued','synced',?10,?10)",
            params![id,connection.base_url,job.display_name,job.full_name,job.url,options.parameter_name,branch,queue_id_from_url(&queue_url),queue_url,timestamp],
        ).map_err(|error| error.to_string())?;
        let record = get_record(&database, &id)?;
        spawn_monitor(app_for_monitor, database.clone(), id);
        Ok(record)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn list_jenkins_publish_records(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<JenkinsPublishRecord>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(&format!(
            "{RECORD_SELECT} ORDER BY datetime(started_at) DESC LIMIT 200"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], record_from_row)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_jenkins_publish_status(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<JenkinsPublishRecord, String> {
    get_record(&state, &id)
}

#[tauri::command]
pub fn open_jenkins_url(state: tauri::State<'_, DatabaseState>, url: String) -> Result<(), String> {
    let base_url = normalize_base_url(&app_meta(&state, META_BASE_URL).unwrap_or_default())?;
    let target = reqwest::Url::parse(&url).map_err(|_| "Jenkins 地址无效。".to_string())?;
    let base = reqwest::Url::parse(&base_url).map_err(|_| "Jenkins 配置无效。".to_string())?;
    if target.scheme() != base.scheme()
        || target.host_str() != base.host_str()
        || target.port_or_known_default() != base.port_or_known_default()
    {
        return Err("只允许打开当前配置的 Jenkins 地址。".to_string());
    }
    #[cfg(windows)]
    {
        let mut command = Command::new("explorer.exe");
        command.arg(target.as_str());
        command.creation_flags(CREATE_NO_WINDOW);
        command
            .spawn()
            .map_err(|error| format!("无法使用默认浏览器打开 Jenkins：{error}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        Err("当前系统暂不支持从工作台打开 Jenkins。".to_string())
    }
}

pub fn resume_active_publishes(app: AppHandle, state: &DatabaseState) -> Result<(), String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT id FROM jenkins_publish_records WHERE status IN ('queued','running')")
        .map_err(|error| error.to_string())?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for id in ids {
        spawn_monitor(app.clone(), state.clone(), id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_and_validates_jenkins_url() {
        assert_eq!(
            normalize_base_url(" https://jenkins.example.com/root/ ").unwrap(),
            "https://jenkins.example.com/root"
        );
        assert!(normalize_base_url("file:///tmp/jenkins").is_err());
        assert!(normalize_base_url("https://user:secret@example.com").is_err());
    }

    #[test]
    fn branch_detection_prefers_known_names() {
        let value = json!({"buildable":true,"property":[{"parameterDefinitions":[
            {"name":"TARGET_BRANCH","choices":["wrong"]},
            {"name":"BRANCH_NAME","choices":["main","develop"]}
        ]}]});
        let options = branch_options_from_value("demo", &value).unwrap();
        assert_eq!(options.parameter_name, "BRANCH_NAME");
        assert_eq!(options.branches, vec!["main", "develop"]);
    }

    #[test]
    fn branch_detection_uses_single_branch_candidate_and_rejects_ambiguity() {
        let value = json!({"buildable":true,"property":[{"parameterDefinitions":[
            {"name":"releaseBranch","choices":["main"]}
        ]}]});
        assert_eq!(
            branch_options_from_value("demo", &value)
                .unwrap()
                .parameter_name,
            "releaseBranch"
        );
        let ambiguous = json!({"buildable":true,"property":[{"parameterDefinitions":[
            {"name":"sourceBranch","choices":["main"]},
            {"name":"targetBranch","choices":["main"]}
        ]}]});
        assert!(branch_options_from_value("demo", &ambiguous).is_err());
    }

    #[test]
    fn queue_id_and_build_results_are_stable() {
        assert_eq!(
            queue_id_from_url("https://jenkins.example.com/queue/item/318/"),
            Some(318)
        );
        assert_eq!(build_result_status("SUCCESS"), "success");
        assert_eq!(build_result_status("ABORTED"), "aborted");
        assert_eq!(build_result_status("UNSTABLE"), "failed");
    }

    #[test]
    fn reads_explicit_jenkins_views_and_inherits_folder_view() {
        let value = json!({"views":[
            {"name":"All","_class":"hudson.model.AllView","jobs":[{"fullName":"folder","url":"https://jenkins/job/folder/"}]},
            {"name":"生产安全","_class":"hudson.model.ListView","jobs":[{"fullName":"folder","url":"https://jenkins/job/folder/"}]},
            {"name":"大数据","_class":"hudson.model.ListView","jobs":[{"fullName":"folder/web","url":"https://jenkins/job/folder/job/web/"}]}
        ]});
        let memberships = view_memberships(&value);
        assert_eq!(
            job_view_names(
                &memberships,
                &["上级 View".to_string()],
                "folder/web",
                "https://jenkins/job/folder/job/web/"
            ),
            vec!["上级 View", "大数据"]
        );
        assert_eq!(
            job_view_names(&memberships, &[], "folder", "https://jenkins/job/folder/"),
            vec!["生产安全"]
        );
    }

    #[test]
    fn reads_changes_from_freestyle_and_pipeline_without_duplicates() {
        let value = json!({
            "changeSet":{"items":[{"commitId":"abcdef123","msg":"fix: 修复问题","author":{"fullName":"张三"},"timestamp":1710000000000_i64}]},
            "changeSets":[
                {"items":[{"commitId":"abcdef123","msg":"fix: 修复问题","author":{"fullName":"张三"},"timestamp":1710000000000_i64}]},
                {"items":[{"id":"987654","msg":"feat: 新增功能","author":{"fullName":"李四"}}]}
            ]
        });
        let changes = changes_from_value(&value);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].commit_id, "abcdef123");
        assert_eq!(changes[0].author, "张三");
        assert_eq!(changes[1].commit_id, "987654");
        let merged = merge_changes(&changes[..1], changes.clone());
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn validates_optional_job_display_name() {
        assert_eq!(
            normalize_display_name("  前端管理端  ").unwrap(),
            Some("前端管理端".to_string())
        );
        assert_eq!(normalize_display_name("  ").unwrap(), None);
        assert!(normalize_display_name(&"x".repeat(81)).is_err());
    }

    #[test]
    fn active_publish_blocks_only_the_same_job_and_branch() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE jenkins_publish_records(
                jenkins_base_url TEXT NOT NULL,
                job_full_name TEXT NOT NULL,
                branch TEXT NOT NULL,
                status TEXT NOT NULL
            );
            INSERT INTO jenkins_publish_records VALUES(
                'https://jenkins.example.com','folder/web','main','queued'
            );",
            )
            .unwrap();

        assert!(has_active_publish(
            &connection,
            "https://jenkins.example.com",
            "folder/web",
            "main"
        )
        .unwrap());
        assert!(!has_active_publish(
            &connection,
            "https://jenkins.example.com",
            "folder/web",
            "develop"
        )
        .unwrap());
        connection
            .execute("UPDATE jenkins_publish_records SET status='success'", [])
            .unwrap();
        assert!(!has_active_publish(
            &connection,
            "https://jenkins.example.com",
            "folder/web",
            "main"
        )
        .unwrap());
    }
}
