use crate::{database::DatabaseState, email, project_identity};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct NotificationFileCursor {
    // 只推进到完整 JSONL 行末，下一次继续读取尚未写完的事件。
    offset: u64,
    modified_ns: u128,
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

fn suppresses_completion_notification(output: &str) -> bool {
    let normalized = output.to_ascii_uppercase();
    normalized.contains("DONT_NOTIFY")
        || (normalized.contains("<HEARTBEAT") && normalized.contains("<AUTOMATION_ID"))
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
    if suppresses_completion_notification(&output) {
        return None;
    }
    let body = compact_message(&output);
    Some(CompletionEvent {
        turn_id,
        completed_at,
        body,
        output,
    })
}

fn session_context(path: &Path) -> (Option<String>, String, bool) {
    let Some(first_line) = File::open(path)
        .ok()
        .and_then(|file| BufReader::new(file).lines().next())
        .and_then(Result::ok)
    else {
        return (None, String::new(), false);
    };
    let Ok(value) = serde_json::from_str::<Value>(&first_line) else {
        return (None, String::new(), false);
    };
    let id = value
        .pointer("/payload/id")
        .or_else(|| value.pointer("/payload/session_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let cwd = value
        .pointer("/payload/cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let is_automation = value
        .pointer("/payload/thread_source")
        .and_then(Value::as_str)
        .is_some_and(|source| source.eq_ignore_ascii_case("automation"));
    (id, cwd, is_automation)
}

fn read_completion_events(
    path: &Path,
    previous: Option<NotificationFileCursor>,
) -> std::io::Result<Option<(Vec<CompletionEvent>, NotificationFileCursor)>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let length = metadata.len();
    let modified_ns = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    if previous.is_some_and(|cursor| cursor.offset == length && cursor.modified_ns == modified_ns) {
        return Ok(None);
    }

    // Codex 可能保留旧修改时间，必须按每个文件的读取位置检测新增内容。
    // 首次发现/文件被替换时沿用尾部扫描，后续增量不再受 2 MiB 限制。
    let incremental = previous.filter(|cursor| cursor.offset < length);
    let start = incremental
        .map(|cursor| cursor.offset)
        .unwrap_or_else(|| length.saturating_sub(2 * 1024 * 1024));
    let mut skip_partial_line = false;
    if incremental.is_none() && start > 0 {
        file.seek(SeekFrom::Start(start - 1))?;
        let mut preceding = [0];
        file.read_exact(&mut preceding)?;
        skip_partial_line = preceding[0] != b'\n';
    } else {
        file.seek(SeekFrom::Start(start))?;
    }
    let mut reader = BufReader::new(file.take(length - start));
    let mut offset = start;
    let mut events = Vec::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = reader.read_until(b'\n', &mut line)?;
        if count == 0 || !line.ends_with(b"\n") {
            break;
        }
        offset += count as u64;
        if skip_partial_line {
            skip_partial_line = false;
            continue;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&line) {
            if let Some(event) = completion_from_value(&value) {
                events.push(event);
            }
        }
    }
    Ok(Some((
        events,
        NotificationFileCursor {
            offset,
            modified_ns,
        },
    )))
}

pub fn sync_codex_notifications_for_state(
    state: &DatabaseState,
) -> Result<NotificationSyncSummary, String> {
    let summary = sync_codex_notifications_from_roots(state, roots())?;
    email::reconcile_notifications_for_state(state)?;
    Ok(summary)
}

fn sync_codex_notifications_from_roots(
    state: &DatabaseState,
    source_roots: Vec<PathBuf>,
) -> Result<NotificationSyncSummary, String> {
    let connection = state.connect()?;
    let baseline = connection
        .query_row(
            "SELECT value FROM app_meta WHERE key='codex_notifications_started_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| error.to_string())?;
    let mut cursors: BTreeMap<String, NotificationFileCursor> = connection
        .query_row(
            "SELECT value FROM app_meta WHERE key='codex_notification_file_cursors'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    drop(connection);

    let mut summary = NotificationSyncSummary::default();
    let mut completed = Vec::new();
    for root in source_roots.into_iter().filter(|root| root.exists()) {
        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl")
            })
        {
            let source_file = entry.path().display().to_string();
            let (events, cursor) =
                match read_completion_events(entry.path(), cursors.get(&source_file).copied()) {
                    Ok(Some(result)) => result,
                    Ok(None) => continue,
                    Err(error) => {
                        // 读取失败时保留旧位置，下一轮重试，不能当作成功读完。
                        eprintln!("读取 Codex 完成事件失败：{source_file}: {error}");
                        continue;
                    }
                };
            cursors.insert(source_file.clone(), cursor);
            summary.files_scanned += 1;
            let (source_id, source_cwd, is_automation) = session_context(entry.path());
            if is_automation {
                continue;
            }
            for event in events {
                if event.completed_at >= baseline {
                    completed.push((
                        source_file.clone(),
                        source_id.clone(),
                        source_cwd.clone(),
                        event,
                    ));
                }
            }
        }
    }

    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for (source_file, source_id, source_cwd, event) in completed {
        let (conversation_title, database_cwd, project_override) = transaction
            .query_row(
                "SELECT COALESCE(NULLIF(title,''),'未命名 Codex 对话'),COALESCE(cwd,''),COALESCE(project_override,'') FROM conversations WHERE id=?1 OR source_file=?2 LIMIT 1",
                params![source_id, source_file],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(|| ("Codex 对话任务".to_string(), String::new(), String::new()));
        let cwd = if database_cwd.trim().is_empty() {
            source_cwd
        } else {
            database_cwd
        };
        if !project_identity::conversation_belongs_to_project(
            &transaction,
            &project_override,
            &cwd,
        )? {
            continue;
        }
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
            "INSERT INTO app_meta(key,value) VALUES('codex_notification_file_cursors',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(&cursors).map_err(|error| error.to_string())?],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
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
    use std::fs::{FileTimes, OpenOptions};
    use std::io::Write;

    fn completion_line(turn_id: &str, output: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({
                "timestamp":"2026-09-18T13:07:31.779Z",
                "type":"event_msg",
                "payload":{
                    "type":"task_complete", "turn_id":turn_id,
                    "completed_at":1789736851_i64, "last_agent_message":output
                }
            })
        )
    }

    #[test]
    fn sync_detects_growth_with_unchanged_mtime_and_persists_each_file_offset() {
        let directory =
            std::env::temp_dir().join(format!("notification-growth-{}", uuid::Uuid::new_v4()));
        let sessions = directory.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let connection = state.connect().unwrap();
        connection.execute("UPDATE app_meta SET value='2026-09-18T00:00:00Z' WHERE key='codex_notifications_started_at'", []).unwrap();
        // 模拟旧版全局时间游标已经越过当前文件时间。
        connection.execute("INSERT INTO app_meta(key,value) VALUES('codex_notification_modified_cursor','9223372036854775807')", []).unwrap();
        connection.execute("INSERT INTO repository_assets(path,name,remote_url,default_branch,last_scanned_at,updated_at) VALUES('F:/example/project','project','','main','now','now')", []).unwrap();
        let path = sessions.join("user.jsonl");
        std::fs::write(&path, format!("{}\n", serde_json::json!({"type":"session_meta","payload":{"id":"session-growth","cwd":"F:/example/project","thread_source":"user"}}))).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(
            sync_codex_notifications_from_roots(&state, vec![sessions.clone()])
                .unwrap()
                .notifications_created,
            0
        );

        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(completion_line("growth", "已完成修改").as_bytes())
            .unwrap();
        // 完成后大量输出超过旧尾部窗口，增量扫描仍需读到该完成事件。
        for _ in 0..2200 {
            writeln!(file, "{}", "x".repeat(1024)).unwrap();
        }
        file.set_times(FileTimes::new().set_modified(modified))
            .unwrap();
        drop(file);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            modified
        );
        assert_eq!(
            sync_codex_notifications_from_roots(&state, vec![sessions.clone()])
                .unwrap()
                .notifications_created,
            1
        );
        let saved: (String, String) = connection
            .query_row(
                "SELECT source_id,output FROM notifications WHERE id='codex-complete-growth'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(saved, ("session-growth".into(), "已完成修改".into()));
        let repeated = sync_codex_notifications_from_roots(&state, vec![sessions.clone()]).unwrap();
        assert_eq!(repeated.files_scanned, 0);
        assert_eq!(repeated.notifications_created, 0);

        for (name, cwd, source, output) in [
            ("automation", "F:/example/project", "automation", "自动任务完成"),
            ("silent", "F:/example/project", "user", "<heartbeat><automation_id>test</automation_id><decision>DONT_NOTIFY</decision></heartbeat>"),
            ("other", "F:/example/unregistered", "user", "普通对话完成"),
        ] {
            std::fs::write(sessions.join(format!("{name}.jsonl")), format!("{}\n{}", serde_json::json!({"type":"session_meta","payload":{"id":name,"cwd":cwd,"thread_source":source}}), completion_line(name, output))).unwrap();
        }
        assert_eq!(
            sync_codex_notifications_from_roots(&state, vec![sessions])
                .unwrap()
                .notifications_created,
            0
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM notifications", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(connection);
        drop(state);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn incomplete_utf8_event_is_retried_and_truncated_file_is_rescanned() {
        let path = std::env::temp_dir().join(format!(
            "notification-partial-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, b"{}\n").unwrap();
        let (_, cursor) = read_completion_events(&path, None).unwrap().unwrap();
        let event = completion_line("partial", &"中文内容".repeat(100));
        let split = event.find('中').unwrap() + 1;
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&event.as_bytes()[..split]).unwrap();
        let (events, partial_cursor) = read_completion_events(&path, Some(cursor))
            .unwrap()
            .unwrap();
        assert!(events.is_empty());
        assert_eq!(partial_cursor.offset, cursor.offset);
        file.write_all(&event.as_bytes()[split..]).unwrap();
        drop(file);
        let (events, cursor) = read_completion_events(&path, Some(partial_cursor))
            .unwrap()
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].turn_id, "partial");
        std::fs::write(&path, completion_line("replacement", "替换后完成")).unwrap();
        let (events, _) = read_completion_events(&path, Some(cursor))
            .unwrap()
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].turn_id, "replacement");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn initial_tail_starting_inside_utf8_does_not_drop_complete_events() {
        let path =
            std::env::temp_dir().join(format!("notification-tail-{}.jsonl", uuid::Uuid::new_v4()));
        let event = completion_line("tail", "正常完成");
        let mut bytes = "中".as_bytes().to_vec();
        bytes.resize(2 * 1024 * 1024 + 1 - event.len(), b'x');
        bytes.push(b'\n');
        bytes.extend_from_slice(event.as_bytes());
        assert!(std::str::from_utf8(&bytes[bytes.len() - 2 * 1024 * 1024..]).is_err());
        std::fs::write(&path, bytes).unwrap();
        let (events, _) = read_completion_events(&path, None).unwrap().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].turn_id, "tail");
        std::fs::remove_file(path).unwrap();
    }

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
    fn ignores_heartbeat_and_dont_notify_completions() {
        for output in [
            "<heartbeat><automation_id>c-2</automation_id><decision>DONT_NOTIFY</decision></heartbeat>",
            "<heartbeat><automation_id>c-3</automation_id><message>脚本正常退出</message></heartbeat>",
            "无需处理 DONT_NOTIFY",
        ] {
            let value = serde_json::json!({
                "timestamp":"2026-09-08T02:20:00Z",
                "type":"event_msg",
                "payload":{
                    "type":"task_complete",
                    "turn_id":"turn-automation",
                    "last_agent_message":output
                }
            });
            assert!(completion_from_value(&value).is_none());
        }
    }

    #[test]
    fn reads_session_id_and_cwd_from_session_metadata() {
        let path = std::env::temp_dir().join(format!(
            "workbench-notification-session-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let mut file = File::create(&path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type":"session_meta",
                "payload":{"id":"session-1","cwd":"F:/TB-project/client"}
            })
        )
        .unwrap();

        assert_eq!(
            session_context(&path),
            (
                Some("session-1".to_string()),
                "F:/TB-project/client".to_string(),
                false
            )
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reads_automation_thread_source_from_session_metadata() {
        let path = std::env::temp_dir().join(format!(
            "workbench-automation-session-{}.jsonl",
            uuid::Uuid::new_v4()
        ));
        let mut file = File::create(&path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "type":"session_meta",
                "payload":{
                    "id":"automation-session-1",
                    "cwd":"C:/Users/11429/Documents/个人工作台",
                    "thread_source":"automation"
                }
            })
        )
        .unwrap();

        assert_eq!(
            session_context(&path),
            (
                Some("automation-session-1".to_string()),
                "C:/Users/11429/Documents/个人工作台".to_string(),
                true
            )
        );
        std::fs::remove_file(path).unwrap();
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
