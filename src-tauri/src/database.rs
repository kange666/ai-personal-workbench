use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 15;

#[derive(Clone)]
pub struct DatabaseState {
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseHealth {
    pub path: String,
    pub schema_version: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenSummary {
    pub conversation_count: i64,
    pub message_count: i64,
    pub active_days: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMetric {
    pub id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub project: String,
    pub model: Option<String>,
    pub updated_at: Option<String>,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub context_used_tokens: i64,
    pub context_window: i64,
    pub archived: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTrendPoint {
    pub date: String,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTokenMetric {
    pub project: String,
    pub conversation_count: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTokenMetric {
    pub model: String,
    pub conversation_count: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub date: Option<String>,
    pub route: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTask {
    pub id: String,
    pub title: String,
    pub project: String,
    pub scope: String,
    pub status: String,
    pub priority: String,
    pub planned_date: Option<String>,
    pub week_start: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub progress: i64,
    pub note: String,
    pub source: String,
    pub source_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

impl DatabaseState {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        let state = Self { path };
        state.initialize()?;
        Ok(state)
    }

    pub fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.path).map_err(|error| error.to_string())?;
        connection
            .busy_timeout(std::time::Duration::from_secs(15))
            .map_err(|error| error.to_string())?;
        Ok(connection)
    }

    fn initialize(&self) -> Result<(), String> {
        let connection = self.connect()?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS app_meta (
                   key TEXT PRIMARY KEY,
                   value TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS tasks (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   project TEXT NOT NULL,
                   scope TEXT NOT NULL CHECK(scope IN ('day','week','project')),
                   status TEXT NOT NULL,
                   priority TEXT NOT NULL,
                   planned_date TEXT,
                   week_start TEXT,
                   start_date TEXT,
                   end_date TEXT,
                   progress INTEGER NOT NULL DEFAULT 0,
                   note TEXT NOT NULL DEFAULT '',
                   source TEXT NOT NULL DEFAULT 'manual',
                   source_id TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   completed_at TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_tasks_scope ON tasks(scope);
                 CREATE INDEX IF NOT EXISTS idx_tasks_planned_date ON tasks(planned_date);
                 CREATE INDEX IF NOT EXISTS idx_tasks_week_start ON tasks(week_start);
                 CREATE TABLE IF NOT EXISTS conversations (
                   id TEXT PRIMARY KEY,
                   source_file TEXT NOT NULL,
                   title TEXT,
                   cwd TEXT,
                   project_override TEXT,
                   started_at TEXT,
                   updated_at TEXT,
                   model TEXT,
                   input_tokens INTEGER NOT NULL DEFAULT 0,
                   cached_input_tokens INTEGER NOT NULL DEFAULT 0,
                   output_tokens INTEGER NOT NULL DEFAULT 0,
                   reasoning_output_tokens INTEGER NOT NULL DEFAULT 0,
                   total_tokens INTEGER NOT NULL DEFAULT 0,
                   context_used_tokens INTEGER NOT NULL DEFAULT 0,
                   context_window INTEGER NOT NULL DEFAULT 0,
                   source_size INTEGER NOT NULL DEFAULT 0,
                   source_modified_ns INTEGER NOT NULL DEFAULT 0,
                   archived INTEGER NOT NULL DEFAULT 0,
                   imported_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_source ON conversations(source_file);
                 CREATE TABLE IF NOT EXISTS token_events (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   conversation_id TEXT NOT NULL,
                   event_time TEXT,
                   input_tokens INTEGER NOT NULL DEFAULT 0,
                   cached_input_tokens INTEGER NOT NULL DEFAULT 0,
                   output_tokens INTEGER NOT NULL DEFAULT 0,
                   reasoning_output_tokens INTEGER NOT NULL DEFAULT 0,
                   total_tokens INTEGER NOT NULL DEFAULT 0,
                   UNIQUE(conversation_id, event_time, total_tokens),
                   FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS conversation_messages (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   conversation_id TEXT NOT NULL,
                   source_index INTEGER NOT NULL,
                   event_time TEXT,
                   role TEXT NOT NULL,
                   content TEXT NOT NULL,
                   UNIQUE(conversation_id,source_index),
                   FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_conversation_messages_time ON conversation_messages(event_time);
                 CREATE TABLE IF NOT EXISTS reports (
                   id TEXT PRIMARY KEY,
                   report_type TEXT NOT NULL,
                   period_start TEXT NOT NULL,
                   period_end TEXT NOT NULL,
                   title TEXT NOT NULL,
                   content_markdown TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'draft',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_reports_period ON reports(report_type,period_start,period_end);
                 CREATE TABLE IF NOT EXISTS git_repositories (
                   path TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   current_branch TEXT,
                   user_name TEXT NOT NULL DEFAULT '',
                   user_email TEXT NOT NULL DEFAULT '',
                   last_scanned_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS git_commits (
                   repository_path TEXT NOT NULL,
                   commit_hash TEXT NOT NULL,
                   committed_at TEXT NOT NULL,
                   subject TEXT NOT NULL,
                   author_name TEXT NOT NULL DEFAULT '',
                   author_email TEXT NOT NULL DEFAULT '',
                   file_count INTEGER NOT NULL DEFAULT 0,
                   additions INTEGER NOT NULL DEFAULT 0,
                   deletions INTEGER NOT NULL DEFAULT 0,
                   PRIMARY KEY(repository_path, commit_hash),
                   FOREIGN KEY(repository_path) REFERENCES git_repositories(path) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_git_commits_time ON git_commits(committed_at);
                 CREATE TABLE IF NOT EXISTS git_worktree_snapshots (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   repository_path TEXT NOT NULL,
                   captured_at TEXT NOT NULL,
                   modified_count INTEGER NOT NULL DEFAULT 0,
                   added_count INTEGER NOT NULL DEFAULT 0,
                   deleted_count INTEGER NOT NULL DEFAULT 0,
                   untracked_count INTEGER NOT NULL DEFAULT 0,
                   additions INTEGER NOT NULL DEFAULT 0,
                   deletions INTEGER NOT NULL DEFAULT 0,
                   FOREIGN KEY(repository_path) REFERENCES git_repositories(path) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS knowledge_items (
                   id TEXT PRIMARY KEY,
                   kind TEXT NOT NULL,
                   title TEXT NOT NULL,
                   content TEXT NOT NULL,
                   project TEXT,
                   source_type TEXT,
                   source_id TEXT,
                   tags TEXT NOT NULL DEFAULT '',
                   confirmed INTEGER NOT NULL DEFAULT 0,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS content_ideas (
                   id TEXT PRIMARY KEY,
                   idea_date TEXT NOT NULL,
                   content_type TEXT NOT NULL DEFAULT 'tech',
                   category TEXT NOT NULL,
                   title TEXT NOT NULL,
                   hook TEXT NOT NULL,
                   script TEXT NOT NULL,
                   storyboard TEXT NOT NULL,
                   visual_prompts TEXT NOT NULL,
                   editing_guide TEXT NOT NULL,
                   cover_title TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'candidate',
                   source TEXT NOT NULL DEFAULT 'local',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   UNIQUE(idea_date,title)
                 );
                 CREATE TABLE IF NOT EXISTS test_runs (
                   id TEXT PRIMARY KEY,
                   menu_id TEXT NOT NULL,
                   project TEXT NOT NULL,
                   menu_name TEXT NOT NULL,
                   mode TEXT NOT NULL,
                   status TEXT NOT NULL,
                   started_at TEXT NOT NULL,
                   finished_at TEXT,
                   report_markdown TEXT NOT NULL DEFAULT '',
                   source_report_path TEXT,
                   output_excerpt TEXT NOT NULL DEFAULT '',
                   error_message TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE IF NOT EXISTS work_sessions (
                   id TEXT PRIMARY KEY,
                   date TEXT NOT NULL,
                   start_time TEXT NOT NULL,
                   end_time TEXT NOT NULL,
                   duration_minutes INTEGER NOT NULL,
                   project TEXT NOT NULL,
                   work_type TEXT NOT NULL,
                   source TEXT NOT NULL CHECK(source IN ('estimated','manual')),
                   note TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );",
            )
            .map_err(|error| error.to_string())?;
        for migration in [
            "ALTER TABLE tasks ADD COLUMN start_date TEXT",
            "ALTER TABLE tasks ADD COLUMN end_date TEXT",
            "ALTER TABLE tasks ADD COLUMN progress INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE knowledge_items ADD COLUMN tags TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE conversations ADD COLUMN context_used_tokens INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE conversations ADD COLUMN context_window INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE conversations ADD COLUMN source_size INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE conversations ADD COLUMN source_modified_ns INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE conversations ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE conversations ADD COLUMN project_override TEXT",
            "ALTER TABLE git_repositories ADD COLUMN user_name TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE git_repositories ADD COLUMN user_email TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE git_commits ADD COLUMN author_name TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE git_commits ADD COLUMN author_email TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tasks ADD COLUMN source_id TEXT",
            "ALTER TABLE content_ideas ADD COLUMN content_type TEXT NOT NULL DEFAULT 'tech'",
        ] {
            let _ = connection.execute(migration, []);
        }
        connection
            .execute(
                "UPDATE tasks SET source='conversation' WHERE source='ai'",
                [],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO app_meta(key,value) VALUES('work_session_gap_minutes','45')
                 ON CONFLICT(key) DO NOTHING",
                [],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO app_meta(key,value) VALUES('schema_version',?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [SCHEMA_VERSION.to_string()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

pub fn ensure_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "数据库路径缺少父目录".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn database_health(state: tauri::State<'_, DatabaseState>) -> Result<DatabaseHealth, String> {
    Ok(DatabaseHealth {
        path: state.path.display().to_string(),
        schema_version: SCHEMA_VERSION,
    })
}

#[tauri::command]
pub fn list_tasks(state: tauri::State<'_, DatabaseState>) -> Result<Vec<WorkTask>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT id,title,project,scope,status,priority,planned_date,week_start,start_date,end_date,progress,note,source,source_id,created_at,updated_at,completed_at FROM tasks ORDER BY created_at DESC")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(WorkTask {
                id: row.get(0)?,
                title: row.get(1)?,
                project: row.get(2)?,
                scope: row.get(3)?,
                status: row.get(4)?,
                priority: row.get(5)?,
                planned_date: row.get(6)?,
                week_start: row.get(7)?,
                start_date: row.get(8)?,
                end_date: row.get(9)?,
                progress: row.get(10)?,
                note: row.get(11)?,
                source: row.get(12)?,
                source_id: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                completed_at: row.get(16)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_task(state: tauri::State<'_, DatabaseState>, task: WorkTask) -> Result<(), String> {
    let connection = state.connect()?;
    connection.execute(
        "INSERT INTO tasks(id,title,project,scope,status,priority,planned_date,week_start,start_date,end_date,progress,note,source,source_id,created_at,updated_at,completed_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title,project=excluded.project,scope=excluded.scope,status=excluded.status,priority=excluded.priority,planned_date=excluded.planned_date,week_start=excluded.week_start,start_date=excluded.start_date,end_date=excluded.end_date,progress=excluded.progress,note=excluded.note,source=excluded.source,source_id=excluded.source_id,updated_at=excluded.updated_at,completed_at=excluded.completed_at",
        params![task.id,task.title,task.project,task.scope,task.status,task.priority,task.planned_date,task.week_start,task.start_date,task.end_date,task.progress,task.note,task.source,task.source_id,task.created_at,task.updated_at,task.completed_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_task(state: tauri::State<'_, DatabaseState>, id: String) -> Result<(), String> {
    state
        .connect()?
        .execute("DELETE FROM tasks WHERE id=?1", [id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn token_summary(state: tauri::State<'_, DatabaseState>) -> Result<TokenSummary, String> {
    let connection = state.connect()?;
    connection.query_row(
        "SELECT COUNT(*),(SELECT COUNT(*) FROM conversation_messages),(SELECT COUNT(DISTINCT date(event_time,'localtime')) FROM token_events WHERE event_time IS NOT NULL),COALESCE(SUM(input_tokens),0),COALESCE(SUM(cached_input_tokens),0),COALESCE(SUM(output_tokens),0),COALESCE(SUM(reasoning_output_tokens),0),COALESCE(SUM(total_tokens),0) FROM conversations",
        [],
        |row| Ok(TokenSummary { conversation_count: row.get(0)?, message_count: row.get(1)?, active_days: row.get(2)?, input_tokens: row.get(3)?, cached_input_tokens: row.get(4)?, output_tokens: row.get(5)?, reasoning_output_tokens: row.get(6)?, total_tokens: row.get(7)? }),
    ).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_conversation_metrics(
    state: tauri::State<'_, DatabaseState>,
    limit: Option<i64>,
) -> Result<Vec<ConversationMetric>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT id,title,cwd,COALESCE(NULLIF(project_override,''),COALESCE(NULLIF(cwd,''),'未归类项目')),model,updated_at,input_tokens,cached_input_tokens,output_tokens,reasoning_output_tokens,total_tokens,context_used_tokens,context_window,archived FROM conversations ORDER BY total_tokens DESC LIMIT ?1"
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([limit.unwrap_or(50).clamp(1, 500)], |row| {
            Ok(ConversationMetric {
                id: row.get(0)?,
                title: row.get(1)?,
                cwd: row.get(2)?,
                project: row.get(3)?,
                model: row.get(4)?,
                updated_at: row.get(5)?,
                input_tokens: row.get(6)?,
                cached_input_tokens: row.get(7)?,
                output_tokens: row.get(8)?,
                reasoning_output_tokens: row.get(9)?,
                total_tokens: row.get(10)?,
                context_used_tokens: row.get(11)?,
                context_window: row.get(12)?,
                archived: row.get::<_, i64>(13)? != 0,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn set_conversation_project(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    project: Option<String>,
) -> Result<(), String> {
    let value = project
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    state
        .connect()?
        .execute(
            "UPDATE conversations SET project_override=?1 WHERE id=?2",
            params![value, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn token_trend(
    state: tauri::State<'_, DatabaseState>,
    days: Option<i64>,
) -> Result<Vec<TokenTrendPoint>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "WITH ordered AS (
           SELECT event_time,
             MAX(input_tokens-LAG(input_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS input_delta,
             MAX(cached_input_tokens-LAG(cached_input_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS cached_delta,
             MAX(output_tokens-LAG(output_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS output_delta,
             MAX(reasoning_output_tokens-LAG(reasoning_output_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS reasoning_delta,
             MAX(total_tokens-LAG(total_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS total_delta
           FROM token_events WHERE event_time IS NOT NULL
         )
         SELECT date(event_time,'localtime'),SUM(input_delta),SUM(cached_delta),SUM(output_delta),SUM(reasoning_delta),SUM(total_delta)
         FROM ordered GROUP BY date(event_time,'localtime') ORDER BY date(event_time,'localtime') DESC LIMIT ?1",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            [match days.unwrap_or(14) {
                value if value <= 0 => 3650,
                value => value.clamp(1, 3650),
            }],
            |row| {
                Ok(TokenTrendPoint {
                    date: row.get(0)?,
                    input_tokens: row.get(1)?,
                    cached_input_tokens: row.get(2)?,
                    output_tokens: row.get(3)?,
                    reasoning_output_tokens: row.get(4)?,
                    total_tokens: row.get(5)?,
                })
            },
        )
        .map_err(|error| error.to_string())?;
    let mut points = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    points.reverse();
    Ok(points)
}

#[tauri::command]
pub fn project_token_metrics(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<ProjectTokenMetric>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT COALESCE(NULLIF(project_override,''),COALESCE(NULLIF(cwd,''),'未归类项目')),COUNT(*),SUM(input_tokens),SUM(cached_input_tokens),SUM(output_tokens),SUM(reasoning_output_tokens),SUM(total_tokens)
         FROM conversations GROUP BY COALESCE(NULLIF(project_override,''),COALESCE(NULLIF(cwd,''),'未归类项目')) ORDER BY SUM(total_tokens) DESC LIMIT 50",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ProjectTokenMetric {
                project: row.get(0)?,
                conversation_count: row.get(1)?,
                input_tokens: row.get(2)?,
                cached_input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                reasoning_output_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn model_token_metrics(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<ModelTokenMetric>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT COALESCE(NULLIF(model,''),'未知模型'),COUNT(*),SUM(input_tokens),SUM(cached_input_tokens),SUM(output_tokens),SUM(reasoning_output_tokens),SUM(total_tokens)
         FROM conversations GROUP BY COALESCE(NULLIF(model,''),'未知模型') ORDER BY SUM(total_tokens) DESC",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ModelTokenMetric {
                model: row.get(0)?,
                conversation_count: row.get(1)?,
                input_tokens: row.get(2)?,
                cached_input_tokens: row.get(3)?,
                output_tokens: row.get(4)?,
                reasoning_output_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn search_workspace(
    state: tauri::State<'_, DatabaseState>,
    query: String,
) -> Result<Vec<WorkspaceSearchResult>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let connection = state.connect()?;
    let mut results = Vec::new();
    let mut collect = |sql: &str, kind: &str, route_prefix: &str| -> Result<(), String> {
        let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([&pattern], |row| {
                let id: String = row.get(0)?;
                Ok(WorkspaceSearchResult {
                    route: format!("{route_prefix}{id}"),
                    id,
                    kind: kind.to_string(),
                    title: row.get(1)?,
                    subtitle: row.get(2)?,
                    date: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        results.extend(
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    };
    collect(
        "SELECT id,title,project,substr(updated_at,1,10) FROM tasks WHERE title LIKE ?1 ESCAPE '\\' OR project LIKE ?1 ESCAPE '\\' OR note LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 15",
        "任务",
        "/tasks?task=",
    )?;
    collect(
        "SELECT id,COALESCE(NULLIF(title,''),'未命名会话'),COALESCE(NULLIF(project_override,''),COALESCE(NULLIF(cwd,''),'未归类项目')) || CASE WHEN archived=1 THEN ' · 归档' ELSE '' END,substr(COALESCE(updated_at,started_at),1,10) FROM conversations WHERE title LIKE ?1 ESCAPE '\\' OR cwd LIKE ?1 ESCAPE '\\' OR project_override LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 20",
        "Codex 对话",
        "/tokens?conversation=",
    )?;
    collect(
        "SELECT id,title,report_type || ' · ' || period_start,period_start FROM reports WHERE title LIKE ?1 ESCAPE '\\' OR content_markdown LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 15",
        "报告",
        "/reports?report=",
    )?;
    collect(
        "SELECT id,title,COALESCE(project,'未归类项目'),substr(updated_at,1,10) FROM knowledge_items WHERE title LIKE ?1 ESCAPE '\\' OR content LIKE ?1 ESCAPE '\\' OR tags LIKE ?1 ESCAPE '\\' ORDER BY updated_at DESC LIMIT 15",
        "知识",
        "/knowledge?item=",
    )?;
    collect(
        "SELECT id,title,category || ' · ' || CASE status WHEN 'selected' THEN '已选择' WHEN 'rejected' THEN '已淘汰' WHEN 'published' THEN '已发布' ELSE '候选' END,idea_date FROM content_ideas WHERE title LIKE ?1 ESCAPE '\\' OR hook LIKE ?1 ESCAPE '\\' OR script LIKE ?1 ESCAPE '\\' ORDER BY idea_date DESC,updated_at DESC LIMIT 15",
        "内容",
        "/content?idea=",
    )?;
    results.sort_by(|a, b| b.date.cmp(&a.date));
    results.truncate(50);
    Ok(results)
}
