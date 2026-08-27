use rusqlite::{params, Connection, MAIN_DB};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 37;

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

// TAPD 多项目下缺陷编号不能单独作为主键；升级时保留原编号并增加项目级复合键。
fn migrate_tapd_composite_keys(connection: &Connection) -> Result<(), String> {
    let has_item_key = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tapd_work_items') WHERE name='item_key')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| error.to_string())?;
    if !has_item_key {
        let migration = connection.execute_batch(
            "PRAGMA foreign_keys=OFF;
             BEGIN IMMEDIATE;
             CREATE TABLE tapd_work_items_v33 (
               item_key TEXT PRIMARY KEY,
               id TEXT NOT NULL,
               workspace_id TEXT NOT NULL,
               item_type TEXT NOT NULL,
               title TEXT NOT NULL,
               description TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL DEFAULT '',
               status_label TEXT NOT NULL DEFAULT '',
               priority TEXT NOT NULL DEFAULT '',
               owner TEXT NOT NULL DEFAULT '',
               creator TEXT NOT NULL DEFAULT '',
               iteration_id TEXT NOT NULL DEFAULT '',
               begin_date TEXT NOT NULL DEFAULT '',
               due_date TEXT NOT NULL DEFAULT '',
               created_at TEXT NOT NULL DEFAULT '',
               modified_at TEXT NOT NULL DEFAULT '',
               source_url TEXT NOT NULL DEFAULT '',
               synced_at TEXT NOT NULL,
               automation_version TEXT NOT NULL DEFAULT '',
               UNIQUE(workspace_id,id)
             );
             INSERT INTO tapd_work_items_v33(
               item_key,id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at,automation_version
             )
             SELECT workspace_id || ':' || id,id,workspace_id,item_type,title,description,status,status_label,priority,owner,creator,iteration_id,begin_date,due_date,created_at,modified_at,source_url,synced_at,
                    COALESCE(NULLIF(modified_at,''),NULLIF(created_at,''),synced_at)
             FROM tapd_work_items;
             CREATE TABLE tapd_codex_jobs_v33 (
               id TEXT PRIMARY KEY,
               item_key TEXT NOT NULL,
               item_id TEXT NOT NULL,
               workspace_id TEXT NOT NULL,
               repository_path TEXT NOT NULL,
               status TEXT NOT NULL DEFAULT 'running',
               thread_id TEXT,
               output TEXT NOT NULL DEFAULT '',
               error_message TEXT NOT NULL DEFAULT '',
               baseline_head TEXT NOT NULL DEFAULT '',
               baseline_worktree TEXT NOT NULL DEFAULT '',
               result_head TEXT NOT NULL DEFAULT '',
               changed_files TEXT NOT NULL DEFAULT '',
               test_summary TEXT NOT NULL DEFAULT '',
               review_status TEXT NOT NULL DEFAULT 'pending',
               review_note TEXT NOT NULL DEFAULT '',
               reviewed_at TEXT,
               trigger_source TEXT NOT NULL DEFAULT 'manual',
               source_modified_at TEXT NOT NULL DEFAULT '',
               trigger_reason TEXT NOT NULL DEFAULT '',
               execution_mode TEXT NOT NULL DEFAULT 'manual',
               execution_block_reason TEXT NOT NULL DEFAULT '',
               started_at TEXT,
               completed_at TEXT,
               test_required INTEGER NOT NULL DEFAULT 0,
               process_report_path TEXT NOT NULL DEFAULT '',
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               FOREIGN KEY(item_key) REFERENCES tapd_work_items_v33(item_key) ON DELETE CASCADE
             );
             INSERT INTO tapd_codex_jobs_v33(
               id,item_key,item_id,workspace_id,repository_path,status,thread_id,output,error_message,baseline_head,baseline_worktree,result_head,changed_files,test_summary,review_status,review_note,reviewed_at,trigger_source,source_modified_at,trigger_reason,execution_mode,execution_block_reason,started_at,completed_at,test_required,process_report_path,created_at,updated_at
             )
             SELECT j.id,i.workspace_id || ':' || i.id,i.id,i.workspace_id,j.repository_path,j.status,j.thread_id,j.output,j.error_message,j.baseline_head,j.baseline_worktree,j.result_head,j.changed_files,j.test_summary,j.review_status,j.review_note,j.reviewed_at,j.trigger_source,
                     '',
                    CASE WHEN j.trigger_source='auto' THEN '历史自动任务' ELSE '人工发送' END,
                    'manual','',
                    CASE WHEN j.status='queued' THEN NULL ELSE j.created_at END,
                    CASE WHEN j.status IN ('completed','failed') THEN j.updated_at ELSE NULL END,
                    CASE WHEN COALESCE((SELECT test_command FROM repository_assets r WHERE r.path=j.repository_path),'')='' THEN 0 ELSE 1 END,
                    j.process_report_path,j.created_at,j.updated_at
             FROM tapd_codex_jobs j
             JOIN tapd_work_items i ON i.id=j.item_id;
             DROP TABLE tapd_codex_jobs;
             DROP TABLE tapd_work_items;
             ALTER TABLE tapd_work_items_v33 RENAME TO tapd_work_items;
             ALTER TABLE tapd_codex_jobs_v33 RENAME TO tapd_codex_jobs;
             COMMIT;
             PRAGMA foreign_keys=ON;",
        );
        if let Err(error) = migration {
            let _ = connection.execute_batch("ROLLBACK; PRAGMA foreign_keys=ON;");
            return Err(format!("TAPD 多项目数据迁移失败：{error}"));
        }
    }
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_tapd_items_type_status ON tapd_work_items(item_type,status);
             CREATE INDEX IF NOT EXISTS idx_tapd_items_workspace ON tapd_work_items(workspace_id,modified_at DESC);
             CREATE INDEX IF NOT EXISTS idx_tapd_codex_jobs_item ON tapd_codex_jobs(item_key,created_at DESC);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_tapd_codex_jobs_auto_version
               ON tapd_codex_jobs(item_key,source_modified_at)
               WHERE trigger_source='auto' AND source_modified_at<>'';",
        )
        .map_err(|error| error.to_string())?;
    let foreign_key_errors = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| error.to_string())?;
    if foreign_key_errors != 0 {
        return Err(format!(
            "TAPD 数据迁移后发现 {foreign_key_errors} 条关联异常。"
        ));
    }
    Ok(())
}

impl DatabaseState {
    pub fn new(path: PathBuf) -> Result<Self, String> {
        let state = Self { path };
        state.backup_before_migration()?;
        state.initialize()?;
        Ok(state)
    }

    fn stored_schema_version(&self) -> Result<Option<i64>, String> {
        if !self.path.exists() {
            return Ok(None);
        }
        let metadata = std::fs::metadata(&self.path).map_err(|error| error.to_string())?;
        if metadata.len() == 0 {
            return Ok(None);
        }
        let connection = self.connect()?;
        let has_meta = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='app_meta')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if !has_meta {
            return Ok(Some(0));
        }
        let stored = connection
            .query_row(
                "SELECT value FROM app_meta WHERE key='schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0);
        Ok(Some(stored))
    }

    fn backup_before_migration(&self) -> Result<Option<PathBuf>, String> {
        let Some(stored_version) = self.stored_schema_version()? else {
            return Ok(None);
        };
        if stored_version >= SCHEMA_VERSION {
            return Ok(None);
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "数据库路径缺少父目录".to_string())?;
        let backup_dir = parent.join("backups");
        std::fs::create_dir_all(&backup_dir).map_err(|error| error.to_string())?;
        let database_name = self
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("workbench");
        let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f");
        let backup_path = backup_dir.join(format!(
            "{database_name}-before-v{SCHEMA_VERSION}-{timestamp}.sqlite3"
        ));
        let connection = self.connect()?;
        connection
            .backup(MAIN_DB, &backup_path, None)
            .map_err(|error| format!("迁移前备份失败：{error}"))?;
        let backup = Connection::open(&backup_path).map_err(|error| error.to_string())?;
        let integrity = backup
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        if integrity != "ok" {
            return Err(format!("迁移前备份完整性检查失败：{integrity}"));
        }
        Ok(Some(backup_path))
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
                   project_path TEXT NOT NULL DEFAULT '',
                   menu_name TEXT NOT NULL,
                   mode TEXT NOT NULL,
                   status TEXT NOT NULL,
                   started_at TEXT NOT NULL,
                   finished_at TEXT,
                   report_markdown TEXT NOT NULL DEFAULT '',
                   source_report_path TEXT,
                   output_excerpt TEXT NOT NULL DEFAULT '',
                   error_message TEXT NOT NULL DEFAULT '',
                   selected_scenarios TEXT NOT NULL DEFAULT '[]',
                   scenario_results TEXT NOT NULL DEFAULT '[]',
                   artifacts TEXT NOT NULL DEFAULT '[]',
                   total_count INTEGER NOT NULL DEFAULT 0,
                   passed_count INTEGER NOT NULL DEFAULT 0,
                   failed_count INTEGER NOT NULL DEFAULT 0,
                   skipped_count INTEGER NOT NULL DEFAULT 0,
                   duration_ms INTEGER NOT NULL DEFAULT 0,
                   exit_code INTEGER,
                   environment_summary TEXT NOT NULL DEFAULT '',
                   cleanup_status TEXT NOT NULL DEFAULT 'not-applicable'
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
                 );
                 CREATE TABLE IF NOT EXISTS repository_assets (
                   path TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   is_pinned INTEGER NOT NULL DEFAULT 0,
                   is_hidden INTEGER NOT NULL DEFAULT 0,
                   category TEXT NOT NULL DEFAULT '待确认',
                   purpose TEXT NOT NULL DEFAULT '',
                   technology_stack TEXT NOT NULL DEFAULT '',
                   main_modules TEXT NOT NULL DEFAULT '',
                   install_command TEXT NOT NULL DEFAULT '',
                   start_command TEXT NOT NULL DEFAULT '',
                   test_command TEXT NOT NULL DEFAULT '',
                   build_command TEXT NOT NULL DEFAULT '',
                   command_source TEXT NOT NULL DEFAULT '',
                   remote_url TEXT NOT NULL DEFAULT '',
                   default_branch TEXT NOT NULL DEFAULT '',
                   has_uncommitted_changes INTEGER NOT NULL DEFAULT 0,
                   changed_file_count INTEGER NOT NULL DEFAULT 0,
                   ahead_count INTEGER NOT NULL DEFAULT 0,
                   behind_count INTEGER NOT NULL DEFAULT 0,
                   inference_status TEXT NOT NULL DEFAULT 'pending',
                   manually_confirmed INTEGER NOT NULL DEFAULT 0,
                   last_scanned_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_repository_assets_category ON repository_assets(category);
                 CREATE TABLE IF NOT EXISTS repository_health_snapshots (
                   id TEXT PRIMARY KEY,
                   repository_path TEXT NOT NULL,
                   health_level TEXT NOT NULL,
                   has_uncommitted_changes INTEGER NOT NULL DEFAULT 0,
                   summary TEXT NOT NULL DEFAULT '',
                   failure_reason TEXT NOT NULL DEFAULT '',
                   verified_at TEXT NOT NULL,
                   FOREIGN KEY(repository_path) REFERENCES repository_assets(path) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_repository_health_path_time ON repository_health_snapshots(repository_path,verified_at);
                 CREATE TABLE IF NOT EXISTS repository_runtime_runs (
                   id TEXT PRIMARY KEY,
                   repository_path TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'starting',
                   command TEXT NOT NULL DEFAULT '',
                   process_id INTEGER NOT NULL DEFAULT 0,
                   local_url TEXT NOT NULL DEFAULT '',
                   log_path TEXT NOT NULL DEFAULT '',
                   log_excerpt TEXT NOT NULL DEFAULT '',
                   error_message TEXT NOT NULL DEFAULT '',
                   started_at TEXT NOT NULL,
                   finished_at TEXT,
                   exit_code INTEGER,
                   FOREIGN KEY(repository_path) REFERENCES repository_assets(path) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_repository_runtime_path_time ON repository_runtime_runs(repository_path,started_at DESC);
                 CREATE TABLE IF NOT EXISTS commit_plans (
                   id TEXT PRIMARY KEY,
                   repository_path TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'draft',
                   risk_level TEXT NOT NULL DEFAULT 'unverified',
                   summary TEXT NOT NULL DEFAULT '',
                   grouping_mode TEXT NOT NULL DEFAULT 'single',
                   generator TEXT NOT NULL DEFAULT 'rules',
                   model TEXT NOT NULL DEFAULT '',
                   generation_warning TEXT NOT NULL DEFAULT '',
                   excluded_files_json TEXT NOT NULL DEFAULT '[]',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS commit_groups (
                   id TEXT PRIMARY KEY,
                   plan_id TEXT NOT NULL,
                   group_order INTEGER NOT NULL DEFAULT 0,
                   title TEXT NOT NULL,
                   commit_message TEXT NOT NULL,
                   files_json TEXT NOT NULL DEFAULT '[]',
                   risk_notes TEXT NOT NULL DEFAULT '',
                   verification_notes TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'suggested',
                   commit_hash TEXT,
                   confirmed_at TEXT,
                   committed_at TEXT,
                   FOREIGN KEY(plan_id) REFERENCES commit_plans(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_commit_groups_plan ON commit_groups(plan_id,group_order);
                 CREATE TABLE IF NOT EXISTS feature_parities (
                   id TEXT PRIMARY KEY,
                   domain TEXT NOT NULL,
                   feature_name TEXT NOT NULL,
                   pc_page TEXT NOT NULL DEFAULT '',
                   app_page TEXT NOT NULL DEFAULT '',
                   parity_status TEXT NOT NULL DEFAULT 'pending',
                   evidence_json TEXT NOT NULL DEFAULT '[]',
                   intentional_difference INTEGER NOT NULL DEFAULT 0,
                   manually_confirmed INTEGER NOT NULL DEFAULT 0,
                   updated_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_feature_parities_domain ON feature_parities(domain,parity_status);
                 CREATE TABLE IF NOT EXISTS api_contracts (
                   id TEXT PRIMARY KEY,
                   feature_id TEXT,
                   platform TEXT NOT NULL,
                   method TEXT NOT NULL,
                   url TEXT NOT NULL,
                   parameters_json TEXT NOT NULL DEFAULT '{}',
                   response_fields_json TEXT NOT NULL DEFAULT '[]',
                   source_file TEXT NOT NULL DEFAULT '',
                   verification_level TEXT NOT NULL DEFAULT 'static',
                   updated_at TEXT NOT NULL,
                   FOREIGN KEY(feature_id) REFERENCES feature_parities(id) ON DELETE SET NULL
                 );
                 CREATE TABLE IF NOT EXISTS regression_cases (
                   id TEXT PRIMARY KEY,
                   feature_id TEXT,
                   platform TEXT NOT NULL,
                   verification_type TEXT NOT NULL,
                   case_name TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'unverified',
                   result_summary TEXT NOT NULL DEFAULT '',
                   source_path TEXT NOT NULL DEFAULT '',
                   verified_at TEXT,
                   FOREIGN KEY(feature_id) REFERENCES feature_parities(id) ON DELETE SET NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_regression_cases_feature ON regression_cases(feature_id,platform);
                 CREATE TABLE IF NOT EXISTS video_jobs (
                   id TEXT PRIMARY KEY,
                   title TEXT NOT NULL,
                   video_type TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'draft',
                   current_stage TEXT NOT NULL DEFAULT 'selection',
                   project_root TEXT NOT NULL DEFAULT '',
                   failure_reason TEXT NOT NULL DEFAULT '',
                   manually_confirmed_type INTEGER NOT NULL DEFAULT 0,
                   content_idea_id TEXT,
                   skill_name TEXT NOT NULL DEFAULT '',
                   codex_thread_id TEXT,
                   codex_output TEXT NOT NULL DEFAULT '',
                   cli_log_path TEXT NOT NULL DEFAULT '',
                   progress_percent INTEGER NOT NULL DEFAULT 0,
                   progress_message TEXT NOT NULL DEFAULT '',
                   last_progress_at TEXT,
                   started_at TEXT,
                   completed_at TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS video_deliverables (
                   id TEXT PRIMARY KEY,
                   video_job_id TEXT NOT NULL,
                   kind TEXT NOT NULL,
                   path TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'missing',
                   quality_summary TEXT NOT NULL DEFAULT '',
                   checked_at TEXT,
                   FOREIGN KEY(video_job_id) REFERENCES video_jobs(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_video_deliverables_job ON video_deliverables(video_job_id,kind);
                 CREATE TABLE IF NOT EXISTS video_publish_records (
                   id TEXT PRIMARY KEY,
                   video_job_id TEXT NOT NULL,
                   platform TEXT NOT NULL DEFAULT '抖音',
                   status TEXT NOT NULL DEFAULT 'ready',
                   publish_url TEXT NOT NULL DEFAULT '',
                   published_at TEXT,
                   views INTEGER NOT NULL DEFAULT 0,
                   likes INTEGER NOT NULL DEFAULT 0,
                   comments INTEGER NOT NULL DEFAULT 0,
                   favorites INTEGER NOT NULL DEFAULT 0,
                   notes TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   FOREIGN KEY(video_job_id) REFERENCES video_jobs(id) ON DELETE CASCADE,
                   UNIQUE(video_job_id,platform)
                 );
                 CREATE INDEX IF NOT EXISTS idx_video_publish_records_status ON video_publish_records(status,updated_at DESC);
                 CREATE TABLE IF NOT EXISTS toolchain_installations (
                   id TEXT PRIMARY KEY,
                   tool_name TEXT NOT NULL,
                   version TEXT NOT NULL DEFAULT '',
                   executable_path TEXT NOT NULL DEFAULT '',
                   source TEXT NOT NULL DEFAULT '',
                   path_priority INTEGER,
                   scanned_at TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_toolchain_installations_name ON toolchain_installations(tool_name);
                 CREATE TABLE IF NOT EXISTS toolchain_conflicts (
                   id TEXT PRIMARY KEY,
                   tool_name TEXT NOT NULL,
                   conflict_type TEXT NOT NULL,
                   summary TEXT NOT NULL,
                   recommended_action TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'unconfirmed',
                   detected_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS weekly_audits (
                   id TEXT PRIMARY KEY,
                   week_start TEXT NOT NULL UNIQUE,
                   status TEXT NOT NULL DEFAULT 'pending',
                   scheduled_at TEXT NOT NULL,
                   started_at TEXT,
                   finished_at TEXT,
                   summary TEXT NOT NULL DEFAULT '',
                   catch_up_run INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE TABLE IF NOT EXISTS audit_checks (
                   id TEXT PRIMARY KEY,
                   audit_id TEXT NOT NULL,
                   check_type TEXT NOT NULL,
                   target TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'unverified',
                   summary TEXT NOT NULL DEFAULT '',
                   details_json TEXT NOT NULL DEFAULT '{}',
                   checked_at TEXT,
                   FOREIGN KEY(audit_id) REFERENCES weekly_audits(id) ON DELETE CASCADE
                 );
                 CREATE TABLE IF NOT EXISTS notifications (
                   id TEXT PRIMARY KEY,
                   kind TEXT NOT NULL,
                   title TEXT NOT NULL,
                   body TEXT NOT NULL DEFAULT '',
                   output TEXT NOT NULL DEFAULT '',
                   source_id TEXT,
                   route TEXT NOT NULL DEFAULT '/',
                   is_read INTEGER NOT NULL DEFAULT 0,
                   review_status TEXT NOT NULL DEFAULT 'pending',
                   review_note TEXT NOT NULL DEFAULT '',
                   reviewed_at TEXT,
                   created_at TEXT NOT NULL,
                   read_at TEXT
                 );
                 CREATE INDEX IF NOT EXISTS idx_notifications_unread ON notifications(is_read,created_at);
                 CREATE TABLE IF NOT EXISTS project_profiles (
                   id TEXT PRIMARY KEY,
                   display_name TEXT NOT NULL,
                   repository_path TEXT NOT NULL DEFAULT '',
                   tapd_workspace_id TEXT NOT NULL DEFAULT '',
                   aliases_json TEXT NOT NULL DEFAULT '[]',
                   category TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE UNIQUE INDEX IF NOT EXISTS idx_project_profiles_repository ON project_profiles(repository_path) WHERE repository_path<>'';
                 CREATE INDEX IF NOT EXISTS idx_project_profiles_name ON project_profiles(display_name);
                 CREATE TABLE IF NOT EXISTS work_inbox_items (
                   id TEXT PRIMARY KEY,
                   source_type TEXT NOT NULL,
                   source_id TEXT NOT NULL,
                   project TEXT NOT NULL DEFAULT '未归类项目',
                   title TEXT NOT NULL,
                   summary TEXT NOT NULL DEFAULT '',
                   detail TEXT NOT NULL DEFAULT '',
                   route TEXT NOT NULL DEFAULT '/',
                   priority TEXT NOT NULL DEFAULT 'normal',
                   workflow_status TEXT NOT NULL DEFAULT 'needs_decision',
                   source_status TEXT NOT NULL DEFAULT '',
                   source_revision TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   UNIQUE(source_type,source_id)
                 );
                 CREATE INDEX IF NOT EXISTS idx_work_inbox_status ON work_inbox_items(workflow_status,priority,updated_at);
                 CREATE INDEX IF NOT EXISTS idx_work_inbox_source ON work_inbox_items(source_type,source_id);
                 CREATE TABLE IF NOT EXISTS quick_captures (
                   id TEXT PRIMARY KEY,
                   kind TEXT NOT NULL DEFAULT 'note',
                   content TEXT NOT NULL,
                   source_url TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'inbox',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS knowledge_versions (
                   id TEXT PRIMARY KEY,
                   knowledge_id TEXT NOT NULL,
                   version_number INTEGER NOT NULL,
                   title TEXT NOT NULL,
                   content TEXT NOT NULL,
                   tags TEXT NOT NULL DEFAULT '',
                   change_source TEXT NOT NULL DEFAULT 'manual',
                   created_at TEXT NOT NULL,
                   FOREIGN KEY(knowledge_id) REFERENCES knowledge_items(id) ON DELETE CASCADE,
                   UNIQUE(knowledge_id,version_number)
                 );
                 CREATE INDEX IF NOT EXISTS idx_knowledge_versions_item ON knowledge_versions(knowledge_id,version_number DESC);
                 CREATE TABLE IF NOT EXISTS knowledge_codex_jobs (
                   id TEXT PRIMARY KEY,
                   knowledge_id TEXT NOT NULL,
                   repository_path TEXT NOT NULL,
                   instruction TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT 'running',
                   thread_id TEXT,
                   output TEXT NOT NULL DEFAULT '',
                   error_message TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   FOREIGN KEY(knowledge_id) REFERENCES knowledge_items(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_knowledge_codex_jobs_item ON knowledge_codex_jobs(knowledge_id,created_at DESC);
                 CREATE INDEX IF NOT EXISTS idx_quick_captures_status ON quick_captures(status,created_at);
                 CREATE TABLE IF NOT EXISTS daily_checkins (
                   date TEXT PRIMARY KEY,
                   energy INTEGER,
                   mood TEXT NOT NULL DEFAULT '',
                   exercise_minutes INTEGER NOT NULL DEFAULT 0,
                   note TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS email_deliveries (
                   notification_id TEXT PRIMARY KEY,
                   status TEXT NOT NULL,
                   attempts INTEGER NOT NULL DEFAULT 0,
                   next_attempt_at TEXT,
                   sent_at TEXT,
                   last_error TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   FOREIGN KEY(notification_id) REFERENCES notifications(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_email_deliveries_due ON email_deliveries(status,next_attempt_at);
                  CREATE TABLE IF NOT EXISTS tapd_projects (
                    workspace_id TEXT PRIMARY KEY,
                    workspace_name TEXT NOT NULL,
                    owner TEXT NOT NULL DEFAULT '',
                    enabled INTEGER NOT NULL DEFAULT 1,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    repository_path TEXT NOT NULL DEFAULT '',
                    auto_enabled INTEGER NOT NULL DEFAULT 0,
                    auto_execute INTEGER NOT NULL DEFAULT 1,
                    trigger_statuses TEXT NOT NULL DEFAULT 'new,reopened',
                    completion_status TEXT NOT NULL DEFAULT '已解决',
                    last_synced_at TEXT,
                    last_error TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                  );
                  CREATE INDEX IF NOT EXISTS idx_tapd_projects_enabled ON tapd_projects(enabled,updated_at DESC);
                  CREATE TABLE IF NOT EXISTS tapd_work_items (
                    item_key TEXT PRIMARY KEY,
                    id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    item_type TEXT NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT '',
                    status_label TEXT NOT NULL DEFAULT '',
                    priority TEXT NOT NULL DEFAULT '',
                    owner TEXT NOT NULL DEFAULT '',
                    creator TEXT NOT NULL DEFAULT '',
                    iteration_id TEXT NOT NULL DEFAULT '',
                    begin_date TEXT NOT NULL DEFAULT '',
                    due_date TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL DEFAULT '',
                    modified_at TEXT NOT NULL DEFAULT '',
                    source_url TEXT NOT NULL DEFAULT '',
                    synced_at TEXT NOT NULL,
                    automation_version TEXT NOT NULL DEFAULT '',
                    UNIQUE(workspace_id,id)
                  );
                  CREATE INDEX IF NOT EXISTS idx_tapd_items_type_status ON tapd_work_items(item_type,status);
                  CREATE TABLE IF NOT EXISTS tapd_codex_jobs (
                    id TEXT PRIMARY KEY,
                    item_key TEXT NOT NULL,
                    item_id TEXT NOT NULL,
                    workspace_id TEXT NOT NULL,
                    repository_path TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'running',
                    thread_id TEXT,
                    output TEXT NOT NULL DEFAULT '',
                    error_message TEXT NOT NULL DEFAULT '',
                    baseline_head TEXT NOT NULL DEFAULT '',
                    baseline_worktree TEXT NOT NULL DEFAULT '',
                    result_head TEXT NOT NULL DEFAULT '',
                    changed_files TEXT NOT NULL DEFAULT '',
                    test_summary TEXT NOT NULL DEFAULT '',
                    review_status TEXT NOT NULL DEFAULT 'pending',
                    review_note TEXT NOT NULL DEFAULT '',
                    reviewed_at TEXT,
                    trigger_source TEXT NOT NULL DEFAULT 'manual',
                    source_modified_at TEXT NOT NULL DEFAULT '',
                    trigger_reason TEXT NOT NULL DEFAULT '',
                    execution_mode TEXT NOT NULL DEFAULT 'manual',
                    execution_block_reason TEXT NOT NULL DEFAULT '',
                    started_at TEXT,
                    completed_at TEXT,
                    test_required INTEGER NOT NULL DEFAULT 0,
                    process_report_path TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY(item_key) REFERENCES tapd_work_items(item_key) ON DELETE CASCADE
                  );
                  CREATE INDEX IF NOT EXISTS idx_tapd_codex_jobs_item ON tapd_codex_jobs(item_key,created_at);",
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
            "ALTER TABLE notifications ADD COLUMN output TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE video_jobs ADD COLUMN content_idea_id TEXT",
            "ALTER TABLE video_jobs ADD COLUMN skill_name TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE video_jobs ADD COLUMN codex_thread_id TEXT",
            "ALTER TABLE video_jobs ADD COLUMN codex_output TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE video_jobs ADD COLUMN cli_log_path TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE video_jobs ADD COLUMN started_at TEXT",
            "ALTER TABLE video_jobs ADD COLUMN completed_at TEXT",
            "ALTER TABLE video_jobs ADD COLUMN progress_percent INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE video_jobs ADD COLUMN progress_message TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE video_jobs ADD COLUMN last_progress_at TEXT",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN baseline_head TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN baseline_worktree TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN result_head TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN changed_files TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN test_summary TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN review_status TEXT NOT NULL DEFAULT 'pending'",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN review_note TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN reviewed_at TEXT",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN trigger_source TEXT NOT NULL DEFAULT 'manual'",
            "ALTER TABLE tapd_codex_jobs ADD COLUMN process_report_path TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE notifications ADD COLUMN review_status TEXT NOT NULL DEFAULT 'pending'",
            "ALTER TABLE notifications ADD COLUMN review_note TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE notifications ADD COLUMN reviewed_at TEXT",
            "ALTER TABLE repository_assets ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE repository_assets ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE repository_assets ADD COLUMN changed_file_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE repository_assets ADD COLUMN ahead_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE repository_assets ADD COLUMN behind_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE tapd_projects ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE tapd_projects ADD COLUMN completion_status TEXT NOT NULL DEFAULT '已解决'",
            "ALTER TABLE test_runs ADD COLUMN project_path TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE test_runs ADD COLUMN selected_scenarios TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE test_runs ADD COLUMN scenario_results TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE test_runs ADD COLUMN artifacts TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE test_runs ADD COLUMN total_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE test_runs ADD COLUMN passed_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE test_runs ADD COLUMN failed_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE test_runs ADD COLUMN skipped_count INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE test_runs ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE test_runs ADD COLUMN exit_code INTEGER",
            "ALTER TABLE test_runs ADD COLUMN environment_summary TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE test_runs ADD COLUMN cleanup_status TEXT NOT NULL DEFAULT 'not-applicable'",
            "ALTER TABLE commit_plans ADD COLUMN grouping_mode TEXT NOT NULL DEFAULT 'single'",
            "ALTER TABLE commit_plans ADD COLUMN generator TEXT NOT NULL DEFAULT 'rules'",
            "ALTER TABLE commit_plans ADD COLUMN model TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE commit_plans ADD COLUMN generation_warning TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE commit_plans ADD COLUMN excluded_files_json TEXT NOT NULL DEFAULT '[]'",
        ] {
            let _ = connection.execute(migration, []);
        }
        connection
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_test_runs_project_menu ON test_runs(project_path,menu_id,started_at DESC)",
                [],
            )
            .map_err(|error| error.to_string())?;
        migrate_tapd_composite_keys(&connection)?;
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
                "INSERT INTO app_meta(key,value) VALUES('codex_notifications_started_at',?1)
                 ON CONFLICT(key) DO NOTHING",
                [chrono::Utc::now().to_rfc3339()],
            )
            .map_err(|error| error.to_string())?;
        for (key, value) in [
            ("codex_email_enabled", "0"),
            ("codex_email_enabled_at", ""),
            ("codex_email_config_status", "unconfigured"),
            ("codex_email_last_error", ""),
            ("tapd_automation_paused", "0"),
        ] {
            connection
                .execute(
                    "INSERT INTO app_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO NOTHING",
                    [key, value],
                )
                .map_err(|error| error.to_string())?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO tapd_projects(workspace_id,workspace_name,owner,enabled,sort_order,repository_path,auto_enabled,auto_execute,trigger_statuses,completion_status,last_synced_at,last_error,created_at,updated_at)
                 VALUES('37583308','安全生产管理',
                   COALESCE(NULLIF((SELECT value FROM app_meta WHERE key='tapd_owner'),''),'刘子世康'),
                   1,0,
                   COALESCE((SELECT value FROM app_meta WHERE key='tapd_auto_fix_repository_path'),''),
                   CASE WHEN LOWER(COALESCE((SELECT value FROM app_meta WHERE key='tapd_auto_fix_enabled'),'')) IN ('true','1','yes','on') THEN 1 ELSE 0 END,
                   1,'new,reopened','已解决',
                   (SELECT value FROM app_meta WHERE key='tapd_last_synced_at'),'',?1,?1)
                 ON CONFLICT(workspace_id) DO NOTHING",
                [&now],
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
    crate::project_identity::sync_project_profiles_for_state(&state)?;
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
    let raw = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    let mut grouped = BTreeMap::<String, ProjectTokenMetric>::new();
    for item in raw {
        let project = crate::project_identity::canonical_project_name(
            &connection,
            &item.project,
            &item.project,
        );
        let entry = grouped
            .entry(project.clone())
            .or_insert(ProjectTokenMetric {
                project,
                conversation_count: 0,
                input_tokens: 0,
                cached_input_tokens: 0,
                output_tokens: 0,
                reasoning_output_tokens: 0,
                total_tokens: 0,
            });
        entry.conversation_count += item.conversation_count;
        entry.input_tokens += item.input_tokens;
        entry.cached_input_tokens += item.cached_input_tokens;
        entry.output_tokens += item.output_tokens;
        entry.reasoning_output_tokens += item.reasoning_output_tokens;
        entry.total_tokens += item.total_tokens;
    }
    let mut values = grouped.into_values().collect::<Vec<_>>();
    values.sort_by_key(|item| std::cmp::Reverse(item.total_tokens));
    Ok(values)
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

#[cfg(test)]
mod migration_tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("workbench-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn backup_files(directory: &Path) -> Vec<PathBuf> {
        let backup_directory = directory.join("backups");
        if !backup_directory.exists() {
            return Vec::new();
        }
        std::fs::read_dir(backup_directory)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("sqlite3"))
            .collect()
    }

    #[test]
    fn backs_up_existing_database_before_additive_migration() {
        let directory = test_directory("migration-backup");
        std::fs::create_dir_all(&directory).unwrap();
        let database_path = directory.join("workbench.sqlite3");
        let connection = Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO app_meta(key,value) VALUES('schema_version','15');
                 CREATE TABLE preserved_data (value TEXT NOT NULL);
                 INSERT INTO preserved_data(value) VALUES('keep-me');
                 CREATE TABLE test_runs (
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
                 );",
            )
            .unwrap();
        drop(connection);

        let state = DatabaseState::new(database_path.clone()).unwrap();
        let upgraded = state.connect().unwrap();
        let version: String = upgraded
            .query_row(
                "SELECT value FROM app_meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
        let repository_assets_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='repository_assets')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(repository_assets_exists);
        let repository_pin_column_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('repository_assets') WHERE name='is_pinned')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(repository_pin_column_exists);
        let repository_hidden_column_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('repository_assets') WHERE name='is_hidden')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(repository_hidden_column_exists);
        for column in ["changed_file_count", "ahead_count", "behind_count"] {
            let exists: bool = upgraded
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('repository_assets') WHERE name=?1)",
                    [column],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "repository_assets 缺少 {column}");
        }
        let repository_runtime_runs_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='repository_runtime_runs')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(repository_runtime_runs_exists);
        for column in [
            "grouping_mode",
            "generator",
            "model",
            "generation_warning",
            "excluded_files_json",
        ] {
            let exists: bool = upgraded
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('commit_plans') WHERE name=?1)",
                    [column],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "commit_plans 缺少 {column}");
        }
        let tapd_sort_column_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tapd_projects') WHERE name='sort_order')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(tapd_sort_column_exists);
        let email_deliveries_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='email_deliveries')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(email_deliveries_exists);
        let test_project_path_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('test_runs') WHERE name='project_path')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(test_project_path_exists);
        let test_project_index_exists: bool = upgraded
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name='idx_test_runs_project_menu')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(test_project_index_exists);
        upgraded
            .execute(
                "INSERT INTO notifications(id,kind,title,created_at) VALUES('codex-complete-test','codex_complete','测试完成','2026-08-07T10:00:00Z')",
                [],
            )
            .unwrap();
        let inserted = upgraded
            .execute(
                "INSERT OR IGNORE INTO email_deliveries(notification_id,status,created_at,updated_at) VALUES('codex-complete-test','pending','2026-08-07T10:00:00Z','2026-08-07T10:00:00Z')",
                [],
            )
            .unwrap();
        let duplicate = upgraded
            .execute(
                "INSERT OR IGNORE INTO email_deliveries(notification_id,status,created_at,updated_at) VALUES('codex-complete-test','pending','2026-08-07T10:00:01Z','2026-08-07T10:00:01Z')",
                [],
            )
            .unwrap();
        assert_eq!(inserted, 1);
        assert_eq!(duplicate, 0);
        drop(upgraded);

        let backups = backup_files(&directory);
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).unwrap();
        let backup_version: String = backup
            .query_row(
                "SELECT value FROM app_meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let preserved: String = backup
            .query_row("SELECT value FROM preserved_data", [], |row| row.get(0))
            .unwrap();
        assert_eq!(backup_version, "15");
        assert_eq!(preserved, "keep-me");
        drop(backup);

        DatabaseState::new(database_path).unwrap();
        assert_eq!(backup_files(&directory).len(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn tapd_composite_key_migration_preserves_history_and_allows_same_id_per_project() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON;
                 CREATE TABLE repository_assets (
                   path TEXT PRIMARY KEY,
                   test_command TEXT NOT NULL DEFAULT ''
                 );
                 INSERT INTO repository_assets(path,test_command) VALUES('F:/client','npm test');
                 CREATE TABLE tapd_work_items (
                   id TEXT PRIMARY KEY,
                   workspace_id TEXT NOT NULL,
                   item_type TEXT NOT NULL,
                   title TEXT NOT NULL,
                   description TEXT NOT NULL DEFAULT '',
                   status TEXT NOT NULL DEFAULT '',
                   status_label TEXT NOT NULL DEFAULT '',
                   priority TEXT NOT NULL DEFAULT '',
                   owner TEXT NOT NULL DEFAULT '',
                   creator TEXT NOT NULL DEFAULT '',
                   iteration_id TEXT NOT NULL DEFAULT '',
                   begin_date TEXT NOT NULL DEFAULT '',
                   due_date TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL DEFAULT '',
                   modified_at TEXT NOT NULL DEFAULT '',
                   source_url TEXT NOT NULL DEFAULT '',
                   synced_at TEXT NOT NULL
                 );
                 INSERT INTO tapd_work_items VALUES(
                   '1001','63985424','bug','旧缺陷','','new','待处理','高','张三','','','','',
                   '2026-08-01T08:00:00Z','2026-08-02T08:00:00Z','https://tapd.cn','2026-08-02T08:01:00Z'
                 );
                 CREATE TABLE tapd_codex_jobs (
                   id TEXT PRIMARY KEY,
                   item_id TEXT NOT NULL,
                   repository_path TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'running',
                   thread_id TEXT,
                   output TEXT NOT NULL DEFAULT '',
                   error_message TEXT NOT NULL DEFAULT '',
                   baseline_head TEXT NOT NULL DEFAULT '',
                   baseline_worktree TEXT NOT NULL DEFAULT '',
                   result_head TEXT NOT NULL DEFAULT '',
                   changed_files TEXT NOT NULL DEFAULT '',
                   test_summary TEXT NOT NULL DEFAULT '',
                   review_status TEXT NOT NULL DEFAULT 'pending',
                   review_note TEXT NOT NULL DEFAULT '',
                   reviewed_at TEXT,
                   trigger_source TEXT NOT NULL DEFAULT 'manual',
                   process_report_path TEXT NOT NULL DEFAULT '',
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 INSERT INTO tapd_codex_jobs VALUES(
                   'job-1','1001','F:/client','completed',NULL,'','', '', '', '', '',
                   '项目测试通过','accepted','',NULL,'auto','',
                   '2026-08-02T09:00:00Z','2026-08-02T09:10:00Z'
                 );
                 INSERT INTO tapd_codex_jobs VALUES(
                   'job-2','1001','F:/client','completed',NULL,'','', '', '', '', '',
                   '项目测试通过','accepted','',NULL,'auto','',
                   '2026-08-03T09:00:00Z','2026-08-03T09:10:00Z'
                 );",
            )
            .unwrap();

        migrate_tapd_composite_keys(&connection).unwrap();

        let item_key: String = connection
            .query_row("SELECT item_key FROM tapd_work_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(item_key, "63985424:1001");
        let jobs: (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*),SUM(CASE WHEN source_modified_at='' THEN 1 ELSE 0 END) FROM tapd_codex_jobs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(jobs, (2, 2));
        connection
            .execute_batch(
                "INSERT INTO tapd_work_items(
                   item_key,id,workspace_id,item_type,title,synced_at,automation_version
                 ) VALUES(
                   '99887766:1001','1001','99887766','bug','另一项目的同号缺陷','2026-08-04T08:00:00Z',''
                 );",
            )
            .unwrap();
        let same_raw_id_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM tapd_work_items WHERE id='1001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(same_raw_id_count, 2);
        let foreign_key_errors: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_errors, 0);
    }
}
