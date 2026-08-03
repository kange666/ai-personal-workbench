use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionSyncSummary {
    pub conversation_suggestions: usize,
    pub report_suggestions: usize,
    pub test_suggestions: usize,
}

fn project_from_path(value: &str) -> String {
    Path::new(value)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("未归类项目")
        .to_string()
}

fn clean_title(value: &str) -> String {
    let mut title = value
        .trim()
        .trim_start_matches(['-', '*', '#', ' ', '：', ':'])
        .replace(['\r', '\n'], " ");
    for marker in [
        "还需要",
        "接下来",
        "下一步",
        "后续",
        "待完成",
        "尚未",
        "继续",
    ] {
        if let Some(index) = title.find(marker) {
            title = title[index..].to_string();
            break;
        }
    }
    title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.chars().count() > 70 {
        title = title.chars().take(70).collect::<String>();
    }
    title.trim_matches(['。', '，', ',', ';', '；']).to_string()
}

fn useful_suggestion(value: &str) -> bool {
    let trimmed = value.trim();
    (8..=180).contains(&trimmed.chars().count())
        && [
            "还需要",
            "接下来",
            "下一步",
            "后续",
            "待完成",
            "尚未",
            "继续",
        ]
        .iter()
        .any(|marker| trimmed.contains(marker))
        && !trimmed.contains("<codex_internal_context")
        && !trimmed.contains("Referenced ChatGPT conversation")
        && !trimmed.contains("PLEASE IMPLEMENT THIS PLAN")
}

fn report_suggestions(content: &str) -> Vec<(String, String)> {
    let mut in_next = false;
    let mut suggestions = Vec::new();
    for line in content.lines() {
        if line.starts_with("## ") {
            in_next = matches!(
                line.trim(),
                "## 下一步计划" | "## 下周建议" | "## 明日建议" | "## 未完成事项"
            );
            continue;
        }
        if !in_next || !line.trim_start().starts_with("- ") {
            continue;
        }
        let raw = line.trim().trim_start_matches("- ").trim();
        if raw.contains("已全部完成") || raw.contains("未识别") || raw.contains("暂无") {
            continue;
        }
        let (project, title) = if let Some(rest) = raw.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                (
                    rest[..end].trim().to_string(),
                    rest[end + 1..].trim().to_string(),
                )
            } else {
                ("未归类项目".to_string(), raw.to_string())
            }
        } else {
            ("未归类项目".to_string(), raw.to_string())
        };
        let title = clean_title(&title);
        if !title.is_empty() {
            suggestions.push((project, title));
        }
    }
    suggestions
}

fn task_exists(state: &DatabaseState, project: &str, title: &str) -> Result<bool, String> {
    let exists: i64 = state
        .connect()?
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE lower(trim(project))=lower(trim(?1)) AND lower(trim(title))=lower(trim(?2)) AND status<>'cancelled')",
            params![project, title],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(exists != 0)
}

fn save_suggestion(
    state: &DatabaseState,
    id: &str,
    title: &str,
    project: &str,
    source: &str,
    source_id: &str,
    note: &str,
) -> Result<bool, String> {
    if task_exists(state, project, title)? {
        return Ok(false);
    }
    let now = Utc::now().to_rfc3339();
    let changed = state
        .connect()?
        .execute(
            "INSERT INTO tasks(id,title,project,scope,status,priority,progress,note,source,source_id,created_at,updated_at)
             VALUES(?1,?2,?3,'project','draft','P1',0,?4,?5,?6,?7,?7)
             ON CONFLICT(id) DO UPDATE SET title=excluded.title,project=excluded.project,note=excluded.note,source=excluded.source,source_id=excluded.source_id,updated_at=excluded.updated_at
             WHERE tasks.status='draft'",
            params![id, title, project, note, source, source_id, now],
        )
        .map_err(|error| error.to_string())?;
    Ok(changed > 0)
}

pub fn sync_task_suggestions_for_state(
    state: &DatabaseState,
) -> Result<SuggestionSyncSummary, String> {
    let mut summary = SuggestionSyncSummary {
        conversation_suggestions: 0,
        report_suggestions: 0,
        test_suggestions: 0,
    };

    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT c.id,COALESCE(NULLIF(c.project_override,''),COALESCE(c.cwd,'')),m.content
             FROM conversations c JOIN conversation_messages m ON m.id=(
               SELECT m2.id FROM conversation_messages m2 WHERE m2.conversation_id=c.id AND m2.role='user' ORDER BY m2.source_index DESC LIMIT 1
             ) ORDER BY COALESCE(c.updated_at,c.started_at) DESC LIMIT 300",
        )
        .map_err(|error| error.to_string())?;
    let conversations = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    drop(connection);
    for (id, cwd, content) in conversations {
        if !useful_suggestion(&content) {
            continue;
        }
        let title = clean_title(&content);
        let project = project_from_path(&cwd);
        if title.chars().count() >= 6
            && save_suggestion(
                state,
                &format!("suggestion:conversation:{id}"),
                &title,
                &project,
                "conversation",
                &id,
                "从 Codex 对话中识别到明确的后续事项，请确认内容和项目后再进入正式计划。",
            )?
        {
            summary.conversation_suggestions += 1;
        }
    }

    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT id,content_markdown FROM reports ORDER BY updated_at DESC LIMIT 40")
        .map_err(|error| error.to_string())?;
    let reports = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    drop(connection);
    for (report_id, content) in reports {
        for (index, (project, title)) in report_suggestions(&content).into_iter().enumerate() {
            if save_suggestion(
                state,
                &format!("suggestion:report:{report_id}:{index}"),
                &title,
                &project,
                "report",
                &report_id,
                "从日报或周报的下一步计划中提取，请确认后进入正式计划。",
            )? {
                summary.report_suggestions += 1;
            }
        }
    }

    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT tr.menu_id,tr.project,tr.menu_name,tr.status,tr.id,COALESCE(NULLIF(tr.error_message,''),tr.output_excerpt)
             FROM test_runs tr WHERE tr.started_at=(SELECT MAX(t2.started_at) FROM test_runs t2 WHERE t2.menu_id=tr.menu_id)",
        )
        .map_err(|error| error.to_string())?;
    let tests = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    drop(connection);
    for (menu_id, project, menu_name, status, run_id, error) in tests {
        let suggestion_id = format!("suggestion:test:{menu_id}");
        if status == "passed" {
            state
                .connect()?
                .execute(
                    "DELETE FROM tasks WHERE id=?1 AND status='draft'",
                    [&suggestion_id],
                )
                .map_err(|cause| cause.to_string())?;
            continue;
        }
        let note = format!(
            "最近一次测试失败：{}。请先查看测试报告中的问题、可能原因、建议检查和建议验证。",
            clean_title(&error)
        );
        if save_suggestion(
            state,
            &suggestion_id,
            &format!("整改 {menu_name} 测试失败问题"),
            &project,
            "test",
            &run_id,
            &note,
        )? {
            summary.test_suggestions += 1;
        }
    }
    Ok(summary)
}

#[tauri::command]
pub fn sync_task_suggestions(
    state: tauri::State<'_, DatabaseState>,
) -> Result<SuggestionSyncSummary, String> {
    sync_task_suggestions_for_state(&state)
}

#[cfg(test)]
mod tests {
    use super::{
        clean_title, report_suggestions, sync_task_suggestions_for_state, useful_suggestion,
    };
    use crate::database::DatabaseState;
    use rusqlite::params;
    use uuid::Uuid;

    #[test]
    fn report_next_steps_become_project_suggestions() {
        let content = "# 周报\n\n## 下一步计划\n\n- [client] 完成案例分享筛选联调\n- 当前周期任务已全部完成。\n\n## 来源";
        assert_eq!(
            report_suggestions(content),
            vec![("client".into(), "完成案例分享筛选联调".into())]
        );
    }

    #[test]
    fn conversation_filter_requires_explicit_pending_language() {
        assert!(useful_suggestion("还需要补充用户管理详情页面"));
        assert!(!useful_suggestion("已经完成用户管理详情页面"));
        assert_eq!(
            clean_title("说明：下一步 完成接口联调。"),
            "下一步 完成接口联调"
        );
    }

    #[test]
    fn failed_test_creates_traceable_draft_and_schema_supports_project_override() {
        let path =
            std::env::temp_dir().join(format!("workbench-suggestions-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let connection = state.connect().unwrap();
        connection.execute(
            "INSERT INTO test_runs(id,menu_id,project,menu_name,mode,status,started_at,report_markdown,error_message) VALUES('run-1','client:case','client','案例分享','mock','failed','2026-08-03T10:00:00+08:00','# 报告','搜索按钮未发起请求')",
            [],
        ).unwrap();
        let has_override: i64 = connection.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name='project_override'",
            [], |row| row.get(0),
        ).unwrap();
        drop(connection);
        assert_eq!(has_override, 1);
        let summary = sync_task_suggestions_for_state(&state).unwrap();
        assert_eq!(summary.test_suggestions, 1);
        let (status, source, source_id): (String, String, Option<String>) = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT status,source,source_id FROM tasks WHERE id='suggestion:test:client:case'",
                params![],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (status.as_str(), source.as_str(), source_id.as_deref()),
            ("draft", "test", Some("run-1"))
        );
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
        }
    }
}
