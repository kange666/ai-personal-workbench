use crate::database::DatabaseState;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

static CODEX_SCAN_LOCK: Mutex<()> = Mutex::new(());

#[path = "codex/quota.rs"]
mod quota;

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexScanSummary {
    pub files_scanned: usize,
    pub normal_files_scanned: usize,
    pub archived_files_scanned: usize,
    pub conversations_imported: usize,
    pub token_events_imported: usize,
    pub messages_imported: usize,
    pub files_unchanged: usize,
    pub archived_conversations_imported: usize,
    pub conversations_total: usize,
    pub archived_conversations_total: usize,
    pub errors: usize,
    pub error_details: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaWindow {
    used_percent: f64,
    remaining_percent: f64,
    window_minutes: i64,
    resets_at: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaSnapshot {
    available: bool,
    captured_at: Option<String>,
    plan_type: Option<String>,
    reset_credits_available: Option<u32>,
    reset_credits_status: String,
    primary: Option<CodexQuotaWindow>,
    secondary: Option<CodexQuotaWindow>,
    source_file: Option<String>,
    source_modified_at: Option<String>,
    freshness: String,
    selection_reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrayQuota {
    pub remaining_percent: u8,
    pub resets_at: i64,
}

#[derive(Default)]
struct SessionData {
    id: String,
    title: Option<String>,
    cwd: Option<String>,
    started_at: Option<String>,
    updated_at: Option<String>,
    model: Option<String>,
    events: Vec<TokenEvent>,
    messages: Vec<ConversationMessage>,
    event_title: Option<String>,
}

struct ConversationMessage {
    source_index: i64,
    event_time: Option<String>,
    role: String,
    content: String,
}

#[derive(Clone, Default)]
struct TokenEvent {
    event_time: Option<String>,
    input: i64,
    cached: i64,
    output: i64,
    reasoning: i64,
    total: i64,
    context_used: i64,
    context_window: i64,
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().map(ToOwned::to_owned)
}

fn integer(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

fn quota_window(value: &Value) -> Option<CodexQuotaWindow> {
    let used_percent = value.get("used_percent")?.as_f64()?.clamp(0.0, 100.0);
    Some(CodexQuotaWindow {
        used_percent,
        remaining_percent: (100.0 - used_percent).clamp(0.0, 100.0),
        window_minutes: value
            .get("window_minutes")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        resets_at: value
            .get("resets_at")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    })
}

fn quota_from_event(value: &Value) -> Option<CodexQuotaSnapshot> {
    if value.get("type").and_then(Value::as_str) != Some("event_msg")
        || value.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
    {
        return None;
    }
    let limits = value.pointer("/payload/rate_limits")?;
    // Codex 会同时记录通用额度和特定模型额度。工作台只展示通用使用限额，
    // 避免把 GPT-5.3-Codex-Spark 等独立额度误认为账户剩余额度。
    if limits.get("limit_id").and_then(Value::as_str) != Some("codex") {
        return None;
    }
    let primary = limits.get("primary").and_then(quota_window);
    let secondary = limits.get("secondary").and_then(quota_window);
    if primary.is_none() && secondary.is_none() {
        return None;
    }
    Some(CodexQuotaSnapshot {
        available: true,
        captured_at: value
            .get("timestamp")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        plan_type: limits
            .get("plan_type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        reset_credits_available: None,
        reset_credits_status: "unavailable".into(),
        primary,
        secondary,
        source_file: None,
        source_modified_at: None,
        freshness: String::new(),
        selection_reason: String::new(),
    })
}

fn quota_from_file_tail(path: &Path) -> Option<CodexQuotaSnapshot> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    for tail_bytes in [
        256 * 1024_u64,
        1024 * 1024,
        4 * 1024 * 1024,
        16 * 1024 * 1024,
    ] {
        let start = length.saturating_sub(tail_bytes);
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut bytes = Vec::with_capacity((length - start).min(tail_bytes) as usize);
        file.read_to_end(&mut bytes).ok()?;
        if let Some(snapshot) = String::from_utf8_lossy(&bytes)
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find_map(|value| quota_from_event(&value))
        {
            return Some(snapshot);
        }
        if start == 0 {
            break;
        }
    }
    None
}

fn snapshot_captured_millis(snapshot: &CodexQuotaSnapshot) -> i64 {
    snapshot
        .captured_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp_millis())
        .unwrap_or_default()
}

fn snapshot_has_active_window(snapshot: &CodexQuotaSnapshot, now: i64) -> bool {
    snapshot
        .primary
        .iter()
        .chain(snapshot.secondary.iter())
        .any(|window| window.resets_at <= 0 || window.resets_at > now)
}

fn decorate_quota_snapshot(
    mut snapshot: CodexQuotaSnapshot,
    path: &Path,
    modified: std::time::SystemTime,
) -> CodexQuotaSnapshot {
    let now = Utc::now();
    let captured = snapshot
        .captured_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    let age_seconds = captured
        .map(|value| now.signed_duration_since(value).num_seconds().max(0))
        .unwrap_or(i64::MAX);
    snapshot.source_file = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned);
    snapshot.source_modified_at = modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| DateTime::from_timestamp(duration.as_secs() as i64, 0))
        .map(|value| value.to_rfc3339());
    snapshot.freshness = if age_seconds <= 10 * 60 {
        "fresh"
    } else if age_seconds <= 24 * 60 * 60 {
        "recent"
    } else {
        "stale"
    }
    .into();
    snapshot.selection_reason =
        "已排除专属模型与过期额度周期，并按通用额度事件时间选择最新快照".into();
    snapshot
}

fn latest_quota_snapshot_from_files(
    mut files: Vec<(std::time::SystemTime, PathBuf)>,
    now: i64,
) -> CodexQuotaSnapshot {
    files.sort_by(|(modified_a, _), (modified_b, _)| modified_b.cmp(modified_a));
    files
        .into_iter()
        .take(50)
        .filter_map(|(modified, path)| {
            let snapshot = quota_from_file_tail(&path)?;
            snapshot_has_active_window(&snapshot, now)
                .then(|| decorate_quota_snapshot(snapshot, &path, modified))
        })
        .max_by_key(snapshot_captured_millis)
        .unwrap_or_default()
}

fn latest_log_quota_snapshot() -> CodexQuotaSnapshot {
    let files = roots()
        .into_iter()
        .filter(|(root, _)| root.exists())
        .flat_map(|(root, _)| {
            WalkDir::new(root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.file_type().is_file()
                        && entry.path().extension().and_then(|value| value.to_str())
                            == Some("jsonl")
                })
                .filter_map(|entry| {
                    let modified = entry.metadata().ok()?.modified().ok()?;
                    Some((modified, entry.into_path()))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    latest_quota_snapshot_from_files(files, Utc::now().timestamp())
}

fn latest_quota_snapshot() -> CodexQuotaSnapshot {
    quota::latest_quota_snapshot()
}

pub(crate) fn latest_tray_quota() -> Option<TrayQuota> {
    let snapshot = latest_quota_snapshot();
    let window = snapshot.primary.or(snapshot.secondary)?;
    Some(TrayQuota {
        remaining_percent: window.remaining_percent.round().clamp(0.0, 100.0) as u8,
        resets_at: window.resets_at,
    })
}

fn title_from_user_message(message: &str) -> Option<String> {
    let mut value = message.trim();
    if let Some((_, request)) = value.split_once("## My request for Codex:") {
        value = request.trim();
    }
    if value.is_empty()
        || value.starts_with("<recommended_plugins>")
        || value.starts_with("<environment_context>")
        || value.starts_with("<codex_internal_context")
        || value.starts_with("<app-context>")
        || value.starts_with("## Referenced ChatGPT conversation:")
        || value.starts_with("# Browser comments:")
        || value.starts_with("The next image is untrusted page evidence")
        || value.starts_with("Untrusted page evidence")
    {
        return None;
    }
    let title = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('<'))?;
    Some(
        title
            .trim_start_matches('#')
            .trim()
            .chars()
            .take(160)
            .collect(),
    )
}

fn parse_session(path: &Path) -> Result<SessionData, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut session = SessionData::default();
    for (source_index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|error| error.to_string())?;
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let outer_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if outer_type == "session_meta" && session.id.is_empty() {
            session.id = string_at(&value, &["payload", "id"]).unwrap_or_default();
            session.cwd = string_at(&value, &["payload", "cwd"]);
            session.started_at = value
                .get("timestamp")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            session.model = string_at(&value, &["payload", "model"]);
        } else if outer_type == "turn_context" {
            session.model = session
                .model
                .or_else(|| string_at(&value, &["payload", "model"]));
            session.updated_at = value
                .get("timestamp")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .or(session.updated_at);
        } else if outer_type == "event_msg"
            && string_at(&value, &["payload", "type"]).as_deref() == Some("token_count")
        {
            let usage = value
                .pointer("/payload/info/total_token_usage")
                .unwrap_or(&Value::Null);
            let last_usage = value
                .pointer("/payload/info/last_token_usage")
                .unwrap_or(&Value::Null);
            let info = value.pointer("/payload/info").unwrap_or(&Value::Null);
            session.events.push(TokenEvent {
                event_time: value
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                input: integer(usage, "input_tokens"),
                cached: integer(usage, "cached_input_tokens"),
                output: integer(usage, "output_tokens"),
                reasoning: integer(usage, "reasoning_output_tokens"),
                total: integer(usage, "total_tokens"),
                context_used: integer(last_usage, "total_tokens"),
                context_window: integer(info, "model_context_window"),
            });
        } else if outer_type == "event_msg"
            && string_at(&value, &["payload", "type"]).as_deref() == Some("user_message")
            && session.event_title.is_none()
        {
            session.event_title = string_at(&value, &["payload", "message"])
                .and_then(|message| title_from_user_message(&message));
        } else if outer_type == "response_item"
            && string_at(&value, &["payload", "type"]).as_deref() == Some("message")
        {
            let role = string_at(&value, &["payload", "role"]);
            if matches!(role.as_deref(), Some("user" | "assistant")) {
                let content = value
                    .pointer("/payload/content")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                if !content.trim().is_empty() {
                    if role.as_deref() == Some("user") && session.title.is_none() {
                        session.title = title_from_user_message(&content);
                    }
                    session.messages.push(ConversationMessage {
                        source_index: source_index as i64,
                        event_time: value
                            .get("timestamp")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned),
                        role: role.unwrap_or_default(),
                        content,
                    });
                }
            }
        }
    }
    if session.id.is_empty() {
        session.id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
    }
    if session.event_title.is_some() {
        session.title = session.event_title.take();
    }
    Ok(session)
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join(".codex")
        })
}

fn roots() -> Vec<(PathBuf, bool)> {
    let codex = codex_home();
    vec![
        (codex.join("sessions"), false),
        (codex.join("archived_sessions"), true),
    ]
}

pub fn scan_codex_sessions_for_state(state: &DatabaseState) -> Result<CodexScanSummary, String> {
    let _scan_guard = CODEX_SCAN_LOCK
        .lock()
        .map_err(|_| "Codex 扫描锁异常，请重启工作台后重试。".to_string())?;
    scan_roots_for_state(state, roots())
}

fn scan_roots_for_state(
    state: &DatabaseState,
    source_roots: Vec<(PathBuf, bool)>,
) -> Result<CodexScanSummary, String> {
    let mut summary = CodexScanSummary::default();
    for (root, archived) in source_roots.into_iter().filter(|(path, _)| path.exists()) {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl")
            })
        {
            summary.files_scanned += 1;
            if archived {
                summary.archived_files_scanned += 1;
            } else {
                summary.normal_files_scanned += 1;
            }
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) => {
                    summary.errors += 1;
                    summary.error_details.push(format!(
                        "{}：读取文件信息失败（{}）",
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("未知文件"),
                        error
                    ));
                    continue;
                }
            };
            let source_size = metadata.len().min(i64::MAX as u64) as i64;
            let source_modified_ns = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_nanos().min(i64::MAX as u128) as i64)
                .unwrap_or_default();
            let connection = state.connect()?;
            let unchanged = connection
                .query_row(
                    "SELECT archived FROM conversations WHERE source_file=?1 AND source_size=?2 AND source_modified_ns=?3",
                    params![path.display().to_string(), source_size, source_modified_ns],
                    |row| row.get::<_, bool>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            if let Some(previous_archived) = unchanged {
                if previous_archived != archived {
                    connection.execute(
                        "UPDATE conversations SET archived=?1 WHERE source_file=?2 AND archived<>?1",
                        params![archived as i64, path.display().to_string()],
                    ).map_err(|error| error.to_string())?;
                }
                summary.files_unchanged += 1;
                continue;
            }
            // 文件解析不持有连接/写事务；恢复流程不必等待大文件读取结束。
            drop(connection);
            let session = match parse_session(path) {
                Ok(session) => session,
                Err(error) => {
                    summary.errors += 1;
                    summary.error_details.push(format!(
                        "{}：解析失败（{}）",
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("未知文件"),
                        error
                    ));
                    continue;
                }
            };
            let result = persist_session(
                state,
                path,
                session,
                source_size,
                source_modified_ns,
                archived,
            );
            match result {
                Ok((messages, events)) => {
                    summary.conversations_imported += 1;
                    summary.messages_imported += messages;
                    summary.token_events_imported += events;
                    if archived {
                        summary.archived_conversations_imported += 1;
                    }
                }
                Err(error) => {
                    summary.errors += 1;
                    summary.error_details.push(format!(
                        "{}：写入失败（{}）",
                        path.file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or("未知文件"),
                        error
                    ));
                }
            }
        }
    }
    let (conversations_total, archived_conversations_total): (i64, i64) = state
        .connect()?
        .query_row(
            "SELECT COUNT(*),COALESCE(SUM(archived),0) FROM conversations",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    summary.conversations_total = conversations_total.max(0) as usize;
    summary.archived_conversations_total = archived_conversations_total.max(0) as usize;
    Ok(summary)
}

fn persist_session(
    state: &DatabaseState,
    path: &Path,
    session: SessionData,
    source_size: i64,
    source_modified_ns: i64,
    archived: bool,
) -> Result<(usize, usize), String> {
    // 一个会话一个短事务，消息/Token 任一写入失败都会回滚，不产生半条会话。
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let final_usage = session.events.last().cloned().unwrap_or_default();
    let imported_at = Utc::now().to_rfc3339();
    transaction.execute(
                "INSERT INTO conversations(id,source_file,title,cwd,started_at,updated_at,model,input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens,context_used_tokens,context_window,source_size,source_modified_ns,archived,imported_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
                 ON CONFLICT(id) DO UPDATE SET source_file=excluded.source_file,title=COALESCE(excluded.title,conversations.title),cwd=excluded.cwd,updated_at=excluded.updated_at,model=excluded.model,input_tokens=excluded.input_tokens,cached_input_tokens=excluded.cached_input_tokens,output_tokens=excluded.output_tokens,reasoning_output_tokens=excluded.reasoning_output_tokens,total_tokens=excluded.total_tokens,context_used_tokens=excluded.context_used_tokens,context_window=excluded.context_window,source_size=excluded.source_size,source_modified_ns=excluded.source_modified_ns,archived=excluded.archived,imported_at=excluded.imported_at",
                params![session.id,path.display().to_string(),session.title,session.cwd,session.started_at,session.updated_at,session.model,final_usage.input,final_usage.cached,final_usage.output,final_usage.reasoning,final_usage.total,final_usage.context_used,final_usage.context_window,source_size,source_modified_ns,archived as i64,imported_at],
            ).map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM conversation_messages WHERE conversation_id=?1",
            [&session.id],
        )
        .map_err(|error| error.to_string())?;
    let mut messages = 0;
    for message in session.messages {
        messages += transaction.execute(
                    "INSERT INTO conversation_messages(conversation_id,source_index,event_time,role,content) VALUES(?1,?2,?3,?4,?5)",
                    params![session.id,message.source_index,message.event_time,message.role,message.content],
                ).map_err(|error| error.to_string())?;
    }
    let mut events = 0;
    for event in session.events {
        events += transaction.execute(
                    "INSERT OR IGNORE INTO token_events(conversation_id,event_time,input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![session.id,event.event_time,event.input,event.cached,event.output,event.reasoning,event.total],
                ).map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok((messages, events))
}

#[tauri::command]
pub async fn scan_codex_sessions(
    state: tauri::State<'_, DatabaseState>,
) -> Result<CodexScanSummary, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || scan_codex_sessions_for_state(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn codex_quota() -> Result<CodexQuotaSnapshot, String> {
    tauri::async_runtime::spawn_blocking(latest_quota_snapshot)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod quota_tests {
    use super::*;

    #[test]
    fn parses_remaining_weekly_quota() {
        let event = serde_json::json!({
            "timestamp":"2026-08-05T07:28:31.497Z",
            "type":"event_msg",
            "payload":{
                "type":"token_count",
                "rate_limits":{
                    "limit_id":"codex",
                    "limit_name":null,
                    "primary":{"used_percent":47.0,"window_minutes":10080,"resets_at":1786233736},
                    "secondary":null,
                    "plan_type":"prolite"
                }
            }
        });
        let quota = quota_from_event(&event).expect("应当识别 Codex 额度快照");
        let primary = quota.primary.expect("应当包含周额度");
        assert_eq!(primary.remaining_percent, 53.0);
        assert_eq!(primary.window_minutes, 10_080);
        assert_eq!(quota.plan_type.as_deref(), Some("prolite"));
    }

    fn quota_event(timestamp: &str, used_percent: f64, resets_at: i64) -> String {
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "rate_limits": {
                    "limit_id": "codex",
                    "limit_name": null,
                    "primary": {
                        "used_percent": used_percent,
                        "window_minutes": 10_080,
                        "resets_at": resets_at
                    },
                    "secondary": null,
                    "plan_type": "prolite"
                }
            }
        })
        .to_string()
    }

    fn spark_quota_event(timestamp: &str, used_percent: f64, resets_at: i64) -> String {
        serde_json::json!({
            "timestamp": timestamp,
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "rate_limits": {
                    "limit_id": "codex_bengalfox",
                    "limit_name": "GPT-5.3-Codex-Spark",
                    "primary": {
                        "used_percent": used_percent,
                        "window_minutes": 300,
                        "resets_at": resets_at
                    },
                    "secondary": {
                        "used_percent": used_percent,
                        "window_minutes": 10_080,
                        "resets_at": resets_at
                    },
                    "plan_type": "prolite"
                }
            }
        })
        .to_string()
    }

    #[test]
    fn ignores_spark_specific_quota_events() {
        let event: Value = serde_json::from_str(&spark_quota_event(
            "2026-08-27T09:17:24Z",
            0.0,
            1_788_427_038,
        ))
        .unwrap();
        assert!(quota_from_event(&event).is_none());
    }

    #[test]
    fn quota_selection_prefers_general_limit_over_newer_spark_limit() {
        let directory =
            std::env::temp_dir().join(format!("workbench-general-quota-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let general_file = directory.join("general.jsonl");
        let spark_file = directory.join("spark.jsonl");
        std::fs::write(
            &general_file,
            quota_event("2026-08-27T09:16:00Z", 19.0, 1_788_272_042),
        )
        .unwrap();
        std::fs::write(
            &spark_file,
            spark_quota_event("2026-08-27T09:17:24Z", 0.0, 1_788_427_038),
        )
        .unwrap();

        let snapshot = latest_quota_snapshot_from_files(
            vec![
                (UNIX_EPOCH, general_file),
                (std::time::SystemTime::now(), spark_file),
            ],
            1_787_800_000,
        );
        let primary = snapshot.primary.expect("应当选择通用额度");
        assert_eq!(primary.remaining_percent, 81.0);
        assert_eq!(primary.window_minutes, 10_080);
        assert_eq!(snapshot.source_file.as_deref(), Some("general.jsonl"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn quota_selection_uses_event_time_instead_of_file_modified_time() {
        let directory =
            std::env::temp_dir().join(format!("workbench-quota-order-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let stale_file = directory.join("stale-but-touched.jsonl");
        let latest_file = directory.join("latest-event.jsonl");
        std::fs::write(
            &stale_file,
            quota_event("2026-08-06T23:30:05Z", 71.0, 1_786_935_253),
        )
        .unwrap();
        std::fs::write(
            &latest_file,
            quota_event("2026-08-10T03:12:17Z", 0.0, 1_786_935_253),
        )
        .unwrap();

        let snapshot = latest_quota_snapshot_from_files(
            vec![
                (std::time::SystemTime::now(), stale_file),
                (UNIX_EPOCH, latest_file),
            ],
            1_786_330_000,
        );
        assert_eq!(snapshot.primary.unwrap().remaining_percent, 100.0);
        assert_eq!(snapshot.source_file.as_deref(), Some("latest-event.jsonl"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn quota_selection_ignores_expired_windows() {
        let directory =
            std::env::temp_dir().join(format!("workbench-quota-expired-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let expired_file = directory.join("expired.jsonl");
        std::fs::write(
            &expired_file,
            quota_event("2026-08-10T03:12:17Z", 71.0, 1_000),
        )
        .unwrap();
        let snapshot = latest_quota_snapshot_from_files(
            vec![(std::time::SystemTime::now(), expired_file)],
            2_000,
        );
        assert!(!snapshot.available);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    #[ignore = "只读本机集成检查，不在匿名单元测试中读取用户历史"]
    fn latest_local_quota_snapshot_is_well_formed_when_available() {
        let quota = latest_quota_snapshot();
        if !quota.available {
            return;
        }
        let window = quota.primary.or(quota.secondary).expect("应当包含额度周期");
        assert!((0.0..=100.0).contains(&window.remaining_percent));
        assert!(window.window_minutes > 0);
        assert!(window.resets_at > 0);
    }

    #[test]
    fn repeated_scan_is_read_only_and_tokens_are_idempotent() {
        let directory =
            std::env::temp_dir().join(format!("astrion-scan-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("test.sqlite3")).unwrap();
        let source = directory.join("sessions");
        std::fs::create_dir_all(&source).unwrap();
        let content = format!(
            "{}\n{}\n",
            serde_json::json!({"type":"session_meta","timestamp":"2026-09-29T01:00:00Z","payload":{"id":"anonymous-session","cwd":"X:/anonymous"}}),
            serde_json::json!({"type":"event_msg","timestamp":"2026-09-29T01:01:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"total_tokens":10}}}})
        );
        std::fs::write(source.join("session.jsonl"), content).unwrap();
        assert_eq!(
            scan_roots_for_state(&state, vec![(source.clone(), false)])
                .unwrap()
                .token_events_imported,
            1
        );
        let observer = state.connect().unwrap();
        let before: i64 = observer
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .unwrap();
        let second = scan_roots_for_state(&state, vec![(source.clone(), false)]).unwrap();
        assert_eq!(second.files_unchanged, 1);
        assert_eq!(second.token_events_imported, 0);
        let after: i64 = observer
            .query_row("PRAGMA data_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(before, after, "无变化扫描不得产生数据库写入");
        drop(observer);
        let archived = scan_roots_for_state(&state, vec![(source, true)]).unwrap();
        assert_eq!(archived.archived_conversations_total, 1);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_token_insert_rolls_back_the_entire_session() {
        let directory =
            std::env::temp_dir().join(format!("astrion-scan-rollback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("test.sqlite3")).unwrap();
        state.connect().unwrap().execute_batch("CREATE TRIGGER reject_token BEFORE INSERT ON token_events BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        let session = SessionData {
            id: "rollback-example".into(),
            events: vec![TokenEvent::default()],
            ..Default::default()
        };
        assert!(
            persist_session(&state, Path::new("anonymous.jsonl"), session, 1, 1, false).is_err()
        );
        let count: i64 = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id='rollback-example'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
