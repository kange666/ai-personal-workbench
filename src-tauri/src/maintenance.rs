use crate::database::{schema_version, DatabaseState, SCHEMA_VERSION};
use chrono::{DateTime, Local, Utc};
use rusqlite::{Connection, MAIN_DB};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::process::Command;

const RELEASE_MANIFEST_URL: &str =
    "https://kange666.github.io/ai-personal-workbench-download/release.json";
const RELEASE_PAGE_URL: &str = "https://kange666.github.io/ai-personal-workbench-download/";
const BACKUP_RETENTION: usize = 10;

fn normalize_proxy_url(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mapped = value
        .split(';')
        .filter_map(|entry| entry.split_once('='))
        .find(|(scheme, _)| scheme.trim().eq_ignore_ascii_case("https"))
        .or_else(|| {
            value
                .split(';')
                .filter_map(|entry| entry.split_once('='))
                .find(|(scheme, _)| scheme.trim().eq_ignore_ascii_case("http"))
        })
        .map(|(_, address)| address.trim())
        .unwrap_or(value);
    let normalized = if mapped.contains("://") {
        mapped.to_string()
    } else {
        format!("http://{mapped}")
    };
    let parsed = reqwest::Url::parse(&normalized).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    Some(normalized)
}

#[cfg(windows)]
fn windows_registry_value(name: &str, kind: &str) -> Option<String> {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = Command::new("reg.exe")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            name,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.split_once(kind)
                .map(|(_, value)| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

fn detected_updater_proxy() -> Option<String> {
    for name in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(value) = std::env::var(name) {
            if let Some(proxy) = normalize_proxy_url(&value) {
                return Some(proxy);
            }
        }
    }
    #[cfg(windows)]
    {
        let enabled = windows_registry_value("ProxyEnable", "REG_DWORD")?;
        if enabled != "0x1" && enabled != "1" {
            return None;
        }
        return windows_registry_value("ProxyServer", "REG_SZ")
            .and_then(|value| normalize_proxy_url(&value));
    }
    #[cfg(not(windows))]
    None
}

#[tauri::command]
pub fn updater_proxy() -> Option<String> {
    detected_updater_proxy()
}

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
    let connection = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
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

fn prune_backups(state: &DatabaseState) -> Result<(), String> {
    let directory = std::fs::canonicalize(backup_directory(state)?)
        .map_err(|error| format!("无法确认备份目录：{error}"))?;
    for entry in list_backup_entries(state)?
        .into_iter()
        .skip(BACKUP_RETENTION)
    {
        let path = PathBuf::from(&entry.path);
        let resolved = std::fs::canonicalize(&path)
            .map_err(|error| format!("无法确认旧备份路径 {}：{error}", path.display()))?;
        if !resolved.starts_with(&directory) {
            return Err(format!("拒绝清理备份目录外的文件：{}", resolved.display()));
        }
        std::fs::remove_file(&resolved)
            .map_err(|error| format!("清理旧备份失败 {}：{error}", resolved.display()))?;
    }
    Ok(())
}

pub fn ensure_daily_backup_for_state(state: &DatabaseState) -> Result<Option<BackupEntry>, String> {
    prune_backups(state)?;
    let today_marker = format!("-daily-{}", Local::now().format("%Y%m%d"));
    if list_backup_entries(state)?
        .iter()
        .any(|entry| entry.file_name.contains(&today_marker))
    {
        return Ok(None);
    }
    let backup = create_backup_for_state(state, "daily")?;
    prune_backups(state)?;
    Ok(Some(backup))
}

#[tauri::command]
pub fn backup_status(state: tauri::State<'_, DatabaseState>) -> Result<BackupStatus, String> {
    prune_backups(&state)?;
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
    let backup = create_backup_for_state(&state, "manual")?;
    prune_backups(&state)?;
    Ok(backup)
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
    let exported = backup_entry(target).ok_or_else(|| "无法读取导出备份".to_string())?;
    prune_backups(&state)?;
    Ok(exported)
}

#[tauri::command]
pub async fn restore_database_backup(
    state: tauri::State<'_, DatabaseState>,
    path: String,
) -> Result<BackupStatus, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || restore_backup_for_state(&database, &path))
        .await
        .map_err(|error| format!("恢复任务异常结束：{error}"))?
}

// 临时库与正式库同目录，仅清理本次恢复创建的唯一目录。
struct RestoreWorkspace(PathBuf);

impl Drop for RestoreWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn restore_backup_for_state(state: &DatabaseState, path: &str) -> Result<BackupStatus, String> {
    let requested = std::fs::canonicalize(&path).map_err(|error| error.to_string())?;
    let allowed =
        std::fs::canonicalize(backup_directory(&state)?).map_err(|error| error.to_string())?;
    if !requested.starts_with(&allowed) {
        return Err("仅允许恢复工作台自己创建并校验过的内部备份".into());
    }
    verify_database(&requested)?;
    let source =
        Connection::open_with_flags(&requested, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| error.to_string())?;
    let version = schema_version(&source)?.ok_or("备份缺少工作台结构版本，无法安全恢复。")?;
    if version > SCHEMA_VERSION {
        return Err("备份来自更新版本，请先升级工作台后再恢复。".into());
    }
    let directory = state
        .path
        .parent()
        .ok_or("数据库目录无效")?
        .join(format!(".restore-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).map_err(|error| error.to_string())?;
    let workspace = RestoreWorkspace(directory);
    let prepared_path = workspace.0.join("prepared.sqlite3");
    source
        .backup(MAIN_DB, &prepared_path, None)
        .map_err(|error| format!("准备恢复副本失败：{error}"))?;
    drop(source);
    // 迁移只发生在恢复副本；失败时正式库和原备份完全不变。
    let prepared = DatabaseState::new(prepared_path.clone())
        .map_err(|error| format!("备份升级失败，当前数据未替换：{error}"))?;
    verify_database(&prepared.path)?;
    let prepared_connection = prepared.connect()?;
    let foreign_key_errors: i64 = prepared_connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    if foreign_key_errors != 0 {
        return Err(format!(
            "备份存在 {foreign_key_errors} 条关联异常，当前数据未替换。"
        ));
    }
    drop(prepared_connection);
    drop(prepared);

    // 先阻止新连接，再等待既有连接释放；备份及替换仅使用本次独占的原始连接。
    let exclusive = state.begin_restore()?;
    let mut destination = Connection::open(&state.path).map_err(|error| error.to_string())?;
    destination
        .busy_timeout(Duration::from_secs(1))
        .map_err(|error| error.to_string())?;
    let protection_path = backup_directory(state)?.join(format!(
        "workbench-pre-restore-{}-{}.sqlite3",
        Local::now().format("%Y%m%d-%H%M%S-%3f"),
        uuid::Uuid::new_v4(),
    ));
    destination
        .backup(MAIN_DB, &protection_path, None)
        .map_err(|error| format!("创建恢复前保护备份失败：{error}"))?;
    verify_database(&protection_path)?;
    // 成功替换之后不再执行可能失败的清理/列表查询，保证调用结果准确代表恢复结果。
    let status = BackupStatus {
        database_path: state.path.to_string_lossy().to_string(),
        backup_directory: allowed.to_string_lossy().to_string(),
        backups: list_backup_entries(state)?,
    };
    destination
        .restore(
            MAIN_DB,
            &prepared_path,
            None::<fn(rusqlite::backup::Progress)>,
        )
        .map_err(|error| format!("恢复数据库失败：{error}"))?;
    exclusive.require_restart();
    Ok(status)
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
pub async fn check_for_updates() -> Result<UpdateStatus, String> {
    // 后台检查不能占用桌面窗口线程，尤其是在网络超时时。
    tauri::async_runtime::spawn_blocking(read_update_status)
        .await
        .map_err(|error| format!("检查更新失败：{error}"))
}

fn read_update_status() -> UpdateStatus {
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

    fn restore_test_database() -> (RestoreWorkspace, DatabaseState) {
        let path = std::env::temp_dir().join(format!("workbench-restore-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        let state = DatabaseState::new(path.join("workbench.sqlite3")).unwrap();
        state
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_meta(key,value) VALUES('restore_test_marker','current')",
                [],
            )
            .unwrap();
        (RestoreWorkspace(path), state)
    }

    fn old_backup(state: &DatabaseState, version: i64) -> PathBuf {
        let path = backup_directory(state)
            .unwrap()
            .join("workbench-old.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE app_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE preserved_data(value TEXT NOT NULL);
            INSERT INTO preserved_data(value) VALUES('old-backup');",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO app_meta(key,value) VALUES('schema_version',?1)",
                [version.to_string()],
            )
            .unwrap();
        path
    }

    fn assert_current_database_unchanged(state: &DatabaseState) {
        let connection = state.connect().unwrap();
        let marker: String = connection
            .query_row(
                "SELECT value FROM app_meta WHERE key='restore_test_marker'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, "current");
        assert_eq!(schema_version(&connection).unwrap(), Some(SCHEMA_VERSION));
        assert!(!std::fs::read_dir(state.path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".restore-")));
    }

    #[test]
    fn restores_old_backup_after_migration_and_requires_process_restart() {
        let (_directory, state) = restore_test_database();
        let path = old_backup(&state, 15);
        let status = restore_backup_for_state(&state, path.to_str().unwrap()).unwrap();
        assert!(state.connect().err().unwrap().contains("重启"));
        let restored = DatabaseState::new(state.path.clone()).unwrap();
        let connection = restored.connect().unwrap();
        assert_eq!(schema_version(&connection).unwrap(), Some(SCHEMA_VERSION));
        assert_eq!(
            connection
                .query_row("SELECT value FROM preserved_data", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "old-backup"
        );
        assert!(connection
            .prepare("SELECT source_id,start_date,end_date,progress FROM tasks")
            .is_ok());
        let protection = status
            .backups
            .iter()
            .find(|entry| entry.kind == "pre-restore")
            .unwrap();
        let protected = Connection::open(&protection.path).unwrap();
        assert_eq!(
            protected
                .query_row(
                    "SELECT value FROM app_meta WHERE key='restore_test_marker'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "current"
        );
        assert_eq!(
            schema_version(&Connection::open(path).unwrap()).unwrap(),
            Some(15)
        );
    }

    #[test]
    fn rejects_corrupt_and_future_backups_without_replacing_current_database() {
        let (_directory, state) = restore_test_database();
        let corrupt = backup_directory(&state)
            .unwrap()
            .join("workbench-corrupt.sqlite3");
        std::fs::write(&corrupt, b"not a sqlite database").unwrap();
        assert!(restore_backup_for_state(&state, corrupt.to_str().unwrap()).is_err());
        assert_current_database_unchanged(&state);
        let future = old_backup(&state, SCHEMA_VERSION + 1);
        assert!(restore_backup_for_state(&state, future.to_str().unwrap())
            .err()
            .unwrap()
            .contains("更新版本"));
        assert_current_database_unchanged(&state);
        assert!(!list_backup_entries(&state)
            .unwrap()
            .iter()
            .any(|entry| entry.kind == "pre-restore"));
    }

    #[test]
    fn failed_backup_migration_leaves_source_and_current_database_unchanged() {
        let (_directory, state) = restore_test_database();
        let source = old_backup(&state, 15);
        Connection::open(&source)
            .unwrap()
            .execute_batch("CREATE TABLE tasks(id TEXT PRIMARY KEY);")
            .unwrap();
        let error = restore_backup_for_state(&state, source.to_str().unwrap())
            .err()
            .unwrap();
        assert!(error.contains("备份升级失败"));
        assert_current_database_unchanged(&state);
        assert_eq!(
            schema_version(&Connection::open(source).unwrap()).unwrap(),
            Some(15)
        );
    }

    #[test]
    fn rejects_backup_with_broken_relationships_before_replacing_current_database() {
        let (_directory, state) = restore_test_database();
        let backup = create_backup_for_state(&state, "manual").unwrap();
        let source = Connection::open(&backup.path).unwrap();
        source
            .execute_batch(
                "PRAGMA foreign_keys=OFF;
            INSERT INTO token_events(conversation_id) VALUES('missing-conversation');",
            )
            .unwrap();
        drop(source);
        assert!(restore_backup_for_state(&state, &backup.path).is_err());
        assert_current_database_unchanged(&state);
    }

    #[test]
    fn failed_sqlite_replacement_rolls_back_and_reopens_normal_access() {
        let (_directory, state) = restore_test_database();
        let source = old_backup(&state, 15);
        // 模拟另一个连接意外持有写事务；即便连接租约之外存在写锁，替换也必须失败且保留旧库。
        let outside_writer = Connection::open(&state.path).unwrap();
        outside_writer.execute_batch("BEGIN IMMEDIATE;").unwrap();
        let error = restore_backup_for_state(&state, source.to_str().unwrap())
            .err()
            .unwrap();
        assert!(error.contains("恢复数据库失败"), "{error}");
        outside_writer.execute_batch("ROLLBACK;").unwrap();
        assert_current_database_unchanged(&state);
        assert!(list_backup_entries(&state)
            .unwrap()
            .iter()
            .any(|entry| entry.kind == "pre-restore"));
    }

    #[test]
    fn restore_waits_for_existing_connections_and_prevents_late_background_writes() {
        use std::sync::mpsc;
        let (_directory, state) = restore_test_database();
        let source = old_backup(&state, 15);
        let held = state.connect().unwrap();
        let background = state.clone();
        let (completed_tx, completed_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = restore_backup_for_state(&background, source.to_str().unwrap());
            completed_tx.send(result).unwrap();
        });
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while state.connect().is_ok() {
            assert!(
                std::time::Instant::now() < deadline,
                "恢复没有及时关闭连接入口"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(completed_rx.try_recv().is_err());
        held.execute(
            "UPDATE app_meta SET value='last-write' WHERE key='restore_test_marker'",
            [],
        )
        .unwrap();
        drop(held);
        let result = completed_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        worker.join().unwrap();
        assert!(state.connect().is_err());
        let protection = result
            .backups
            .iter()
            .find(|entry| entry.kind == "pre-restore")
            .unwrap();
        assert_eq!(
            Connection::open(&protection.path)
                .unwrap()
                .query_row(
                    "SELECT value FROM app_meta WHERE key='restore_test_marker'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "last-write"
        );
    }

    #[test]
    fn compares_semantic_versions() {
        assert!(is_newer_version("V1.4", "1.3.1"));
        assert!(is_newer_version("1.3.2", "V1.3.1"));
        assert!(!is_newer_version("V1.3.1", "1.3.1"));
        assert!(!is_newer_version("1.2", "1.3.1"));
    }

    #[test]
    fn normalizes_windows_proxy_values() {
        assert_eq!(
            normalize_proxy_url("127.0.0.1:10808"),
            Some("http://127.0.0.1:10808".into())
        );
        assert_eq!(
            normalize_proxy_url("http=127.0.0.1:8080;https=127.0.0.1:10808"),
            Some("http://127.0.0.1:10808".into())
        );
        assert_eq!(normalize_proxy_url("socks5://127.0.0.1:10808"), None);
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

    #[test]
    fn keeps_only_ten_newest_internal_backups_across_all_kinds() {
        let directory = std::env::temp_dir().join(format!(
            "workbench-backup-retention-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();

        for index in 0..12 {
            let kind = match index % 3 {
                0 => "daily",
                1 => "manual",
                _ => "pre-restore",
            };
            create_backup_for_state(&state, kind).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }

        prune_backups(&state).unwrap();
        let backups = list_backup_entries(&state).unwrap();
        assert_eq!(backups.len(), BACKUP_RETENTION);
        assert!(backups.iter().all(|entry| Path::new(&entry.path).exists()));

        std::fs::remove_dir_all(directory).unwrap();
    }
}
