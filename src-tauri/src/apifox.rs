use crate::database::DatabaseState;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use keyring::Entry;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, COOKIE},
    Method, Url,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const CREDENTIAL_SERVICE: &str = "ai-personal-workbench";
const CREDENTIAL_USER: &str = "apifox-access-token";
const APIFOX_EXPORT_URL: &str = "https://api.apifox.com/v1/projects";
const APIFOX_API_VERSION: &str = "2024-03-28";
const MAX_OPENAPI_BYTES: u64 = 32 * 1024 * 1024;
const MAX_API_TEST_RESPONSE_BYTES: usize = 1024 * 1024;
const API_EXPORT_SERVER_ADDRESS: &str = "127.0.0.1:17890";

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
    pub project_profile_id: Option<String>,
    pub external_project_id: String,
    pub apifox_project_name: String,
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
    pub external_project_id: String,
    #[serde(default)]
    pub apifox_project_name: String,
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
    pub favorite: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointDetail {
    #[serde(flatten)]
    pub summary: ApiEndpointSummary,
    pub external_project_id: String,
    pub apifox_project_name: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTestConfig {
    pub source_id: String,
    pub base_url: String,
    pub token_header: String,
    pub token_configured: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTestConfigUpdate {
    pub source_id: String,
    pub base_url: String,
    pub token_header: String,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTestResult {
    pub url: String,
    pub method: String,
    pub status: u16,
    pub status_text: String,
    pub success: bool,
    pub elapsed_ms: u128,
    pub content_type: String,
    pub request_data: Value,
    pub response_data: Value,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTestPreview {
    pub endpoint_id: String,
    pub url: String,
    pub method: String,
    pub content_type: String,
    pub headers: HashMap<String, String>,
    pub request_data: Value,
    pub body: Option<Value>,
    pub requires_confirmation: bool,
    pub warning: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTestExecutionRequest {
    pub endpoint_id: String,
    pub url: String,
    pub body: Option<Value>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTagExport {
    pub source_id: String,
    pub tag_path: String,
    pub openapi_url: String,
    pub endpoint_count: i64,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCodeTemplate {
    pub source_id: String,
    pub client: String,
    pub function_prefix: String,
    pub import_path: String,
    pub include_import: bool,
    pub typescript: bool,
}

#[derive(Clone)]
pub struct ApiExportServerState {
    base_url: String,
    error: String,
}

#[derive(Debug)]
struct PreparedApiTest {
    url: Url,
    method: Method,
    headers: HeaderMap,
    content_type: String,
    request_data: Value,
    body: Option<Value>,
}

#[derive(Debug, Clone)]
struct SourceRow {
    id: String,
    apifox_project_name: String,
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

fn api_test_token_entry(source_id: &str) -> Result<Entry, String> {
    Entry::new(
        CREDENTIAL_SERVICE,
        &format!("api-test-token-{}", stable_hash(source_id)),
    )
    .map_err(|error| error.to_string())
}

fn api_test_token(source_id: &str) -> Option<String> {
    api_test_token_entry(source_id)
        .ok()?
        .get_password()
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

fn strip_apifox_extensions(value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !key.to_ascii_lowercase().starts_with("x-apifox-"))
                .map(|(key, child)| (key.clone(), strip_apifox_extensions(child)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(strip_apifox_extensions).collect()),
        _ => value.clone(),
    }
}

fn source_id(external_project_id: &str) -> String {
    format!("api-source-{}", stable_hash(external_project_id))
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
    let connection = state.connect()?;
    connection.execute(
        "UPDATE api_sources SET sync_status=CASE WHEN last_synced_at IS NULL THEN 'error' ELSE 'stale' END,last_error=CASE WHEN last_error='' THEN '上次同步未正常完成。' ELSE last_error END WHERE sync_status='syncing'",
        [],
    ).map_err(|error| error.to_string())?;
    let mut statement = connection.prepare(
        "SELECT id,project_profile_id,external_project_id,apifox_project_name,document_title,openapi_version,sync_status,endpoint_count,last_synced_at,last_error,created_at,updated_at
         FROM api_sources
         ORDER BY COALESCE(NULLIF(apifox_project_name,''),NULLIF(document_title,''),external_project_id) COLLATE NOCASE",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ApiSource {
                id: row.get(0)?,
                project_profile_id: row.get(1)?,
                external_project_id: row.get(2)?,
                apifox_project_name: row.get(3)?,
                document_title: row.get(4)?,
                openapi_version: row.get(5)?,
                sync_status: row.get(6)?,
                endpoint_count: row.get(7)?,
                last_synced_at: row.get(8)?,
                last_error: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
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
    let id = save_api_source_record(&state, &source)?;
    list_api_sources(state)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "Apifox 项目保存后无法读取。".to_string())
}

fn save_api_source_record(
    state: &DatabaseState,
    source: &ApiSourceUpdate,
) -> Result<String, String> {
    let external_id = source.external_project_id.trim();
    let apifox_project_name =
        validate_apifox_project_name(&source.apifox_project_name, external_id)?;
    if !valid_external_project_id(external_id) {
        return Err("Apifox 项目 ID 只能包含字母、数字、短横线或下划线。".into());
    }
    let connection = state.connect()?;
    if source.id.trim().is_empty() {
        let exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM api_sources WHERE external_project_id=?1)",
                [external_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if exists {
            return Err("该 Apifox 项目 ID 已经存在，请直接编辑现有项目。".into());
        }
    }
    let id = if source.id.trim().is_empty() {
        source_id(external_id)
    } else {
        source.id.trim().to_string()
    };
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO api_sources(id,project_profile_id,provider,external_project_id,apifox_project_name,created_at,updated_at)
         VALUES(?1,NULL,'apifox',?2,?3,?4,?4)
         ON CONFLICT(id) DO UPDATE SET external_project_id=excluded.external_project_id,apifox_project_name=excluded.apifox_project_name,updated_at=excluded.updated_at",
        params![id, external_id, apifox_project_name, now],
    ).map_err(|error| {
        if error.to_string().contains("UNIQUE constraint failed") {
            "该 Apifox 项目 ID 已经存在，请直接编辑现有项目。".to_string()
        } else {
            error.to_string()
        }
    })?;
    Ok(id)
}

fn validate_apifox_project_name(value: &str, external_id: &str) -> Result<String, String> {
    let value = value.trim();
    let value = if value.is_empty() { external_id } else { value };
    if value.len() > 120 || value.contains(['\r', '\n']) {
        return Err("Apifox 项目名称不能超过 120 个字符，也不能包含换行。".into());
    }
    Ok(value.to_string())
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
    if let Ok(entry) = api_test_token_entry(&source_id) {
        let _ = entry.delete_credential();
    }
    Ok(())
}

fn source_row(state: &DatabaseState, id: &str) -> Result<SourceRow, String> {
    state
        .connect()?
        .query_row(
            "SELECT id,COALESCE(NULLIF(apifox_project_name,''),NULLIF(document_title,''),'Apifox ' || external_project_id),external_project_id,last_synced_at FROM api_sources WHERE id=?1",
            [id],
            |row| {
                Ok(SourceRow {
                    id: row.get(0)?,
                    apifox_project_name: row.get(1)?,
                    external_project_id: row.get(2)?,
                    last_synced_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Apifox 接口项目不存在。".to_string())
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
    let root = strip_apifox_extensions(&fetch_openapi(&source.external_project_id, &token)?);
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
            .prepare("SELECT id,document_hash,is_favorite FROM api_endpoints WHERE source_id=?1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([&source.id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, String>(1)?, row.get::<_, bool>(2)?),
                ))
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
                .is_some_and(|(hash, _)| hash != &item.document_hash)
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
            "INSERT INTO api_endpoints(id,source_id,operation_id,method,path,title,description,tags_json,deprecated,document_json,document_hash,search_text,is_favorite,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![endpoint.id,source.id,endpoint.operation_id,endpoint.method,endpoint.path,endpoint.title,endpoint.description,serde_json::to_string(&endpoint.tags).map_err(|error| error.to_string())?,endpoint.deprecated,serde_json::to_string(&endpoint.document).map_err(|error| error.to_string())?,endpoint.document_hash,endpoint.search_text,existing.get(&endpoint.id).map(|(_,favorite)|*favorite).unwrap_or(false),now],
        ).map_err(|error| error.to_string())?;
    }
    transaction.execute(
        "UPDATE api_sources SET document_title=?2,openapi_version=?3,sync_status='ready',endpoint_count=?4,content_hash=?5,openapi_document_json=?6,last_synced_at=?7,last_error='',updated_at=?7 WHERE id=?1",
        params![source.id,document_title,version,endpoints.len() as i64,content_hash,serde_json::to_string(&root).map_err(|error| error.to_string())?,now],
    ).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(ApiSyncSummary {
        source_id: source.id.clone(),
        project_name: source.apifox_project_name.clone(),
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
                    project_name: source.apifox_project_name,
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
        "SELECT id,source_id,operation_id,method,path,title,description,tags_json,deprecated,is_favorite,updated_at,search_text FROM api_endpoints WHERE source_id=?1 ORDER BY path,method",
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
                    favorite: row.get(9)?,
                    updated_at: row.get(10)?,
                },
                row.get::<_, String>(11)?,
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
        "SELECT e.id,e.source_id,e.operation_id,e.method,e.path,e.title,e.description,e.tags_json,e.deprecated,e.is_favorite,e.updated_at,s.external_project_id,COALESCE(NULLIF(s.apifox_project_name,''),NULLIF(s.document_title,''),'Apifox ' || s.external_project_id),s.document_title,s.openapi_version,s.last_synced_at,e.document_json
         FROM api_endpoints e JOIN api_sources s ON s.id=e.source_id WHERE e.id=?1",
        [endpoint_id],
        |row| {
            let raw: String = row.get(16)?;
            Ok(ApiEndpointDetail {
                summary: ApiEndpointSummary { id:row.get(0)?,source_id:row.get(1)?,operation_id:row.get(2)?,method:row.get(3)?,path:row.get(4)?,title:row.get(5)?,description:row.get(6)?,tags:tags_from_json(row.get(7)?),deprecated:row.get(8)?,favorite:row.get(9)?,updated_at:row.get(10)? },
                external_project_id:row.get(11)?,apifox_project_name:row.get(12)?,document_title:row.get(13)?,openapi_version:row.get(14)?,last_synced_at:row.get(15)?,document:serde_json::from_str(&raw).unwrap_or_else(|_| json!({})),
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

fn normalized_tag(value: &str) -> String {
    value
        .split(['/', '\\', '›', '>'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn tag_includes(tag: &str, requested: &str) -> bool {
    let tag = normalized_tag(tag);
    let requested = normalized_tag(requested);
    tag == requested || tag.starts_with(&format!("{requested}/"))
}

fn tag_endpoint_count(
    state: &DatabaseState,
    source_id: &str,
    tag_path: &str,
) -> Result<i64, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT tags_json FROM api_endpoints WHERE source_id=?1")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([source_id], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    let mut count = 0_i64;
    for row in rows {
        if tags_from_json(row.map_err(|error| error.to_string())?)
            .iter()
            .any(|tag| tag_includes(tag, tag_path))
        {
            count += 1;
        }
    }
    Ok(count)
}

fn tag_export_url(base_url: &str, source_id: &str, tag_path: &str) -> String {
    let encoded_tag = URL_SAFE_NO_PAD.encode(normalized_tag(tag_path).as_bytes());
    format!(
        "{}/openapi/{}/{}.json?version=3.0",
        base_url.trim_end_matches('/'),
        source_id,
        encoded_tag
    )
}

fn sanitized_document_url(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return value.to_string();
    };
    if !url.username().is_empty() {
        let _ = url.set_username("");
    }
    if url.password().is_some() {
        let _ = url.set_password(None);
    }
    let pairs = url
        .query_pairs()
        .map(|(key, value)| {
            let value = if sensitive_name(&key) {
                "[已隐藏]".into()
            } else {
                value.into_owned()
            };
            (key.into_owned(), value)
        })
        .collect::<Vec<_>>();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}

#[tauri::command]
pub fn get_api_tag_export(
    state: tauri::State<'_, DatabaseState>,
    server: tauri::State<'_, ApiExportServerState>,
    source_id: String,
    tag_path: String,
) -> Result<ApiTagExport, String> {
    let source_id = source_id.trim();
    let tag_path = normalized_tag(&tag_path);
    if tag_path.is_empty() || tag_path.len() > 500 {
        return Err("接口标签无效。".into());
    }
    let count = tag_endpoint_count(&state, source_id, &tag_path)?;
    if count == 0 {
        return Err("该标签下没有可导出的接口。".into());
    }
    // 旧版本只缓存接口详情；生成本地 URL 前先确认完整根文档已经通过新版本同步。
    build_tag_openapi(&state, source_id, &tag_path, "3.0")?;
    if !server.error.is_empty() {
        return Err(server.error.clone());
    }
    Ok(ApiTagExport {
        source_id: source_id.to_string(),
        tag_path: tag_path.clone(),
        openapi_url: tag_export_url(&server.base_url, source_id, &tag_path),
        endpoint_count: count,
        available: true,
    })
}

fn sanitize_openapi_export(value: &Value, field_name: &str, in_properties: bool) -> Value {
    match value {
        Value::Object(object) => {
            let effective_name = object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(field_name);
            Value::Object(
                object
                    .iter()
                    .filter(|(key, _)| !key.to_ascii_lowercase().starts_with("x-apifox-"))
                    .map(|(key, child)| {
                        let must_hide_example =
                            matches!(key.as_str(), "example" | "examples" | "default")
                                && sensitive_name(effective_name);
                        let must_hide_value = sensitive_name(key)
                            && !in_properties
                            && !child.is_object()
                            && !child.is_array();
                        let value = if must_hide_example || must_hide_value {
                            Value::String("[已隐藏]".into())
                        } else if key == "url" {
                            child
                                .as_str()
                                .map(sanitized_document_url)
                                .map(Value::String)
                                .unwrap_or_else(|| sanitize_openapi_export(child, key, false))
                        } else {
                            sanitize_openapi_export(child, key, key == "properties")
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| sanitize_openapi_export(item, field_name, in_properties))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn convert_openapi_31_to_30(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("$schema");
            object.remove("jsonSchemaDialect");
            if let Some(Value::Array(types)) = object.get("type") {
                let nullable = types.iter().any(|item| item.as_str() == Some("null"));
                let remaining = types
                    .iter()
                    .filter(|item| item.as_str() != Some("null"))
                    .cloned()
                    .collect::<Vec<_>>();
                if nullable && remaining.len() == 1 {
                    object.insert("type".into(), remaining[0].clone());
                    object.insert("nullable".into(), Value::Bool(true));
                }
            }
            if let Some(constant) = object.remove("const") {
                object.entry("enum").or_insert_with(|| json!([constant]));
            }
            for child in object.values_mut() {
                convert_openapi_31_to_30(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                convert_openapi_31_to_30(child);
            }
        }
        _ => {}
    }
}

fn build_tag_openapi(
    state: &DatabaseState,
    source_id: &str,
    tag_path: &str,
    requested_version: &str,
) -> Result<Value, String> {
    let raw = state
        .connect()?
        .query_row(
            "SELECT openapi_document_json FROM api_sources WHERE id=?1",
            [source_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "接口文档项目关联不存在。".to_string())?;
    let root: Value = serde_json::from_str(&raw)
        .map_err(|_| "完整 OpenAPI 缓存无法解析，请重新同步该项目。".to_string())?;
    build_tag_openapi_document(root, tag_path, requested_version)
}

fn build_tag_openapi_document(
    mut root: Value,
    tag_path: &str,
    requested_version: &str,
) -> Result<Value, String> {
    let paths = root
        .get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| "完整 OpenAPI 缓存尚未生成，请重新同步该项目。".to_string())?;
    let mut filtered_paths = Map::new();
    for (path, path_item) in paths {
        let Some(path_object) = path_item.as_object() else {
            continue;
        };
        let mut filtered_item = Map::new();
        for key in ["summary", "description", "servers", "parameters"] {
            if let Some(value) = path_object.get(key) {
                filtered_item.insert(key.into(), value.clone());
            }
        }
        let mut matched = false;
        for method in operation_methods() {
            let Some(operation) = path_object.get(*method) else {
                continue;
            };
            let includes_tag = operation
                .get("tags")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .any(|tag| tag_includes(tag, tag_path));
            if includes_tag {
                filtered_item.insert((*method).into(), operation.clone());
                matched = true;
            }
        }
        if matched {
            filtered_paths.insert(path.clone(), Value::Object(filtered_item));
        }
    }
    if filtered_paths.is_empty() {
        return Err("该标签下没有可导出的接口。".into());
    }
    root.as_object_mut()
        .ok_or_else(|| "OpenAPI 根节点格式异常。".to_string())?
        .insert("paths".into(), Value::Object(filtered_paths));
    if let Some(title) = root.pointer_mut("/info/title") {
        if let Some(value) = title.as_str() {
            *title = Value::String(format!("{value} - {tag_path}"));
        }
    }
    let mut root = sanitize_openapi_export(&root, "", false);
    if requested_version == "3.0" {
        if let Some(object) = root.as_object_mut() {
            object.insert("openapi".into(), Value::String("3.0.3".into()));
            object.remove("webhooks");
        }
        convert_openapi_31_to_30(&mut root);
    }
    Ok(root)
}

fn write_http_response(stream: &mut TcpStream, status: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)
}

fn serve_export_request(state: &DatabaseState, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buffer = [0_u8; 8192];
    let Ok(size) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..size]);
    let Some(request_line) = request.lines().next() else {
        return;
    };
    let parts = request_line.split_whitespace().collect::<Vec<_>>();
    if parts.len() < 2 || parts[0] != "GET" {
        let body = r#"{"error":"仅支持 GET 请求"}"#.as_bytes();
        let _ = write_http_response(&mut stream, "405 Method Not Allowed", body);
        return;
    }
    let (path, query) = parts[1].split_once('?').unwrap_or((parts[1], ""));
    if path == "/health" {
        let _ = write_http_response(&mut stream, "200 OK", br#"{"status":"ok"}"#);
        return;
    }
    let route = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
    if route.len() != 3 || route[0] != "openapi" || !route[2].ends_with(".json") {
        let _ = write_http_response(
            &mut stream,
            "404 Not Found",
            r#"{"error":"OpenAPI 地址不存在"}"#.as_bytes(),
        );
        return;
    }
    let encoded_tag = route[2].trim_end_matches(".json");
    let tag_path = URL_SAFE_NO_PAD
        .decode(encoded_tag)
        .ok()
        .and_then(|value| String::from_utf8(value).ok());
    let Some(tag_path) = tag_path else {
        let _ = write_http_response(
            &mut stream,
            "400 Bad Request",
            r#"{"error":"标签编码无效"}"#.as_bytes(),
        );
        return;
    };
    let version = query
        .split('&')
        .find_map(|part| part.strip_prefix("version="))
        .unwrap_or("3.0");
    if !matches!(version, "3.0" | "3.1") {
        let _ = write_http_response(
            &mut stream,
            "400 Bad Request",
            r#"{"error":"仅支持 OpenAPI 3.0 或 3.1"}"#.as_bytes(),
        );
        return;
    }
    match build_tag_openapi(state, route[1], &tag_path, version) {
        Ok(document) => {
            let body = serde_json::to_vec_pretty(&document).unwrap_or_else(|_| b"{}".to_vec());
            let _ = write_http_response(&mut stream, "200 OK", &body);
        }
        Err(error) => {
            let body =
                serde_json::to_vec(&json!({"error":error})).unwrap_or_else(|_| b"{}".to_vec());
            let _ = write_http_response(&mut stream, "404 Not Found", &body);
        }
    }
}

pub fn start_api_export_server(state: DatabaseState) -> ApiExportServerState {
    match TcpListener::bind(API_EXPORT_SERVER_ADDRESS) {
        Ok(listener) => {
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    match stream {
                        Ok(stream) => serve_export_request(&state, stream),
                        Err(error) => eprintln!("本地 OpenAPI 导出服务连接失败：{error}"),
                    }
                }
            });
            ApiExportServerState {
                base_url: format!("http://{API_EXPORT_SERVER_ADDRESS}"),
                error: String::new(),
            }
        }
        Err(error) => ApiExportServerState {
            base_url: String::new(),
            error: format!("本地 OpenAPI 导出服务无法启动（端口 17890）：{error}"),
        },
    }
}

fn validate_test_base_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let parsed = Url::parse(value)
        .map_err(|_| "请求基地址格式无效，请填写完整的 http 或 https 地址。".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("请求基地址只支持完整的 http 或 https 地址。".into());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("请求基地址不能包含账号或密码。".into());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("请求基地址不能包含查询参数或锚点。".into());
    }
    Ok(value.trim_end_matches('/').to_string())
}

fn validate_token_header(value: &str) -> Result<String, String> {
    let value = value.trim();
    let name = HeaderName::from_bytes(value.as_bytes())
        .map_err(|_| "请求头名称格式无效，例如 Authorization 或 hlzt-token。".to_string())?;
    if ["host", "content-length", "transfer-encoding"].contains(&name.as_str()) {
        return Err("该请求头由网络组件自动管理，请换一个 Token 请求头名称。".into());
    }
    Ok(name.as_str().to_string())
}

fn api_test_config_for_state(
    state: &DatabaseState,
    source_id: &str,
) -> Result<ApiTestConfig, String> {
    state
        .connect()?
        .query_row(
            "SELECT request_base_url,request_token_header FROM api_sources WHERE id=?1",
            [source_id],
            |row| {
                Ok(ApiTestConfig {
                    source_id: source_id.to_string(),
                    base_url: row.get(0)?,
                    token_header: row.get(1)?,
                    token_configured: api_test_token(source_id).is_some(),
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "接口文档项目关联不存在。".into())
}

#[tauri::command]
pub fn get_api_test_config(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
) -> Result<ApiTestConfig, String> {
    api_test_config_for_state(&state, source_id.trim())
}

#[tauri::command]
pub fn save_api_test_config(
    state: tauri::State<'_, DatabaseState>,
    config: ApiTestConfigUpdate,
) -> Result<ApiTestConfig, String> {
    let source_id = config.source_id.trim();
    let base_url = validate_test_base_url(&config.base_url)?;
    let token_header = validate_token_header(&config.token_header)?;
    let changed = state
        .connect()?
        .execute(
            "UPDATE api_sources SET request_base_url=?2,request_token_header=?3,updated_at=?4 WHERE id=?1",
            params![source_id, base_url, token_header, Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("接口文档项目关联不存在。".into());
    }
    let token = config.token.trim();
    if !token.is_empty() {
        HeaderValue::from_str(token).map_err(|_| "Token 包含无法写入请求头的字符。".to_string())?;
        api_test_token_entry(source_id)?
            .set_password(token)
            .map_err(|error| format!("保存接口测试 Token 失败：{error}"))?;
    }
    api_test_config_for_state(&state, source_id)
}

#[tauri::command]
pub fn clear_api_test_token(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
) -> Result<ApiTestConfig, String> {
    let source_id = source_id.trim();
    api_test_config_for_state(&state, source_id)?;
    match api_test_token_entry(source_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => api_test_config_for_state(&state, source_id),
        Err(error) => Err(format!("清除接口测试 Token 失败：{error}")),
    }
}

fn generated_string(name: &str, format: &str) -> String {
    let name = name.to_ascii_lowercase().replace(['-', '_'], "");
    if format == "uuid" {
        return "00000000-0000-4000-8000-000000000001".into();
    }
    if format == "date" || name.ends_with("date") {
        return Utc::now().format("%Y-%m-%d").to_string();
    }
    if matches!(format, "date-time" | "datetime") || name.contains("time") || name.ends_with("at") {
        return Utc::now().to_rfc3339();
    }
    if name.contains("email") {
        return "test@example.com".into();
    }
    if name.contains("mobile") || name.contains("phone") || name.contains("tel") {
        return "13800000000".into();
    }
    if name.contains("url") || name.contains("link") {
        return "https://example.com".into();
    }
    if name.ends_with("id") || name == "id" {
        return "1".into();
    }
    if name.contains("name") {
        return "自动测试名称".into();
    }
    if name.contains("title") {
        return "自动测试标题".into();
    }
    if name.contains("code") {
        return "TEST001".into();
    }
    if name.contains("password") {
        return "Test@123456".into();
    }
    if name.contains("token") || name.contains("secret") {
        return "test-token".into();
    }
    if name.contains("description") || name.contains("remark") || name.contains("content") {
        return "自动生成的接口测试数据".into();
    }
    "test".into()
}

fn generated_value(schema: &Value, name: &str, depth: usize) -> Value {
    if depth > 6 {
        return Value::Null;
    }
    if let Some(value) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    {
        return value.clone();
    }
    for variant in ["oneOf", "anyOf"] {
        if let Some(value) = schema
            .get(variant)
            .and_then(Value::as_array)
            .and_then(|items| items.first())
        {
            return generated_value(value, name, depth + 1);
        }
    }
    if let Some(items) = schema.get("allOf").and_then(Value::as_array) {
        let mut result = Map::new();
        for item in items {
            if let Value::Object(object) = generated_value(item, name, depth + 1) {
                result.extend(object);
            }
        }
        if !result.is_empty() {
            return Value::Object(result);
        }
    }
    let inferred_type = if schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some()
    {
        "object"
    } else {
        "string"
    };
    match schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or(inferred_type)
    {
        "object" => Value::Object(
            schema
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| {
                    properties
                        .iter()
                        .filter(|(_, value)| {
                            !value
                                .get("readOnly")
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                        })
                        .map(|(key, value)| (key.clone(), generated_value(value, key, depth + 1)))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        "array" => Value::Array(vec![generated_value(
            schema.get("items").unwrap_or(&Value::Null),
            name,
            depth + 1,
        )]),
        "integer" => json!(1),
        "number" => json!(1.0),
        "boolean" => json!(true),
        "null" => Value::Null,
        _ => Value::String(generated_string(
            name,
            schema.get("format").and_then(Value::as_str).unwrap_or(""),
        )),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn encode_path_value(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (*byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn openapi_server_url(document: &Value) -> String {
    let Some(server) = document
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return String::new();
    };
    let mut url = server
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(variables) = server.get("variables").and_then(Value::as_object) {
        for (name, value) in variables {
            if let Some(default) = value.get("default").and_then(Value::as_str) {
                url = url.replace(&format!("{{{name}}}"), default);
            }
        }
    }
    url
}

fn prepare_api_test(
    detail: &ApiEndpointDetail,
    config: &ApiTestConfig,
    token: Option<&str>,
) -> Result<PreparedApiTest, String> {
    let base_url = if config.base_url.trim().is_empty() {
        openapi_server_url(&detail.document)
    } else {
        config.base_url.clone()
    };
    let base_url = validate_test_base_url(&base_url).and_then(|value| {
        if value.is_empty() {
            Err("该接口没有可用的服务地址，请先配置项目的请求基地址。".to_string())
        } else {
            Ok(value)
        }
    })?;
    let mut path = detail.summary.path.clone();
    let mut query_values = Vec::new();
    let mut header_values = HeaderMap::new();
    let mut cookie_values = Vec::new();
    let mut preview = Map::new();
    for parameter in detail
        .document
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = parameter.get("name").and_then(Value::as_str).unwrap_or("");
        let location = parameter.get("in").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() || sensitive_name(name) {
            continue;
        }
        let value = generated_value(parameter.get("schema").unwrap_or(&Value::Null), name, 0);
        let text = scalar_text(&value);
        preview.insert(format!("{location}.{name}"), value.clone());
        match location {
            "path" => path = path.replace(&format!("{{{name}}}"), &encode_path_value(&text)),
            "query" => query_values.push((name.to_string(), text)),
            "header" => {
                let header_name = HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| format!("接口参数中的请求头名称无效：{name}"))?;
                let header_value = HeaderValue::from_str(&text)
                    .map_err(|_| format!("接口参数 {name} 无法写入请求头。"))?;
                header_values.insert(header_name, header_value);
            }
            "cookie" => cookie_values.push(format!("{name}={text}")),
            _ => {}
        }
    }
    if path.contains('{') {
        return Err("接口路径仍包含未定义的 Path 参数，无法自动生成测试地址。".into());
    }
    let mut url = Url::parse(&format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
    .map_err(|_| "自动生成的接口测试地址无效。".to_string())?;
    {
        let mut pairs = url.query_pairs_mut();
        for (name, value) in query_values {
            pairs.append_pair(&name, &value);
        }
    }
    if !cookie_values.is_empty() {
        header_values.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_values.join("; "))
                .map_err(|_| "自动生成的 Cookie 参数无效。".to_string())?,
        );
    }
    if let Some(token) = token {
        header_values.insert(
            HeaderName::from_bytes(config.token_header.as_bytes())
                .map_err(|_| "Token 请求头名称无效。".to_string())?,
            HeaderValue::from_str(token).map_err(|_| "Token 无法写入请求头。".to_string())?,
        );
    }
    let mut content_type = String::new();
    let mut body = None;
    if let Some(content) = detail
        .document
        .pointer("/requestBody/content")
        .and_then(Value::as_object)
    {
        let selected = content
            .get_key_value("application/json")
            .or_else(|| content.get_key_value("application/x-www-form-urlencoded"))
            .or_else(|| content.iter().next());
        if let Some((kind, definition)) = selected {
            if kind == "multipart/form-data" {
                return Err("该接口使用 multipart/form-data，暂不能自动生成文件上传测试。".into());
            }
            content_type = kind.clone();
            let generated = generated_value(
                definition.get("schema").unwrap_or(&Value::Null),
                "requestBody",
                0,
            );
            preview.insert("body".into(), redacted_value(&generated, "body"));
            body = Some(generated);
        }
    }
    Ok(PreparedApiTest {
        url,
        method: Method::from_bytes(detail.summary.method.as_bytes())
            .map_err(|_| "接口请求方法无效。".to_string())?,
        headers: header_values,
        content_type,
        request_data: Value::Object(preview),
        body,
    })
}

fn send_api_test(
    prepared: PreparedApiTest,
    response_secret: Option<&str>,
) -> Result<ApiTestResult, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("创建接口测试请求失败：{error}"))?;
    let mut request = client
        .request(prepared.method.clone(), prepared.url.clone())
        .headers(prepared.headers.clone());
    if let Some(body) = &prepared.body {
        request = if prepared.content_type == "application/x-www-form-urlencoded" {
            request.form(body)
        } else if prepared.content_type.contains("json") {
            request.json(body)
        } else {
            request
                .header(CONTENT_TYPE, &prepared.content_type)
                .body(scalar_text(body))
        };
    }
    let started = Instant::now();
    let response = request
        .send()
        .map_err(|error| format!("接口请求失败：{error}"))?;
    let elapsed_ms = started.elapsed().as_millis();
    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let mut bytes = Vec::new();
    response
        .take((MAX_API_TEST_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取接口响应失败：{error}"))?;
    let truncated = bytes.len() > MAX_API_TEST_RESPONSE_BYTES;
    bytes.truncate(MAX_API_TEST_RESPONSE_BYTES);
    let mut text = String::from_utf8_lossy(&bytes).to_string();
    if let Some(secret) = response_secret {
        text = text.replace(secret, "[已隐藏]");
    }
    let response_data = if text.trim().is_empty() {
        Value::Null
    } else if let Ok(value) = serde_json::from_str::<Value>(&text) {
        redacted_value(&value, "response")
    } else {
        Value::String(text)
    };
    Ok(ApiTestResult {
        url: prepared.url.to_string(),
        method: prepared.method.to_string(),
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        success: status.is_success(),
        elapsed_ms,
        content_type,
        request_data: prepared.request_data,
        response_data,
        truncated,
    })
}

fn api_test_requires_confirmation(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn api_test_preview(endpoint_id: &str, prepared: &PreparedApiTest) -> ApiTestPreview {
    let mut headers: HashMap<String, String> = prepared
        .headers
        .iter()
        .map(|(name, value)| {
            let value = if sensitive_name(name.as_str()) {
                "[已配置，发送时安全注入]".into()
            } else {
                value.to_str().unwrap_or("[无法预览]").to_string()
            };
            (name.as_str().to_string(), value)
        })
        .collect();
    if !prepared.content_type.is_empty() {
        headers.insert("Content-Type".into(), prepared.content_type.clone());
    }
    let requires_confirmation = api_test_requires_confirmation(&prepared.method);
    ApiTestPreview {
        endpoint_id: endpoint_id.to_string(),
        url: prepared.url.to_string(),
        method: prepared.method.to_string(),
        content_type: prepared.content_type.clone(),
        headers,
        request_data: prepared.request_data.clone(),
        body: prepared
            .body
            .clone()
            .map(|value| redacted_value(&value, "body")),
        requires_confirmation,
        warning: if requires_confirmation {
            "该请求可能创建、修改或删除真实业务数据，请核对地址和请求体后再发送。".into()
        } else {
            "请核对最终请求地址和自动生成的数据后再发送。".into()
        },
    }
}

#[tauri::command]
pub fn preview_api_endpoint_test(
    state: tauri::State<'_, DatabaseState>,
    endpoint_id: String,
) -> Result<ApiTestPreview, String> {
    let detail = endpoint_detail(&state, endpoint_id.trim())?;
    let config = api_test_config_for_state(&state, &detail.summary.source_id)?;
    let token = api_test_token(&detail.summary.source_id);
    let prepared = prepare_api_test(&detail, &config, token.as_deref())?;
    Ok(api_test_preview(&detail.summary.id, &prepared))
}

fn apply_api_test_execution(
    mut prepared: PreparedApiTest,
    request: &ApiTestExecutionRequest,
) -> Result<PreparedApiTest, String> {
    if api_test_requires_confirmation(&prepared.method) && !request.confirmed {
        return Err("写操作接口必须确认可能修改真实业务数据后才能发送。".into());
    }
    let url = Url::parse(request.url.trim()).map_err(|_| "测试请求 URL 格式无效。".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("测试请求 URL 必须是无账号、密码和锚点的完整 http 或 https 地址。".into());
    }
    let same_origin = url.scheme() == prepared.url.scheme()
        && url.host_str() == prepared.url.host_str()
        && url.port_or_known_default() == prepared.url.port_or_known_default();
    if !same_origin {
        return Err("为避免误发请求，只允许修改当前项目同一服务地址下的路径和查询参数。".into());
    }
    prepared.url = url;
    if prepared.body.is_some() {
        let body = request.body.clone().unwrap_or(Value::Null);
        prepared.body = Some(body.clone());
        if let Some(preview) = prepared.request_data.as_object_mut() {
            preview.insert("body".into(), redacted_value(&body, "body"));
        }
    } else if request.body.as_ref().is_some_and(|value| !value.is_null()) {
        return Err("该接口文档没有请求体，不能额外发送 Body。".into());
    }
    Ok(prepared)
}

#[tauri::command]
pub fn execute_api_endpoint_test(
    state: tauri::State<'_, DatabaseState>,
    request: ApiTestExecutionRequest,
) -> Result<ApiTestResult, String> {
    let detail = endpoint_detail(&state, request.endpoint_id.trim())?;
    let config = api_test_config_for_state(&state, &detail.summary.source_id)?;
    let token = api_test_token(&detail.summary.source_id);
    let prepared = prepare_api_test(&detail, &config, token.as_deref())?;
    let prepared = apply_api_test_execution(prepared, &request)?;
    send_api_test(prepared, token.as_deref())
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
        detail.apifox_project_name,
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

fn javascript_identifier(value: &str) -> String {
    let mut result = String::new();
    let mut uppercase_next = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            if result.is_empty() && character.is_ascii_digit() {
                result.push_str("api");
            }
            result.push(if uppercase_next {
                character.to_ascii_uppercase()
            } else {
                character
            });
            uppercase_next = false;
        } else {
            uppercase_next = true;
        }
    }
    if result.is_empty() {
        "apiRequest".into()
    } else {
        result
    }
}

fn path_parameter_names(path: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = String::new();
    let mut inside = false;
    for character in path.chars() {
        match character {
            '{' if !inside => {
                inside = true;
                current.clear();
            }
            '}' if inside => {
                inside = false;
                if !current.trim().is_empty() {
                    names.push(javascript_identifier(current.trim()));
                }
            }
            _ if inside => current.push(character),
            _ => {}
        }
    }
    names
}

fn default_code_template(source_id: &str) -> ApiCodeTemplate {
    ApiCodeTemplate {
        source_id: source_id.to_string(),
        client: "request".into(),
        function_prefix: "_".into(),
        import_path: String::new(),
        include_import: false,
        typescript: false,
    }
}

fn validate_code_template(mut template: ApiCodeTemplate) -> Result<ApiCodeTemplate, String> {
    template.source_id = template.source_id.trim().to_string();
    template.client = template.client.trim().to_ascii_lowercase();
    if !matches!(
        template.client.as_str(),
        "request" | "axios" | "uni-request"
    ) {
        return Err("代码模板仅支持 request、axios 或 uni.request。".into());
    }
    template.function_prefix = template.function_prefix.trim().to_string();
    if template.function_prefix.len() > 32
        || template
            .function_prefix
            .chars()
            .any(|character| !character.is_ascii_alphanumeric() && !matches!(character, '_' | '$'))
    {
        return Err("函数名前缀只能包含字母、数字、下划线或 $，且不能超过 32 个字符。".into());
    }
    template.import_path = template.import_path.trim().to_string();
    if template.import_path.len() > 300 || template.import_path.contains(['\r', '\n', '\'', '"']) {
        return Err("请求库导入路径格式无效。".into());
    }
    if template.include_import
        && template.client != "uni-request"
        && template.import_path.is_empty()
    {
        template.import_path = if template.client == "axios" {
            "axios".into()
        } else {
            "@/utils/request".into()
        };
    }
    Ok(template)
}

fn code_template_for_state(
    state: &DatabaseState,
    source_id: &str,
) -> Result<ApiCodeTemplate, String> {
    state
        .connect()?
        .query_row(
            "SELECT code_client,code_function_prefix,code_import_path,code_include_import,code_typescript FROM api_sources WHERE id=?1",
            [source_id],
            |row| {
                Ok(ApiCodeTemplate {
                    source_id: source_id.to_string(),
                    client: row.get(0)?,
                    function_prefix: row.get(1)?,
                    import_path: row.get(2)?,
                    include_import: row.get(3)?,
                    typescript: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "接口文档项目关联不存在。".into())
}

#[tauri::command]
pub fn get_api_code_template(
    state: tauri::State<'_, DatabaseState>,
    source_id: String,
) -> Result<ApiCodeTemplate, String> {
    code_template_for_state(&state, source_id.trim())
}

#[tauri::command]
pub fn save_api_code_template(
    state: tauri::State<'_, DatabaseState>,
    template: ApiCodeTemplate,
) -> Result<ApiCodeTemplate, String> {
    let template = validate_code_template(template)?;
    let changed = state
        .connect()?
        .execute(
            "UPDATE api_sources SET code_client=?2,code_function_prefix=?3,code_import_path=?4,code_include_import=?5,code_typescript=?6,updated_at=?7 WHERE id=?1",
            params![template.source_id,template.client,template.function_prefix,template.import_path,template.include_import,template.typescript,Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("接口文档项目关联不存在。".into());
    }
    Ok(template)
}

fn endpoint_request_code(detail: &ApiEndpointDetail, template: &ApiCodeTemplate) -> String {
    let method = detail.summary.method.to_ascii_lowercase();
    let fallback_name = format!(
        "{}{}",
        method,
        detail
            .summary
            .path
            .split('/')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let value = javascript_identifier(part.trim_matches(['{', '}']));
                let mut characters = value.chars();
                characters
                    .next()
                    .map(|first| {
                        format!(
                            "{}{}",
                            first.to_ascii_uppercase(),
                            characters.collect::<String>()
                        )
                    })
                    .unwrap_or_default()
            })
            .collect::<String>()
    );
    let mut function_base = javascript_identifier(
        detail
            .summary
            .operation_id
            .trim()
            .trim_start_matches('_')
            .is_empty()
            .then_some(fallback_name.as_str())
            .unwrap_or_else(|| detail.summary.operation_id.trim().trim_start_matches('_')),
    );
    if template
        .function_prefix
        .chars()
        .last()
        .is_some_and(|character| character.is_ascii_alphanumeric())
    {
        let mut characters = function_base.chars();
        function_base = characters
            .next()
            .map(|first| {
                format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    characters.collect::<String>()
                )
            })
            .unwrap_or_default();
    }
    let function_name = format!("{}{}", template.function_prefix, function_base);
    let path_parameters = path_parameter_names(&detail.summary.path);
    let payload_name = if matches!(method.as_str(), "get" | "delete" | "head" | "options") {
        "params"
    } else {
        "data"
    };
    let mut parameters = path_parameters
        .iter()
        .map(|name| {
            if template.typescript {
                format!("{name}: string | number")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>();
    parameters.push(if template.typescript {
        format!("{payload_name}: Record<string, unknown>")
    } else {
        payload_name.to_string()
    });
    let mut request_path = detail.summary.path.replace('`', "\\`");
    for name in &path_parameters {
        request_path = request_path.replace(&format!("{{{name}}}"), &format!("${{{name}}}"));
    }
    let url_literal = if path_parameters.is_empty() {
        format!("'{}'", request_path.replace('\'', "\\'"))
    } else {
        format!("`{request_path}`")
    };
    let title = detail
        .summary
        .title
        .replace("*/", "* /")
        .replace(['\r', '\n'], " ");
    let mut output = String::new();
    if template.include_import && template.client != "uni-request" {
        let import_name = if template.client == "axios" {
            "axios"
        } else {
            "request"
        };
        output.push_str(&format!(
            "import {import_name} from '{}'\n\n",
            template.import_path
        ));
    }
    output.push_str(&format!("/**\n *  {title}\n"));
    for name in &path_parameters {
        let schema_type = detail
            .document
            .get("parameters")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    item.get("in").and_then(Value::as_str) == Some("path")
                        && item.get("name").and_then(Value::as_str) == Some(name)
                })
            })
            .and_then(|item| item.pointer("/schema/type"))
            .and_then(Value::as_str)
            .map(|value| match value {
                "integer" | "number" => "number",
                "boolean" => "boolean",
                _ => "string",
            })
            .unwrap_or("string");
        output.push_str(&format!(" *  @param {{{schema_type}}} {name} - 路径参数\n"));
    }
    let request_client = match template.client.as_str() {
        "axios" => "axios",
        "uni-request" => "uni.request",
        _ => "request",
    };
    let method_literal = if template.client == "uni-request" {
        method.to_ascii_uppercase()
    } else {
        method.clone()
    };
    output.push_str(&format!(
        " *  @param {{Object}} {payload_name} - {}\n */\nexport function {function_name}({}) {{\n  return {request_client}({{\n    url: {url_literal},\n    method: '{method_literal}',\n    {payload_name}\n  }})\n}}",
        if payload_name == "params" {
            "查询参数"
        } else {
            "请求数据"
        },
        parameters.join(", ")
    ));
    output
}

#[tauri::command]
pub fn render_api_endpoint_request_code(
    state: tauri::State<'_, DatabaseState>,
    endpoint_id: String,
) -> Result<String, String> {
    let detail = endpoint_detail(&state, &endpoint_id)?;
    let template = code_template_for_state(&state, &detail.summary.source_id)?;
    Ok(endpoint_request_code(&detail, &template))
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
                favorite: false,
                updated_at: "2026-08-31".into(),
            },
            external_project_id: "1001".into(),
            apifox_project_name: "项目".into(),
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

    #[test]
    fn request_code_matches_the_expected_vue_request_wrapper_format() {
        let detail = ApiEndpointDetail {
            summary: ApiEndpointSummary {
                id: "endpoint-1".into(),
                source_id: "source-1".into(),
                operation_id: "updateCulturalActivity".into(),
                method: "PUT".into(),
                path: "/detail".into(),
                title: "修改文化活动".into(),
                description: String::new(),
                tags: vec!["文化活动".into()],
                deprecated: false,
                favorite: false,
                updated_at: String::new(),
            },
            external_project_id: "1001".into(),
            apifox_project_name: "项目".into(),
            document_title: "示例".into(),
            openapi_version: "3.1.0".into(),
            last_synced_at: None,
            document: json!({}),
        };
        assert_eq!(
            endpoint_request_code(&detail, &default_code_template("source-1")),
            "/**\n *  修改文化活动\n *  @param {Object} data - 请求数据\n */\nexport function _updateCulturalActivity(data) {\n  return request({\n    url: '/detail',\n    method: 'put',\n    data\n  })\n}"
        );
    }

    #[test]
    fn tag_export_keeps_child_endpoints_components_and_masks_sensitive_examples() {
        let root = json!({
            "openapi":"3.1.0",
            "info":{"title":"示例"},
            "servers":[{"url":"https://user:password@example.com/v1?apiKey=real-secret&locale=zh-CN"}],
            "paths":{
                "/detail":{"put":{"tags":["文化服务/文化活动"],"responses":{"200":{"description":"ok"}},"x-apifox-folder":"private"}},
                "/other":{"get":{"tags":["其他"],"responses":{"200":{"description":"ok"}}}}
            },
            "components":{"schemas":{"Account":{"type":"object","properties":{"password":{"type":["string","null"],"example":"real-secret"}}}}}
        });
        let export = build_tag_openapi_document(root, "文化服务", "3.0").unwrap();
        assert_eq!(export.get("openapi").and_then(Value::as_str), Some("3.0.3"));
        assert!(export.pointer("/paths/~1detail/put").is_some());
        assert!(export.pointer("/paths/~1other").is_none());
        assert_eq!(
            export
                .pointer("/components/schemas/Account/properties/password/type")
                .and_then(Value::as_str),
            Some("string")
        );
        assert_eq!(
            export
                .pointer("/components/schemas/Account/properties/password/nullable")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            export
                .pointer("/components/schemas/Account/properties/password/example")
                .and_then(Value::as_str),
            Some("[已隐藏]")
        );
        assert!(!export.to_string().contains("x-apifox"));
        assert!(!export.to_string().contains("real-secret"));
        assert!(!export.to_string().contains("user:password"));
        assert!(export
            .pointer("/servers/0/url")
            .and_then(Value::as_str)
            .unwrap()
            .contains("locale=zh-CN"));
        assert!(
            tag_export_url("http://127.0.0.1:17890", "source-1", "文化服务")
                .starts_with("http://127.0.0.1:17890/openapi/source-1/")
        );
    }

    #[test]
    fn local_export_http_service_returns_readable_tag_openapi_json() {
        let database_path = std::env::temp_dir().join(format!(
            "workbench-openapi-export-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let state = DatabaseState::new(database_path.clone()).unwrap();
        let now = Utc::now().to_rfc3339();
        state
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO project_profiles(id,display_name,created_at,updated_at) VALUES('project-1','项目',?1,?1)",
                [&now],
            )
            .unwrap();
        let root = json!({"openapi":"3.1.0","info":{"title":"示例"},"paths":{"/items":{"get":{"tags":["业务/列表"],"responses":{"200":{"description":"ok"}}}}},"components":{"schemas":{"Item":{"type":"object"}}}});
        state
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO api_sources(id,external_project_id,openapi_document_json,created_at,updated_at) VALUES('source-1','1001',?1,?2,?2)",
                params![serde_json::to_string(&root).unwrap(),now],
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server_state = state.clone();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_export_request(&server_state, stream);
        });
        let tag = URL_SAFE_NO_PAD.encode("业务".as_bytes());
        let mut client = TcpStream::connect(address).unwrap();
        write!(client,"GET /openapi/source-1/{tag}.json?version=3.0 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        server.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let document: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            document.get("openapi").and_then(Value::as_str),
            Some("3.0.3")
        );
        assert!(document.pointer("/paths/~1items/get").is_some());
        let _ = std::fs::remove_file(database_path);
    }

    #[test]
    fn project_code_template_supports_axios_import_and_typescript() {
        let (_, _, endpoints) = parse_openapi(&sample(), "source-1").unwrap();
        let item = &endpoints[0];
        let detail = ApiEndpointDetail {
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
                favorite: false,
                updated_at: String::new(),
            },
            external_project_id: "1001".into(),
            apifox_project_name: "项目".into(),
            document_title: "示例".into(),
            openapi_version: "3.1.0".into(),
            last_synced_at: None,
            document: item.document.clone(),
        };
        let template = validate_code_template(ApiCodeTemplate {
            source_id: "source-1".into(),
            client: "axios".into(),
            function_prefix: "api".into(),
            import_path: "axios".into(),
            include_import: true,
            typescript: true,
        })
        .unwrap();
        let code = endpoint_request_code(&detail, &template);
        assert!(code.starts_with("import axios from 'axios'"));
        assert!(code.contains(
            "export function apiGetUser(id: string | number, params: Record<string, unknown>)"
        ));
        assert!(code.contains("return axios({"));
    }

    #[test]
    fn api_test_preparation_generates_typed_values_and_keeps_token_out_of_preview() {
        let detail = ApiEndpointDetail {
            summary: ApiEndpointSummary {
                id: "endpoint-1".into(),
                source_id: "source-1".into(),
                operation_id: "createUser".into(),
                method: "POST".into(),
                path: "/users/{id}".into(),
                title: "新增用户".into(),
                description: String::new(),
                tags: vec!["用户".into()],
                deprecated: false,
                favorite: false,
                updated_at: String::new(),
            },
            external_project_id: "1001".into(),
            apifox_project_name: "项目".into(),
            document_title: "示例".into(),
            openapi_version: "3.1.0".into(),
            last_synced_at: None,
            document: json!({
                "servers":[{"url":"https://api.example.com/v1"}],
                "parameters":[
                    {"name":"id","in":"path","schema":{"type":"integer"}},
                    {"name":"email","in":"query","schema":{"type":"string"}},
                    {"name":"Authorization","in":"header","schema":{"type":"string"}}
                ],
                "requestBody":{"content":{"application/json":{"schema":{"type":"object","properties":{
                    "userName":{"type":"string"},"mobile":{"type":"string"},"enabled":{"type":"boolean"},"age":{"type":"integer"}
                }}}}}
            }),
        };
        let config = ApiTestConfig {
            source_id: "source-1".into(),
            base_url: String::new(),
            token_header: "Authorization".into(),
            token_configured: true,
        };
        let prepared = prepare_api_test(&detail, &config, Some("Bearer real-secret")).unwrap();
        assert_eq!(
            prepared.url.as_str(),
            "https://api.example.com/v1/users/1?email=test%40example.com"
        );
        assert_eq!(prepared.body.as_ref().unwrap()["userName"], "自动测试名称");
        assert_eq!(prepared.body.as_ref().unwrap()["mobile"], "13800000000");
        assert_eq!(prepared.body.as_ref().unwrap()["enabled"], true);
        assert_eq!(prepared.body.as_ref().unwrap()["age"], 1);
        assert_eq!(
            prepared.headers.get("authorization").unwrap(),
            "Bearer real-secret"
        );
        let api_preview = api_test_preview("endpoint-1", &prepared);
        assert!(api_preview.requires_confirmation);
        assert_eq!(
            api_preview.headers.get("authorization").map(String::as_str),
            Some("[已配置，发送时安全注入]")
        );
        let preview = prepared.request_data.to_string();
        assert!(!preview.contains("real-secret"));
        assert!(!preview.contains("Authorization"));
        assert!(apply_api_test_execution(
            prepared,
            &ApiTestExecutionRequest {
                endpoint_id: "endpoint-1".into(),
                url: "https://api.example.com/v1/users/1?email=changed%40example.com".into(),
                body: Some(json!({"userName":"修改后"})),
                confirmed: false,
            }
        )
        .is_err());
        let prepared = prepare_api_test(&detail, &config, Some("Bearer real-secret")).unwrap();
        assert!(apply_api_test_execution(
            prepared,
            &ApiTestExecutionRequest {
                endpoint_id: "endpoint-1".into(),
                url: "https://other.example.com/users/1".into(),
                body: Some(json!({"userName":"修改后"})),
                confirmed: true,
            }
        )
        .is_err());
        let prepared = prepare_api_test(&detail, &config, Some("Bearer real-secret")).unwrap();
        let applied = apply_api_test_execution(
            prepared,
            &ApiTestExecutionRequest {
                endpoint_id: "endpoint-1".into(),
                url: "https://api.example.com/v1/users/2".into(),
                body: Some(json!({"userName":"修改后"})),
                confirmed: true,
            },
        )
        .unwrap();
        assert_eq!(applied.body.unwrap()["userName"], "修改后");
    }

    #[test]
    fn api_test_returns_real_json_response_and_redacts_secrets() {
        use std::io::{Read as _, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);
            let body = r#"{"code":200,"token":"server-secret","echo":"Bearer real-secret"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let prepared = PreparedApiTest {
            url: Url::parse(&format!("http://{address}/users/1")).unwrap(),
            method: Method::GET,
            headers: HeaderMap::new(),
            content_type: String::new(),
            request_data: json!({"path.id":1}),
            body: None,
        };
        let result = send_api_test(prepared, Some("Bearer real-secret")).unwrap();
        server.join().unwrap();
        assert_eq!(result.status, 200);
        assert!(result.success);
        assert_eq!(result.response_data["code"], 200);
        assert_eq!(result.response_data["token"], "[已隐藏]");
        assert_eq!(result.response_data["echo"], "[已隐藏]");
        assert!(!result.truncated);
    }

    #[test]
    fn apifox_project_can_be_saved_without_a_project_profile() {
        let database_path = std::env::temp_dir().join(format!(
            "workbench-apifox-project-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let state = DatabaseState::new(database_path.clone()).unwrap();
        let saved_id = save_api_source_record(
            &state,
            &ApiSourceUpdate {
                id: String::new(),
                external_project_id: "1001".into(),
                apifox_project_name: "统一接口项目".into(),
            },
        )
        .unwrap();
        assert_eq!(saved_id, source_id("1001"));
        let connection = state.connect().unwrap();
        let stored: (Option<String>, String) = connection
            .query_row(
                "SELECT project_profile_id,apifox_project_name FROM api_sources WHERE external_project_id='1001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (None, "统一接口项目".into()));
        drop(connection);
        assert!(save_api_source_record(
            &state,
            &ApiSourceUpdate {
                id: String::new(),
                external_project_id: "1001".into(),
                apifox_project_name: "重复项目".into(),
            },
        )
        .is_err());
        let _ = std::fs::remove_file(database_path);
    }
}
