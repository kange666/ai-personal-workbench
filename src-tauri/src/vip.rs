use crate::database::DatabaseState;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const VIP_CODE: &str = "5975";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VipStatus {
    active: bool,
}

fn active_for_state(state: &DatabaseState) -> bool {
    state
        .connect()
        .ok()
        .and_then(|connection| {
            connection
                .query_row(
                    "SELECT value FROM app_meta WHERE key='vip_enabled'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
        })
        .as_deref()
        == Some("1")
}

fn save_active(state: &DatabaseState, active: bool) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES('vip_enabled',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![if active { "1" } else { "0" }],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn status(state: &DatabaseState) -> VipStatus {
    VipStatus {
        active: active_for_state(state),
    }
}

#[tauri::command]
pub fn vip_status(state: tauri::State<'_, DatabaseState>) -> VipStatus {
    status(&state)
}

#[tauri::command]
pub fn activate_vip(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
    code: String,
) -> Result<VipStatus, String> {
    if code.trim() != VIP_CODE {
        return Err("VIP 码不正确，请重新输入。".into());
    }
    save_active(&state, true)?;
    let current = status(&state);
    let _ = app.emit("vip-status-changed", current.clone());
    Ok(current)
}

#[tauri::command]
pub fn deactivate_vip(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
) -> Result<VipStatus, String> {
    save_active(&state, false)?;
    let current = status(&state);
    let _ = app.emit("vip-status-changed", current.clone());
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn only_fixed_code_is_accepted() {
        assert_eq!(VIP_CODE, "5975");
        assert_ne!("5974", VIP_CODE);
    }

    #[test]
    fn vip_state_is_persisted_locally() {
        let path = std::env::temp_dir().join(format!("workbench-vip-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        save_active(&state, false).unwrap();
        assert!(!active_for_state(&state));
        save_active(&state, true).unwrap();
        assert!(active_for_state(&state));
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
