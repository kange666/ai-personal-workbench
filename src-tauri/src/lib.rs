mod ai;
mod apifox;
mod audit;
mod capture;
mod codex;
mod codex_video;
mod content;
mod database;
mod email;
mod git;
mod inbox;
mod jenkins;
mod knowledge;
mod maintenance;
mod notifications;
mod parity;
mod parity_catalog;
mod project_identity;
mod reports;
mod suggestions;
mod tapd;
mod testing;
mod toolchain;
mod videos;
mod vip;
mod wellbeing;
mod worktime;

use chrono::{Duration, Timelike};
use database::{ensure_parent, DatabaseState};
use rusqlite::OptionalExtension;
use std::{path::PathBuf, sync::OnceLock};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

const TRAY_ICON_STYLE_KEY: &str = "tray_icon_style";
const TRAY_ICON_SIZE: usize = 64;
static TRAY_FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TrayIconStyle {
    A,
    #[default]
    B,
    C,
    F,
}

impl TrayIconStyle {
    fn from_saved(value: Option<&str>) -> Self {
        match value {
            Some("A") => Self::A,
            Some("C") => Self::C,
            Some("F") => Self::F,
            _ => Self::B,
        }
    }

    fn from_input(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_uppercase().as_str() {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "C" => Ok(Self::C),
            "F" => Ok(Self::F),
            _ => Err("托盘数字风格仅支持 A、B、C、F。".into()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::F => "F",
        }
    }
}

fn tray_icon_style_for_state(state: &DatabaseState) -> Result<TrayIconStyle, String> {
    let value = state
        .connect()?
        .query_row(
            "SELECT value FROM app_meta WHERE key=?1",
            [TRAY_ICON_STYLE_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(TrayIconStyle::from_saved(value.as_deref()))
}

#[tauri::command]
fn tray_icon_style(state: tauri::State<'_, DatabaseState>) -> Result<String, String> {
    Ok(tray_icon_style_for_state(&state)?.as_str().to_string())
}

#[tauri::command(rename_all = "camelCase")]
fn set_tray_icon_style(
    app: tauri::AppHandle,
    state: tauri::State<'_, DatabaseState>,
    style: String,
) -> Result<String, String> {
    let style = TrayIconStyle::from_input(&style)?;
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [TRAY_ICON_STYLE_KEY, style.as_str()],
        )
        .map_err(|error| error.to_string())?;
    if let Some(tray) = app.tray_by_id("workbench-tray") {
        let quota = codex::latest_tray_quota();
        tray.set_icon(Some(quota_tray_icon(
            quota.as_ref().map(|value| value.remaining_percent),
            style,
        )))
        .map_err(|error| error.to_string())?;
        tray.set_tooltip(Some(quota_tray_tooltip(quota.as_ref())))
            .map_err(|error| error.to_string())?;
    }
    Ok(style.as_str().to_string())
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn glyph(value: char) -> [u8; 7] {
    match value {
        '0' => [0b1111, 0b1001, 0b1001, 0b1001, 0b1001, 0b1001, 0b1111],
        '1' => [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0010, 0b0111],
        '2' => [0b1111, 0b0001, 0b0001, 0b1111, 0b1000, 0b1000, 0b1111],
        '3' => [0b1111, 0b0001, 0b0001, 0b0111, 0b0001, 0b0001, 0b1111],
        '4' => [0b1001, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b0001],
        '5' => [0b1111, 0b1000, 0b1000, 0b1111, 0b0001, 0b0001, 0b1111],
        '6' => [0b1111, 0b1000, 0b1000, 0b1111, 0b1001, 0b1001, 0b1111],
        '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b1000, 0b1000, 0b1000],
        '8' => [0b1111, 0b1001, 0b1001, 0b1111, 0b1001, 0b1001, 0b1111],
        '9' => [0b1111, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001, 0b1111],
        '-' => [0, 0, 0, 0b1111, 0, 0, 0],
        _ => [0; 7],
    }
}

fn tray_text_metrics(length: usize) -> (i32, i32, i32) {
    match length {
        0 | 1 => (6, 7, 0),
        2 => (4, 6, 8),
        _ => (3, 5, 4),
    }
}

fn tray_font() -> Option<&'static fontdue::Font> {
    TRAY_FONT
        .get_or_init(|| {
            let windows_dir = std::env::var_os("WINDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
            let fonts_dir = windows_dir.join("Fonts");
            // Windows 11 的任务栏优先使用 Segoe UI Variable；旧系统依次回退到
            // Segoe UI Semibold 和 Segoe UI，避免字体缺失导致托盘图标消失。
            ["SegUIVar.ttf", "seguisb.ttf", "segoeui.ttf"]
                .into_iter()
                .find_map(|file_name| {
                    let bytes = std::fs::read(fonts_dir.join(file_name)).ok()?;
                    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
                })
        })
        .as_ref()
}

fn tray_font_size(length: usize) -> f32 {
    match length {
        0 | 1 => 56.0,
        2 => 50.0,
        _ => 35.0,
    }
}

fn system_font_text_mask(text: &str) -> Option<Vec<u8>> {
    let font = tray_font()?;
    let font_size = tray_font_size(text.chars().count());
    let glyphs = text
        .chars()
        .map(|character| font.rasterize(character, font_size))
        .collect::<Vec<_>>();
    let cell_width = glyphs
        .iter()
        .map(|(metrics, _)| metrics.width)
        .max()
        .unwrap_or(0);
    let max_height = glyphs
        .iter()
        .map(|(metrics, _)| metrics.height)
        .max()
        .unwrap_or(0);
    if cell_width == 0 || max_height == 0 {
        return None;
    }

    let letter_spacing = if glyphs.len() == 2 { 3 } else { 1 };
    let text_width = glyphs.len() * cell_width + glyphs.len().saturating_sub(1) * letter_spacing;
    if text_width > TRAY_ICON_SIZE {
        return None;
    }

    let start_x = (TRAY_ICON_SIZE - text_width) / 2;
    let start_y = (TRAY_ICON_SIZE - max_height) / 2;
    let mut mask = vec![0_u8; TRAY_ICON_SIZE * TRAY_ICON_SIZE];
    for (index, (metrics, bitmap)) in glyphs.into_iter().enumerate() {
        let glyph_x =
            start_x + index * (cell_width + letter_spacing) + (cell_width - metrics.width) / 2;
        let glyph_y = start_y + (max_height - metrics.height) / 2;
        for y in 0..metrics.height {
            for x in 0..metrics.width {
                let alpha = bitmap[y * metrics.width + x];
                let offset = (glyph_y + y) * TRAY_ICON_SIZE + glyph_x + x;
                mask[offset] = mask[offset].max(alpha);
            }
        }
    }
    Some(mask)
}

fn bitmap_text_mask(text: &str) -> Vec<u8> {
    let (scale_x, scale_y, gap) = tray_text_metrics(text.len());
    const GLYPH_WIDTH: i32 = 4;
    let text_width =
        text.len() as i32 * GLYPH_WIDTH * scale_x + (text.len().saturating_sub(1) as i32 * gap);
    let start_x = (TRAY_ICON_SIZE as i32 - text_width) / 2;
    let start_y = (TRAY_ICON_SIZE as i32 - 7 * scale_y) / 2;
    let mut mask = vec![0_u8; TRAY_ICON_SIZE * TRAY_ICON_SIZE];
    for (index, character) in text.chars().enumerate() {
        let rows = glyph(character);
        let glyph_x = start_x + index as i32 * (GLYPH_WIDTH * scale_x + gap);
        for (row, bits) in rows.iter().enumerate() {
            for column in 0_i32..GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                    continue;
                }
                for offset_y in 0..scale_y {
                    for offset_x in 0..scale_x {
                        let x = glyph_x + column * scale_x + offset_x;
                        let y = start_y + row as i32 * scale_y + offset_y;
                        if x >= 0
                            && y >= 0
                            && x < TRAY_ICON_SIZE as i32
                            && y < TRAY_ICON_SIZE as i32
                        {
                            mask[y as usize * TRAY_ICON_SIZE + x as usize] = 255;
                        }
                    }
                }
            }
        }
    }
    mask
}

fn tray_text_mask(text: &str) -> Vec<u8> {
    system_font_text_mask(text).unwrap_or_else(|| bitmap_text_mask(text))
}

fn tray_palette(percent: Option<u8>) -> ([u8; 3], [u8; 3], [u8; 3]) {
    match percent {
        Some(value) if value <= 10 => ([244, 111, 120], [199, 62, 79], [255, 156, 160]),
        Some(value) if value <= 25 => ([230, 159, 73], [191, 104, 45], [247, 191, 112]),
        Some(_) => ([116, 91, 255], [68, 47, 184], [174, 160, 255]),
        None => ([91, 99, 121], [57, 63, 80], [130, 139, 164]),
    }
}

fn inside_rounded_square(x: i32, y: i32, inset: i32, radius: i32) -> bool {
    let min = inset;
    let max = 63 - inset;
    if x < min || y < min || x > max || y > max {
        return false;
    }
    let center_x = x.clamp(min + radius, max - radius);
    let center_y = y.clamp(min + radius, max - radius);
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
}

fn quota_tray_icon(percent: Option<u8>, style: TrayIconStyle) -> Image<'static> {
    let mut rgba = vec![0_u8; TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4];
    let (top, bottom, _border) = tray_palette(percent);
    for y in 0..TRAY_ICON_SIZE {
        for x in 0..TRAY_ICON_SIZE {
            let x = x as i32;
            let y = y as i32;
            let offset = (y as usize * TRAY_ICON_SIZE + x as usize) * 4;
            let color = match style {
                TrayIconStyle::A => {
                    if !inside_rounded_square(x, y, 0, 11) {
                        continue;
                    }
                    let ratio = y as u16;
                    [
                        ((top[0] as u16 * (63 - ratio) + bottom[0] as u16 * ratio) / 63) as u8,
                        ((top[1] as u16 * (63 - ratio) + bottom[1] as u16 * ratio) / 63) as u8,
                        ((top[2] as u16 * (63 - ratio) + bottom[2] as u16 * ratio) / 63) as u8,
                    ]
                }
                TrayIconStyle::B => {
                    if !inside_rounded_square(x, y, 0, 11) {
                        continue;
                    }
                    let ratio = y as u16;
                    let light_top = [255_u8, 255, 255];
                    let light_bottom = [232_u8, 231, 242];
                    [
                        ((light_top[0] as u16 * (63 - ratio) + light_bottom[0] as u16 * ratio) / 63)
                            as u8,
                        ((light_top[1] as u16 * (63 - ratio) + light_bottom[1] as u16 * ratio) / 63)
                            as u8,
                        ((light_top[2] as u16 * (63 - ratio) + light_bottom[2] as u16 * ratio) / 63)
                            as u8,
                    ]
                }
                TrayIconStyle::C => {
                    let dx = x as f64 - 31.5;
                    let dy = y as f64 - 31.5;
                    let distance = (dx * dx + dy * dy).sqrt();
                    // C 方案在系统托盘中只保留高对比度圆环；透明圆心避免数字和底色
                    // 挤占有限像素，约 12px 的原始环宽缩放后仍接近 3px。
                    if !(18.0..=30.5).contains(&distance) {
                        continue;
                    }
                    let angle = (dx.atan2(-dy) + std::f64::consts::TAU) % std::f64::consts::TAU;
                    let progress = percent.unwrap_or(0).min(100) as f64 / 100.0;
                    if angle / std::f64::consts::TAU <= progress {
                        [238, 233, 255]
                    } else {
                        [58, 62, 80]
                    }
                }
                TrayIconStyle::F => {
                    if !inside_rounded_square(x, y, 0, 11) {
                        continue;
                    }
                    let ratio = y as u16;
                    let dark_top = [21_u8, 29, 42];
                    let dark_bottom = [7_u8, 12, 21];
                    [
                        ((dark_top[0] as u16 * (63 - ratio) + dark_bottom[0] as u16 * ratio) / 63)
                            as u8,
                        ((dark_top[1] as u16 * (63 - ratio) + dark_bottom[1] as u16 * ratio) / 63)
                            as u8,
                        ((dark_top[2] as u16 * (63 - ratio) + dark_bottom[2] as u16 * ratio) / 63)
                            as u8,
                    ]
                }
            };
            rgba[offset..offset + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    }

    if style == TrayIconStyle::C {
        return Image::new_owned(rgba, TRAY_ICON_SIZE as u32, TRAY_ICON_SIZE as u32);
    }

    let text = percent
        .map(|value| value.min(100).to_string())
        .unwrap_or_else(|| "--".to_string());
    let text_mask = tray_text_mask(&text);

    let digit_color = match style {
        TrayIconStyle::A | TrayIconStyle::C => [255, 255, 255, 255],
        TrayIconStyle::B => [bottom[0], bottom[1], bottom[2], 255],
        TrayIconStyle::F => match percent {
            Some(value) if value <= 10 => [255, 112, 126, 255],
            Some(value) if value <= 25 => [255, 184, 92, 255],
            Some(_) => [101, 255, 202, 255],
            None => [154, 164, 181, 255],
        },
    };
    for y in 0..TRAY_ICON_SIZE as i32 {
        for x in 0..TRAY_ICON_SIZE as i32 {
            let mask_offset = y as usize * TRAY_ICON_SIZE + x as usize;
            let alpha = text_mask[mask_offset] as u16;
            if alpha > 0 {
                let offset = mask_offset * 4;
                for channel in 0..3 {
                    rgba[offset + channel] = ((rgba[offset + channel] as u16 * (255 - alpha)
                        + digit_color[channel] as u16 * alpha)
                        / 255) as u8;
                }
                rgba[offset + 3] = 255;
            }
        }
    }
    Image::new_owned(rgba, TRAY_ICON_SIZE as u32, TRAY_ICON_SIZE as u32)
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
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(git::ProjectProcessState::default())
        .manage(testing::TestProcessState::default())
        .manage(testing::TestCaseGenerationState::default())
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

            let path = app.path().app_data_dir()?.join("workbench.sqlite3");
            ensure_parent(&path).map_err(std::io::Error::other)?;
            let state = DatabaseState::new(path).map_err(std::io::Error::other)?;
            testing::recover_incomplete_test_runs(&state).map_err(std::io::Error::other)?;
            app.manage(state.clone());

            let show_item = MenuItem::with_id(app, "show", "打开工作台", true, None::<&str>)?;
            let email_item = CheckMenuItem::with_id(
                app,
                "codex-email-toggle",
                "Codex完成邮件（未配置）",
                true,
                false,
                None::<&str>,
            )?;
            app.manage(email::EmailTrayMenuItem(email_item.clone()));
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &email_item, &quit_item])?;
            let initial_quota = codex::latest_tray_quota();
            let initial_tray_style = tray_icon_style_for_state(&state).unwrap_or_default();
            let initial_icon = quota_tray_icon(
                initial_quota
                    .as_ref()
                    .map(|value| value.remaining_percent),
                initial_tray_style,
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
                    "codex-email-toggle" => {
                        let state = app.state::<DatabaseState>();
                        let current = email::status_for_state(&state);
                        if email::set_enabled_for_state(&state, !current.enabled).is_err() {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                                let _ = window.eval("window.location.hash='#/settings'");
                            }
                        }
                        email::sync_tray_menu(app, &state);
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
            let quota_state = state.clone();
            std::thread::spawn(move || {
                let mut previous = initial_quota;
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(30));
                    let current = codex::latest_tray_quota();
                    if current == previous {
                        continue;
                    }
                    if let Some(tray) = quota_app.tray_by_id("workbench-tray") {
                        let style = tray_icon_style_for_state(&quota_state).unwrap_or_default();
                        let icon = quota_tray_icon(
                            current.as_ref().map(|value| value.remaining_percent),
                            style,
                        );
                        let _ = tray.set_icon(Some(icon));
                        let _ = tray.set_tooltip(Some(quota_tray_tooltip(current.as_ref())));
                    }
                    previous = current;
                }
            });

            app.manage(apifox::start_api_export_server(state.clone()));
            jenkins::resume_active_publishes(app.handle().clone(), &state)
                .map_err(std::io::Error::other)?;
            email::initialize_for_state(&state).map_err(std::io::Error::other)?;
            email::sync_tray_menu(app.handle(), &state);
            let history_state = state.clone();
            let history_app = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(error) = maintenance::ensure_daily_backup_for_state(&history_state) {
                    eprintln!("启动时创建每日数据库备份失败：{error}");
                }
                if let Err(error) = parity::sync_feature_parity_for_state(&history_state) {
                    eprintln!("启动时同步 PC/APP 对照矩阵失败：{error}");
                }
                if let Err(error) = videos::sync_video_pipeline_for_state(&history_state) {
                    eprintln!("启动时同步视频生产流水线失败：{error}");
                }
                match reports::sync_history_if_sources_changed(&history_state) {
                    Ok(_) => { let _ = history_app.emit("codex-data-updated", ()); }
                    Err(error) => eprintln!("启动时同步 Codex 历史失败：{error}"),
                }
                if let Err(error) = knowledge::sync_knowledge_for_state(&history_state) {
                    eprintln!("启动时自动整理知识失败：{error}");
                }
                if let Err(error) = suggestions::sync_task_suggestions_for_state(&history_state) {
                    eprintln!("启动时提取任务建议失败：{error}");
                }
                if let Err(error) = notifications::sync_codex_notifications_for_state(&history_state) {
                    eprintln!("启动时同步 Codex 完成提醒失败：{error}");
                }
                email::sync_tray_menu(&history_app, &history_state);
            });
            let token_refresh_state = state.clone();
            let token_refresh_app = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(30));
                loop {
                    match codex::scan_codex_sessions_for_state(&token_refresh_state) {
                        Ok(_) => { let _ = token_refresh_app.emit("codex-data-updated", ()); }
                        Err(error) => eprintln!("自动刷新 Codex Token 失败：{error}"),
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10 * 60));
                }
            });
            let notification_state = state.clone();
            std::thread::spawn(move || loop {
                if let Err(error) = notifications::sync_codex_notifications_for_state(&notification_state) {
                    eprintln!("同步 Codex 完成提醒失败：{error}");
                }
                std::thread::sleep(std::time::Duration::from_secs(15));
            });
            let email_state = state.clone();
            let email_app = app.handle().clone();
            std::thread::spawn(move || loop {
                let task_state = email_state.clone();
                match tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(
                    move || email::process_due_deliveries_for_state(&task_state),
                )) {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        email::record_worker_error_for_state(&email_state, &error);
                        eprintln!("处理 Codex 完成邮件失败：{error}");
                    }
                    Err(error) => {
                        email::record_worker_error_for_state(&email_state, &error.to_string());
                        eprintln!("邮件后台任务异常结束：{error}");
                    }
                }
                email::sync_tray_menu(&email_app, &email_state);
                std::thread::sleep(std::time::Duration::from_secs(15));
            });
            let maintenance_state = state.clone();
            std::thread::spawn(move || loop {
                let now = chrono::Local::now();
                let maintenance_date = now.format("%Y-%m-%d").to_string();
                if let Err(error) = audit::ensure_weekly_audit_for_state(&maintenance_state) {
                    eprintln!("自动周检或漏跑补偿失败：{error}");
                }
                if let Err(error) = tauri::async_runtime::block_on(
                    content::ensure_content_for_date(maintenance_state.clone(), now.date_naive()),
                ) {
                    eprintln!("自动补齐当天内容失败：{error}");
                }
                let last_maintenance = maintenance_state.connect().ok().and_then(|connection| {
                    connection.query_row("SELECT value FROM app_meta WHERE key='last_daily_maintenance_date'", [], |row| row.get::<_, String>(0)).ok()
                });
                if now.hour() >= 22 && last_maintenance.as_deref() != Some(&maintenance_date) {
                    let mut failed = false;
                    for result in [
                        codex::scan_codex_sessions_for_state(&maintenance_state).map(|_| ()),
                        git::scan_git_repositories_for_state(&maintenance_state).map(|_| ()),
                        reports::ensure_scheduled_reports(&maintenance_state).map(|_| ()),
                        knowledge::sync_knowledge_for_state(&maintenance_state).map(|_| ()),
                        suggestions::sync_task_suggestions_for_state(&maintenance_state).map(|_| ()),
                    ] {
                        if let Err(error) = result { failed = true; eprintln!("每日维护失败：{error}"); }
                    }
                    if !failed {
                        if let Ok(connection) = maintenance_state.connect() {
                            let _ = connection.execute("INSERT INTO app_meta(key,value) VALUES('last_daily_maintenance_date',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [&maintenance_date]);
                        }
                    }
                    let tomorrow = now.date_naive() + Duration::days(1);
                    if let Err(error) = tauri::async_runtime::block_on(
                        content::ensure_content_for_date(maintenance_state.clone(), tomorrow),
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
            tray_icon_style,
            set_tray_icon_style,
            database::database_health,
            capture::list_quick_captures,
            capture::save_quick_capture,
            capture::archive_quick_capture,
            capture::delete_quick_capture,
            wellbeing::get_daily_checkin,
            wellbeing::save_daily_checkin,
            maintenance::backup_status,
            maintenance::create_database_backup,
            maintenance::export_database_backup,
            maintenance::restore_database_backup,
            maintenance::check_for_updates,
            maintenance::updater_proxy,
            database::list_tasks,
            database::save_task,
            database::delete_task,
            suggestions::sync_task_suggestions,
            tapd::tapd_status,
            tapd::list_tapd_projects,
            tapd::save_tapd_project,
            tapd::remove_tapd_project,
            tapd::save_tapd_project_automation,
            tapd::preview_tapd_project_automation,
            tapd::set_tapd_automation_paused,
            tapd::save_tapd_auto_fix_settings,
            tapd::save_tapd_credentials,
            tapd::clear_tapd_credentials,
            tapd::test_tapd_connection,
            tapd::sync_tapd_items,
            tapd::list_tapd_items,
            tapd::list_tapd_codex_jobs,
            tapd::execute_tapd_codex_job,
            tapd::read_tapd_process_report,
            tapd::start_tapd_codex_job,
            tapd::continue_tapd_codex_job,
            tapd::run_tapd_codex_job_tests,
            tapd::review_tapd_codex_job,
            database::token_summary,
            database::list_conversation_metrics,
            database::set_conversation_project,
            database::token_trend,
            database::project_token_metrics,
            database::model_token_metrics,
            database::search_workspace,
            apifox::apifox_credential_status,
            apifox::save_apifox_token,
            apifox::clear_apifox_token,
            apifox::list_api_sources,
            apifox::save_api_source,
            apifox::remove_api_source,
            apifox::sync_api_source,
            apifox::sync_all_api_sources,
            apifox::list_api_endpoints,
            apifox::get_api_endpoint,
            apifox::get_api_tag_export,
            apifox::get_api_test_config,
            apifox::save_api_test_config,
            apifox::clear_api_test_token,
            apifox::preview_api_endpoint_test,
            apifox::execute_api_endpoint_test,
            apifox::get_api_code_template,
            apifox::save_api_code_template,
            apifox::render_api_endpoint_markdown,
            apifox::render_api_endpoint_request_code,
            content::list_content_ideas,
            content::generate_daily_content,
            content::update_content_status,
            codex::scan_codex_sessions,
            codex::codex_quota,
            codex_video::codex_cli_status,
            codex_video::content_video_job,
            codex_video::start_content_video_job,
            notifications::sync_codex_notifications,
            notifications::list_notifications,
            notifications::mark_notification_read,
            notifications::mark_all_notifications_read,
            notifications::review_notification,
            inbox::list_inbox_items,
            inbox::update_inbox_status,
            inbox::create_task_from_inbox,
            jenkins::jenkins_connection_status,
            jenkins::test_jenkins_connection,
            jenkins::save_jenkins_connection,
            jenkins::list_jenkins_jobs,
            jenkins::list_jenkins_job_branches,
            jenkins::set_jenkins_job_favorite,
            jenkins::set_jenkins_job_display_name,
            jenkins::trigger_jenkins_publish,
            jenkins::list_jenkins_publish_records,
            jenkins::get_jenkins_publish_status,
            jenkins::open_jenkins_url,
            email::email_notification_status,
            email::save_qq_email_config,
            email::delete_qq_email_config,
            email::test_qq_email,
            email::set_codex_email_enabled,
            email::retry_failed_emails,
            git::scan_git_repositories,
            git::git_scan_configuration,
            git::git_scan_status,
            git::save_git_scan_configuration,
            git::list_repository_assets,
            project_identity::list_project_profiles,
            project_identity::save_project_profile,
            git::repository_asset_details,
            git::save_repository_asset,
            git::generate_commit_plan,
            git::set_repository_pinned,
            git::set_repository_hidden,
            git::set_repository_category,
            git::start_repository_project,
            git::stop_repository_project,
            git::list_running_repository_projects,
            git::open_repository_runtime_url,
            git::git_credential_status,
            git::save_git_default_credential,
            git::clear_git_default_credential,
            git::git_repository_status,
            git::git_fetch_repository,
            git::git_pull_repository,
            git::git_resolve_pull_conflicts,
            git::git_stage_repository_changes,
            git::git_repository_file_diff,
            git::git_unstage_repository_changes,
            git::git_abort_repository_merge,
            git::git_stash_repository_changes,
            git::git_restore_repository_stash,
            git::git_push_repository,
            git::git_switch_repository_branch,
            git::git_merge_repository_branch,
            git::git_revert_repository_commit,
            git::execute_commit_plan_group,
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
            knowledge::list_knowledge_versions,
            knowledge::list_knowledge_codex_jobs,
            knowledge::start_knowledge_codex_job,
            ai::ai_status,
            ai::save_deepseek_key,
            ai::clear_deepseek_key,
            ai::test_deepseek,
            ai::translate_text,
            ai::refine_report_with_ai,
            ai::ask_knowledge,
            testing::list_test_menus,
            testing::list_test_projects,
            testing::list_test_scenarios,
            testing::list_test_suites,
            testing::start_test_case_generation,
            testing::get_test_case_generation,
            testing::recommend_tests_from_git,
            testing::list_test_runs,
            testing::read_test_report,
            testing::read_test_artifact,
            testing::preflight_test,
            testing::start_test_run,
            testing::get_test_run,
            testing::cancel_test_run,
            testing::export_test_report_pdf,
            testing::export_test_report_markdown,
            testing::get_existing_test_report_pdf,
            testing::open_test_report_pdf,
            parity::sync_feature_parity,
            parity::list_feature_parity,
            parity::save_feature_parity_review,
            videos::list_local_videos,
            videos::read_video_cover,
            videos::open_local_video,
            videos::reveal_local_video,
            videos::video_project_details,
            videos::reveal_local_file,
            videos::sync_video_pipeline,
            videos::list_video_jobs,
            videos::save_video_job_type,
            videos::list_video_publish_records,
            videos::save_video_publish_record,
            vip::vip_status,
            vip::activate_vip,
            vip::deactivate_vip,
            toolchain::scan_toolchains,
            toolchain::list_toolchains,
            audit::run_weekly_audit,
            audit::ensure_weekly_audit,
            audit::list_weekly_audits,
            worktime::list_work_sessions,
            worktime::work_summary,
            worktime::save_work_session,
            worktime::delete_work_session,
            worktime::work_time_settings,
            worktime::save_work_time_settings,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build ASTRION");
    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = app_handle.state::<git::ProjectProcessState>();
            let database = app_handle.state::<database::DatabaseState>();
            git::stop_all_repository_projects(&state, &database);
        }
    });
}

#[cfg(test)]
mod tray_tests {
    use super::*;

    #[test]
    fn renders_remaining_percentage_as_tray_icon() {
        let icon = quota_tray_icon(Some(53), TrayIconStyle::B);
        assert_eq!(icon.width(), 64);
        assert_eq!(icon.height(), 64);
        assert_eq!(icon.rgba().len(), 64 * 64 * 4);
        assert!(icon
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 235 && pixel[1] > 235 && pixel[2] > 235));
        let opaque_pixels = icon
            .rgba()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] == 255)
            .count();
        assert!(opaque_pixels > 3_850, "托盘背景应接近填满 64px 画布");
    }

    #[test]
    fn tray_icon_uses_clear_low_quota_warning_without_changing_digits() {
        let low = quota_tray_icon(Some(5), TrayIconStyle::B);
        let normal = quota_tray_icon(Some(55), TrayIconStyle::B);
        assert_ne!(low.rgba(), normal.rgba(), "低额度应使用更醒目的暖色数字");
    }

    #[test]
    fn three_digit_quota_remains_tall_and_readable() {
        let mask = tray_text_mask("100");
        let visible_pixels = mask.iter().filter(|alpha| **alpha >= 32).count();
        let rows = (0..TRAY_ICON_SIZE)
            .filter(|y| {
                mask[y * TRAY_ICON_SIZE..(y + 1) * TRAY_ICON_SIZE]
                    .iter()
                    .any(|alpha| *alpha >= 32)
            })
            .collect::<Vec<_>>();
        let height = rows.last().unwrap() - rows.first().unwrap() + 1;
        assert!(visible_pixels > 250, "三位数必须保留足够的可见笔画");
        assert!(height >= 24, "三位数不能缩成难以识别的小字");
    }

    #[test]
    fn two_digit_quota_has_visible_spacing_at_tray_scale() {
        let mask = tray_text_mask("11");
        let active_columns = (0..TRAY_ICON_SIZE)
            .map(|x| (0..TRAY_ICON_SIZE).any(|y| mask[y * TRAY_ICON_SIZE + x] >= 32))
            .collect::<Vec<_>>();
        let first = active_columns.iter().position(|active| *active).unwrap();
        let last = active_columns.iter().rposition(|active| *active).unwrap();
        let widest_internal_gap = active_columns[first..=last]
            .split(|active| *active)
            .map(|gap| gap.len())
            .max()
            .unwrap_or(0);
        assert!(widest_internal_gap >= 3, "两位数字之间应保留清晰间距");
    }

    #[cfg(windows)]
    #[test]
    fn loads_windows_system_font_for_tray_digits() {
        assert!(
            tray_font().is_some(),
            "Windows 应优先使用 Segoe UI 系统字体"
        );
        let mask = tray_text_mask("53");
        assert!(
            mask.iter().any(|alpha| (1..255).contains(alpha)),
            "系统字体应包含抗锯齿灰度边缘"
        );
    }

    #[test]
    fn circular_style_keeps_progress_ring_visible() {
        let icon = quota_tray_icon(Some(54), TrayIconStyle::C);
        let pixel = |x: usize, y: usize| {
            let offset = (y * 64 + x) * 4;
            &icon.rgba()[offset..offset + 4]
        };
        assert_ne!(pixel(32, 3), pixel(3, 32), "环形进度与剩余轨道应可区分");
        assert!(
            pixel(32, 3)[0..3].iter().all(|channel| *channel >= 230),
            "C 方案进度段应使用明显的偏白高亮色"
        );
        assert!(
            pixel(3, 32)[0..3].iter().all(|channel| *channel <= 80),
            "C 方案剩余轨道应保持深色"
        );
        assert_eq!(pixel(32, 3)[3], 255);
        assert_eq!(pixel(3, 32)[3], 255);
        assert_eq!(pixel(32, 32)[3], 0, "C 方案圆心应透明且不显示数字");
        for y in 0..TRAY_ICON_SIZE {
            for x in 0..TRAY_ICON_SIZE {
                let dx = x as f64 - 31.5;
                let dy = y as f64 - 31.5;
                if (dx * dx + dy * dy).sqrt() < 18.0 {
                    assert_eq!(pixel(x, y)[3], 0, "C 方案圆环内部不能出现数字或底色");
                }
            }
        }
    }

    #[test]
    fn supported_tray_styles_render_distinct_icons_and_default_to_b() {
        let styles = [
            TrayIconStyle::A,
            TrayIconStyle::B,
            TrayIconStyle::C,
            TrayIconStyle::F,
        ];
        let icons = styles.map(|style| quota_tray_icon(Some(57), style));
        for index in 0..icons.len() {
            for other in index + 1..icons.len() {
                assert_ne!(icons[index].rgba(), icons[other].rgba());
            }
        }
        assert_eq!(TrayIconStyle::from_saved(None), TrayIconStyle::B);
        assert_eq!(TrayIconStyle::from_saved(Some("unknown")), TrayIconStyle::B);
        assert!(TrayIconStyle::from_input("D").is_err());
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
