use crate::{database::DatabaseState, project_identity};
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use std::collections::HashSet;
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
    pub linked_task_id: Option<String>,
    pub linked_task_status: Option<String>,
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
            "SELECT id,title,project,note,priority,status,created_at,updated_at FROM tasks
             WHERE source<>'inbox' AND (status='draft' OR id IN (SELECT source_id FROM work_inbox_items WHERE source_type='task_suggestion'))
             ORDER BY updated_at DESC LIMIT 200",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let status = row.get::<_, String>(5)?;
            let workflow_status = match status.as_str() {
                "done" | "cancelled" => "done",
                "doing" | "todo" => "in_progress",
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
    let identities = project_identity::ProjectIdentityMap::load(connection)?;
    let mut seen = HashSet::new();
    let mut statement = connection
        .prepare(
            "SELECT tr.id,tr.project,tr.menu_name,tr.mode,tr.status,COALESCE(tr.error_message,''),tr.started_at,COALESCE(tr.finished_at,tr.started_at),tr.project_path,tr.menu_id
             FROM test_runs tr
             ORDER BY julianday(tr.started_at) DESC,tr.id DESC",
        )
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map([], |row| {
            let id = row.get::<_, String>(0)?;
            let project = row.get::<_, String>(1)?;
            let mode = row.get::<_, String>(3)?;
            let path = row.get::<_, String>(8)?;
            let menu = row.get::<_, String>(9)?;
            let key = (identities.identity_key(&project, &path), menu, mode.clone());
            if !seen.insert(key) {
                return Ok(None);
            }
            let status = row.get::<_, String>(4)?;
            let updated_at = row.get::<_, String>(7)?;
            let (result_label, workflow, priority) = match status.as_str() {
                "passed" => ("通过", "done", "low"),
                "queued" | "running" => ("进行中", "in_progress", "normal"),
                "cancelled" => ("已取消", "archived", "low"),
                _ => ("失败", "needs_decision", "high"),
            };
            Ok(Some(InboxSeed {
                id: format!("test:{id}"),
                source_type: "test".into(),
                source_id: id.clone(),
                raw_project: project,
                source_path: path,
                title: format!("测试{}：{}", result_label, row.get::<_, String>(2)?),
                summary: row.get(5)?,
                detail: format!(
                    "执行方式：{}。失败结果需要确认是否继续修改；通过结果会自动完成。",
                    mode
                ),
                route: format!("/testing?run={id}"),
                priority: priority.into(),
                workflow_status: workflow.into(),
                source_status: status.clone(),
                source_revision: format!("{status}:{updated_at}"),
                created_at: row.get(6)?,
                updated_at,
            }))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(items.into_iter().flatten().collect())
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
    let tests = test_seeds(&connection)?;
    let current_tests = tests
        .iter()
        .map(|seed| seed.source_id.clone())
        .collect::<HashSet<_>>();
    let groups = [
        notification_seeds(&connection)?,
        task_seeds(&connection)?,
        tests,
        repository_seeds(&connection)?,
        tapd_job_seeds(&connection)?,
        video_seeds(&connection)?,
    ];
    let mut count = 0usize;
    for seed in groups.into_iter().flatten() {
        upsert(&connection, seed)?;
        count += 1;
    }
    let older_tests = {
        let mut query = connection.prepare("SELECT id,source_id FROM work_inbox_items WHERE source_type='test' AND source_status<>'superseded'").map_err(|error| error.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    for (id, source_id) in older_tests {
        if !current_tests.contains(&source_id) {
            connection.execute("UPDATE work_inbox_items SET workflow_status='archived',source_status='superseded' WHERE id=?1", [id]).map_err(|error| error.to_string())?;
        }
    }
    sync_linked_tasks(&connection)?;
    Ok(count)
}

fn sync_linked_tasks(connection: &Connection) -> Result<(), String> {
    // 兼容旧版已转出的任务；只关联已有任务，不重建或复制任务。
    connection
        .execute_batch(
            "INSERT OR IGNORE INTO inbox_task_links(inbox_id,task_id,source_revision,task_status)
        SELECT i.id,t.id,i.source_revision,t.status FROM work_inbox_items i JOIN tasks t ON t.id=(
            SELECT t2.id FROM tasks t2 WHERE (t2.source='inbox' AND t2.source_id=i.id)
               OR (i.source_type='task_suggestion' AND t2.id=i.source_id AND t2.status<>'draft')
            ORDER BY t2.created_at,t2.id LIMIT 1);",
        )
        .map_err(|error| error.to_string())?;
    let rows = {
        let mut query = connection.prepare("SELECT i.id,i.source_revision,l.source_revision,l.task_status,t.status
            FROM work_inbox_items i JOIN inbox_task_links l ON l.inbox_id=i.id JOIN tasks t ON t.id=l.task_id
            WHERE i.workflow_status<>'archived' AND i.source_status<>'superseded'").map_err(|error| error.to_string())?;
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    for (id, source_revision, linked_revision, previous_task_status, task_status) in rows {
        if source_revision != linked_revision && task_status == previous_task_status {
            continue;
        }
        let workflow = if matches!(task_status.as_str(), "done" | "cancelled") {
            "done"
        } else {
            "in_progress"
        };
        connection.execute("UPDATE work_inbox_items SET workflow_status=?1 WHERE id=?2 AND workflow_status<>?1", params![workflow,id]).map_err(|error| error.to_string())?;
        connection.execute("UPDATE inbox_task_links SET source_revision=?1,task_status=?2 WHERE inbox_id=?3 AND (source_revision<>?1 OR task_status<>?2)", params![source_revision,task_status,id]).map_err(|error| error.to_string())?;
    }
    Ok(())
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
            "SELECT i.id,i.source_type,i.source_id,i.project,i.title,i.summary,i.detail,i.route,i.priority,i.workflow_status,i.source_status,i.created_at,i.updated_at,l.task_id,t.status
             FROM work_inbox_items i LEFT JOIN inbox_task_links l ON l.inbox_id=i.id LEFT JOIN tasks t ON t.id=l.task_id
             WHERE i.source_type<>'video' AND (?1='' OR i.workflow_status=?1)
               AND NOT(i.source_type='task_suggestion' AND EXISTS(SELECT 1 FROM tasks old WHERE old.id=i.source_id AND old.source='inbox'))
             ORDER BY CASE i.workflow_status WHEN 'needs_decision' THEN 0 WHEN 'in_progress' THEN 1 WHEN 'new' THEN 2 WHEN 'done' THEN 3 ELSE 4 END,
                       CASE i.priority WHEN 'high' THEN 0 WHEN 'normal' THEN 1 ELSE 2 END,datetime(i.updated_at) DESC,i.id
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
                linked_task_id: row.get(13)?,
                linked_task_status: row.get(14)?,
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
    update_inbox_status_for_state(&state, &id, &status)
}

fn update_inbox_status_for_state(
    state: &DatabaseState,
    id: &str,
    status: &str,
) -> Result<(), String> {
    if !["needs_decision", "in_progress", "done", "archived"].contains(&status) {
        return Err("无效的待处理状态。".into());
    }
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    let changed = transaction
        .execute(
            "UPDATE work_inbox_items SET workflow_status=?1,updated_at=?2 WHERE id=?3",
            params![status, now, id],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("待处理事项不存在，请刷新后重试。".into());
    }
    if status != "archived" {
        let task_status = match status {
            "done" => "done",
            "in_progress" => "doing",
            _ => "todo",
        };
        transaction.execute("UPDATE tasks SET status=?1,progress=CASE WHEN ?1='done' THEN 100 WHEN status IN ('done','cancelled') THEN 0 ELSE progress END,
            completed_at=CASE WHEN ?1='done' THEN ?2 ELSE NULL END,updated_at=?2
            WHERE id IN (SELECT task_id FROM inbox_task_links WHERE inbox_id=?3)", params![task_status,now,id]).map_err(|error| error.to_string())?;
        transaction.execute("UPDATE inbox_task_links SET task_status=?1,source_revision=(SELECT source_revision FROM work_inbox_items WHERE id=?2) WHERE inbox_id=?2", params![task_status,id]).map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn create_task_from_inbox(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<String, String> {
    create_task_for_state(&state, &id)
}

fn create_task_for_state(state: &DatabaseState, id: &str) -> Result<String, String> {
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let (title, project, summary, priority, source_type, source_id, revision): (String, String, String, String, String, String, String) = transaction
        .query_row(
            "SELECT title,project,summary,priority,source_type,source_id,source_revision FROM work_inbox_items WHERE id=?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        )
        .map_err(|error| error.to_string())?;
    let existing = transaction.query_row(
        "SELECT id FROM tasks WHERE (source='inbox' AND source_id=?1) OR (?2='task_suggestion' AND id=?3) ORDER BY created_at,id LIMIT 1",
        params![id,source_type,source_id],
        |row| row.get::<_, String>(0),
    ).optional().map_err(|error| error.to_string())?;
    let task_id = existing
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = Utc::now().to_rfc3339();
    let planned_date = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let task_priority = if priority == "high" {
        "P0"
    } else if priority == "low" {
        "P2"
    } else {
        "P1"
    };
    if existing.is_none() {
        transaction
        .execute(
            "INSERT INTO tasks(id,title,project,scope,status,priority,planned_date,progress,note,source,source_id,created_at,updated_at)
             VALUES(?1,?2,?3,'day','todo',?4,?5,0,?6,'inbox',?7,?8,?8)",
            params![task_id, title, project, task_priority, planned_date, summary, id, now],
        )
        .map_err(|error| error.to_string())?;
    } else {
        transaction.execute("UPDATE tasks SET status='todo',planned_date=COALESCE(planned_date,?1),updated_at=?2 WHERE id=?3 AND status='draft'", params![planned_date,now,task_id]).map_err(|error| error.to_string())?;
    }
    let task_status: String = transaction
        .query_row("SELECT status FROM tasks WHERE id=?1", [&task_id], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO inbox_task_links(inbox_id,task_id,source_revision,task_status) VALUES(?1,?2,?3,?4)
        ON CONFLICT(inbox_id) DO UPDATE SET task_id=excluded.task_id,source_revision=excluded.source_revision,task_status=excluded.task_status", params![id,task_id,revision,task_status]).map_err(|error| error.to_string())?;
    let workflow = if matches!(task_status.as_str(), "done" | "cancelled") {
        "done"
    } else {
        "in_progress"
    };
    transaction
        .execute(
            "UPDATE work_inbox_items SET workflow_status=?1,updated_at=?2 WHERE id=?3",
            params![workflow, now, id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseState;

    struct TestDirectory(std::path::PathBuf);
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_database() -> (TestDirectory, DatabaseState) {
        let directory =
            std::env::temp_dir().join(format!("workbench-inbox-flow-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        (TestDirectory(directory), state)
    }

    fn seed_quick_item(state: &DatabaseState) {
        state.connect().unwrap().execute("INSERT INTO work_inbox_items(id,source_type,source_id,project,title,source_revision,created_at,updated_at)
            VALUES('item-1','quick_capture','capture-1','演示项目','检查接口','revision-1','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')", []).unwrap();
    }

    fn item(state: &DatabaseState) -> InboxItem {
        list_inbox_items_for_state(state, None, None)
            .unwrap()
            .into_iter()
            .find(|item| item.id == "item-1")
            .unwrap()
    }

    #[test]
    fn linked_task_completes_reopens_and_does_not_hide_new_source_revision() {
        let (_directory, state) = test_database();
        seed_quick_item(&state);
        let task = create_task_for_state(&state, "item-1").unwrap();
        assert_eq!(create_task_for_state(&state, "item-1").unwrap(), task);
        let current = item(&state);
        assert_eq!(current.linked_task_id.as_deref(), Some(task.as_str()));
        assert_eq!(current.workflow_status, "in_progress");
        assert!(!list_inbox_items_for_state(&state, None, None)
            .unwrap()
            .iter()
            .any(|item| item.source_type == "task_suggestion"));
        state
            .connect()
            .unwrap()
            .execute(
                "UPDATE tasks SET status='done',progress=100 WHERE id=?1",
                [&task],
            )
            .unwrap();
        assert_eq!(item(&state).workflow_status, "done");
        state.connect().unwrap().execute("UPDATE work_inbox_items SET source_revision='revision-2',workflow_status='needs_decision' WHERE id='item-1'",[]).unwrap();
        assert_eq!(item(&state).workflow_status, "needs_decision");
        assert_eq!(item(&state).workflow_status, "needs_decision");
        update_inbox_status_for_state(&state, "item-1", "in_progress").unwrap();
        let linked = item(&state);
        assert_eq!(linked.linked_task_status.as_deref(), Some("doing"));
        assert_eq!(linked.workflow_status, "in_progress");
        update_inbox_status_for_state(&state, "item-1", "done").unwrap();
        assert_eq!(item(&state).linked_task_status.as_deref(), Some("done"));
        state
            .connect()
            .unwrap()
            .execute("UPDATE tasks SET status='todo' WHERE id=?1", [&task])
            .unwrap();
        assert_eq!(item(&state).workflow_status, "in_progress");
        assert_eq!(
            state
                .connect()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM tasks WHERE source='inbox'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn arranging_suggested_task_uses_existing_task_and_keeps_it_in_progress() {
        let (_directory, state) = test_database();
        state.connect().unwrap().execute_batch("INSERT INTO tasks(id,title,project,scope,status,priority,source,created_at,updated_at)
            VALUES('draft-task','整理说明','演示项目','day','draft','P1','conversation','2026-09-01T00:00:00Z','2026-09-01T00:00:00Z');").unwrap();
        sync_inbox_for_state(&state).unwrap();
        assert_eq!(
            create_task_for_state(&state, "task:draft-task").unwrap(),
            "draft-task"
        );
        let rows = list_inbox_items_for_state(&state, None, None).unwrap();
        let linked = rows
            .iter()
            .find(|item| item.id == "task:draft-task")
            .unwrap();
        assert_eq!(linked.workflow_status, "in_progress");
        assert_eq!(linked.linked_task_id.as_deref(), Some("draft-task"));
        assert_eq!(
            state
                .connect()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn creating_task_is_transactional_and_concurrent_requests_are_idempotent() {
        let (_directory, state) = test_database();
        seed_quick_item(&state);
        state.connect().unwrap().execute_batch("CREATE TRIGGER reject_link BEFORE INSERT ON inbox_task_links BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(create_task_for_state(&state, "item-1").is_err());
        assert_eq!(
            state
                .connect()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        state
            .connect()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_link;")
            .unwrap();
        let workers = (0..2)
            .map(|_| {
                let state = state.clone();
                std::thread::spawn(move || create_task_for_state(&state, "item-1").unwrap())
            })
            .collect::<Vec<_>>();
        let ids = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids[0], ids[1]);
        assert_eq!(
            state
                .connect()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    fn insert_test(
        state: &DatabaseState,
        id: &str,
        path: &str,
        mode: &str,
        status: &str,
        time: &str,
    ) {
        state.connect().unwrap().execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at,finished_at)
            VALUES(?1,'shared-menu','client',?2,'菜单',?3,?4,?5,?5)", params![id,path,mode,status,time]).unwrap();
    }

    #[test]
    fn latest_tests_use_project_identity_menu_and_mode_without_cross_project_overwrites() {
        let (_directory, state) = test_database();
        state.connect().unwrap().execute("INSERT INTO project_profiles(id,display_name,repository_path,aliases_json,created_at,updated_at)
            VALUES('profile-a','项目A','F:/new/client','[\"F:/old/client\"]','2026-09-01','2026-09-01')",[]).unwrap();
        insert_test(
            &state,
            "a-old",
            "F:/old/client",
            "api",
            "failed",
            "2026-09-01T00:00:00Z",
        );
        insert_test(
            &state,
            "b-api",
            "F:/other/client",
            "api",
            "failed",
            "2026-09-01T01:00:00Z",
        );
        insert_test(
            &state,
            "a-browser",
            "F:/new/client",
            "browser",
            "failed",
            "2026-09-01T02:00:00Z",
        );
        sync_inbox_for_state(&state).unwrap();
        insert_test(
            &state,
            "a-new",
            "F:/new/client",
            "api",
            "passed",
            "2026-09-01T03:00:00Z",
        );
        let rows = list_inbox_items_for_state(&state, None, None).unwrap();
        assert_eq!(
            rows.iter()
                .find(|item| item.source_id == "b-api")
                .unwrap()
                .workflow_status,
            "needs_decision"
        );
        assert_eq!(
            rows.iter()
                .find(|item| item.source_id == "a-browser")
                .unwrap()
                .workflow_status,
            "needs_decision"
        );
        assert_eq!(
            rows.iter()
                .find(|item| item.source_id == "a-new")
                .unwrap()
                .workflow_status,
            "done"
        );
        assert_eq!(
            rows.iter()
                .find(|item| item.source_id == "a-old")
                .unwrap()
                .workflow_status,
            "archived"
        );
        let seeds = test_seeds(&state.connect().unwrap()).unwrap();
        assert_eq!(seeds.len(), 3);
        insert_test(
            &state,
            "b-running",
            "F:/other/client",
            "api",
            "running",
            "2026-09-01T04:00:00Z",
        );
        let rows = list_inbox_items_for_state(&state, None, None).unwrap();
        let running = rows
            .iter()
            .find(|item| item.source_id == "b-running")
            .unwrap();
        assert_eq!(running.workflow_status, "in_progress");
        assert!(running.title.contains("进行中"));
    }

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
