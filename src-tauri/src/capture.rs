use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCapture {
    id: String,
    kind: String,
    content: String,
    source_url: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickCaptureInput {
    kind: String,
    content: String,
    source_url: String,
}

#[tauri::command]
pub fn list_quick_captures(
    state: tauri::State<'_, DatabaseState>,
    include_archived: Option<bool>,
) -> Result<Vec<QuickCapture>, String> {
    let connection = state.connect()?;
    let sql = if include_archived.unwrap_or(false) {
        "SELECT id,kind,content,source_url,status,created_at,updated_at FROM quick_captures ORDER BY created_at DESC LIMIT 200"
    } else {
        "SELECT id,kind,content,source_url,status,created_at,updated_at FROM quick_captures WHERE status='inbox' ORDER BY created_at DESC LIMIT 100"
    };
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(QuickCapture {
                id: row.get(0)?,
                kind: row.get(1)?,
                content: row.get(2)?,
                source_url: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_quick_capture(
    state: tauri::State<'_, DatabaseState>,
    input: QuickCaptureInput,
) -> Result<QuickCapture, String> {
    save_quick_capture_for_state(&state, input)
}

fn compact_task_title(content: &str) -> String {
    let first_line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(content);
    let mut characters = first_line.chars();
    let title = characters.by_ref().take(80).collect::<String>();
    if characters.next().is_some() {
        format!("{title}…")
    } else {
        title
    }
}

fn save_quick_capture_for_state(
    state: &DatabaseState,
    input: QuickCaptureInput,
) -> Result<QuickCapture, String> {
    if !["note", "idea", "url", "task"].contains(&input.kind.as_str()) {
        return Err("快速记录类型无效。".into());
    }
    let content = input.content.trim();
    if content.is_empty() {
        return Err("请输入要记录的内容。".into());
    }
    if input.kind == "url"
        && !input.source_url.trim().starts_with("http://")
        && !input.source_url.trim().starts_with("https://")
    {
        return Err("网址记录需要填写以 http:// 或 https:// 开头的链接。".into());
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let routed_to_inbox = input.kind == "task";
    let status = if routed_to_inbox { "routed" } else { "inbox" };
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO quick_captures(id,kind,content,source_url,status,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",
            params![id,input.kind,content,input.source_url.trim(),status,now],
        )
        .map_err(|error| error.to_string())?;
    if routed_to_inbox {
        transaction
            .execute(
                "INSERT INTO work_inbox_items(id,source_type,source_id,project,title,summary,detail,route,priority,workflow_status,source_status,source_revision,created_at,updated_at)
                 VALUES(?1,'quick_capture',?2,'未归类项目',?3,?4,'通过快速记录创建，尚未自动排入日历；确认后可转为今日任务。','', 'normal','needs_decision','待处理',?5,?5,?5)",
                params![format!("quick_capture:{id}"),id,compact_task_title(content),content,now],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(QuickCapture {
        id,
        kind: input.kind,
        content: content.into(),
        source_url: input.source_url.trim().into(),
        status: status.into(),
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub fn archive_quick_capture(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "UPDATE quick_captures SET status='archived',updated_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_quick_capture(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("快速记录 ID 无效。".into());
    }
    let affected = state
        .connect()?
        .execute("DELETE FROM quick_captures WHERE id=?1", params![id])
        .map_err(|error| error.to_string())?;
    if affected == 0 {
        return Err("这条快速记录不存在或已经删除。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{save_quick_capture_for_state, QuickCaptureInput};
    use crate::database::DatabaseState;

    #[test]
    fn task_capture_is_routed_to_work_inbox_without_staying_in_capture_inbox() {
        let directory =
            std::env::temp_dir().join(format!("workbench-capture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();

        let saved = save_quick_capture_for_state(
            &state,
            QuickCaptureInput {
                kind: "task".into(),
                content: "整理本周开发总结\n补充发布风险".into(),
                source_url: String::new(),
            },
        )
        .unwrap();

        assert_eq!(saved.status, "routed");
        let connection = state.connect().unwrap();
        let capture_status = connection
            .query_row(
                "SELECT status FROM quick_captures WHERE id=?1",
                [&saved.id],
                |row| row.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(capture_status, "routed");
        let inbox = connection
            .query_row(
                "SELECT source_type,title,summary,workflow_status FROM work_inbox_items WHERE source_id=?1",
                [&saved.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            inbox,
            (
                "quick_capture".into(),
                "整理本周开发总结".into(),
                "整理本周开发总结\n补充发布风险".into(),
                "needs_decision".into(),
            )
        );

        drop(connection);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
