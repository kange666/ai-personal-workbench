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
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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
            let mut tray_builder = TrayIconBuilder::with_id("workbench-tray")
                .tooltip("AI 个人工作台")
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
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            tray_builder.build(app)?;

            let path = app.path().app_data_dir()?.join("workbench.sqlite3");
            ensure_parent(&path).map_err(std::io::Error::other)?;
            let state = DatabaseState::new(path).map_err(std::io::Error::other)?;
            app.manage(state.clone());
            tauri::async_runtime::spawn(content::ensure_today_content(state.clone()));
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
                        let content_state = state.clone();
                        let tomorrow = now.date_naive() + Duration::days(1);
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = content::ensure_content_for_date(content_state, tomorrow).await { eprintln!("生成第二天内容标题失败：{error}"); }
                        });
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
            git::scan_git_repositories,
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
