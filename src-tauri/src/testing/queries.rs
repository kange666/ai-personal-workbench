//! 测试历史查询与后台摘要。大报告只对限定的历史/详情请求解码。
use super::*;
use std::collections::HashSet;

const SUMMARY_COLUMNS: &str = "id,menu_id,project,project_path,menu_name,mode,status,started_at,finished_at,'' AS report_markdown,source_report_path,'' AS output_excerpt,substr(error_message,1,2000),'[]' AS selected_scenarios,'[]' AS scenario_results,'[]' AS artifacts,total_count,passed_count,failed_count,skipped_count,duration_ms,exit_code,'' AS environment_summary,cleanup_status";

pub(super) fn latest_run_for_project(
    state: &DatabaseState,
    menu_id: &str,
    path: &Path,
) -> Result<Option<TestRun>, String> {
    let connection = state.connect()?;
    let identities = project_identity::ProjectIdentityMap::load(&connection)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let expected = identities.identity_key(name, &display_path(path));
    let mut statement = connection.prepare("SELECT id,project,project_path FROM test_runs WHERE menu_id=?1 ORDER BY started_at DESC,id DESC").map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([menu_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (id, project, path) = row.map_err(|error| error.to_string())?;
        if identities.identity_key(&project, &path) == expected {
            return connection
                .query_row(
                    &format!("SELECT {TEST_RUN_COLUMNS} FROM test_runs WHERE id=?1"),
                    [id],
                    row_to_run,
                )
                .optional()
                .map_err(|error| error.to_string());
        }
    }
    Ok(None)
}

pub(super) fn list_runs_for_state(
    state: &DatabaseState,
    menu_id: Option<&str>,
    project_path: Option<&str>,
    summary: bool,
) -> Result<Vec<TestRun>, String> {
    let connection = state.connect()?;
    let identities = project_identity::ProjectIdentityMap::load(&connection)?;
    let project = project_path
        .filter(|path| !path.trim().is_empty())
        .map(|path| {
            let (root, name) = canonical_project_asset(state, path)?;
            Ok::<_, String>(identities.identity_key(&name, &display_path(&root)))
        })
        .transpose()?;
    // 先扫描轻量标识，匹配规范项目后才加载最多300条完整报告。
    // 全局历史无需项目过滤时直接 LIMIT，避免先读取所有正文再在内存截断。
    let ids = if project.is_some() || summary {
        let mut statement = connection.prepare("SELECT id,project,project_path,menu_id,mode,status FROM test_runs WHERE (?1 IS NULL OR menu_id=?1) ORDER BY started_at DESC,id DESC").map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([menu_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut selected = Vec::new();
        let mut seen = HashSet::new();
        let mut completed_count = 0;
        for row in rows {
            let (id, name, path, menu, mode, status) = row.map_err(|error| error.to_string())?;
            let canonical = identities.identity_key(&name, &path);
            if project
                .as_ref()
                .is_some_and(|project| project != &canonical)
            {
                continue;
            }
            if summary {
                let newest = seen.insert((canonical, menu, mode));
                // 运行中的记录可能早于数百个已结束分组，不能被历史摘要上限截断。
                // 已结束记录仍只取各分组最新一条，最多300条；活动记录独立保留。
                if !matches!(status.as_str(), "queued" | "running") {
                    if !newest || completed_count >= 300 {
                        continue;
                    }
                    completed_count += 1;
                }
            }
            selected.push(id);
            if !summary && selected.len() >= 300 {
                break;
            }
        }
        Some(serde_json::to_string(&selected).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let columns = if summary {
        SUMMARY_COLUMNS
    } else {
        TEST_RUN_COLUMNS
    };
    // 摘要的已结束上限已在轻量扫描中应用，最终查询不能再次截掉活动记录。
    let limit = if summary { "" } else { " LIMIT 300" };
    let sql = format!("SELECT {columns} FROM test_runs WHERE (?1 IS NULL OR menu_id=?1) AND (?2 IS NULL OR id IN (SELECT value FROM json_each(?2))) ORDER BY started_at DESC,id DESC{limit}");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![menu_id, ids], row_to_run)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_keeps_distinct_projects_and_modes_without_report_payloads() {
        let directory = std::env::temp_dir().join(format!("astrion-test-query-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("db.sqlite3")).unwrap();
        let connection = state.connect().unwrap();
        for (id, project, mode, status, time) in [
            ("a-old", "alpha", "mock", "failed", "2026-09-01"),
            ("a-new", "alpha", "mock", "passed", "2026-09-02"),
            ("b", "beta", "mock", "failed", "2026-09-01"),
            ("a-real", "alpha", "real", "failed", "2026-09-01"),
        ] {
            connection.execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at,report_markdown) VALUES(?1,'shared-menu',?2,?2,'匿名页面',?3,?4,?5,'large report')",params![id,project,mode,status,time]).unwrap();
        }
        drop(connection);
        let summary = list_runs_for_state(&state, None, None, true).unwrap();
        assert_eq!(summary.len(), 3);
        assert!(!summary.iter().any(|run| run.id == "a-old"));
        assert!(summary.iter().all(|run| run.report_markdown.is_empty()));
        let history = list_runs_for_state(&state, None, None, false).unwrap();
        assert_eq!(history.len(), 4);
        assert!(history
            .iter()
            .all(|run| run.report_markdown == "large report"));
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn history_is_limited_and_deterministically_sorted() {
        let directory = std::env::temp_dir().join(format!("astrion-test-limit-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("db.sqlite3")).unwrap();
        let mut connection = state.connect().unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..310 {
            transaction.execute("INSERT INTO test_runs(id,menu_id,project,menu_name,mode,status,started_at) VALUES(?1,'menu','anonymous','菜单','mock','passed','2026-09-01')",[format!("run-{index:03}")]).unwrap();
        }
        transaction.commit().unwrap();
        drop(connection);
        let rows = list_runs_for_state(&state, None, None, false).unwrap();
        assert_eq!(rows.len(), 300);
        assert_eq!(rows[0].id, "run-309");
        assert_eq!(rows[299].id, "run-010");
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn summary_keeps_all_older_active_runs_beyond_completed_group_limit() {
        let directory =
            std::env::temp_dir().join(format!("astrion-test-active-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("db.sqlite3")).unwrap();
        let mut connection = state.connect().unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..310 {
            transaction.execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at) VALUES(?1,'menu',?1,?1,'菜单','mock','passed','2026-09-02')",[format!("run-{index:03}")]).unwrap();
        }
        for (id, status) in [("old-running", "running"), ("old-queued", "queued")] {
            // 同分组已有较新的完成记录，仍需保留真实的活动执行。
            transaction.execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at) VALUES(?1,'menu','run-309','run-309','菜单','mock',?2,'2026-09-01')",params![id,status]).unwrap();
        }
        transaction.commit().unwrap();
        drop(connection);
        let rows = list_runs_for_state(&state, None, None, true).unwrap();
        assert_eq!(rows.len(), 302);
        assert_eq!(
            rows.iter().filter(|run| run.status == "passed").count(),
            300
        );
        assert!(rows.iter().any(|run| run.id == "old-running"));
        assert!(rows.iter().any(|run| run.id == "old-queued"));
        assert!(!rows.iter().any(|run| run.id == "run-009"));
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn empty_legacy_paths_only_match_unique_explicit_project_aliases() {
        let directory =
            std::env::temp_dir().join(format!("astrion-test-legacy-{}", Uuid::new_v4()));
        let alpha = directory.join("alpha").join("client");
        let beta = directory.join("beta").join("client");
        fs::create_dir_all(&alpha).unwrap();
        fs::create_dir_all(&beta).unwrap();
        let alpha_path = display_path(&alpha.canonicalize().unwrap());
        let beta_path = display_path(&beta.canonicalize().unwrap());
        let state = DatabaseState::new(directory.join("db.sqlite3")).unwrap();
        let connection = state.connect().unwrap();
        for (id, path, aliases) in [
            ("alpha", &alpha_path, r#"["client","legacy-alpha"]"#),
            ("beta", &beta_path, r#"["client"]"#),
        ] {
            connection.execute("INSERT INTO repository_assets(path,name,last_scanned_at,updated_at) VALUES(?1,'client','now','now')",[path]).unwrap();
            connection.execute("INSERT INTO project_profiles(id,display_name,repository_path,aliases_json,created_at,updated_at) VALUES(?1,?1,?2,?3,'now','now')",params![id,path,aliases]).unwrap();
        }
        for (id, project, path, time) in [
            ("alpha-current", "client", alpha_path.as_str(), "2026-09-01"),
            ("beta-current", "client", beta_path.as_str(), "2026-09-01"),
            ("legacy-known", "legacy-alpha", "", "2026-09-03"),
            ("legacy-ambiguous", "client", "", "2026-09-04"),
        ] {
            connection.execute("INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at) VALUES(?1,'menu',?2,?3,'菜单','mock','passed',?4)",params![id,project,path,time]).unwrap();
        }
        drop(connection);
        let alpha_rows = list_runs_for_state(&state, None, Some(&alpha_path), false).unwrap();
        assert_eq!(
            alpha_rows
                .iter()
                .map(|run| run.id.as_str())
                .collect::<Vec<_>>(),
            ["legacy-known", "alpha-current"]
        );
        let beta_rows = list_runs_for_state(&state, None, Some(&beta_path), false).unwrap();
        assert_eq!(
            beta_rows
                .iter()
                .map(|run| run.id.as_str())
                .collect::<Vec<_>>(),
            ["beta-current"]
        );
        assert_eq!(
            latest_run_for_project(&state, "menu", &alpha)
                .unwrap()
                .unwrap()
                .id,
            "legacy-known"
        );
        assert_eq!(
            latest_run_for_project(&state, "menu", &beta)
                .unwrap()
                .unwrap()
                .id,
            "beta-current"
        );
        // 无法安全归属的旧记录仍保留在全局历史中，不删除或猜测归入重名项目。
        assert_eq!(
            list_runs_for_state(&state, None, None, false)
                .unwrap()
                .len(),
            4
        );
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }
}
