use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};

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
    let raw_key = normalized(raw_project);
    let path_key = normalized(source_path);
    let raw_folder = normalized(&folder_name(raw_project));
    for profile in profiles {
        let repository_key = normalized(&profile.repository_path);
        if !repository_key.is_empty()
            && ((!path_key.is_empty() && path_key.starts_with(&repository_key))
                || (!raw_key.is_empty() && raw_key.starts_with(&repository_key)))
        {
            return profile.display_name;
        }
        let direct = std::iter::once(profile.display_name.as_str())
            .chain(profile.aliases.iter().map(String::as_str))
            .chain(std::iter::once(profile.tapd_workspace_id.as_str()))
            .any(|alias| {
                let alias_key = normalized(alias);
                !alias_key.is_empty() && (alias_key == raw_key || alias_key == raw_folder)
            });
        if direct {
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
}
