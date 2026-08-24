use crate::{database::DatabaseState, project_identity};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

const DEFAULT_GAP_MINUTES: i64 = 45;
const SINGLE_EVENT_MINUTES: i64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSession {
    pub id: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i64,
    pub project: String,
    pub work_type: String,
    pub source: String,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkBreakdown {
    pub name: String,
    pub minutes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyWorkMinutes {
    pub date: String,
    pub minutes: i64,
    pub estimated_minutes: i64,
    pub manual_minutes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSummary {
    pub start_date: String,
    pub end_date: String,
    pub total_minutes: i64,
    pub estimated_minutes: i64,
    pub manual_minutes: i64,
    pub has_manual_corrections: bool,
    pub by_project: Vec<WorkBreakdown>,
    pub by_type: Vec<WorkBreakdown>,
    pub daily: Vec<DailyWorkMinutes>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTimeSettings {
    pub gap_minutes: i64,
}

#[derive(Debug)]
struct ActivityEvent {
    time: NaiveDateTime,
    project: String,
    work_type: String,
    kind: String,
}

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|error| error.to_string())
}

fn parse_local_event(value: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").ok()
}

fn time_text(value: NaiveDateTime) -> String {
    format!("{:02}:{:02}", value.hour(), value.minute())
}

fn work_type_for(kind: &str, text: &str) -> String {
    let lower = text.to_lowercase();
    if kind == "test"
        || lower.contains("测试")
        || lower.contains("playwright")
        || lower.contains("e2e")
    {
        "测试验证"
    } else if lower.contains("修复")
        || lower.contains("bug")
        || lower.contains("问题")
        || lower.contains("故障")
    {
        "问题修复"
    } else if lower.contains("部署")
        || lower.contains("发布")
        || lower.contains("jenkins")
        || lower.contains("docker")
    {
        "部署"
    } else if kind == "report"
        || kind == "knowledge"
        || lower.contains("文档")
        || lower.contains("方案")
        || lower.contains("报告")
    {
        "方案与文档"
    } else if lower.contains("调研") || lower.contains("分析") || lower.contains("研究") {
        "调研"
    } else {
        "功能开发"
    }
    .to_string()
}

fn gap_minutes(state: &DatabaseState) -> Result<i64, String> {
    let value = state
        .connect()?
        .query_row(
            "SELECT value FROM app_meta WHERE key='work_session_gap_minutes'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(value
        .and_then(|item| item.parse::<i64>().ok())
        .unwrap_or(DEFAULT_GAP_MINUTES)
        .clamp(15, 120))
}

fn activity_events(
    state: &DatabaseState,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<ActivityEvent>, String> {
    project_identity::sync_project_profiles_for_state(state)?;
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT local_time,project,kind,detail FROM (
               SELECT strftime('%Y-%m-%dT%H:%M:%S',m.event_time,'localtime') local_time,
                 COALESCE(NULLIF(c.project_override,''),COALESCE(NULLIF(c.cwd,''),'未归类项目')) project,'codex' kind,m.content detail
               FROM conversation_messages m JOIN conversations c ON c.id=m.conversation_id
               WHERE m.event_time IS NOT NULL AND date(m.event_time,'localtime') BETWEEN ?1 AND ?2
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',gc.committed_at,'localtime'),gr.name,'git',gc.subject
               FROM git_commits gc JOIN git_repositories gr ON gr.path=gc.repository_path
               WHERE date(gc.committed_at,'localtime') BETWEEN ?1 AND ?2
                 AND (gr.user_name='' OR lower(trim(gc.author_name))=lower(trim(gr.user_name)))
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',t.updated_at,'localtime'),t.project,'task',t.title||' '||t.note
               FROM tasks t WHERE date(t.updated_at,'localtime') BETWEEN ?1 AND ?2
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',tr.started_at,'localtime'),tr.project,'test',tr.menu_name||' '||tr.mode
               FROM test_runs tr WHERE date(tr.started_at,'localtime') BETWEEN ?1 AND ?2
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',tr.finished_at,'localtime'),tr.project,'test',tr.menu_name||' '||tr.mode
               FROM test_runs tr WHERE tr.finished_at IS NOT NULL AND date(tr.finished_at,'localtime') BETWEEN ?1 AND ?2
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',r.updated_at,'localtime'),'AI个人工作台','report',r.title
               FROM reports r WHERE date(r.updated_at,'localtime') BETWEEN ?1 AND ?2
               UNION ALL
               SELECT strftime('%Y-%m-%dT%H:%M:%S',k.updated_at,'localtime'),COALESCE(NULLIF(k.project,''),'AI个人工作台'),'knowledge',k.title
               FROM knowledge_items k WHERE date(k.updated_at,'localtime') BETWEEN ?1 AND ?2
             ) WHERE local_time IS NOT NULL ORDER BY local_time",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![start_date, end_date], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    let mut events = Vec::new();
    for row in rows {
        let (time, raw_project, kind, detail) = row.map_err(|error| error.to_string())?;
        let Some(time) = parse_local_event(&time) else {
            continue;
        };
        events.push(ActivityEvent {
            time,
            project: project_identity::canonical_project_name(
                &connection,
                &raw_project,
                &raw_project,
            ),
            work_type: work_type_for(&kind, &detail),
            kind,
        });
    }
    Ok(events)
}

fn most_frequent(values: impl Iterator<Item = String>, fallback: &str) -> String {
    let mut counts = HashMap::<String, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.0.cmp(&left.0)))
        .map(|item| item.0)
        .unwrap_or_else(|| fallback.to_string())
}

fn estimated_sessions(events: Vec<ActivityEvent>, threshold: i64) -> Vec<WorkSession> {
    let mut groups: Vec<Vec<ActivityEvent>> = Vec::new();
    for event in events {
        let should_join = groups
            .last()
            .and_then(|group| group.last())
            .is_some_and(|last| {
                event.time.date() == last.time.date()
                    && event.time.signed_duration_since(last.time).num_minutes() <= threshold
            });
        if should_join {
            groups.last_mut().unwrap().push(event);
        } else {
            groups.push(vec![event]);
        }
    }
    let now = Utc::now().to_rfc3339();
    groups
        .into_iter()
        .filter_map(|group| {
            let first = group.first()?.time;
            let last = group.last()?.time;
            let end = if group.len() == 1 {
                first + Duration::minutes(SINGLE_EVENT_MINUTES)
            } else {
                last
            };
            let duration = end
                .signed_duration_since(first)
                .num_minutes()
                .max(SINGLE_EVENT_MINUTES);
            let project =
                most_frequent(group.iter().map(|item| item.project.clone()), "未归类项目");
            let work_type =
                most_frequent(group.iter().map(|item| item.work_type.clone()), "功能开发");
            let kinds = most_frequent(group.iter().map(|item| item.kind.clone()), "本地");
            Some(WorkSession {
                id: format!("estimated-{}", Uuid::new_v4()),
                date: first.date().format("%Y-%m-%d").to_string(),
                start_time: time_text(first),
                end_time: time_text(end),
                duration_minutes: duration,
                project,
                work_type,
                source: "estimated".to_string(),
                note: format!("根据 {} 等本地活动估算，共 {} 个活动点", kinds, group.len()),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
        })
        .collect()
}

fn insert_session(connection: &rusqlite::Connection, session: &WorkSession) -> Result<(), String> {
    connection.execute(
        "INSERT INTO work_sessions(id,date,start_time,end_time,duration_minutes,project,work_type,source,note,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![session.id,session.date,session.start_time,session.end_time,session.duration_minutes,session.project,session.work_type,session.source,session.note,session.created_at,session.updated_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn rebuild_estimated_sessions(
    state: &DatabaseState,
    start_date: &str,
    end_date: &str,
) -> Result<usize, String> {
    parse_date(start_date)?;
    parse_date(end_date)?;
    let sessions = estimated_sessions(
        activity_events(state, start_date, end_date)?,
        gap_minutes(state)?,
    );
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM work_sessions WHERE source='estimated' AND date BETWEEN ?1 AND ?2",
            params![start_date, end_date],
        )
        .map_err(|error| error.to_string())?;
    for session in &sessions {
        insert_session(&transaction, session)?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(sessions.len())
}

fn session_rows(
    state: &DatabaseState,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<WorkSession>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT id,date,start_time,end_time,duration_minutes,project,work_type,source,note,created_at,updated_at
         FROM work_sessions WHERE date BETWEEN ?1 AND ?2 ORDER BY date DESC,start_time",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![start_date, end_date], |row| {
            Ok(WorkSession {
                id: row.get(0)?,
                date: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                duration_minutes: row.get(4)?,
                project: row.get(5)?,
                work_type: row.get(6)?,
                source: row.get(7)?,
                note: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn minute_of_day(value: &str) -> Option<i64> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .ok()
        .map(|time| i64::from(time.hour()) * 60 + i64::from(time.minute()))
}

fn overlaps(left: &WorkSession, right: &WorkSession) -> bool {
    if left.date != right.date {
        return false;
    }
    let Some((left_start, left_end, right_start, right_end)) = minute_of_day(&left.start_time)
        .zip(minute_of_day(&left.end_time))
        .zip(minute_of_day(&right.start_time).zip(minute_of_day(&right.end_time)))
        .map(|((a, b), (c, d))| (a, b, c, d))
    else {
        return false;
    };
    left_start < right_end && right_start < left_end
}

fn effective_sessions(sessions: &[WorkSession]) -> Vec<WorkSession> {
    let manual = sessions
        .iter()
        .filter(|item| item.source == "manual")
        .cloned()
        .collect::<Vec<_>>();
    let mut effective = manual.clone();
    effective.extend(
        sessions
            .iter()
            .filter(|item| {
                item.source == "estimated" && !manual.iter().any(|manual| overlaps(item, manual))
            })
            .cloned(),
    );
    effective
}

pub fn summary_for_range(
    state: &DatabaseState,
    start_date: &str,
    end_date: &str,
    refresh: bool,
) -> Result<WorkSummary, String> {
    project_identity::sync_project_profiles_for_state(state)?;
    parse_date(start_date)?;
    parse_date(end_date)?;
    if refresh {
        rebuild_estimated_sessions(state, start_date, end_date)?;
    }
    let sessions = session_rows(state, start_date, end_date)?;
    let effective = effective_sessions(&sessions);
    let estimated_minutes = sessions
        .iter()
        .filter(|item| item.source == "estimated")
        .map(|item| item.duration_minutes)
        .sum();
    let manual_minutes = sessions
        .iter()
        .filter(|item| item.source == "manual")
        .map(|item| item.duration_minutes)
        .sum();
    let total_minutes = effective.iter().map(|item| item.duration_minutes).sum();
    let mut projects = BTreeMap::<String, i64>::new();
    let mut types = BTreeMap::<String, i64>::new();
    let mut daily = BTreeMap::<String, (i64, i64, i64)>::new();
    let connection = state.connect()?;
    for item in &effective {
        let project =
            project_identity::canonical_project_name(&connection, &item.project, &item.project);
        *projects.entry(project).or_default() += item.duration_minutes;
        *types.entry(item.work_type.clone()).or_default() += item.duration_minutes;
        daily.entry(item.date.clone()).or_default().0 += item.duration_minutes;
    }
    for item in &sessions {
        let entry = daily.entry(item.date.clone()).or_default();
        if item.source == "estimated" {
            entry.1 += item.duration_minutes;
        } else {
            entry.2 += item.duration_minutes;
        }
    }
    let sort_breakdown = |values: BTreeMap<String, i64>| {
        let mut values = values
            .into_iter()
            .map(|(name, minutes)| WorkBreakdown { name, minutes })
            .collect::<Vec<_>>();
        values.sort_by_key(|item| std::cmp::Reverse(item.minutes));
        values
    };
    Ok(WorkSummary {
        start_date: start_date.to_string(),
        end_date: end_date.to_string(),
        total_minutes,
        estimated_minutes,
        manual_minutes,
        has_manual_corrections: manual_minutes > 0,
        by_project: sort_breakdown(projects),
        by_type: sort_breakdown(types),
        daily: daily
            .into_iter()
            .map(
                |(date, (minutes, estimated_minutes, manual_minutes))| DailyWorkMinutes {
                    date,
                    minutes,
                    estimated_minutes,
                    manual_minutes,
                },
            )
            .collect(),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_work_sessions(
    state: tauri::State<'_, DatabaseState>,
    start_date: String,
    end_date: String,
    refresh: Option<bool>,
) -> Result<Vec<WorkSession>, String> {
    parse_date(&start_date)?;
    parse_date(&end_date)?;
    if refresh.unwrap_or(true) {
        rebuild_estimated_sessions(&state, &start_date, &end_date)?;
    }
    session_rows(&state, &start_date, &end_date)
}

#[tauri::command(rename_all = "camelCase")]
pub fn work_summary(
    state: tauri::State<'_, DatabaseState>,
    start_date: String,
    end_date: String,
    refresh: Option<bool>,
) -> Result<WorkSummary, String> {
    summary_for_range(&state, &start_date, &end_date, refresh.unwrap_or(true))
}

#[tauri::command]
pub fn save_work_session(
    state: tauri::State<'_, DatabaseState>,
    mut session: WorkSession,
) -> Result<WorkSession, String> {
    parse_date(&session.date)?;
    let start =
        minute_of_day(&session.start_time).ok_or_else(|| "开始时间格式无效。".to_string())?;
    let end = minute_of_day(&session.end_time).ok_or_else(|| "结束时间格式无效。".to_string())?;
    if end <= start {
        return Err("结束时间必须晚于开始时间。".into());
    }
    if session.project.trim().is_empty() || session.work_type.trim().is_empty() {
        return Err("项目和工作类型不能为空。".into());
    }
    session.duration_minutes = session.duration_minutes.clamp(1, 1440);
    session.source = "manual".into();
    let now = Utc::now().to_rfc3339();
    if session.id.trim().is_empty() || session.id.starts_with("estimated-") {
        session.id = Uuid::new_v4().to_string();
        session.created_at = now.clone();
    }
    session.updated_at = now;
    state.connect()?.execute(
        "INSERT INTO work_sessions(id,date,start_time,end_time,duration_minutes,project,work_type,source,note,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,'manual',?8,?9,?10)
         ON CONFLICT(id) DO UPDATE SET date=excluded.date,start_time=excluded.start_time,end_time=excluded.end_time,duration_minutes=excluded.duration_minutes,project=excluded.project,work_type=excluded.work_type,source='manual',note=excluded.note,updated_at=excluded.updated_at",
        params![session.id,session.date,session.start_time,session.end_time,session.duration_minutes,session.project,session.work_type,session.note,session.created_at,session.updated_at],
    ).map_err(|error| error.to_string())?;
    Ok(session)
}

#[tauri::command]
pub fn delete_work_session(
    state: tauri::State<'_, DatabaseState>,
    id: String,
) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "DELETE FROM work_sessions WHERE id=?1 AND source='manual'",
            [id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn work_time_settings(
    state: tauri::State<'_, DatabaseState>,
) -> Result<WorkTimeSettings, String> {
    Ok(WorkTimeSettings {
        gap_minutes: gap_minutes(&state)?,
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn save_work_time_settings(
    state: tauri::State<'_, DatabaseState>,
    gap_minutes: i64,
) -> Result<WorkTimeSettings, String> {
    if !(15..=120).contains(&gap_minutes) {
        return Err("工时估算间隔必须在 15—120 分钟之间。".into());
    }
    state.connect()?.execute("INSERT INTO app_meta(key,value) VALUES('work_session_gap_minutes',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [gap_minutes.to_string()]).map_err(|error| error.to_string())?;
    Ok(WorkTimeSettings { gap_minutes })
}

#[cfg(test)]
mod tests {
    use super::{
        activity_events, effective_sessions, estimated_sessions, ActivityEvent, WorkSession,
    };
    use crate::database::DatabaseState;
    use chrono::NaiveDate;

    fn event(time: &str, project: &str) -> ActivityEvent {
        ActivityEvent {
            time: NaiveDate::from_ymd_opt(2026, 8, 3)
                .unwrap()
                .and_time(chrono::NaiveTime::parse_from_str(time, "%H:%M").unwrap()),
            project: project.into(),
            work_type: "功能开发".into(),
            kind: "codex".into(),
        }
    }

    #[test]
    fn activity_gap_builds_expected_intervals() {
        let sessions = estimated_sessions(
            vec![
                event("09:10", "client"),
                event("09:35", "client"),
                event("10:05", "client"),
                event("10:40", "client"),
                event("12:00", "APP"),
            ],
            45,
        );
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].duration_minutes, 90);
        assert_eq!(sessions[1].duration_minutes, 15);
    }

    #[test]
    fn manual_session_replaces_overlapping_estimate_but_keeps_original() {
        let base = WorkSession {
            id: "a".into(),
            date: "2026-08-03".into(),
            start_time: "09:00".into(),
            end_time: "11:00".into(),
            duration_minutes: 120,
            project: "client".into(),
            work_type: "功能开发".into(),
            source: "estimated".into(),
            note: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let mut manual = base.clone();
        manual.id = "m".into();
        manual.start_time = "09:30".into();
        manual.end_time = "10:30".into();
        manual.duration_minutes = 60;
        manual.source = "manual".into();
        let effective = effective_sessions(&[base.clone(), manual.clone()]);
        assert_eq!(effective.len(), 1);
        assert_eq!(effective[0].id, "m");
        assert_eq!(base.source, "estimated");
    }

    #[test]
    fn all_required_local_activity_sources_are_collected() {
        let path =
            std::env::temp_dir().join(format!("worktime-sources-{}.sqlite3", uuid::Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let connection = state.connect().unwrap();
        connection.execute_batch("INSERT INTO conversations(id,source_file,cwd,imported_at) VALUES('c','c.json','F:\\TB-project\\client','2026-08-03 08:00:00');
          INSERT INTO conversation_messages(conversation_id,source_index,event_time,role,content) VALUES('c',1,'2026-08-03 09:00:00','user','开发功能');
          INSERT INTO git_repositories(path,name,last_scanned_at) VALUES('F:\\TB-project\\client','client','2026-08-03 09:10:00');
          INSERT INTO git_commits(repository_path,commit_hash,committed_at,subject) VALUES('F:\\TB-project\\client','h','2026-08-03 09:20:00','feat: 新增功能');
          INSERT INTO tasks(id,title,project,scope,status,priority,note,source,created_at,updated_at) VALUES('t','修改任务','client','day','todo','P1','','manual','2026-08-03 09:25:00','2026-08-03 09:25:00');
          INSERT INTO test_runs(id,menu_id,project,menu_name,mode,status,started_at,finished_at) VALUES('tr','m','client','字典管理','mock','passed','2026-08-03 09:30:00','2026-08-03 09:40:00');
          INSERT INTO reports(id,report_type,period_start,period_end,title,created_at,updated_at) VALUES('r','daily','2026-08-03','2026-08-03','日报','2026-08-03 09:45:00','2026-08-03 09:45:00');
          INSERT INTO knowledge_items(id,kind,title,content,created_at,updated_at) VALUES('k','experience','知识编辑','内容','2026-08-03 09:50:00','2026-08-03 09:50:00');").unwrap();
        drop(connection);
        let events = activity_events(&state, "2026-08-03", "2026-08-03").unwrap();
        let kinds = events
            .iter()
            .map(|item| item.kind.as_str())
            .collect::<std::collections::HashSet<_>>();
        for required in ["codex", "git", "task", "test", "report", "knowledge"] {
            assert!(kinds.contains(required), "missing {required}");
        }
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
