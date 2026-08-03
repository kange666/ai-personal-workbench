use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

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
    pub errors: usize,
    pub error_details: Vec<String>,
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

fn roots() -> Vec<(PathBuf, bool)> {
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    let codex = PathBuf::from(profile).join(".codex");
    vec![
        (codex.join("sessions"), false),
        (codex.join("archived_sessions"), true),
    ]
}

pub fn scan_codex_sessions_for_state(state: &DatabaseState) -> Result<CodexScanSummary, String> {
    let mut summary = CodexScanSummary::default();
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for (root, archived) in roots().into_iter().filter(|(path, _)| path.exists()) {
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
            let unchanged = transaction
                .query_row(
                    "SELECT 1 FROM conversations WHERE source_file=?1 AND source_size=?2 AND source_modified_ns=?3",
                    params![path.display().to_string(), source_size, source_modified_ns],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|error| error.to_string())?
                .is_some();
            if unchanged {
                let _ = transaction.execute(
                    "UPDATE conversations SET archived=?1 WHERE source_file=?2",
                    params![archived as i64, path.display().to_string()],
                );
                summary.files_unchanged += 1;
                continue;
            }
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
            let final_usage = session.events.last().cloned().unwrap_or_default();
            let imported_at = Utc::now().to_rfc3339();
            let result = transaction.execute(
                "INSERT INTO conversations(id,source_file,title,cwd,started_at,updated_at,model,input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens,context_used_tokens,context_window,source_size,source_modified_ns,archived,imported_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
                 ON CONFLICT(id) DO UPDATE SET source_file=excluded.source_file,title=COALESCE(excluded.title,conversations.title),cwd=excluded.cwd,updated_at=excluded.updated_at,model=excluded.model,input_tokens=excluded.input_tokens,cached_input_tokens=excluded.cached_input_tokens,output_tokens=excluded.output_tokens,reasoning_output_tokens=excluded.reasoning_output_tokens,total_tokens=excluded.total_tokens,context_used_tokens=excluded.context_used_tokens,context_window=excluded.context_window,source_size=excluded.source_size,source_modified_ns=excluded.source_modified_ns,archived=excluded.archived,imported_at=excluded.imported_at",
                params![session.id,path.display().to_string(),session.title,session.cwd,session.started_at,session.updated_at,session.model,final_usage.input,final_usage.cached,final_usage.output,final_usage.reasoning,final_usage.total,final_usage.context_used,final_usage.context_window,source_size,source_modified_ns,archived as i64,imported_at],
            );
            if let Err(error) = result {
                summary.errors += 1;
                summary.error_details.push(format!(
                    "{}：写入失败（{}）",
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("未知文件"),
                    error
                ));
                continue;
            }
            summary.conversations_imported += 1;
            if archived {
                summary.archived_conversations_imported += 1;
            }
            let _ = transaction.execute(
                "DELETE FROM conversation_messages WHERE conversation_id=?1",
                [&session.id],
            );
            for message in session.messages {
                if transaction.execute(
                    "INSERT INTO conversation_messages(conversation_id,source_index,event_time,role,content) VALUES(?1,?2,?3,?4,?5)",
                    params![session.id,message.source_index,message.event_time,message.role,message.content],
                ).is_ok() { summary.messages_imported += 1; }
            }
            for event in session.events {
                if transaction.execute(
                    "INSERT OR IGNORE INTO token_events(conversation_id,event_time,input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![session.id,event.event_time,event.input,event.cached,event.output,event.reasoning,event.total],
                ).is_ok() { summary.token_events_imported += 1; }
            }
        }
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(summary)
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
