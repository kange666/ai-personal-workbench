use crate::database::DatabaseState;
use chrono::{NaiveDate, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyCheckin {
    pub date: String,
    pub energy: Option<i64>,
    pub mood: String,
    pub exercise_minutes: i64,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub fn get_daily_checkin(
    state: tauri::State<'_, DatabaseState>,
    date: String,
) -> Result<Option<DailyCheckin>, String> {
    NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|error| error.to_string())?;
    state
        .connect()?
        .query_row(
            "SELECT date,energy,mood,exercise_minutes,note,created_at,updated_at FROM daily_checkins WHERE date=?1",
            [date],
            |row| Ok(DailyCheckin { date: row.get(0)?, energy: row.get(1)?, mood: row.get(2)?, exercise_minutes: row.get(3)?, note: row.get(4)?, created_at: row.get(5)?, updated_at: row.get(6)? }),
        )
        .optional()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_daily_checkin(
    state: tauri::State<'_, DatabaseState>,
    mut checkin: DailyCheckin,
) -> Result<DailyCheckin, String> {
    NaiveDate::parse_from_str(&checkin.date, "%Y-%m-%d").map_err(|error| error.to_string())?;
    if checkin
        .energy
        .is_some_and(|value| !(1..=5).contains(&value))
    {
        return Err("精力仅支持 1 到 5。".to_string());
    }
    if checkin.exercise_minutes < 0 || checkin.exercise_minutes > 1440 {
        return Err("运动分钟数需要在 0 到 1440 之间。".to_string());
    }
    let now = Utc::now().to_rfc3339();
    if checkin.created_at.trim().is_empty() {
        checkin.created_at = now.clone();
    }
    checkin.updated_at = now;
    state.connect()?.execute(
        "INSERT INTO daily_checkins(date,energy,mood,exercise_minutes,note,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(date) DO UPDATE SET energy=excluded.energy,mood=excluded.mood,exercise_minutes=excluded.exercise_minutes,note=excluded.note,updated_at=excluded.updated_at",
        params![checkin.date,checkin.energy,checkin.mood.trim(),checkin.exercise_minutes,checkin.note.trim(),checkin.created_at,checkin.updated_at],
    ).map_err(|error| error.to_string())?;
    Ok(checkin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn checkin_is_optional_and_updateable() {
        let path =
            std::env::temp_dir().join(format!("workbench-checkin-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let connection = state.connect().unwrap();
        connection.execute("INSERT INTO daily_checkins(date,energy,mood,exercise_minutes,note,created_at,updated_at) VALUES('2026-08-10',4,'平稳',30,'散步','now','now')", []).unwrap();
        let energy: i64 = connection
            .query_row(
                "SELECT energy FROM daily_checkins WHERE date='2026-08-10'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(energy, 4);
        drop(connection);
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
