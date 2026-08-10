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
    if !["note", "idea", "url"].contains(&input.kind.as_str()) {
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
    state
        .connect()?
        .execute(
            "INSERT INTO quick_captures(id,kind,content,source_url,status,created_at,updated_at) VALUES(?1,?2,?3,?4,'inbox',?5,?5)",
            params![id,input.kind,content,input.source_url.trim(),now],
        )
        .map_err(|error| error.to_string())?;
    Ok(QuickCapture {
        id,
        kind: input.kind,
        content: content.into(),
        source_url: input.source_url.trim().into(),
        status: "inbox".into(),
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
