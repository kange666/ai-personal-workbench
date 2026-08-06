use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;

const TOOLS: &[&str] = &["node", "python", "ffmpeg", "git", "cargo", "hyperframes"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainInstallation {
    pub id: String,
    pub tool_name: String,
    pub version: String,
    pub executable_path: String,
    pub source: String,
    pub path_priority: i64,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainConflict {
    pub id: String,
    pub tool_name: String,
    pub conflict_type: String,
    pub summary: String,
    pub recommended_action: String,
    pub status: String,
    pub detected_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainInventory {
    pub installations: Vec<ToolchainInstallation>,
    pub conflicts: Vec<ToolchainConflict>,
}

fn where_paths(tool: &str) -> Vec<String> {
    let Ok(output) = Command::new("where.exe").arg(tool).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| seen.insert(line.to_ascii_lowercase()))
        .map(str::to_string)
        .collect()
}

fn read_version(path: &str) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let output = if matches!(extension.as_str(), "cmd" | "bat") {
        Command::new("cmd.exe")
            .args(["/d", "/c", path, "--version"])
            .output()
    } else {
        Command::new(path).arg("--version").output()
    };
    let Ok(output) = output else {
        return "无法读取".into();
    };
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("无法读取")
        .chars()
        .take(120)
        .collect()
}

fn path_source(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.contains(".codex") || lower.contains("codex-runtimes") {
        "Codex bundled"
    } else if lower.contains("windowsapps") {
        "Windows alias"
    } else if lower.contains("scoop") {
        "Scoop"
    } else if lower.contains("chocolatey") {
        "Chocolatey"
    } else if lower.contains("rustup") || lower.contains(".cargo") {
        "Rustup/Cargo"
    } else {
        "System/User PATH"
    }
}

pub fn scan_toolchains_for_state(state: &DatabaseState) -> Result<ToolchainInventory, String> {
    let now = Utc::now().to_rfc3339();
    let connection = state.connect()?;
    connection
        .execute("DELETE FROM toolchain_installations", [])
        .map_err(|error| error.to_string())?;
    connection
        .execute("DELETE FROM toolchain_conflicts", [])
        .map_err(|error| error.to_string())?;
    for tool in TOOLS {
        let paths = where_paths(tool);
        let mut versions = BTreeMap::<String, usize>::new();
        for (index, path) in paths.iter().enumerate() {
            let version = read_version(path);
            *versions.entry(version.clone()).or_default() += 1;
            let id = format!("tool:{tool}:{index}");
            connection.execute(
                "INSERT INTO toolchain_installations(id,tool_name,version,executable_path,source,path_priority,scanned_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![id,tool,version,path,path_source(path),index as i64,now],
            ).map_err(|error| error.to_string())?;
        }
        if paths.len() > 1 {
            let id = format!("conflict:{tool}:multiple-paths");
            connection.execute(
                "INSERT INTO toolchain_conflicts(id,tool_name,conflict_type,summary,recommended_action,status,detected_at) VALUES(?1,?2,'multiple-paths',?3,?4,'unconfirmed',?5)",
                params![id,tool,format!("PATH 中发现 {} 个 {tool} 入口，当前优先使用 {}。",paths.len(),paths[0]),"先确认项目实际使用的版本；如无问题可保留。需要整理时再人工调整 PATH，工作台不会自动删除或改写。",now],
            ).map_err(|error| error.to_string())?;
        }
        let readable_versions = versions
            .keys()
            .filter(|version| version.as_str() != "无法读取")
            .collect::<Vec<_>>();
        if readable_versions.len() > 1 {
            let id = format!("conflict:{tool}:versions");
            connection.execute(
                "INSERT INTO toolchain_conflicts(id,tool_name,conflict_type,summary,recommended_action,status,detected_at) VALUES(?1,?2,'version-mismatch',?3,?4,'unconfirmed',?5)",
                params![id,tool,format!("{tool} 的多个入口返回不同版本：{}。",readable_versions.into_iter().cloned().collect::<Vec<_>>().join("；")),"以项目锁定版本和实际构建命令为准，确认后再人工统一；不要直接删除当前可用版本。",now],
            ).map_err(|error| error.to_string())?;
        }
    }
    list_toolchains_for_state(state)
}

pub fn list_toolchains_for_state(state: &DatabaseState) -> Result<ToolchainInventory, String> {
    let connection = state.connect()?;
    let mut installations_statement = connection.prepare("SELECT id,tool_name,version,executable_path,source,COALESCE(path_priority,0),scanned_at FROM toolchain_installations ORDER BY tool_name,path_priority").map_err(|error| error.to_string())?;
    let installations = installations_statement
        .query_map([], |row| {
            Ok(ToolchainInstallation {
                id: row.get(0)?,
                tool_name: row.get(1)?,
                version: row.get(2)?,
                executable_path: row.get(3)?,
                source: row.get(4)?,
                path_priority: row.get(5)?,
                scanned_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut conflicts_statement = connection.prepare("SELECT id,tool_name,conflict_type,summary,recommended_action,status,detected_at FROM toolchain_conflicts ORDER BY tool_name,conflict_type").map_err(|error| error.to_string())?;
    let conflicts = conflicts_statement
        .query_map([], |row| {
            Ok(ToolchainConflict {
                id: row.get(0)?,
                tool_name: row.get(1)?,
                conflict_type: row.get(2)?,
                summary: row.get(3)?,
                recommended_action: row.get(4)?,
                status: row.get(5)?,
                detected_at: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(ToolchainInventory {
        installations,
        conflicts,
    })
}

#[tauri::command]
pub fn scan_toolchains(
    state: tauri::State<'_, DatabaseState>,
) -> Result<ToolchainInventory, String> {
    scan_toolchains_for_state(&state)
}

#[tauri::command]
pub fn list_toolchains(
    state: tauri::State<'_, DatabaseState>,
) -> Result<ToolchainInventory, String> {
    list_toolchains_for_state(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn inventory_records_tools_and_only_recommends_manual_changes() {
        let path =
            std::env::temp_dir().join(format!("workbench-toolchain-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let inventory = scan_toolchains_for_state(&state).unwrap();
        assert!(inventory
            .installations
            .iter()
            .any(|item| item.tool_name == "git"));
        assert!(inventory.conflicts.iter().all(|item| {
            item.recommended_action.contains("人工")
                && !item.recommended_action.contains("请自动")
                && !item.recommended_action.contains("自动改写")
        }));
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
