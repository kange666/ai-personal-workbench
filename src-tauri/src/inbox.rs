use crate::{database::DatabaseState, project_identity};
use chrono::{Local, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub project: String,
    pub title: String,
    pub summary: String,
    pub detail: String,
    pub route: String,
    pub priority: String,
    pub workflow_status: String,
    pub source_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
struct InboxSeed {
    id: String,
    source_type: String,
    source_id: String,
    raw_project: String,
    source_path: String,
    title: String,
    summary: String,
    detail: String,
    route: String,
    priority: String,
    workflow_status: String,
    source_status: String,
    source_revision: String,
    created_at: String,
    updated_at: String,
}

fn upsert(connection: &Connection, seed: InboxSeed) -> Result<(), String> {
    let project =
        project_identity::canonical_project_name(connection, &seed.raw_project, &seed.source_path);
    connection
        .execute(
            "INSERT INTO work_inbox_items(id,source_type,source_id,project,title,summary,detail,route,priority,workflow_status,source_status,source_revision,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(source_type,source_id) DO UPDATE SET project=excluded.project,title=excluded.title,summary=excluded.summary,detail=excluded.detail,route=excluded.route,priority=excluded.priority,
               workflow_status=CASE WHEN work_inbox_items.source_revision=excluded.source_revision THEN work_inbox_items.workflow_status ELSE excluded.workflow_status END,
               source_status=excluded.source_status,source_revision=excluded.source_revision,updated_at=excluded.updated_at",
            params![seed.id,seed.source_type,seed.source_id,project,seed.title,seed.summary,seed.detail,seed.route,seed.priority,seed.workflow_status,seed.source_status,seed.source_revision,seed.created_at,seed.updated_at],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn notification_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT n.id,n.kind,n.title,n.body,n.output,COALESCE(n.source_id,''),n.route,n.review_status,n.created_at,
                    COALESCE(NULLIF(c.project_override,''),NULLIF(c.cwd,''),NULLIF(tp.workspace_name,''),NULLIF(tjp.workspace_name,''),'未归类项目'),
                    COALESCE(NULLIF(c.cwd,''),NULLIF(tp.repository_path,''),NULLIF(tj.repository_path,''),''),
                    COALESCE(tw.item_key,'')
             FROM notifications n
             LEFT JOIN conversations c ON c.id=n.source_id
             LEFT JOIN tapd_work_items tw ON tw.id=n.source_id AND instr(n.route,'project=' || tw.workspace_id)>0
             LEFT JOIN tapd_projects tp ON tp.workspace_id=tw.workspace_id
             LEFT JOIN tapd_codex_jobs tj ON tj.id=n.source_id
             LEFT JOIN tapd_projects tjp ON tjp.workspace_id=tj.workspace_id
             ORDER BY n.created_at DESC LIMIT 300",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let kind = row.get::<_, String>(1)?;
            let review = row.get::<_, String>(7)?;
            let created_at = row.get::<_, String>(8)?;
            let notification_id = row.get::<_, String>(0)?;
            let raw_source_id = row.get::<_, String>(5)?;
            let tapd_item_key = row.get::<_, String>(11)?;
            let source_type = if kind == "tapd_item" { "tapd" } else { "codex" };
            let source_id = if kind == "tapd_item" && !tapd_item_key.is_empty() {
                tapd_item_key
            } else if raw_source_id.is_empty() {
                notification_id.clone()
            } else {
                raw_source_id
            };
            let workflow_status = match review.as_str() {
                "accepted" => "done",
                "follow_up" => "in_progress",
                _ => "needs_decision",
            };
            Ok(InboxSeed {
                id: format!("notification:{source_type}:{source_id}"),
                source_type: source_type.into(),
                source_id,
                raw_project: row.get(9)?,
                source_path: row.get(10)?,
                title: row.get(2)?,
                summary: row.get(3)?,
                detail: row.get(4)?,
                route: row.get(6)?,
                priority: if kind == "tapd_item" {
                    "high"
                } else {
                    "normal"
                }
                .into(),
                workflow_status: workflow_status.into(),
                source_status: review.clone(),
                source_revision: format!("{review}:{created_at}"),
                created_at: created_at.clone(),
                updated_at: created_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut items = items;
    // 查询先保留最近 300 条，再按时间正序写入，确保同一来源最终保留最新状态。
    items.reverse();
    Ok(items)
}

fn task_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id,title,project,note,priority,status,created_at,updated_at FROM tasks WHERE status='draft' OR source='inbox' ORDER BY updated_at DESC LIMIT 200",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let status = row.get::<_, String>(5)?;
            let workflow_status = match status.as_str() {
                "done" | "cancelled" => "done",
                "doing" => "in_progress",
                _ => "needs_decision",
            };
            let source_id = row.get::<_, String>(0)?;
            let updated_at = row.get::<_, String>(7)?;
            Ok(InboxSeed {
                id: format!("task:{source_id}"),
                source_type: "task_suggestion".into(),
                source_id: source_id.clone(),
                raw_project: row.get(2)?,
                source_path: String::new(),
                title: row.get(1)?,
                summary: row.get(3)?,
                detail: "该事项由报告、测试或 Codex 工作记录自动建议，需要你确认后再进入日程。"
                    .into(),
                route: format!("/calendar?tab=tasks&task={source_id}"),
                priority: match row.get::<_, String>(4)?.as_str() {
                    "P0" => "high",
                    "P2" => "low",
                    _ => "normal",
                }
                .into(),
                workflow_status: workflow_status.into(),
                source_status: status.clone(),
                source_revision: format!("{status}:{updated_at}"),
                created_at: row.get(6)?,
                updated_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

fn test_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT tr.id,tr.project,tr.menu_name,tr.mode,tr.status,COALESCE(tr.error_message,''),tr.started_at,COALESCE(tr.finished_at,tr.started_at)
             FROM test_runs tr
             WHERE tr.id IN (SELECT tr2.id FROM test_runs tr2 WHERE tr2.menu_id=tr.menu_id ORDER BY tr2.started_at DESC LIMIT 1)",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let status = row.get::<_, String>(4)?;
            let updated_at = row.get::<_, String>(7)?;
            Ok(InboxSeed {
                id: format!("test:{id}"),
                source_type: "test".into(),
                source_id: id.clone(),
                raw_project: row.get(1)?,
                source_path: String::new(),
                title: format!(
                    "测试{}：{}",
                    if status == "passed" {
                        "通过"
                    } else {
                        "失败"
                    },
                    row.get::<_, String>(2)?
                ),
                summary: row.get(5)?,
                detail: format!(
                    "执行方式：{}。失败结果需要确认是否继续修改；通过结果会自动完成。",
                    row.get::<_, String>(3)?
                ),
                route: format!("/testing?run={id}"),
                priority: if status == "passed" { "low" } else { "high" }.into(),
                workflow_status: if status == "passed" {
                    "done"
                } else {
                    "needs_decision"
                }
                .into(),
                source_status: status.clone(),
                source_revision: format!("{status}:{updated_at}"),
                created_at: row.get(6)?,
                updated_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

fn repository_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT path,name,category,has_uncommitted_changes,changed_file_count,ahead_count,behind_count,updated_at,
                    COALESCE((SELECT status FROM repository_runtime_runs r WHERE r.repository_path=a.path ORDER BY r.started_at DESC LIMIT 1),''),
                    COALESCE((SELECT error_message FROM repository_runtime_runs r WHERE r.repository_path=a.path ORDER BY r.started_at DESC LIMIT 1),'')
             FROM repository_assets a WHERE is_hidden=0",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let path = row.get::<_, String>(0)?;
            let dirty = row.get::<_, i64>(3)? != 0;
            let changed = row.get::<_, i64>(4)?;
            let ahead = row.get::<_, i64>(5)?;
            let behind = row.get::<_, i64>(6)?;
            let updated_at = row.get::<_, String>(7)?;
            let runtime = row.get::<_, String>(8)?;
            let runtime_error = row.get::<_, String>(9)?;
            let needs_attention = dirty || behind > 0 || runtime == "failed";
            let summary = if runtime == "failed" {
                if runtime_error.trim().is_empty() {
                    "本地项目启动失败，请查看运行日志。".into()
                } else {
                    runtime_error
                }
            } else if behind > 0 {
                format!("本地分支落后远程 {behind} 个提交，需要决定是否拉取。")
            } else if dirty {
                format!("工作区有 {changed} 个未提交文件，需要确认提交、保留或继续修改。")
            } else {
                "项目当前没有需要处理的本地风险。".into()
            };
            Ok(InboxSeed {
                id: format!("repository:{path}"),
                source_type: "repository".into(),
                source_id: path.clone(),
                raw_project: row.get(1)?,
                source_path: path.clone(),
                title: format!(
                    "{}：{}",
                    row.get::<_, String>(1)?,
                    if runtime == "failed" {
                        "启动失败"
                    } else if behind > 0 {
                        "需要更新代码"
                    } else if dirty {
                        "存在未提交修改"
                    } else {
                        "状态正常"
                    }
                ),
                summary,
                detail: format!(
                    "分类：{}；领先 {ahead}，落后 {behind}，变更文件 {changed}。",
                    row.get::<_, String>(2)?
                ),
                route: "/projects".into(),
                priority: if runtime == "failed" {
                    "high"
                } else if behind > 0 {
                    "normal"
                } else {
                    "low"
                }
                .into(),
                workflow_status: if needs_attention {
                    "needs_decision"
                } else {
                    "done"
                }
                .into(),
                source_status: if needs_attention {
                    "attention"
                } else {
                    "clean"
                }
                .into(),
                source_revision: format!("{dirty}:{changed}:{ahead}:{behind}:{runtime}"),
                created_at: updated_at.clone(),
                updated_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

fn tapd_job_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT j.id,j.repository_path,j.status,j.review_status,j.error_message,j.output,j.created_at,j.updated_at,w.title,p.workspace_name
             FROM tapd_codex_jobs j JOIN tapd_work_items w ON w.item_key=j.item_key LEFT JOIN tapd_projects p ON p.workspace_id=j.workspace_id
             ORDER BY j.updated_at DESC LIMIT 200",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let status = row.get::<_, String>(2)?;
            let review = row.get::<_, String>(3)?;
            let updated_at = row.get::<_, String>(7)?;
            let workflow_status = if review == "accepted" {
                "done"
            } else if review == "changes_requested" || status == "running" || status == "queued" {
                "in_progress"
            } else {
                "needs_decision"
            };
            Ok(InboxSeed {
                id: format!("tapd_job:{id}"),
                source_type: "tapd_job".into(),
                source_id: id.clone(),
                raw_project: row.get(9)?,
                source_path: row.get(1)?,
                title: format!("TAPD 自动处理：{}", row.get::<_, String>(8)?),
                summary: if status == "failed" {
                    row.get(4)?
                } else {
                    row.get(5)?
                },
                detail: "Codex 修改完成后需要通过测试门槛和人工确认，才会回写 TAPD 为已解决。"
                    .into(),
                route: format!("/tapd?job={id}"),
                priority: if status == "failed" { "high" } else { "normal" }.into(),
                workflow_status: workflow_status.into(),
                source_status: format!("{status}:{review}"),
                source_revision: format!("{status}:{review}:{updated_at}"),
                created_at: row.get(6)?,
                updated_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

fn video_seeds(connection: &Connection) -> Result<Vec<InboxSeed>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id,title,status,progress_message,failure_reason,project_root,created_at,updated_at FROM video_jobs ORDER BY updated_at DESC LIMIT 100",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let status = row.get::<_, String>(2)?;
            let updated_at = row.get::<_, String>(7)?;
            let failed = status == "failed" || status == "needs-attention";
            let running = matches!(status.as_str(), "queued" | "running" | "finalizing");
            Ok(InboxSeed {
                id: format!("video:{id}"),
                source_type: "video".into(),
                source_id: id.clone(),
                raw_project: "视频创作".into(),
                source_path: row.get(5)?,
                title: format!(
                    "视频{}：{}",
                    if failed {
                        "需要处理"
                    } else if running {
                        "生成中"
                    } else {
                        "已完成"
                    },
                    row.get::<_, String>(1)?
                ),
                summary: if failed { row.get(4)? } else { row.get(3)? },
                detail: "查看视频中心可读取生成进度、Codex 输出和交付文件。".into(),
                route: format!("/videos?job={id}"),
                priority: if failed { "high" } else { "low" }.into(),
                workflow_status: if failed {
                    "needs_decision"
                } else if running {
                    "in_progress"
                } else {
                    "done"
                }
                .into(),
                source_status: status.clone(),
                source_revision: format!("{status}:{updated_at}"),
                created_at: row.get(6)?,
                updated_at,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

pub fn sync_inbox_for_state(state: &DatabaseState) -> Result<usize, String> {
    project_identity::sync_project_profiles_for_state(state)?;
    let connection = state.connect()?;
    let groups = [
        notification_seeds(&connection)?,
        task_seeds(&connection)?,
        test_seeds(&connection)?,
        repository_seeds(&connection)?,
        tapd_job_seeds(&connection)?,
        video_seeds(&connection)?,
    ];
    let mut count = 0usize;
    for seed in groups.into_iter().flatten() {
        upsert(&connection, seed)?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_inbox_items(
    state: tauri::State<'_, DatabaseState>,
    status: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<InboxItem>, String> {
    list_inbox_items_for_state(&state, status, limit)
}

fn list_inbox_items_for_state(
    state: &DatabaseState,
    status: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<InboxItem>, String> {
    sync_inbox_for_state(state)?;
    let connection = state.connect()?;
    let status = status.unwrap_or_default();
    let mut statement = connection
        .prepare(
            "SELECT id,source_type,source_id,project,title,summary,detail,route,priority,workflow_status,source_status,created_at,updated_at
             FROM work_inbox_items
             WHERE source_type<>'video' AND (?1='' OR workflow_status=?1)
             ORDER BY CASE workflow_status WHEN 'needs_decision' THEN 0 WHEN 'in_progress' THEN 1 WHEN 'new' THEN 2 WHEN 'done' THEN 3 ELSE 4 END,
                      CASE priority WHEN 'high' THEN 0 WHEN 'normal' THEN 1 ELSE 2 END,datetime(updated_at) DESC
             LIMIT ?2",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map(params![status, limit.unwrap_or(200).clamp(1, 500)], |row| {
            Ok(InboxItem {
                id: row.get(0)?,
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                project: row.get(3)?,
                title: row.get(4)?,
                summary: row.get(5)?,
                detail: row.get(6)?,
                route: row.get(7)?,
                priority: row.get(8)?,
                workflow_status: row.get(9)?,
                source_status: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items)
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_inbox_status(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    status: String,
) -> Result<(), String> {
    if !["needs_decision", "in_progress", "done", "archived"].contains(&status.as_str()) {
        return Err("无效的待处理状态。".into());
    }
    let changed = state
        .connect()?
        .execute(
            "UPDATE work_inbox_items SET workflow_status=?1,updated_at=?2 WHERE id=?3",
            params![status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("待处理事项不存在，请刷新后重试。".into());
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn create_task_from_inbox(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<String, String> {
    let connection = state.connect()?;
    let (title, project, summary, priority): (String, String, String, String) = connection
        .query_row(
            "SELECT title,project,summary,priority FROM work_inbox_items WHERE id=?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| error.to_string())?;
    if let Ok(existing) = connection.query_row(
        "SELECT id FROM tasks WHERE source='inbox' AND source_id=?1 LIMIT 1",
        [&id],
        |row| row.get::<_, String>(0),
    ) {
        return Ok(existing);
    }
    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let planned_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let task_priority = if priority == "high" {
        "P0"
    } else if priority == "low" {
        "P2"
    } else {
        "P1"
    };
    connection
        .execute(
            "INSERT INTO tasks(id,title,project,scope,status,priority,planned_date,progress,note,source,source_id,created_at,updated_at)
             VALUES(?1,?2,?3,'day','todo',?4,?5,0,?6,'inbox',?7,?8,?8)",
            params![task_id, title, project, task_priority, planned_date, summary, id, now],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE work_inbox_items SET workflow_status='in_progress',updated_at=?1 WHERE id=?2",
            params![now, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use super::{list_inbox_items_for_state, sync_inbox_for_state};
    use crate::database::DatabaseState;

    #[test]
    fn inbox_status_contract_is_small_and_explicit() {
        for value in ["needs_decision", "in_progress", "done", "archived"] {
            assert!(!value.is_empty());
        }
    }

    #[test]
    fn fresh_database_can_materialize_repository_attention() {
        let directory =
            std::env::temp_dir().join(format!("workbench-inbox-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        state
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO repository_assets(path,name,has_uncommitted_changes,changed_file_count,last_scanned_at,updated_at) VALUES(?1,?2,1,3,?3,?3)",
                rusqlite::params![r"F:\TB-project\client", "client", "2026-08-24T09:00:00+08:00"],
            )
            .unwrap();
        state
            .connect()
            .unwrap()
            .execute_batch(
                "INSERT INTO notifications(id,kind,title,body,source_id,created_at) VALUES
                   ('n1','codex_complete','旧结果','旧摘要','conversation-1','2026-08-24T08:00:00+08:00'),
                   ('n2','codex_complete','最新结果','最新摘要','conversation-1','2026-08-24T09:00:00+08:00');",
            )
            .unwrap();

        assert_eq!(sync_inbox_for_state(&state).unwrap(), 3);
        let row = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT project,workflow_status,source_type FROM work_inbox_items WHERE source_type='repository' LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "client".into(),
                "needs_decision".into(),
                "repository".into()
            )
        );
        let codex_rows = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT COUNT(*),MAX(title) FROM work_inbox_items WHERE source_type='codex'",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(codex_rows, (1, "最新结果".into()));

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn inbox_list_excludes_video_sources() {
        let directory =
            std::env::temp_dir().join(format!("workbench-inbox-video-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let connection = state.connect().unwrap();
        connection
            .execute_batch(
                "INSERT INTO work_inbox_items(id,source_type,source_id,project,title,created_at,updated_at)
                 VALUES('manual-1','quick_capture','capture-1','未归类项目','整理发布说明','2026-09-02T10:00:00Z','2026-09-02T10:00:00Z');
                 INSERT INTO video_jobs(id,title,video_type,project_root,status,created_at,updated_at)
                 VALUES('video-1','视频任务','tech','F:/video','failed','2026-09-02T10:00:00Z','2026-09-02T10:00:00Z');",
            )
            .unwrap();
        drop(connection);

        let items = list_inbox_items_for_state(&state, None, Some(100)).unwrap();
        assert!(items.iter().any(|item| item.source_type == "quick_capture"));
        assert!(items.iter().all(|item| item.source_type != "video"));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
