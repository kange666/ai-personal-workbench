mod ai;
mod codex;
mod content;
mod database;
mod git;
mod knowledge;
mod reports;
mod suggestions;
mod testing;
mod videos;
mod worktime;

use chrono::{Duration, Timelike};
use database::{ensure_parent, DatabaseState};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn glyph(value: char) -> [u8; 7] {
    match value {
        '0' => [
            0b11111, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b11111,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b11111, 0b00001, 0b00001, 0b11111, 0b10000, 0b10000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00001, 0b00001, 0b01111, 0b00001, 0b00001, 0b11111,
        ],
        '4' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b00001, 0b00001, 0b00001,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11111, 0b00001, 0b00001, 0b11111,
        ],
        '6' => [
            0b11111, 0b10000, 0b10000, 0b11111, 0b10001, 0b10001, 0b11111,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b11111, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b11111,
        ],
        '9' => [
            0b11111, 0b10001, 0b10001, 0b11111, 0b00001, 0b00001, 0b11111,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        _ => [0; 7],
    }
}

fn quota_tray_icon(percent: Option<u8>) -> Image<'static> {
    const SIZE: usize = 64;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let center = (SIZE as i32 - 1) / 2;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - center;
            let dy = y as i32 - center;
            if dx * dx + dy * dy <= 30 * 30 {
                let offset = (y * SIZE + x) * 4;
                rgba[offset..offset + 4].copy_from_slice(&[117, 100, 245, 255]);
            }
        }
    }

    let text = percent
        .map(|value| value.min(100).to_string())
        .unwrap_or_else(|| "--".to_string());
    let scale = match text.len() {
        1 => 7,
        2 => 5,
        _ => 3,
    };
    let gap = scale;
    let text_width = text.len() as i32 * 5 * scale + (text.len().saturating_sub(1) as i32 * gap);
    let start_x = (SIZE as i32 - text_width) / 2;
    let start_y = (SIZE as i32 - 7 * scale) / 2;
    for (index, character) in text.chars().enumerate() {
        let rows = glyph(character);
        let glyph_x = start_x + index as i32 * (5 * scale + gap);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                for offset_y in 0..scale {
                    for offset_x in 0..scale {
                        let x = glyph_x + column * scale + offset_x;
                        let y = start_y + row as i32 * scale + offset_y;
                        if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
                            continue;
                        }
                        let offset = (y as usize * SIZE + x as usize) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
                    }
                }
            }
        }
    }
    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}

fn quota_tray_tooltip(quota: Option<&codex::TrayQuota>) -> String {
    match quota {
        Some(value) => {
            let reset = chrono::DateTime::from_timestamp(value.resets_at, 0)
                .map(|time| {
                    time.with_timezone(&chrono::Local)
                        .format("%m月%d日 %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|| "未知时间".to_string());
            format!(
                "Codex 剩余用量 {}% · {} 重置",
                value.remaining_percent, reset
            )
        }
        None => "Codex 剩余用量暂无快照".to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;
            #[cfg(not(debug_assertions))]
            {
                use tauri_plugin_autostart::ManagerExt;
                if !app.autolaunch().is_enabled().unwrap_or(false) {
                    let _ = app.autolaunch().enable();
                }
            }

            let show_item = MenuItem::with_id(app, "show", "打开工作台", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let initial_quota = codex::latest_tray_quota();
            let initial_icon = quota_tray_icon(
                initial_quota
                    .as_ref()
                    .map(|value| value.remaining_percent),
            );
            let tray_builder = TrayIconBuilder::with_id("workbench-tray")
                .tooltip(quota_tray_tooltip(initial_quota.as_ref()))
                .icon(initial_icon)
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                });
            tray_builder.build(app)?;

            let quota_app = app.handle().clone();
            std::thread::spawn(move || {
                let mut previous = initial_quota;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    let current = codex::latest_tray_quota();
                    if current == previous {
                        continue;
                    }
                    if let Some(tray) = quota_app.tray_by_id("workbench-tray") {
                        let icon = quota_tray_icon(
                            current.as_ref().map(|value| value.remaining_percent),
                        );
                        let _ = tray.set_icon(Some(icon));
                        let _ = tray.set_tooltip(Some(quota_tray_tooltip(current.as_ref())));
                    }
                    previous = current;
                }
            });

            let path = app.path().app_data_dir()?.join("workbench.sqlite3");
            ensure_parent(&path).map_err(std::io::Error::other)?;
            let state = DatabaseState::new(path).map_err(std::io::Error::other)?;
            app.manage(state.clone());
            let history_state = state.clone();
            std::thread::spawn(move || {
                if let Err(error) = reports::sync_history_if_sources_changed(&history_state) {
                    eprintln!("启动时同步 Codex 历史失败：{error}");
                }
                if let Err(error) = knowledge::sync_knowledge_for_state(&history_state) {
                    eprintln!("启动时自动整理知识失败：{error}");
                }
                if let Err(error) = suggestions::sync_task_suggestions_for_state(&history_state) {
                    eprintln!("启动时提取任务建议失败：{error}");
                }
            });
            std::thread::spawn(move || loop {
                let now = chrono::Local::now();
                let maintenance_date = now.format("%Y-%m-%d").to_string();
                if let Err(error) = tauri::async_runtime::block_on(
                    content::ensure_content_for_date(state.clone(), now.date_naive()),
                ) {
                    eprintln!("自动补齐当天内容失败：{error}");
                }
                let last_maintenance = state.connect().ok().and_then(|connection| {
                    connection.query_row("SELECT value FROM app_meta WHERE key='last_daily_maintenance_date'", [], |row| row.get::<_, String>(0)).ok()
                });
                if now.hour() >= 22 && last_maintenance.as_deref() != Some(&maintenance_date) {
                    let mut failed = false;
                    for result in [
                        codex::scan_codex_sessions_for_state(&state).map(|_| ()),
                        git::scan_git_repositories_for_state(&state).map(|_| ()),
                        reports::ensure_scheduled_reports(&state).map(|_| ()),
                        knowledge::sync_knowledge_for_state(&state).map(|_| ()),
                        suggestions::sync_task_suggestions_for_state(&state).map(|_| ()),
                    ] {
                        if let Err(error) = result { failed = true; eprintln!("每日维护失败：{error}"); }
                    }
                    if !failed {
                        if let Ok(connection) = state.connect() {
                            let _ = connection.execute("INSERT INTO app_meta(key,value) VALUES('last_daily_maintenance_date',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&maintenance_date]);
                        }
                    }
                    let tomorrow = now.date_naive() + Duration::days(1);
                    if let Err(error) = tauri::async_runtime::block_on(
                        content::ensure_content_for_date(state.clone(), tomorrow),
                    ) {
                        eprintln!("生成第二天内容标题失败：{error}");
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            database::database_health,
            database::list_tasks,
            database::save_task,
            database::delete_task,
            suggestions::sync_task_suggestions,
            database::token_summary,
            database::list_conversation_metrics,
            database::set_conversation_project,
            database::token_trend,
            database::project_token_metrics,
            database::model_token_metrics,
            database::search_workspace,
            content::list_content_ideas,
            content::generate_daily_content,
            content::update_content_status,
            codex::scan_codex_sessions,
            codex::codex_quota,
            git::scan_git_repositories,
            git::git_scan_configuration,
            git::save_git_scan_configuration,
            git::list_repository_assets,
            git::repository_asset_details,
            git::save_repository_asset,
            reports::list_reports,
            reports::report_sources,
            reports::generate_report,
            reports::backfill_historical_reports,
            reports::daily_activity,
            reports::history_coverage,
            reports::save_report,
            reports::set_report_locked,
            knowledge::list_knowledge,
            knowledge::sync_knowledge,
            knowledge::save_knowledge,
            knowledge::delete_knowledge,
            ai::ai_status,
            ai::save_deepseek_key,
            ai::clear_deepseek_key,
            ai::test_deepseek,
            ai::refine_report_with_ai,
            ai::ask_knowledge,
            testing::list_test_menus,
            testing::list_test_runs,
            testing::read_test_report,
            testing::start_test_run,
            videos::list_local_videos,
            videos::read_video_cover,
            videos::open_local_video,
            videos::reveal_local_video,
            videos::video_project_details,
            videos::reveal_local_file,
            worktime::list_work_sessions,
            worktime::work_summary,
            worktime::save_work_session,
            worktime::delete_work_session,
            worktime::work_time_settings,
            worktime::save_work_time_settings,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AI Personal Workbench");
}

#[cfg(test)]
mod tray_tests {
    use super::*;

    #[test]
    fn renders_remaining_percentage_as_tray_icon() {
        let icon = quota_tray_icon(Some(53));
        assert_eq!(icon.width(), 64);
        assert_eq!(icon.height(), 64);
        assert_eq!(icon.rgba().len(), 64 * 64 * 4);
        assert!(icon
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 255, 255, 255]));
        assert!(icon
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel == [117, 100, 245, 255]));
    }

    #[test]
    fn tooltip_explains_remaining_quota() {
        let value = codex::TrayQuota {
            remaining_percent: 53,
            resets_at: 1_786_233_736,
        };
        let tooltip = quota_tray_tooltip(Some(&value));
        assert!(tooltip.contains("53%"));
        assert!(tooltip.contains("重置"));
    }
}
