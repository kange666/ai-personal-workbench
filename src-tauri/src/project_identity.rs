use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfile {
    pub id: String,
    pub display_name: String,
    pub repository_path: String,
    pub tapd_workspace_id: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfileUpdate {
    pub id: String,
    pub display_name: String,
    pub repository_path: String,
    pub tapd_workspace_id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub category: String,
}

fn normalized(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_lowercase()
}

fn folder_name(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or("未归类项目")
        .to_string()
}

fn stable_profile_id(path: &str) -> String {
    let mut hasher = DefaultHasher::new();
    normalized(path).hash(&mut hasher);
    format!("project-{:016x}", hasher.finish())
}

fn clean_aliases(values: impl IntoIterator<Item = String>, display_name: &str) -> Vec<String> {
    let display_key = normalized(display_name);
    let mut seen = HashSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| normalized(value) != display_key)
        .filter(|value| seen.insert(normalized(value)))
        .take(30)
        .collect()
}

fn aliases_from_json(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value).unwrap_or_default()
}

fn profile_matches(profile: &ProjectProfile, raw_project: &str, source_path: &str) -> bool {
    let raw_key = normalized(raw_project);
    let path_key = normalized(source_path);
    let raw_folder = normalized(&folder_name(raw_project));
    let repository_key = normalized(&profile.repository_path);
    if !repository_key.is_empty()
        && ((!path_key.is_empty() && path_key.starts_with(&repository_key))
            || (!raw_key.is_empty() && raw_key.starts_with(&repository_key)))
    {
        return true;
    }
    std::iter::once(profile.display_name.as_str())
        .chain(profile.aliases.iter().map(String::as_str))
        .chain(std::iter::once(profile.tapd_workspace_id.as_str()))
        .any(|alias| {
            let alias_key = normalized(alias);
            !alias_key.is_empty()
                && (alias_key == raw_key
                    || alias_key == raw_folder
                    || (!path_key.is_empty() && path_key.starts_with(&alias_key)))
        })
}

fn rewrite_path_references(value: &str, old_path: &str, new_path: &str) -> String {
    let old_windows = old_path.replace('/', "\\");
    let new_windows = new_path.replace('/', "\\");
    let old_forward = old_path.replace('\\', "/");
    let new_forward = new_path.replace('\\', "/");
    value
        .replace(&old_windows, &new_windows)
        .replace(&old_forward, &new_forward)
}

fn merge_renamed_profile(
    connection: &Connection,
    source: &ProjectProfile,
    target: &ProjectProfile,
    target_asset_name: &str,
) -> Result<(), String> {
    let aliases = clean_aliases(
        target
            .aliases
            .iter()
            .cloned()
            .chain(source.aliases.iter().cloned())
            .chain([
                source.display_name.clone(),
                source.repository_path.clone(),
                folder_name(&source.repository_path),
                target_asset_name.to_string(),
            ]),
        &target.display_name,
    );
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| error.to_string())?;

    transaction
        .execute(
            "UPDATE api_sources SET project_profile_id=?1 WHERE project_profile_id=?2",
            params![target.id, source.id],
        )
        .map_err(|error| error.to_string())?;

    let old_labels = clean_aliases(
        source.aliases.iter().cloned().chain([
            source.display_name.clone(),
            folder_name(&source.repository_path),
        ]),
        &target.display_name,
    );
    for (table, column) in [
        ("tasks", "project"),
        ("work_sessions", "project"),
        ("knowledge_items", "project"),
        ("work_inbox_items", "project"),
        ("test_runs", "project"),
        ("conversations", "project_override"),
    ] {
        let sql =
            format!("UPDATE {table} SET {column}=?1 WHERE lower(trim({column}))=lower(trim(?2))");
        for label in &old_labels {
            transaction
                .execute(&sql, params![target.display_name, label])
                .map_err(|error| error.to_string())?;
        }
    }

    for (table, column) in [
        ("tapd_projects", "repository_path"),
        ("tapd_codex_jobs", "repository_path"),
        ("knowledge_codex_jobs", "repository_path"),
        ("commit_plans", "repository_path"),
    ] {
        let sql = format!(
            "UPDATE {table} SET {column}=?1 WHERE lower(replace({column},'/','\\'))=lower(replace(?2,'/','\\'))"
        );
        transaction
            .execute(
                &sql,
                params![target.repository_path, source.repository_path],
            )
            .map_err(|error| error.to_string())?;
    }

    transaction
        .execute(
            "INSERT OR IGNORE INTO git_commits(repository_path,commit_hash,committed_at,subject,file_count,additions,deletions,author_name,author_email)
             SELECT ?1,commit_hash,committed_at,subject,file_count,additions,deletions,author_name,author_email
             FROM git_commits WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?2,'/','\\'))",
            params![target.repository_path, source.repository_path],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM git_commits WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?1,'/','\\'))",
            [&source.repository_path],
        )
        .map_err(|error| error.to_string())?;
    for table in ["git_worktree_snapshots"] {
        let sql = format!(
            "UPDATE {table} SET repository_path=?1 WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?2,'/','\\'))"
        );
        transaction
            .execute(
                &sql,
                params![target.repository_path, source.repository_path],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction
        .execute(
            "DELETE FROM git_repositories WHERE lower(replace(path,'/','\\'))=lower(replace(?1,'/','\\'))",
            [&source.repository_path],
        )
        .map_err(|error| error.to_string())?;

    for table in ["repository_health_snapshots", "repository_runtime_runs"] {
        let sql = format!(
            "UPDATE {table} SET repository_path=?1 WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?2,'/','\\'))"
        );
        transaction
            .execute(
                &sql,
                params![target.repository_path, source.repository_path],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction
        .execute(
            "UPDATE repository_assets
             SET purpose=CASE WHEN trim(purpose)='' THEN COALESCE((SELECT purpose FROM repository_assets WHERE lower(replace(path,'/','\\'))=lower(replace(?2,'/','\\'))),'') ELSE purpose END,
                 category=CASE WHEN trim(category)='' OR category='待确认' THEN COALESCE(NULLIF((SELECT category FROM repository_assets WHERE lower(replace(path,'/','\\'))=lower(replace(?2,'/','\\'))),''),category) ELSE category END,
                 is_pinned=MAX(is_pinned,COALESCE((SELECT is_pinned FROM repository_assets WHERE lower(replace(path,'/','\\'))=lower(replace(?2,'/','\\'))),0)),
                 manually_confirmed=MAX(manually_confirmed,COALESCE((SELECT manually_confirmed FROM repository_assets WHERE lower(replace(path,'/','\\'))=lower(replace(?2,'/','\\'))),0))
             WHERE lower(replace(path,'/','\\'))=lower(replace(?1,'/','\\'))",
            params![target.repository_path, source.repository_path],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM repository_assets WHERE lower(replace(path,'/','\\'))=lower(replace(?1,'/','\\'))",
            [&source.repository_path],
        )
        .map_err(|error| error.to_string())?;

    let test_runs = {
        let mut statement = transaction
            .prepare(
                "SELECT id,project_path,report_markdown,source_report_path,output_excerpt,error_message,scenario_results,artifacts,environment_summary FROM test_runs",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    for (
        id,
        project_path,
        report,
        source_report,
        output,
        error,
        scenarios,
        artifacts,
        environment,
    ) in test_runs
    {
        let next_project_path = rewrite_path_references(
            &project_path,
            &source.repository_path,
            &target.repository_path,
        );
        let next_report =
            rewrite_path_references(&report, &source.repository_path, &target.repository_path);
        let next_source_report = source_report.as_deref().map(|value| {
            rewrite_path_references(value, &source.repository_path, &target.repository_path)
        });
        let next_output =
            rewrite_path_references(&output, &source.repository_path, &target.repository_path);
        let next_error =
            rewrite_path_references(&error, &source.repository_path, &target.repository_path);
        let next_scenarios =
            rewrite_path_references(&scenarios, &source.repository_path, &target.repository_path);
        let next_artifacts =
            rewrite_path_references(&artifacts, &source.repository_path, &target.repository_path);
        let next_environment = rewrite_path_references(
            &environment,
            &source.repository_path,
            &target.repository_path,
        );
        if next_project_path != project_path
            || next_report != report
            || next_source_report != source_report
            || next_output != output
            || next_error != error
            || next_scenarios != scenarios
            || next_artifacts != artifacts
            || next_environment != environment
        {
            transaction
                .execute(
                    "UPDATE test_runs SET project_path=?1,report_markdown=?2,source_report_path=?3,output_excerpt=?4,error_message=?5,scenario_results=?6,artifacts=?7,environment_summary=?8 WHERE id=?9",
                    params![next_project_path, next_report, next_source_report, next_output, next_error, next_scenarios, next_artifacts, next_environment, id],
                )
                .map_err(|error| error.to_string())?;
        }
    }

    transaction
        .execute(
            "UPDATE project_profiles SET aliases_json=?1,tapd_workspace_id=CASE WHEN trim(tapd_workspace_id)='' THEN ?2 ELSE tapd_workspace_id END,updated_at=?3 WHERE id=?4",
            params![serde_json::to_string(&aliases).map_err(|error| error.to_string())?, source.tapd_workspace_id, Utc::now().to_rfc3339(), target.id],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM project_profiles WHERE id=?1", [&source.id])
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn migrate_renamed_project_profiles(connection: &Connection) -> Result<usize, String> {
    let profiles = project_profiles(connection)?;
    let assets = {
        let mut statement = connection
            .prepare("SELECT path,name,remote_url,default_branch FROM repository_assets")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    let mut migrated = 0;
    for source in profiles
        .iter()
        .filter(|profile| !profile.repository_path.trim().is_empty())
        .filter(|profile| !Path::new(&profile.repository_path).exists())
    {
        let Some((_, _, source_remote, source_branch)) = assets
            .iter()
            .find(|(path, _, _, _)| normalized(path) == normalized(&source.repository_path))
        else {
            continue;
        };
        if source_remote.trim().is_empty() || source_branch.trim().is_empty() {
            continue;
        }
        let candidates = assets
            .iter()
            .filter(|(path, _, remote, branch)| {
                Path::new(path).exists()
                    && normalized(path) != normalized(&source.repository_path)
                    && remote.eq_ignore_ascii_case(source_remote)
                    && branch.eq_ignore_ascii_case(source_branch)
            })
            .filter_map(|asset| {
                profiles
                    .iter()
                    .find(|profile| normalized(&profile.repository_path) == normalized(&asset.0))
                    .map(|profile| (profile, asset.1.as_str()))
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            continue;
        }
        let (target, asset_name) = candidates[0];
        merge_renamed_profile(connection, source, target, asset_name)?;
        migrated += 1;
    }
    Ok(migrated)
}

fn merge_profile_aliases(
    connection: &Connection,
    id: &str,
    display_name: &str,
    additions: Vec<String>,
) -> Result<(), String> {
    let current = connection
        .query_row(
            "SELECT aliases_json FROM project_profiles WHERE id=?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| "[]".to_string());
    let aliases = clean_aliases(
        aliases_from_json(&current).into_iter().chain(additions),
        display_name,
    );
    connection
        .execute(
            "UPDATE project_profiles SET aliases_json=?1 WHERE id=?2",
            params![
                serde_json::to_string(&aliases).map_err(|error| error.to_string())?,
                id
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn sync_project_profiles_for_state(state: &DatabaseState) -> Result<usize, String> {
    let connection = state.connect()?;
    let now = Utc::now().to_rfc3339();
    let repositories = {
        let mut statement = connection
            .prepare("SELECT path,name,category FROM repository_assets ORDER BY updated_at DESC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    let mut created = 0usize;
    for (path, name, category) in repositories {
        if !Path::new(&path).exists() {
            continue;
        }
        let existing = connection
            .query_row(
                "SELECT id,display_name FROM project_profiles WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?1,'/','\\')) LIMIT 1",
                [&path],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((id, display_name)) = existing {
            merge_profile_aliases(
                &connection,
                &id,
                &display_name,
                vec![name, path.clone(), folder_name(&path)],
            )?;
            continue;
        }
        let id = stable_profile_id(&path);
        let display_name = if name.trim().is_empty() {
            folder_name(&path)
        } else {
            name.trim().to_string()
        };
        let aliases = clean_aliases(vec![path.clone(), folder_name(&path)], &display_name);
        connection
            .execute(
                "INSERT OR IGNORE INTO project_profiles(id,display_name,repository_path,tapd_workspace_id,aliases_json,category,created_at,updated_at) VALUES(?1,?2,?3,'',?4,?5,?6,?6)",
                params![id, display_name, path, serde_json::to_string(&aliases).map_err(|error| error.to_string())?, category, now],
            )
            .map_err(|error| error.to_string())?;
        created += 1;
    }

    let tapd_projects = {
        let mut statement = connection
            .prepare("SELECT workspace_id,workspace_name,repository_path FROM tapd_projects WHERE enabled=1")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    for (workspace_id, workspace_name, repository_path) in tapd_projects {
        if repository_path.trim().is_empty() {
            continue;
        }
        if let Some((id, display_name)) = connection
            .query_row(
                "SELECT id,display_name FROM project_profiles WHERE lower(replace(repository_path,'/','\\'))=lower(replace(?1,'/','\\')) LIMIT 1",
                [&repository_path],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
        {
            connection
                .execute(
                    "UPDATE project_profiles SET tapd_workspace_id=?1,updated_at=?2 WHERE id=?3",
                    params![workspace_id, now, id],
                )
                .map_err(|error| error.to_string())?;
            merge_profile_aliases(
                &connection,
                &id,
                &display_name,
                vec![workspace_name, repository_path],
            )?;
        }
    }
    Ok(created)
}

pub fn project_profiles(connection: &Connection) -> Result<Vec<ProjectProfile>, String> {
    let mut statement = connection
        .prepare("SELECT id,display_name,repository_path,tapd_workspace_id,aliases_json,category,created_at,updated_at FROM project_profiles ORDER BY display_name COLLATE NOCASE")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ProjectProfile {
                id: row.get(0)?,
                display_name: row.get(1)?,
                repository_path: row.get(2)?,
                tapd_workspace_id: row.get(3)?,
                aliases: aliases_from_json(&row.get::<_, String>(4)?),
                category: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn canonical_project_name(
    connection: &Connection,
    raw_project: &str,
    source_path: &str,
) -> String {
    let profiles = project_profiles(connection).unwrap_or_default();
    for profile in profiles {
        if profile_matches(&profile, raw_project, source_path) {
            return profile.display_name;
        }
    }
    if raw_project.trim().is_empty() {
        "未归类项目".to_string()
    } else {
        folder_name(raw_project)
    }
}

#[tauri::command]
pub fn list_project_profiles(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<ProjectProfile>, String> {
    sync_project_profiles_for_state(&state)?;
    project_profiles(&state.connect()?)
}

#[tauri::command]
pub fn save_project_profile(
    state: tauri::State<'_, DatabaseState>,
    profile: ProjectProfileUpdate,
) -> Result<ProjectProfile, String> {
    let display_name = profile.display_name.trim();
    if display_name.is_empty() {
        return Err("项目名称不能为空。".into());
    }
    let id = if profile.id.trim().is_empty() {
        if profile.repository_path.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            stable_profile_id(&profile.repository_path)
        }
    } else {
        profile.id.clone()
    };
    let aliases = clean_aliases(profile.aliases, display_name);
    let now = Utc::now().to_rfc3339();
    let connection = state.connect()?;
    connection
        .execute(
            "INSERT INTO project_profiles(id,display_name,repository_path,tapd_workspace_id,aliases_json,category,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?7)
             ON CONFLICT(id) DO UPDATE SET display_name=excluded.display_name,repository_path=excluded.repository_path,tapd_workspace_id=excluded.tapd_workspace_id,aliases_json=excluded.aliases_json,category=excluded.category,updated_at=excluded.updated_at",
            params![id, display_name, profile.repository_path.trim(), profile.tapd_workspace_id.trim(), serde_json::to_string(&aliases).map_err(|error| error.to_string())?, profile.category.trim(), now],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "该本地项目目录已经关联到其他规范项目。".to_string()
            } else {
                error.to_string()
            }
        })?;
    project_profiles(&connection)?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "项目映射保存后无法读取。".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn aliases_are_trimmed_and_deduplicated() {
        let aliases = clean_aliases(
            vec![
                " client ".into(),
                "CLIENT".into(),
                r"F:\TB-project\client".into(),
            ],
            "安全生产管理",
        );
        assert_eq!(aliases.len(), 2);
    }

    #[test]
    fn folder_name_handles_windows_paths() {
        assert_eq!(folder_name(r"F:\TB-project\client\"), "client");
    }

    #[test]
    fn renamed_repository_merges_profile_and_moves_history_without_rewriting_conversation_cwd() {
        let directory = std::env::temp_dir().join(format!(
            "workbench-project-identity-{}",
            uuid::Uuid::new_v4()
        ));
        let target = directory.join("scaq-client");
        let old = directory.join("client");
        fs::create_dir_all(&target).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let connection = state.connect().unwrap();
        let old_path = old.to_string_lossy().replace('\\', "/");
        let target_path = target.to_string_lossy().replace('\\', "/");
        connection.execute(
            "INSERT INTO repository_assets(path,name,remote_url,default_branch,last_scanned_at,updated_at) VALUES(?1,'client','https://example.test/client.git','scaq-dev','now','now')",
            [&old_path],
        ).unwrap();
        connection.execute(
            "INSERT INTO repository_assets(path,name,remote_url,default_branch,last_scanned_at,updated_at) VALUES(?1,'scaq-client','https://example.test/client.git','scaq-dev','now','now')",
            [&target_path],
        ).unwrap();
        connection.execute(
            "INSERT INTO project_profiles(id,display_name,repository_path,tapd_workspace_id,aliases_json,category,created_at,updated_at) VALUES('old','client',?1,'','[\"client\"]','tb','now','now')",
            [&old_path],
        ).unwrap();
        connection.execute(
            "INSERT INTO project_profiles(id,display_name,repository_path,tapd_workspace_id,aliases_json,category,created_at,updated_at) VALUES('target','生产安全pc',?1,'37583308','[\"scaq-client\"]','scaq','now','now')",
            [&target_path],
        ).unwrap();
        connection.execute(
            "INSERT INTO tasks(id,title,project,scope,status,priority,created_at,updated_at) VALUES('task','任务','client','day','todo','normal','now','now')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO test_runs(id,menu_id,project,project_path,menu_name,mode,status,started_at,report_markdown,source_report_path) VALUES('run','menu','client',?1,'菜单','mock','passed','now',?2,NULL)",
            params![old_path, format!("报告路径：{old_path}/e2e/report.md")],
        ).unwrap();
        connection.execute(
            "INSERT INTO conversations(id,source_file,title,cwd,imported_at) VALUES('conversation','source.jsonl','历史任务',?1,'now')",
            [&old_path],
        ).unwrap();

        assert_eq!(migrate_renamed_project_profiles(&connection).unwrap(), 1);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM project_profiles", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        let aliases: String = connection
            .query_row(
                "SELECT aliases_json FROM project_profiles WHERE id='target'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(aliases.contains(&old_path));
        assert_eq!(
            connection
                .query_row("SELECT project FROM tasks WHERE id='task'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            "生产安全pc"
        );
        let (project, project_path, report): (String, String, String) = connection
            .query_row(
                "SELECT project,project_path,report_markdown FROM test_runs WHERE id='run'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(project, "生产安全pc");
        assert_eq!(project_path, target_path);
        assert!(report.contains(&target_path));
        assert_eq!(
            connection
                .query_row(
                    "SELECT cwd FROM conversations WHERE id='conversation'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            old_path
        );
        assert_eq!(
            canonical_project_name(&connection, "client", &old_path),
            "生产安全pc"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM repository_assets WHERE path=?1",
                    [&old_path],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        drop(connection);
        drop(state);
        fs::remove_dir_all(directory).unwrap();
    }
}
