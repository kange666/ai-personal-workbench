use crate::{database::DatabaseState, email};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchNotification {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub output: String,
    pub source_id: Option<String>,
    pub route: String,
    pub is_read: bool,
    pub created_at: String,
    pub read_at: Option<String>,
    pub review_status: String,
    pub review_note: String,
    pub reviewed_at: Option<String>,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSyncSummary {
    pub files_scanned: usize,
    pub notifications_created: usize,
}

#[derive(Debug)]
struct CompletionEvent {
    turn_id: String,
    completed_at: String,
    body: String,
    output: String,
}

fn roots() -> Vec<PathBuf> {
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    let codex = PathBuf::from(profile).join(".codex");
    vec![codex.join("sessions"), codex.join("archived_sessions")]
}

fn source_modified_ns(path: &Path) -> i64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn compact_message(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = compact.chars();
    let excerpt = characters.by_ref().take(180).collect::<String>();
    if characters.next().is_some() {
        format!("{excerpt}…")
    } else if excerpt.is_empty() {
        "Codex 已完成本轮任务。".to_string()
    } else {
        excerpt
    }
}

fn text_excerpt(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let excerpt = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{excerpt}…")
    } else {
        excerpt
    }
}

fn clean_title_line(value: &str) -> String {
    value
        .trim()
        .trim_start_matches(|character: char| {
            matches!(character, '#' | '-' | '*' | '•' | '✓' | '✅' | ' ')
        })
        .replace("**", "")
        .replace('`', "")
        .trim()
        .trim_end_matches(['：', ':', '。', '.'])
        .trim()
        .to_string()
}

fn markdown_link_label(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = trimmed.find("](")?;
    let label = trimmed.get(1..end)?.trim();
    (!label.is_empty()).then(|| label.to_string())
}

fn project_name(project_override: &str, cwd: &str, conversation_title: &str) -> String {
    let configured = project_override.trim();
    if !configured.is_empty() {
        return text_excerpt(configured, 18);
    }
    let path = cwd.trim().trim_end_matches(['\\', '/']);
    if let Some(name) = path
        .rsplit(|character| matches!(character, '\\' | '/'))
        .find(|part| !part.trim().is_empty())
    {
        return text_excerpt(name.trim(), 18);
    }
    markdown_link_label(conversation_title)
        .map(|value| text_excerpt(&value, 18))
        .unwrap_or_else(|| "Codex".to_string())
}

fn task_summary(output: &str, body: &str, original_title: &str) -> String {
    let source = if output.trim().is_empty() {
        body
    } else {
        output
    };
    let lines = source
        .lines()
        .map(clean_title_line)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let first = lines.first().cloned().unwrap_or_default();
    let first_bullet = source
        .lines()
        .find(|line| {
            let line = line.trim_start();
            line.starts_with("- ") || line.starts_with("• ") || line.starts_with("✅")
        })
        .map(clean_title_line)
        .filter(|line| !line.is_empty());
    let completed = first
        .strip_prefix("已完成")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let generic_heading = first.contains("需求已完成")
        || first.starts_with("完成情况")
        || first.starts_with("验收结果")
        || completed.is_some_and(|value| {
            value.starts_with(|character: char| character.is_ascii_digit())
                && value.contains("项调整")
        });
    let mut summary = if generic_heading {
        first_bullet.unwrap_or(first)
    } else if let Some(value) = completed {
        value.to_string()
    } else if first.chars().count() <= 24 {
        first
    } else {
        first_bullet.unwrap_or(first)
    };
    if source.contains("演示任务") {
        summary = summary.replace("该任务", "无关演示任务");
    }
    if summary.is_empty() {
        summary = clean_title_line(
            original_title
                .strip_prefix("Codex 任务已完成：")
                .unwrap_or(original_title),
        );
    }
    if summary.is_empty() {
        summary = "任务已完成".to_string();
    }
    text_excerpt(&summary, 24)
}

fn notification_title(
    project_override: &str,
    cwd: &str,
    conversation_title: &str,
    body: &str,
    output: &str,
    original_title: &str,
) -> String {
    format!(
        "{}：{}",
        project_name(project_override, cwd, conversation_title),
        task_summary(output, body, original_title)
    )
}

fn completion_from_value(value: &Value) -> Option<CompletionEvent> {
    if value.get("type").and_then(Value::as_str) != Some("event_msg")
        || value.pointer("/payload/type").and_then(Value::as_str) != Some("task_complete")
    {
        return None;
    }
    let turn_id = value.pointer("/payload/turn_id")?.as_str()?.to_string();
    let completed_at = value
        .pointer("/payload/completed_at")
        .and_then(Value::as_str)
        .or_else(|| value.get("timestamp").and_then(Value::as_str))?
        .to_string();
    let output = value
        .pointer("/payload/last_agent_message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or("Codex 已完成本轮任务。")
        .to_string();
    let body = compact_message(&output);
    Some(CompletionEvent {
        turn_id,
        completed_at,
        body,
        output,
    })
}

fn session_id(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let first_line = BufReader::new(file).lines().next()?.ok()?;
    let value = serde_json::from_str::<Value>(&first_line).ok()?;
    value
        .pointer("/payload/id")
        .or_else(|| value.pointer("/payload/session_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn completions_from_tail(path: &Path) -> Vec<CompletionEvent> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let length = match file.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => return Vec::new(),
    };
    let start = length.saturating_sub(2 * 1024 * 1024);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut content = String::new();
    if file.read_to_string(&mut content).is_err() {
        return Vec::new();
    }
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| completion_from_value(&value))
        .collect()
}

pub fn sync_codex_notifications_for_state(
    state: &DatabaseState,
) -> Result<NotificationSyncSummary, String> {
    let connection = state.connect()?;
    let baseline = connection
        .query_row(
            "SELECT value FROM app_meta WHERE key='codex_notifications_started_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| error.to_string())?;
    let previous_cursor = connection
        .query_row(
            "SELECT value FROM app_meta WHERE key='codex_notification_modified_cursor'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default();
    drop(connection);

    let mut summary = NotificationSyncSummary::default();
    let mut latest_cursor = previous_cursor;
    let mut completed = Vec::new();
    for root in roots().into_iter().filter(|root| root.exists()) {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl")
            })
        {
            let modified = source_modified_ns(entry.path());
            latest_cursor = latest_cursor.max(modified);
            if modified <= previous_cursor {
                continue;
            }
            summary.files_scanned += 1;
            let source_id = session_id(entry.path());
            for event in completions_from_tail(entry.path()) {
                if event.completed_at >= baseline {
                    completed.push((entry.path().display().to_string(), source_id.clone(), event));
                }
            }
        }
    }

    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for (source_file, source_id, event) in completed {
        let (conversation_title, cwd, project_override) = transaction
            .query_row(
                "SELECT COALESCE(NULLIF(title,''),'未命名 Codex 对话'),COALESCE(cwd,''),COALESCE(project_override,'') FROM conversations WHERE id=?1 OR source_file=?2 LIMIT 1",
                params![source_id, source_file],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| ("Codex 对话任务".to_string(), String::new(), String::new()));
        let notification_id = format!("codex-complete-{}", event.turn_id);
        let title = notification_title(
            &project_override,
            &cwd,
            &conversation_title,
            &event.body,
            &event.output,
            "Codex 任务已完成",
        );
        let route = source_id
            .as_ref()
            .map(|id| format!("/tokens?conversation={id}"))
            .unwrap_or_else(|| "/tokens".to_string());
        let inserted = transaction
            .execute(
                "INSERT OR IGNORE INTO notifications(id,kind,title,body,output,source_id,route,is_read,created_at)
                 VALUES(?1,'codex_complete',?2,?3,?4,?5,?6,0,?7)",
                params![notification_id, title, event.body, event.output, source_id, route, event.completed_at],
            )
            .map_err(|error| error.to_string())?;
        if inserted == 0 {
            transaction
                .execute(
                    "UPDATE notifications SET title=?1,body=?2,output=?3 WHERE id=?4",
                    params![title, event.body, event.output, notification_id],
                )
                .map_err(|error| error.to_string())?;
        }
        summary.notifications_created += inserted;
    }
    transaction
        .execute(
            "INSERT INTO app_meta(key,value) VALUES('codex_notification_modified_cursor',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [latest_cursor.to_string()],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    email::reconcile_notifications_for_state(state)?;
    Ok(summary)
}

#[tauri::command]
pub async fn sync_codex_notifications(
    state: tauri::State<'_, DatabaseState>,
) -> Result<NotificationSyncSummary, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sync_codex_notifications_for_state(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn list_notifications(
    state: tauri::State<'_, DatabaseState>,
    limit: Option<i64>,
) -> Result<Vec<WorkbenchNotification>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT n.id,n.kind,n.title,n.body,n.output,n.source_id,n.route,n.is_read,n.created_at,n.read_at,n.review_status,n.review_note,n.reviewed_at,
                         COALESCE(c.project_override,''),COALESCE(c.cwd,''),COALESCE(c.title,'')
                  FROM notifications n LEFT JOIN conversations c ON c.id=n.source_id
                  ORDER BY n.is_read ASC,n.created_at DESC LIMIT ?1")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([limit.unwrap_or(30).clamp(1, 100)], |row| {
            let kind = row.get::<_, String>(1)?;
            let original_title = row.get::<_, String>(2)?;
            let body = row.get::<_, String>(3)?;
            let output = row.get::<_, String>(4)?;
            Ok(WorkbenchNotification {
                id: row.get(0)?,
                kind: kind.clone(),
                title: if kind == "tapd_item" {
                    original_title.clone()
                } else {
                    notification_title(
                        &row.get::<_, String>(13)?,
                        &row.get::<_, String>(14)?,
                        &row.get::<_, String>(15)?,
                        &body,
                        &output,
                        &original_title,
                    )
                },
                body,
                output,
                source_id: row.get(5)?,
                route: row.get(6)?,
                is_read: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
                read_at: row.get(9)?,
                review_status: row.get(10)?,
                review_note: row.get(11)?,
                reviewed_at: row.get(12)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn mark_notification_read(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "UPDATE notifications SET is_read=1,read_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn mark_all_notifications_read(state: tauri::State<'_, DatabaseState>) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "UPDATE notifications SET is_read=1,read_at=?1 WHERE is_read=0",
            [Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn review_notification(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    decision: String,
    note: String,
) -> Result<(), String> {
    if !["accepted", "follow_up"].contains(&decision.as_str()) {
        return Err("无效的处理结论。".into());
    }
    let now = Utc::now().to_rfc3339();
    state
        .connect()?
        .execute(
            "UPDATE notifications SET review_status=?1,review_note=?2,reviewed_at=?3,is_read=1,read_at=COALESCE(read_at,?3) WHERE id=?4",
            params![decision, note.trim(), now, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_real_codex_task_complete_event() {
        let value = serde_json::json!({
            "timestamp":"2026-08-07T02:20:00Z",
            "type":"event_msg",
            "payload":{
                "type":"task_complete",
                "turn_id":"turn-1",
                "completed_at":"2026-08-07T02:20:00Z",
                "last_agent_message":"已完成页面修复。\n\n构建检查通过。"
            }
        });
        let event = completion_from_value(&value).expect("应识别任务完成事件");
        assert_eq!(event.turn_id, "turn-1");
        assert_eq!(event.body, "已完成页面修复。 构建检查通过。");
        assert_eq!(event.output, "已完成页面修复。\n\n构建检查通过。");
    }

    #[test]
    fn ignores_non_completion_events() {
        let value = serde_json::json!({"type":"event_msg","payload":{"type":"task_started"}});
        assert!(completion_from_value(&value).is_none());
    }

    #[test]
    fn notification_title_uses_project_and_compact_task_summary() {
        let title = notification_title(
            "",
            r"C:\Users\11429\Documents\个人工作台",
            "[AI个人工作台设计](chatgpt-conversation://example) 很长的原始标题",
            "已完成窗口头部优化。",
            "已完成窗口头部优化：\n\n- 三个按钮统一为 40×40px。",
            "旧标题",
        );
        assert_eq!(title, "个人工作台：窗口头部优化");
    }

    #[test]
    fn historical_demo_completion_gets_a_readable_title() {
        let title = notification_title(
            "",
            r"C:\Users\11429\Documents\个人工作台",
            "复杂原始标题",
            "这是早期原型遗留的演示任务，不是你的真实任务，现已彻底清理。",
            "这是早期原型遗留的演示任务，不是你的真实任务，现已彻底清理：\n\n- 删除本地数据库中的该任务。",
            "旧标题",
        );
        assert_eq!(title, "个人工作台：删除本地数据库中的无关演示任务");
    }
}
