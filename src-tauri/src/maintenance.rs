use crate::database::DatabaseState;
use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, MAIN_DB};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

const RELEASE_MANIFEST_URL: &str =
    "https://kange666.github.io/ai-personal-workbench-download/release.json";
const RELEASE_PAGE_URL: &str = "https://kange666.github.io/ai-personal-workbench-download/";
const DAILY_BACKUP_RETENTION: usize = 14;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    path: String,
    file_name: String,
    kind: String,
    created_at: String,
    size_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    database_path: String,
    backup_directory: String,
    backups: Vec<BackupEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    current_version: String,
    latest_version: String,
    update_available: bool,
    published_at: String,
    release_url: String,
    installer_url: String,
    portable_url: String,
    checked_at: String,
    message: String,
}

fn backup_directory(state: &DatabaseState) -> Result<PathBuf, String> {
    let parent = state
        .path
        .parent()
        .ok_or_else(|| "数据库路径缺少父目录".to_string())?;
    let directory = parent.join("backups");
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

fn backup_kind(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if name.contains("-daily-") {
        "daily"
    } else if name.contains("-export-") {
        "export"
    } else if name.contains("-pre-restore-") {
        "pre-restore"
    } else if name.contains("-before-v") {
        "migration"
    } else {
        "manual"
    }
    .into()
}

fn backup_entry(path: PathBuf) -> Option<BackupEntry> {
    let metadata = std::fs::metadata(&path).ok()?;
    let created_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .and_then(|value| DateTime::from_timestamp(value.as_secs() as i64, 0))
        .map(|value| value.to_rfc3339())
        .unwrap_or_default();
    Some(BackupEntry {
        file_name: path.file_name()?.to_string_lossy().to_string(),
        kind: backup_kind(&path),
        path: path.to_string_lossy().to_string(),
        created_at,
        size_bytes: metadata.len(),
    })
}

fn list_backup_entries(state: &DatabaseState) -> Result<Vec<BackupEntry>, String> {
    let mut entries = std::fs::read_dir(backup_directory(state)?)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("sqlite3"))
        .filter_map(backup_entry)
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

fn verify_database(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    let integrity = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    if integrity != "ok" {
        return Err(format!("备份完整性检查失败：{integrity}"));
    }
    Ok(())
}

fn create_backup_for_state(state: &DatabaseState, kind: &str) -> Result<BackupEntry, String> {
    let timestamp = Local::now().format("%Y%m%d-%H%M%S-%3f");
    let path = backup_directory(state)?.join(format!("workbench-{kind}-{timestamp}.sqlite3"));
    state
        .connect()?
        .backup(MAIN_DB, &path, None)
        .map_err(|error| format!("创建数据库备份失败：{error}"))?;
    verify_database(&path)?;
    backup_entry(path).ok_or_else(|| "无法读取新建备份".to_string())
}

fn prune_daily_backups(state: &DatabaseState) -> Result<(), String> {
    let daily = list_backup_entries(state)?
        .into_iter()
        .filter(|entry| entry.kind == "daily")
        .collect::<Vec<_>>();
    for entry in daily.into_iter().skip(DAILY_BACKUP_RETENTION) {
        let _ = std::fs::remove_file(entry.path);
    }
    Ok(())
}

pub fn ensure_daily_backup_for_state(state: &DatabaseState) -> Result<Option<BackupEntry>, String> {
    let today_marker = format!("-daily-{}", Local::now().format("%Y%m%d"));
    if list_backup_entries(state)?
        .iter()
        .any(|entry| entry.file_name.contains(&today_marker))
    {
        return Ok(None);
    }
    let backup = create_backup_for_state(state, "daily")?;
    prune_daily_backups(state)?;
    Ok(Some(backup))
}

#[tauri::command]
pub fn backup_status(state: tauri::State<'_, DatabaseState>) -> Result<BackupStatus, String> {
    let directory = backup_directory(&state)?;
    Ok(BackupStatus {
        database_path: state.path.to_string_lossy().to_string(),
        backup_directory: directory.to_string_lossy().to_string(),
        backups: list_backup_entries(&state)?,
    })
}

#[tauri::command]
pub fn create_database_backup(
    state: tauri::State<'_, DatabaseState>,
) -> Result<BackupEntry, String> {
    create_backup_for_state(&state, "manual")
}

#[tauri::command]
pub fn export_database_backup(
    state: tauri::State<'_, DatabaseState>,
) -> Result<BackupEntry, String> {
    let source = create_backup_for_state(&state, "export")?;
    let profile = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
    let export_directory = profile.join("Documents").join("AI个人工作台备份");
    std::fs::create_dir_all(&export_directory).map_err(|error| error.to_string())?;
    let target = export_directory.join(&source.file_name);
    std::fs::copy(&source.path, &target).map_err(|error| format!("导出备份失败：{error}"))?;
    verify_database(&target)?;
    backup_entry(target).ok_or_else(|| "无法读取导出备份".to_string())
}

#[tauri::command]
pub fn restore_database_backup(
    state: tauri::State<'_, DatabaseState>,
    path: String,
) -> Result<BackupStatus, String> {
    let requested = std::fs::canonicalize(&path).map_err(|error| error.to_string())?;
    let allowed =
        std::fs::canonicalize(backup_directory(&state)?).map_err(|error| error.to_string())?;
    if !requested.starts_with(&allowed) {
        return Err("仅允许恢复工作台自己创建并校验过的内部备份".into());
    }
    verify_database(&requested)?;
    create_backup_for_state(&state, "pre-restore")?;
    let mut destination = state.connect()?;
    destination
        .restore(MAIN_DB, &requested, None::<fn(rusqlite::backup::Progress)>)
        .map_err(|error| format!("恢复数据库失败：{error}"))?;
    verify_database(&state.path)?;
    backup_status(state)
}

fn version_parts(value: &str) -> Vec<u64> {
    value
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or_default())
        .collect()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let mut latest_parts = version_parts(latest);
    let mut current_parts = version_parts(current);
    let width = latest_parts.len().max(current_parts.len());
    latest_parts.resize(width, 0);
    current_parts.resize(width, 0);
    latest_parts > current_parts
}

#[tauri::command]
pub fn check_for_updates() -> UpdateStatus {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let checked_at = Utc::now().to_rfc3339();
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .and_then(|client| client.get(RELEASE_MANIFEST_URL).send())
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Value>());
    let Ok(manifest) = response else {
        return UpdateStatus {
            current_version: current,
            latest_version: String::new(),
            update_available: false,
            published_at: String::new(),
            release_url: RELEASE_PAGE_URL.into(),
            installer_url: String::new(),
            portable_url: String::new(),
            checked_at,
            message: "暂时无法连接下载页，当前版本仍可正常使用".into(),
        };
    };
    let latest = manifest["version"].as_str().unwrap_or_default().to_string();
    let available = !latest.is_empty() && is_newer_version(&latest, &current);
    UpdateStatus {
        current_version: current,
        latest_version: latest.clone(),
        update_available: available,
        published_at: manifest["publishedAt"].as_str().unwrap_or_default().into(),
        release_url: manifest["releaseUrl"]
            .as_str()
            .unwrap_or(RELEASE_PAGE_URL)
            .into(),
        installer_url: manifest["installer"]["url"]
            .as_str()
            .unwrap_or_default()
            .into(),
        portable_url: manifest["portable"]["url"]
            .as_str()
            .unwrap_or_default()
            .into(),
        checked_at,
        message: if available {
            format!("发现新版本 {latest}")
        } else {
            "当前已是最新版本".into()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions() {
        assert!(is_newer_version("V1.4", "1.3.1"));
        assert!(is_newer_version("1.3.2", "V1.3.1"));
        assert!(!is_newer_version("V1.3.1", "1.3.1"));
        assert!(!is_newer_version("1.2", "1.3.1"));
    }

    #[test]
    fn creates_and_verifies_manual_backup() {
        let directory =
            std::env::temp_dir().join(format!("workbench-maintenance-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        let backup = create_backup_for_state(&state, "manual").unwrap();
        assert!(Path::new(&backup.path).exists());
        verify_database(Path::new(&backup.path)).unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
