use crate::database::DatabaseState;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use rusqlite::params;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditCheck {
    pub check_type: String,
    pub target: String,
    pub status: String,
    pub summary: String,
    pub details_json: String,
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAudit {
    pub id: String,
    pub week_start: String,
    pub status: String,
    pub scheduled_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary: String,
    pub catch_up_run: bool,
    pub checks: Vec<AuditCheck>,
}

fn monday(date: NaiveDate) -> NaiveDate {
    date - Duration::days(date.weekday().num_days_from_monday() as i64)
}

fn scheduled_at(week_start: NaiveDate) -> String {
    Local
        .from_local_datetime(&week_start.and_hms_opt(9, 0, 0).expect("valid time"))
        .single()
        .expect("local Monday 09:00 should be valid")
        .to_rfc3339()
}

fn scalar_pair(state: &DatabaseState, sql: &str) -> Result<(i64, i64), String> {
    state
        .connect()?
        .query_row(sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| error.to_string())
}

fn add_check(
    state: &DatabaseState,
    audit_id: &str,
    check_type: &str,
    target: &str,
    status: &str,
    summary: &str,
    details: serde_json::Value,
    checked_at: &str,
) -> Result<(), String> {
    state.connect()?.execute(
        "INSERT INTO audit_checks(id,audit_id,check_type,target,status,summary,details_json,checked_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![Uuid::new_v4().to_string(),audit_id,check_type,target,status,summary,details.to_string(),checked_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn run_weekly_audit_for_state(
    state: &DatabaseState,
    week_start: NaiveDate,
    catch_up: bool,
) -> Result<WeeklyAudit, String> {
    crate::parity::sync_feature_parity_for_state(state)?;
    crate::videos::sync_video_pipeline_for_state(state)?;
    let toolchains = crate::toolchain::scan_toolchains_for_state(state)?;
    let started = Utc::now().to_rfc3339();
    let id = format!("weekly-audit-{week_start}");
    state.connect()?.execute(
        "INSERT INTO weekly_audits(id,week_start,status,scheduled_at,started_at,summary,catch_up_run) VALUES(?1,?2,'running',?3,?4,'',?5) ON CONFLICT(week_start) DO UPDATE SET status='running',started_at=excluded.started_at,finished_at=NULL,summary='',catch_up_run=excluded.catch_up_run",
        params![id,week_start.to_string(),scheduled_at(week_start),started,catch_up as i64],
    ).map_err(|error| error.to_string())?;
    state
        .connect()?
        .execute("DELETE FROM audit_checks WHERE audit_id=?1", [&id])
        .map_err(|error| error.to_string())?;
    let checked_at = Utc::now().to_rfc3339();
    let quick_check: String = state
        .connect()?
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    add_check(
        state,
        &id,
        "database",
        "本地数据库",
        if quick_check == "ok" {
            "passed"
        } else {
            "failed"
        },
        if quick_check == "ok" {
            "SQLite 完整性检查通过"
        } else {
            "SQLite 完整性检查未通过"
        },
        json!({"result":quick_check}),
        &checked_at,
    )?;

    let (conversations, archived) = scalar_pair(
        state,
        "SELECT COUNT(*),COALESCE(SUM(archived),0) FROM conversations",
    )?;
    add_check(
        state,
        &id,
        "codex-history",
        "Codex 全部对话",
        if conversations > 0 {
            "passed"
        } else {
            "failed"
        },
        &format!("已索引 {conversations} 个对话，其中 {archived} 个归档"),
        json!({"conversations":conversations,"archived":archived}),
        &checked_at,
    )?;

    let (repositories, dirty) = scalar_pair(
        state,
        "SELECT COUNT(*),COALESCE(SUM(has_uncommitted_changes),0) FROM repository_assets",
    )?;
    add_check(
        state,
        &id,
        "repositories",
        "本地 Git 仓库",
        if repositories == 0 {
            "failed"
        } else if dirty > 0 {
            "attention"
        } else {
            "passed"
        },
        &format!("已索引 {repositories} 个仓库，{dirty} 个有未提交改动"),
        json!({"repositories":repositories,"dirty":dirty}),
        &checked_at,
    )?;

    let (daily_reports, weekly_reports) = scalar_pair(state,"SELECT COALESCE(SUM(CASE WHEN report_type='daily' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN report_type='weekly' THEN 1 ELSE 0 END),0) FROM reports")?;
    add_check(
        state,
        &id,
        "reports",
        "日报与周报",
        if daily_reports > 0 && weekly_reports > 0 {
            "passed"
        } else {
            "attention"
        },
        &format!("现有日报 {daily_reports} 份、周报 {weekly_reports} 份"),
        json!({"daily":daily_reports,"weekly":weekly_reports}),
        &checked_at,
    )?;

    let (parities, unverified) = scalar_pair(state,"SELECT (SELECT COUNT(*) FROM feature_parities),(SELECT COUNT(*) FROM regression_cases WHERE status='unverified')")?;
    add_check(
        state,
        &id,
        "pc-app-parity",
        "PC / APP 对照",
        if parities >= 4 && unverified == 0 {
            "passed"
        } else if parities >= 4 {
            "attention"
        } else {
            "failed"
        },
        &format!("{parities} 个全量功能条目已建矩阵，{unverified} 个接口/浏览器验证尚未执行"),
        json!({"features":parities,"unverified":unverified}),
        &checked_at,
    )?;

    let (complete_videos, complete_types) = scalar_pair(
        state,
        "SELECT COUNT(*),COUNT(DISTINCT video_type) FROM video_jobs WHERE status='complete'",
    )?;
    add_check(
        state,
        &id,
        "video-pipeline",
        "三类视频流水线",
        if complete_types >= 3 {
            "passed"
        } else {
            "failed"
        },
        &format!("{complete_videos} 个项目交付完整，覆盖 {complete_types}/3 种视频类型"),
        json!({"complete":complete_videos,"types":complete_types}),
        &checked_at,
    )?;

    let conflicts = toolchains.conflicts.len() as i64;
    add_check(
        state,
        &id,
        "toolchain",
        "本机工具链",
        if conflicts == 0 {
            "passed"
        } else {
            "attention"
        },
        &format!("发现 {conflicts} 个待人工确认的重复入口或版本差异"),
        json!({"installations":toolchains.installations.len(),"conflicts":conflicts}),
        &checked_at,
    )?;

    let (failed, attention) = state.connect()?.query_row(
        "SELECT COALESCE(SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN status='attention' THEN 1 ELSE 0 END),0) FROM audit_checks WHERE audit_id=?1",
        [&id], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,i64>(1)?)),
    ).map_err(|error|error.to_string())?;
    let status = if failed > 0 {
        "failed"
    } else if attention > 0 {
        "attention"
    } else {
        "passed"
    };
    let summary = format!("周检完成：{failed} 项失败，{attention} 项需要关注。所有检查均为只读或本地索引，不会自动修改其他项目。{}", if catch_up { "本次为漏跑补偿。" } else { "" });
    let finished = Utc::now().to_rfc3339();
    state
        .connect()?
        .execute(
            "UPDATE weekly_audits SET status=?2,finished_at=?3,summary=?4 WHERE id=?1",
            params![id, status, finished, summary],
        )
        .map_err(|error| error.to_string())?;
    list_weekly_audits_for_state(state)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "周检结果保存后未找到。".into())
}

pub fn ensure_weekly_audit_for_state(state: &DatabaseState) -> Result<Option<WeeklyAudit>, String> {
    let now = Local::now();
    let week_start = monday(now.date_naive());
    let scheduled = Local
        .from_local_datetime(&week_start.and_hms_opt(9, 0, 0).expect("valid time"))
        .single()
        .expect("valid local time");
    if now < scheduled {
        return Ok(None);
    }
    let exists: i64 = state
        .connect()?
        .query_row(
            "SELECT COUNT(*) FROM weekly_audits WHERE week_start=?1 AND status!='running'",
            [week_start.to_string()],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if exists > 0 {
        return Ok(None);
    }
    Ok(Some(run_weekly_audit_for_state(
        state,
        week_start,
        now.signed_duration_since(scheduled).num_minutes() > 5,
    )?))
}

pub fn list_weekly_audits_for_state(state: &DatabaseState) -> Result<Vec<WeeklyAudit>, String> {
    let connection = state.connect()?;
    let mut statement=connection.prepare("SELECT id,week_start,status,scheduled_at,started_at,finished_at,summary,catch_up_run FROM weekly_audits ORDER BY week_start DESC LIMIT 52").map_err(|error|error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut audits = Vec::new();
    for (id, week_start, status, scheduled_at, started_at, finished_at, summary, catch_up) in rows {
        let mut checks_statement=connection.prepare("SELECT check_type,target,status,summary,details_json,checked_at FROM audit_checks WHERE audit_id=?1 ORDER BY check_type").map_err(|error|error.to_string())?;
        let checks = checks_statement
            .query_map([&id], |row| {
                Ok(AuditCheck {
                    check_type: row.get(0)?,
                    target: row.get(1)?,
                    status: row.get(2)?,
                    summary: row.get(3)?,
                    details_json: row.get(4)?,
                    checked_at: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        audits.push(WeeklyAudit {
            id,
            week_start,
            status,
            scheduled_at,
            started_at,
            finished_at,
            summary,
            catch_up_run: catch_up != 0,
            checks,
        });
    }
    Ok(audits)
}

#[tauri::command]
pub fn run_weekly_audit(state: tauri::State<'_, DatabaseState>) -> Result<WeeklyAudit, String> {
    run_weekly_audit_for_state(&state, monday(Local::now().date_naive()), false)
}

#[tauri::command]
pub fn ensure_weekly_audit(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Option<WeeklyAudit>, String> {
    ensure_weekly_audit_for_state(&state)
}

#[tauri::command]
pub fn list_weekly_audits(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<WeeklyAudit>, String> {
    list_weekly_audits_for_state(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn monday_is_stable_for_every_day() {
        let monday_date = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        for offset in 0..7 {
            assert_eq!(monday(monday_date + Duration::days(offset)), monday_date);
        }
    }

    #[test]
    fn weekly_audit_records_seven_checks_and_catchup_flag() {
        let path = std::env::temp_dir().join(format!("workbench-audit-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        state.connect().unwrap().execute("INSERT INTO conversations(id,source_file,title,started_at,updated_at,imported_at) VALUES('c','c.jsonl','demo','2026-08-03','2026-08-03','2026-08-03')",[]).unwrap();
        state.connect().unwrap().execute("INSERT INTO repository_assets(path,name,has_uncommitted_changes,last_scanned_at,updated_at) VALUES('x','demo',0,'2026-08-03','2026-08-03')",[]).unwrap();
        state.connect().unwrap().execute("INSERT INTO reports(id,report_type,period_start,period_end,title,content_markdown,status,created_at,updated_at) VALUES('d','daily','2026-08-03','2026-08-03','d','x','generated','x','x'),('w','weekly','2026-08-03','2026-08-09','w','x','generated','x','x')",[]).unwrap();
        let audit =
            run_weekly_audit_for_state(&state, NaiveDate::from_ymd_opt(2026, 8, 3).unwrap(), true)
                .unwrap();
        assert_eq!(audit.checks.len(), 7);
        assert!(audit.catch_up_run);
        assert!(audit.summary.contains("漏跑补偿"));
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
