use crate::{
    database::DatabaseState,
    knowledge::{save_knowledge_for_state, KnowledgeItem},
};
use chrono::Utc;
use keyring::Entry;
use reqwest::blocking::Client;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Duration;

const CREDENTIAL_SERVICE: &str = "ai-personal-workbench";
const CREDENTIAL_USER: &str = "apifox-access-token";
const APIFOX_EXPORT_URL: &str = "https://api.apifox.com/v1/projects";
const APIFOX_API_VERSION: &str = "2024-03-28";
const MAX_OPENAPI_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApifoxCredentialStatus {
    pub configured: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSource {
    pub id: String,
    pub project_profile_id: String,
    pub project_name: String,
    pub repository_path: String,
    pub external_project_id: String,
    pub document_title: String,
    pub openapi_version: String,
    pub sync_status: String,
    pub endpoint_count: i64,
    pub last_synced_at: Option<String>,
    pub last_error: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSourceUpdate {
    pub id: String,
    pub project_profile_id: String,
    pub external_project_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointFilter {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub tag: String,
    pub deprecated: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointSummary {
    pub id: String,
    pub source_id: String,
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointDetail {
    #[serde(flatten)]
    pub summary: ApiEndpointSummary,
    pub project_profile_id: String,
    pub project_name: String,
    pub repository_path: String,
    pub document_title: String,
    pub openapi_version: String,
    pub last_synced_at: Option<String>,
    pub document: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSyncSummary {
    pub source_id: String,
    pub project_name: String,
    pub status: String,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub total: usize,
    pub synced_at: Option<String>,
    pub error: String,
}

#[derive(Debug, Clone)]
struct SourceRow {
    id: String,
    project_name: String,
    external_project_id: String,
    last_synced_at: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedEndpoint {
    id: String,
    operation_id: String,
    method: String,
    path: String,
    title: String,
    description: String,
    tags: Vec<String>,
    deprecated: bool,
    document: Value,
    document_hash: String,
    search_text: String,
}

fn credential_entry() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|error| error.to_string())
}

fn access_token() -> Result<String, String> {
    credential_entry()?
        .get_password()
        .map(|value| value.trim().to_string())
        .map_err(|_| "尚未配置 Apifox API 访问令牌，请先在设置中保存。".to_string())
        .and_then(|value| {
            if value.is_empty() {
                Err("Apifox API 访问令牌为空，请重新保存。".to_string())
            } else {
                Ok(value)
            }
        })
}

#[tauri::command]
pub fn apifox_credential_status() -> ApifoxCredentialStatus {
    ApifoxCredentialStatus {
        configured: access_token().is_ok(),
        source: if access_token().is_ok() {
            "Windows 凭据库".into()
        } else {
            "未配置".into()
        },
    }
}

#[tauri::command]
pub fn save_apifox_token(token: String) -> Result<(), String> {
    let token = token.trim();
    if token.len() < 12 || token.chars().any(char::is_whitespace) {
        return Err("请输入完整的 Apifox API 访问令牌。".into());
    }
    credential_entry()?
        .set_password(token)
        .map_err(|error| format!("保存 Apifox 令牌失败：{error}"))
}

#[tauri::command]
pub fn clear_apifox_token() -> Result<(), String> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("删除 Apifox 令牌失败：{error}")),
    }
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn source_id(project_profile_id: &str) -> String {
    format!("api-source-{}", stable_hash(project_profile_id))
}

fn endpoint_id(source_id: &str, method: &str, path: &str) -> String {
    format!(
        "api-endpoint-{}",
        stable_hash(&format!(
            "{source_id}|{}|{path}",
            method.to_ascii_lowercase()
        ))
    )
}

fn valid_external_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[tauri::command]
pub fn list_api_sources(state: tauri::State<'_, DatabaseState>) -> Result<Vec<ApiSource>, String> {
    crate::project_identity::sync_project_profiles_for_state(&state)?;
    let connection = state.connect()?;
    connection.execute(
        "UPDATE api_sources SET sync_status=CASE WHEN last_synced_at IS NULL THEN 'error' ELSE 'stale' END,last_error=CASE WHEN last_error='' THEN '上次同步未正常完成。' ELSE last_error END WHERE sync_status='syncing'",
        [],
    ).map_err(|error| error.to_string())?;
    let mut statement = connection.prepare(
        "SELECT s.id,s.project_profile_id,p.display_name,p.repository_path,s.external_project_id,s.document_title,s.openapi_version,s.sync_status,s.endpoint_count,s.last_synced_at,s.last_error,s.created_at,s.updated_at
         FROM api_sources s JOIN project_profiles p ON p.id=s.project_profile_id
         ORDER BY p.display_name COLLATE NOCASE",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ApiSource {
                id: row.get(0)?,
                project_profile_id: row.get(1)?,
                project_name: row.get(2)?,
                repository_path: row.get(3)?,
                external_project_id: row.get(4)?,
                document_title: row.get(5)?,
                openapi_version: row.get(6)?,
                sync_status: row.get(7)?,
                endpoint_count: row.get(8)?,
                last_synced_at: row.get(9)?,
                last_error: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_api_source(
    state: tauri::State<'_, DatabaseState>,
    source: ApiSourceUpdate,
) -> Result<ApiSource, String> {
    crate::project_identity::sync_project_profiles_for_state(&state)?;
    let profile_id = source.project_profile_id.trim();
    let external_id = source.external_project_id.trim();
    if profile_id.is_empty() {
        return Err("请选择要关联的规范项目。".into());
    }
    if !valid_external_project_id(external_id) {
        return Err("Apifox 项目 ID 只能包含字母、数字、短横线或下划线。".into());
    }
    let connection = state.connect()?;
    let profile_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM project_profiles WHERE id=?1)",
            [profile_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| error.to_string())?;
    if !profile_exists {
        return Err("所选规范项目不存在，请重新同步项目资产。".into());
    }
    let id = if source.id.trim().is_empty() {
        source_id(profile_id)
    } else {
        source.id.trim().to_string()
    };
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO api_sources(id,project_profile_id,provider,external_project_id,created_at,updated_at)
         VALUES(?1,?2,'apifox',?3,?4,?4)
         ON CONFLICT(id) DO UPDATE SET project_profile_id=excluded.project_profile_id,external_project_id=excluded.external_project_id,updated_at=excluded.updated_at",
        params![id, profile_id, external_id, now],
    ).map_err(|error| {
        if error.to_string().contains("UNIQUE constraint failed") {
            "该规范项目或 Apifox 项目 ID 已经存在关联。".to_string()
        } else {
            error.to_string()
        }
    })?;
    list_api_sources(state)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Apifox 项目关联保存后无法读取。".to_string())
}

#[tauri::command]
pub fn remove_api_source(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
) -> Result<(), String> {
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM api_endpoints WHERE source_id=?1", [&source_id])
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM api_sources WHERE id=?1", [&source_id])
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

fn source_row(state: &DatabaseState, id: &str) -> Result<SourceRow, String> {
    state
        .connect()?
        .query_row(
            "SELECT s.id,p.display_name,s.external_project_id,s.last_synced_at FROM api_sources s JOIN project_profiles p ON p.id=s.project_profile_id WHERE s.id=?1",
            [id],
            |row| {
                Ok(SourceRow {
                    id: row.get(0)?,
                    project_name: row.get(1)?,
                    external_project_id: row.get(2)?,
                    last_synced_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "接口文档项目关联不存在。".to_string())
}

fn export_payload() -> Value {
    json!({
        "scope": { "type": "ALL" },
        "options": {
            "includeApifoxExtensionProperties": false,
            "addFoldersToTags": true
        },
        "oasVersion": "3.1",
        "exportFormat": "JSON"
    })
}

fn fetch_openapi(project_id: &str, token: &str) -> Result<Value, String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| format!("创建 Apifox 请求失败：{error}"))?;
    let response = client
        .post(format!(
            "{APIFOX_EXPORT_URL}/{project_id}/export-openapi?locale=zh-CN"
        ))
        .header("X-Apifox-Api-Version", APIFOX_API_VERSION)
        .bearer_auth(token)
        .json(&export_payload())
        .send()
        .map_err(|error| format!("无法连接 Apifox：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "Apifox 令牌无效或已过期，请在设置中重新保存。".into(),
            403 => "当前 Apifox 账号没有该项目的导出权限。".into(),
            404 => "未找到 Apifox 项目，请检查项目 ID。".into(),
            429 => "Apifox 请求过于频繁，请稍后再同步。".into(),
            value => format!("Apifox 导出接口返回 HTTP {value}。"),
        });
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_OPENAPI_BYTES)
    {
        return Err("Apifox OpenAPI 文档超过 32 MB，为避免占用过多本机内存已停止同步。".into());
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("读取 Apifox OpenAPI 文档失败：{error}"))?;
    if bytes.len() as u64 > MAX_OPENAPI_BYTES {
        return Err("Apifox OpenAPI 文档超过 32 MB，为避免占用过多本机内存已停止同步。".into());
    }
    serde_json::from_slice::<Value>(&bytes)
        .map_err(|error| format!("Apifox 返回的 OpenAPI JSON 无法解析：{error}"))
}

fn resolve_value(
    value: &Value,
    root: &Value,
    active_refs: &mut HashSet<String>,
    warnings: &mut BTreeSet<String>,
    depth: usize,
) -> Value {
    if depth > 24 {
        warnings.insert("文档引用层级超过 24 层，深层结构未继续展开。".into());
        return value.clone();
    }
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                if !reference.starts_with("#/") {
                    warnings.insert(format!("外部引用未联网读取：{reference}"));
                    return value.clone();
                }
                if !active_refs.insert(reference.to_string()) {
                    warnings.insert(format!("检测到循环引用：{reference}"));
                    return json!({ "$ref": reference, "x-workbench-warning": "循环引用未展开" });
                }
                let resolved = root
                    .pointer(&reference[1..])
                    .map(|target| resolve_value(target, root, active_refs, warnings, depth + 1));
                active_refs.remove(reference);
                if let Some(Value::Object(mut target)) = resolved {
                    for (key, sibling) in object.iter().filter(|(key, _)| key.as_str() != "$ref") {
                        target.insert(
                            key.clone(),
                            resolve_value(sibling, root, active_refs, warnings, depth + 1),
                        );
                    }
                    return Value::Object(target);
                }
                warnings.insert(format!("本地引用无法解析：{reference}"));
                value.clone()
            } else {
                Value::Object(
                    object
                        .iter()
                        .map(|(key, child)| {
                            (
                                key.clone(),
                                resolve_value(child, root, active_refs, warnings, depth + 1),
                            )
                        })
                        .collect(),
                )
            }
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_value(item, root, active_refs, warnings, depth + 1))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn operation_methods() -> &'static [&'static str] {
    &[
        "get", "post", "put", "patch", "delete", "head", "options", "trace",
    ]
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn parse_openapi(
    root: &Value,
    source_id: &str,
) -> Result<(String, String, Vec<ParsedEndpoint>), String> {
    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| "OpenAPI 文档缺少 paths，无法读取接口。".to_string())?;
    let document_title = text(root.pointer("/info/title"));
    let version = root
        .get("openapi")
        .or_else(|| root.get("swagger"))
        .and_then(Value::as_str)
        .unwrap_or("未知")
        .to_string();
    let mut endpoints = Vec::new();
    for (path, path_item) in paths {
        let Some(path_object) = path_item.as_object() else {
            continue;
        };
        let path_parameters = path_object
            .get("parameters")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for method in operation_methods() {
            let Some(operation) = path_object.get(*method).and_then(Value::as_object) else {
                continue;
            };
            let operation_id = text(operation.get("operationId"));
            let summary = text(operation.get("summary"));
            let title = if !summary.is_empty() {
                summary
            } else if !operation_id.is_empty() {
                operation_id.clone()
            } else {
                format!("{} {path}", method.to_ascii_uppercase())
            };
            let description = text(operation.get("description"));
            let tags: Vec<String> = operation
                .get("tags")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let deprecated = operation
                .get("deprecated")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut parameters = path_parameters.clone();
            parameters.extend(
                operation
                    .get("parameters")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );
            let mut raw_document = Map::new();
            raw_document.insert("parameters".into(), Value::Array(parameters));
            for key in ["requestBody", "responses", "security", "servers"] {
                if let Some(value) = operation.get(key).or_else(|| root.get(key)) {
                    raw_document.insert(key.into(), value.clone());
                }
            }
            let mut warnings = BTreeSet::new();
            let document = resolve_value(
                &Value::Object(raw_document),
                root,
                &mut HashSet::new(),
                &mut warnings,
                0,
            );
            let mut document = document.as_object().cloned().unwrap_or_default();
            document.insert(
                "warnings".into(),
                Value::Array(warnings.into_iter().map(Value::String).collect()),
            );
            let document = Value::Object(document);
            let serialized = serde_json::to_string(&document).map_err(|error| error.to_string())?;
            let search_text = format!(
                "{} {} {} {} {} {}",
                method,
                path,
                title,
                description,
                operation_id,
                tags.join(" ")
            )
            .to_lowercase();
            endpoints.push(ParsedEndpoint {
                id: endpoint_id(source_id, method, path),
                operation_id,
                method: method.to_ascii_uppercase(),
                path: path.clone(),
                title,
                description,
                tags,
                deprecated,
                document_hash: stable_hash(&serialized),
                document,
                search_text,
            });
        }
    }
    endpoints.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.method.cmp(&right.method))
    });
    Ok((document_title, version, endpoints))
}

fn mark_sync_failure(state: &DatabaseState, source: &SourceRow, error: &str) {
    let status = if source.last_synced_at.is_some() {
        "stale"
    } else {
        "error"
    };
    let clean = error.chars().take(500).collect::<String>();
    let _ = state.connect().and_then(|connection| {
        connection
            .execute(
                "UPDATE api_sources SET sync_status=?2,last_error=?3,updated_at=?4 WHERE id=?1",
                params![source.id, status, clean, Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|value| value.to_string())
    });
}

fn sync_source(state: &DatabaseState, source: &SourceRow) -> Result<ApiSyncSummary, String> {
    state
        .connect()?
        .execute(
            "UPDATE api_sources SET sync_status='syncing',last_error='',updated_at=?2 WHERE id=?1",
            params![source.id, Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    let token = access_token()?;
    let root = fetch_openapi(&source.external_project_id, &token)?;
    let (document_title, version, endpoints) = parse_openapi(&root, &source.id)?;
    let now = Utc::now().to_rfc3339();
    let content_hash =
        stable_hash(&serde_json::to_string(&root).map_err(|error| error.to_string())?);
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let existing = {
        let mut statement = transaction
            .prepare("SELECT id,document_hash FROM api_endpoints WHERE source_id=?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([&source.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(|error| error.to_string())?
    };
    let current_ids = endpoints
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let added = endpoints
        .iter()
        .filter(|item| !existing.contains_key(&item.id))
        .count();
    let updated = endpoints
        .iter()
        .filter(|item| {
            existing
                .get(&item.id)
                .is_some_and(|hash| hash != &item.document_hash)
        })
        .count();
    let removed = existing
        .keys()
        .filter(|id| !current_ids.contains(id.as_str()))
        .count();
    transaction
        .execute("DELETE FROM api_endpoints WHERE source_id=?1", [&source.id])
        .map_err(|error| error.to_string())?;
    for endpoint in &endpoints {
        transaction.execute(
            "INSERT INTO api_endpoints(id,source_id,operation_id,method,path,title,description,tags_json,deprecated,document_json,document_hash,search_text,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![endpoint.id,source.id,endpoint.operation_id,endpoint.method,endpoint.path,endpoint.title,endpoint.description,serde_json::to_string(&endpoint.tags).map_err(|error| error.to_string())?,endpoint.deprecated,serde_json::to_string(&endpoint.document).map_err(|error| error.to_string())?,endpoint.document_hash,endpoint.search_text,now],
        ).map_err(|error| error.to_string())?;
    }
    transaction.execute(
        "UPDATE api_sources SET document_title=?2,openapi_version=?3,sync_status='ready',endpoint_count=?4,content_hash=?5,last_synced_at=?6,last_error='',updated_at=?6 WHERE id=?1",
        params![source.id,document_title,version,endpoints.len() as i64,content_hash,now],
    ).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(ApiSyncSummary {
        source_id: source.id.clone(),
        project_name: source.project_name.clone(),
        status: "ready".into(),
        added,
        updated,
        removed,
        total: endpoints.len(),
        synced_at: Some(now),
        error: String::new(),
    })
}

#[tauri::command]
pub fn sync_api_source(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
) -> Result<ApiSyncSummary, String> {
    let source = source_row(&state, &source_id)?;
    match sync_source(&state, &source) {
        Ok(summary) => Ok(summary),
        Err(error) => {
            mark_sync_failure(&state, &source, &error);
            Err(error)
        }
    }
}

#[tauri::command]
pub fn sync_all_api_sources(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<ApiSyncSummary>, String> {
    let sources = list_api_sources(state.clone())?;
    let mut summaries = Vec::new();
    for item in sources {
        let source = source_row(&state, &item.id)?;
        match sync_source(&state, &source) {
            Ok(summary) => summaries.push(summary),
            Err(error) => {
                mark_sync_failure(&state, &source, &error);
                summaries.push(ApiSyncSummary {
                    source_id: source.id,
                    project_name: source.project_name,
                    status: if source.last_synced_at.is_some() {
                        "stale"
                    } else {
                        "error"
                    }
                    .into(),
                    added: 0,
                    updated: 0,
                    removed: 0,
                    total: 0,
                    synced_at: source.last_synced_at,
                    error,
                });
            }
        }
    }
    Ok(summaries)
}

fn tags_from_json(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

#[tauri::command]
pub fn list_api_endpoints(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
    filter: Option<ApiEndpointFilter>,
) -> Result<Vec<ApiEndpointSummary>, String> {
    let filter = filter.unwrap_or_default();
    let needle = filter.query.trim().to_lowercase();
    let method = filter.method.trim().to_ascii_uppercase();
    let tag = filter.tag.trim().to_lowercase();
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT id,source_id,operation_id,method,path,title,description,tags_json,deprecated,updated_at,search_text FROM api_endpoints WHERE source_id=?1 ORDER BY path,method",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([source_id], |row| {
            let tags = tags_from_json(row.get(7)?);
            Ok((
                ApiEndpointSummary {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    operation_id: row.get(2)?,
                    method: row.get(3)?,
                    path: row.get(4)?,
                    title: row.get(5)?,
                    description: row.get(6)?,
                    tags,
                    deprecated: row.get(8)?,
                    updated_at: row.get(9)?,
                },
                row.get::<_, String>(10)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        let (item, search_text) = row.map_err(|error| error.to_string())?;
        if !needle.is_empty() && !search_text.contains(&needle) {
            continue;
        }
        if !method.is_empty() && item.method != method {
            continue;
        }
        if !tag.is_empty() && !item.tags.iter().any(|value| value.to_lowercase() == tag) {
            continue;
        }
        if filter
            .deprecated
            .is_some_and(|value| item.deprecated != value)
        {
            continue;
        }
        result.push(item);
    }
    Ok(result)
}

fn endpoint_detail(state: &DatabaseState, endpoint_id: &str) -> Result<ApiEndpointDetail, String> {
    state.connect()?.query_row(
        "SELECT e.id,e.source_id,e.operation_id,e.method,e.path,e.title,e.description,e.tags_json,e.deprecated,e.updated_at,s.project_profile_id,p.display_name,p.repository_path,s.document_title,s.openapi_version,s.last_synced_at,e.document_json
         FROM api_endpoints e JOIN api_sources s ON s.id=e.source_id JOIN project_profiles p ON p.id=s.project_profile_id WHERE e.id=?1",
        [endpoint_id],
        |row| {
            let raw: String = row.get(16)?;
            Ok(ApiEndpointDetail {
                summary: ApiEndpointSummary { id:row.get(0)?,source_id:row.get(1)?,operation_id:row.get(2)?,method:row.get(3)?,path:row.get(4)?,title:row.get(5)?,description:row.get(6)?,tags:tags_from_json(row.get(7)?),deprecated:row.get(8)?,updated_at:row.get(9)? },
                project_profile_id:row.get(10)?, project_name:row.get(11)?,repository_path:row.get(12)?,document_title:row.get(13)?,openapi_version:row.get(14)?,last_synced_at:row.get(15)?,document:serde_json::from_str(&raw).unwrap_or_else(|_| json!({})),
            })
        },
    ).optional().map_err(|error| error.to_string())?.ok_or_else(|| "接口文档不存在，可能已在最近同步中删除。".into())
}

#[tauri::command]
pub fn get_api_endpoint(
    state: tauri::State<'_, DatabaseState>,
    endpoint_id: String,
) -> Result<ApiEndpointDetail, String> {
    endpoint_detail(&state, &endpoint_id)
}

fn sensitive_name(value: &str) -> bool {
    let name = value.to_ascii_lowercase().replace(['-', '_'], "");
    [
        "authorization",
        "token",
        "apikey",
        "password",
        "secret",
        "cookie",
        "session",
    ]
    .iter()
    .any(|part| name.contains(part))
}

fn redacted_value(value: &Value, key: &str) -> Value {
    if sensitive_name(key) && !value.is_null() {
        return Value::String("[已隐藏]".into());
    }
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(child_key, child)| (child_key.clone(), redacted_value(child, child_key)))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| redacted_value(item, key)).collect())
        }
        _ => value.clone(),
    }
}

fn markdown_value(value: &Value, key: &str) -> String {
    let value = redacted_value(value, key);
    match value {
        Value::String(text) => text,
        Value::Null => String::new(),
        other => serde_json::to_string_pretty(&other).unwrap_or_default(),
    }
}

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn schema_type(schema: &Value) -> String {
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        if kind == "array" {
            return format!(
                "array<{}>",
                schema_type(schema.get("items").unwrap_or(&Value::Null))
            );
        }
        let format = text(schema.get("format"));
        return if format.is_empty() {
            kind.into()
        } else {
            format!("{kind}({format})")
        };
    }
    if schema.get("oneOf").is_some() {
        "oneOf".into()
    } else if schema.get("anyOf").is_some() {
        "anyOf".into()
    } else if schema.get("allOf").is_some() {
        "allOf".into()
    } else if schema.get("properties").is_some() {
        "object".into()
    } else {
        "未知".into()
    }
}

fn schema_rows(
    schema: &Value,
    prefix: &str,
    required: bool,
    depth: usize,
    rows: &mut Vec<Vec<String>>,
) {
    if depth > 6 {
        return;
    }
    if !prefix.is_empty() {
        rows.push(vec![
            prefix.into(),
            schema_type(schema),
            if required { "是".into() } else { "否".into() },
            text(schema.get("description")),
            markdown_value(schema.get("example").unwrap_or(&Value::Null), prefix),
        ]);
    }
    let required_names = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            let child_prefix = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            schema_rows(
                child,
                &child_prefix,
                required_names.contains(name.as_str()),
                depth + 1,
                rows,
            );
        }
    }
    if schema.get("type").and_then(Value::as_str) == Some("array") {
        if let Some(items) = schema.get("items") {
            schema_rows(items, &format!("{prefix}[]"), required, depth + 1, rows);
        }
    }
}

fn write_table(output: &mut String, headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        output.push_str("暂无。\n\n");
        return;
    }
    output.push('|');
    for header in headers {
        output.push_str(&format!(" {header} |"));
    }
    output.push('\n');
    output.push('|');
    for _ in headers {
        output.push_str(" --- |");
    }
    output.push('\n');
    for row in rows {
        output.push('|');
        for value in row {
            output.push_str(&format!(" {} |", cell(value)));
        }
        output.push('\n');
    }
    output.push('\n');
}

fn render_markdown(detail: &ApiEndpointDetail) -> String {
    let mut output = format!(
        "# {}\n\n- 项目：{}\n- 接口：`{} {}`\n- OpenAPI：{}\n- 最后同步：{}\n",
        detail.summary.title,
        detail.project_name,
        detail.summary.method,
        detail.summary.path,
        detail.openapi_version,
        detail.last_synced_at.as_deref().unwrap_or("尚未同步"),
    );
    if !detail.summary.operation_id.is_empty() {
        output.push_str(&format!(
            "- Operation ID：`{}`\n",
            detail.summary.operation_id
        ));
    }
    if !detail.summary.tags.is_empty() {
        output.push_str(&format!("- 标签：{}\n", detail.summary.tags.join("、")));
    }
    if detail.summary.deprecated {
        output.push_str("- 状态：已弃用\n");
    }
    output.push_str(&format!(
        "\n## 说明\n\n{}\n\n",
        if detail.summary.description.is_empty() {
            "暂无说明。"
        } else {
            &detail.summary.description
        }
    ));

    output.push_str("## 请求参数\n\n");
    let parameter_rows = detail
        .document
        .get("parameters")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|parameter| {
                    let name = text(parameter.get("name"));
                    let location = text(parameter.get("in"));
                    let schema = parameter.get("schema").unwrap_or(&Value::Null);
                    vec![
                        name.clone(),
                        location,
                        schema_type(schema),
                        if parameter
                            .get("required")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            "是".into()
                        } else {
                            "否".into()
                        },
                        text(parameter.get("description")),
                        markdown_value(
                            parameter
                                .get("example")
                                .or_else(|| schema.get("example"))
                                .unwrap_or(&Value::Null),
                            &name,
                        ),
                    ]
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    write_table(
        &mut output,
        &["名称", "位置", "类型", "必填", "说明", "示例"],
        &parameter_rows,
    );

    output.push_str("## 请求体\n\n");
    if let Some(content) = detail
        .document
        .pointer("/requestBody/content")
        .and_then(Value::as_object)
    {
        for (content_type, body) in content {
            output.push_str(&format!("### `{content_type}`\n\n"));
            let mut rows = Vec::new();
            schema_rows(
                body.get("schema").unwrap_or(&Value::Null),
                "",
                detail
                    .document
                    .pointer("/requestBody/required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                0,
                &mut rows,
            );
            write_table(
                &mut output,
                &["字段", "类型", "必填", "说明", "示例"],
                &rows,
            );
            if let Some(example) = body.get("example") {
                output.push_str(&format!(
                    "```json\n{}\n```\n\n",
                    markdown_value(example, "body")
                ));
            }
        }
    } else {
        output.push_str("无请求体。\n\n");
    }

    output.push_str("## 响应\n\n");
    if let Some(responses) = detail.document.get("responses").and_then(Value::as_object) {
        for (status, response) in responses {
            output.push_str(&format!(
                "### {status} {}\n\n",
                text(response.get("description"))
            ));
            if let Some(content) = response.get("content").and_then(Value::as_object) {
                for (content_type, body) in content {
                    output.push_str(&format!("`{content_type}`\n\n"));
                    let mut rows = Vec::new();
                    schema_rows(
                        body.get("schema").unwrap_or(&Value::Null),
                        "",
                        false,
                        0,
                        &mut rows,
                    );
                    write_table(
                        &mut output,
                        &["字段", "类型", "必填", "说明", "示例"],
                        &rows,
                    );
                    if let Some(example) = body.get("example") {
                        output.push_str(&format!(
                            "```json\n{}\n```\n\n",
                            markdown_value(example, "response")
                        ));
                    }
                    if let Some(examples) = body.get("examples") {
                        output.push_str(&format!(
                            "```json\n{}\n```\n\n",
                            markdown_value(examples, "response")
                        ));
                    }
                }
            }
        }
    } else {
        output.push_str("暂无响应定义。\n\n");
    }
    output.push_str("## 鉴权\n\n");
    let security = detail
        .document
        .get("security")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .flat_map(|item| item.keys().cloned())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    output.push_str(if security.is_empty() {
        "未声明鉴权方案。\n\n"
    } else {
        ""
    });
    for name in security {
        output.push_str(&format!("- {name}\n"));
    }
    if let Some(warnings) = detail.document.get("warnings").and_then(Value::as_array) {
        if !warnings.is_empty() {
            output.push_str("\n## 解析提示\n\n");
            for warning in warnings.iter().filter_map(Value::as_str) {
                output.push_str(&format!("- {warning}\n"));
            }
        }
    }
    output
}

#[tauri::command]
pub fn render_api_endpoint_markdown(
    state: tauri::State<'_, DatabaseState>,
    endpoint_id: String,
) -> Result<String, String> {
    Ok(render_markdown(&endpoint_detail(&state, &endpoint_id)?))
}

#[tauri::command]
pub fn save_api_endpoint_to_knowledge(
    state: tauri::State<'_, DatabaseState>,
    endpoint_id: String,
) -> Result<KnowledgeItem, String> {
    let detail = endpoint_detail(&state, &endpoint_id)?;
    let markdown = render_markdown(&detail);
    let now = Utc::now().to_rfc3339();
    save_knowledge_for_state(
        &state,
        KnowledgeItem {
            id: format!("api:{}", detail.summary.id),
            kind: "experience".into(),
            title: format!("[API] {}", detail.summary.title),
            content: markdown,
            project: Some(detail.project_name),
            source_type: Some("api".into()),
            source_id: Some(detail.summary.id),
            tags: format!(
                "API,Apifox,{}{}",
                detail.summary.method,
                if detail.summary.tags.is_empty() {
                    String::new()
                } else {
                    format!(",{}", detail.summary.tags.join(","))
                }
            ),
            confirmed: true,
            created_at: now.clone(),
            updated_at: now,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({
          "openapi":"3.1.0","info":{"title":"示例服务"},
          "paths":{"/users/{id}":{"get":{"summary":"查询用户","operationId":"getUser","tags":["用户"],"parameters":[{"name":"id","in":"path","required":true,"schema":{"type":"string"}},{"name":"Authorization","in":"header","example":"Bearer real-secret"}],"responses":{"200":{"description":"成功","content":{"application/json":{"schema":{"$ref":"#/components/schemas/User"},"example":{"id":"1","token":"secret"}}}}}}}},
          "components":{"schemas":{"User":{"type":"object","required":["id"],"properties":{"id":{"type":"string"},"name":{"type":"string"}}}}}
        })
    }

    #[test]
    fn parses_local_refs_and_keeps_stable_endpoint_ids() {
        let (_, version, endpoints) = parse_openapi(&sample(), "source-1").unwrap();
        assert_eq!(version, "3.1.0");
        assert_eq!(endpoints.len(), 1);
        assert_eq!(
            endpoints[0].id,
            endpoint_id("source-1", "get", "/users/{id}")
        );
        assert_eq!(
            endpoints[0]
                .document
                .pointer("/responses/200/content/application~1json/schema/properties/id/type")
                .and_then(Value::as_str),
            Some("string")
        );
    }

    #[test]
    fn export_contract_uses_the_fixed_read_only_openapi_shape() {
        let payload = export_payload();
        assert_eq!(APIFOX_API_VERSION, "2024-03-28");
        assert_eq!(
            payload.pointer("/scope/type").and_then(Value::as_str),
            Some("ALL")
        );
        assert_eq!(
            payload.get("oasVersion").and_then(Value::as_str),
            Some("3.1")
        );
        assert_eq!(
            payload.get("exportFormat").and_then(Value::as_str),
            Some("JSON")
        );
        assert_eq!(
            payload
                .pointer("/options/addFoldersToTags")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .pointer("/options/includeApifoxExtensionProperties")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn circular_and_external_refs_do_not_trigger_network_resolution() {
        let root = json!({"openapi":"3.1.0","info":{"title":"循环"},"paths":{"/x":{"get":{"responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Node"}}}}}}}},"components":{"schemas":{"Node":{"type":"object","properties":{"next":{"$ref":"#/components/schemas/Node"},"remote":{"$ref":"https://example.com/schema.json"}}}}}});
        let (_, _, items) = parse_openapi(&root, "s").unwrap();
        let warnings = items[0]
            .document
            .get("warnings")
            .and_then(Value::as_array)
            .unwrap();
        assert!(warnings.len() >= 2);
    }

    #[test]
    fn markdown_masks_sensitive_examples() {
        let (_, _, items) = parse_openapi(&sample(), "source-1").unwrap();
        let item = &items[0];
        let markdown = render_markdown(&ApiEndpointDetail {
            summary: ApiEndpointSummary {
                id: item.id.clone(),
                source_id: "source-1".into(),
                operation_id: item.operation_id.clone(),
                method: item.method.clone(),
                path: item.path.clone(),
                title: item.title.clone(),
                description: item.description.clone(),
                tags: item.tags.clone(),
                deprecated: false,
                updated_at: "2026-08-31".into(),
            },
            project_profile_id: "p".into(),
            project_name: "项目".into(),
            repository_path: "F:/project".into(),
            document_title: "示例".into(),
            openapi_version: "3.1.0".into(),
            last_synced_at: Some("2026-08-31".into()),
            document: item.document.clone(),
        });
        assert!(markdown.contains("查询用户"));
        assert!(markdown.contains("[已隐藏]"));
        assert!(!markdown.contains("real-secret"));
        assert!(!markdown.contains("\"token\": \"secret\""));
    }
}
