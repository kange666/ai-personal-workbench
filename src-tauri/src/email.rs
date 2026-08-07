use crate::database::DatabaseState;
use chrono::{DateTime, Duration, FixedOffset, Local, NaiveTime, Utc};
use keyring::Entry;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::Duration as StdDuration;
use tauri::{AppHandle, Emitter, Manager};

const CREDENTIAL_SERVICE: &str = "AI Personal Workbench";
const CREDENTIAL_USER: &str = "qq-smtp";
const SMTP_HOST: &str = "smtp.qq.com";
const SMTP_PORT: u16 = 465;
const DEFAULT_AFTER_TIME: &str = "17:40";
pub struct EmailTrayMenuItem(pub tauri::menu::CheckMenuItem<tauri::Wry>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QqEmailCredential {
    email: String,
    auth_code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailNotificationStatus {
    pub configured: bool,
    pub enabled: bool,
    pub state: String,
    pub masked_email: String,
    pub after_time: String,
    pub last_error: String,
    pub retrying_count: i64,
    pub failed_count: i64,
}

#[derive(Debug)]
struct DeliveryMessage {
    notification_id: String,
    title: String,
    body: String,
    output: String,
    source_id: String,
    project: String,
    completed_at: String,
    attempts: i64,
}

fn credential_entry() -> Result<Entry, String> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|error| error.to_string())
}

fn credential() -> Result<QqEmailCredential, String> {
    let raw = credential_entry()?
        .get_password()
        .map_err(|_| "尚未配置 QQ 邮箱和 SMTP 授权码。".to_string())?;
    let credential = serde_json::from_str::<QqEmailCredential>(&raw)
        .map_err(|_| "Windows 凭据库中的 QQ 邮件配置无效，请重新保存。".to_string())?;
    if !valid_qq_email(&credential.email) || credential.auth_code.trim().is_empty() {
        return Err("QQ 邮箱或 SMTP 授权码无效，请重新保存。".into());
    }
    Ok(credential)
}

fn valid_qq_email(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let Some(account) = value.strip_suffix("@qq.com") else {
        return false;
    };
    !account.is_empty() && account.chars().all(|character| character.is_ascii_digit())
}

fn masked_email(value: &str) -> String {
    let Some((account, domain)) = value.split_once('@') else {
        return String::new();
    };
    let visible = account.chars().take(2).collect::<String>();
    format!("{visible}****@{domain}")
}

fn meta(state: &DatabaseState, key: &str) -> String {
    state
        .connect()
        .ok()
        .and_then(|connection| {
            connection
                .query_row("SELECT value FROM app_meta WHERE key=?1", [key], |row| {
                    row.get::<_, String>(0)
                })
                .ok()
        })
        .unwrap_or_default()
}

fn set_meta(state: &DatabaseState, key: &str, value: &str) -> Result<(), String> {
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [key, value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn enabled(state: &DatabaseState) -> bool {
    meta(state, "codex_email_enabled") == "1"
}

fn after_time(state: &DatabaseState) -> String {
    let value = meta(state, "codex_email_after_time");
    if NaiveTime::parse_from_str(&value, "%H:%M").is_ok() {
        value
    } else {
        DEFAULT_AFTER_TIME.into()
    }
}

fn is_authentication_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("authentication")
        || lower.contains("auth failed")
        || lower.contains("535")
        || lower.contains("invalid login")
        || lower.contains("credentials")
}

fn friendly_send_error(error: &str) -> String {
    if is_authentication_error(error) {
        "QQ邮箱认证失败，请重新生成或保存 SMTP 授权码。".into()
    } else if error.to_ascii_lowercase().contains("timed out")
        || error.to_ascii_lowercase().contains("connect")
    {
        "QQ 邮件服务器连接失败，请检查网络后重试。".into()
    } else if error.to_ascii_lowercase().contains("tls")
        || error.to_ascii_lowercase().contains("certificate")
    {
        "QQ 邮件 SSL/TLS 连接失败，请检查系统时间和网络证书。".into()
    } else {
        "QQ 邮件发送失败，请检查网络和 SMTP 配置后重试。".into()
    }
}

fn retry_plan(
    attempts_after_failure: i64,
    authentication_error: bool,
) -> (&'static str, Option<i64>) {
    if authentication_error || attempts_after_failure > 3 {
        ("failed", None)
    } else {
        (
            "retrying",
            Some([1, 5, 15][(attempts_after_failure - 1) as usize]),
        )
    }
}

pub fn status_for_state(state: &DatabaseState) -> EmailNotificationStatus {
    let credential = credential().ok();
    let configured = credential.is_some();
    let retrying_count = state
        .connect()
        .ok()
        .and_then(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM email_deliveries WHERE status='retrying'",
                    [],
                    |row| row.get(0),
                )
                .ok()
        })
        .unwrap_or(0);
    let failed_count = state
        .connect()
        .ok()
        .and_then(|connection| {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM email_deliveries WHERE status='failed'",
                    [],
                    |row| row.get(0),
                )
                .ok()
        })
        .unwrap_or(0);
    let config_status = meta(state, "codex_email_config_status");
    let has_error = config_status == "error" || retrying_count > 0 || failed_count > 0;
    let current_enabled = enabled(state) && configured;
    EmailNotificationStatus {
        configured,
        enabled: current_enabled,
        state: if !configured {
            "unconfigured"
        } else if has_error {
            "error"
        } else if config_status != "ready" {
            "unverified"
        } else if current_enabled {
            "ready"
        } else {
            "disabled"
        }
        .into(),
        masked_email: credential
            .as_ref()
            .map(|value| masked_email(&value.email))
            .unwrap_or_default(),
        after_time: after_time(state),
        last_error: meta(state, "codex_email_last_error"),
        retrying_count,
        failed_count,
    }
}

pub fn initialize_for_state(state: &DatabaseState) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    set_meta(state, "codex_email_runtime_started_at", &now)?;
    state
        .connect()?
        .execute(
            "UPDATE email_deliveries SET status='skipped_disabled',last_error='工作台上次退出前尚未开始发送，本次启动不补发',updated_at=?1 WHERE status='pending'",
            [&now],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn completion_is_after_threshold(completed_at: &str, threshold: &str) -> bool {
    let Ok(completed) = DateTime::parse_from_rfc3339(completed_at) else {
        return false;
    };
    let Ok(threshold) = NaiveTime::parse_from_str(threshold, "%H:%M") else {
        return false;
    };
    let beijing = FixedOffset::east_opt(8 * 60 * 60).expect("固定东八区偏移有效");
    completed.with_timezone(&beijing).time() >= threshold
}

fn occurred_before(value: &str, boundary: &str) -> bool {
    match (
        DateTime::parse_from_rfc3339(value),
        DateTime::parse_from_rfc3339(boundary),
    ) {
        (Ok(value), Ok(boundary)) => value < boundary,
        _ => true,
    }
}

fn delivery_decision(
    completed_at: &str,
    threshold: &str,
    runtime_started_at: &str,
    enabled_at: &str,
    current_enabled: bool,
    configured: bool,
) -> (&'static str, String) {
    if !completion_is_after_threshold(completed_at, threshold) {
        return (
            "skipped_before_time",
            format!("完成时间早于北京时间 {threshold}"),
        );
    }
    if runtime_started_at.is_empty() || occurred_before(completed_at, runtime_started_at) {
        return (
            "skipped_disabled",
            "任务在工作台本次启动前已经完成，不补发邮件".into(),
        );
    }
    if !current_enabled
        || !configured
        || enabled_at.is_empty()
        || occurred_before(completed_at, enabled_at)
    {
        return ("skipped_disabled", "任务完成时邮件通知未开启".into());
    }
    ("pending", String::new())
}

pub fn reconcile_notifications_for_state(state: &DatabaseState) -> Result<usize, String> {
    let current_enabled = enabled(state);
    let configured = credential().is_ok();
    let runtime_started_at = meta(state, "codex_email_runtime_started_at");
    let enabled_at = meta(state, "codex_email_enabled_at");
    let threshold = after_time(state);
    let now = Utc::now().to_rfc3339();
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT n.id,n.created_at FROM notifications n LEFT JOIN email_deliveries d ON d.notification_id=n.id WHERE n.kind='codex_complete' AND d.notification_id IS NULL ORDER BY n.created_at",
        )
        .map_err(|error| error.to_string())?;
    let missing = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    let mut created = 0;
    for (notification_id, completed_at) in missing {
        let (status, reason) = delivery_decision(
            &completed_at,
            &threshold,
            &runtime_started_at,
            &enabled_at,
            current_enabled,
            configured,
        );
        created += connection
            .execute(
                "INSERT OR IGNORE INTO email_deliveries(notification_id,status,attempts,next_attempt_at,sent_at,last_error,created_at,updated_at) VALUES(?1,?2,0,NULL,NULL,?3,?4,?4)",
                params![notification_id,status,reason,now],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(created)
}

fn compact_line(value: &str, max_chars: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn output_excerpt(value: &str) -> String {
    value.chars().take(2000).collect()
}

fn summary_lines(body: &str, output: &str) -> Vec<String> {
    let mut lines = output
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("```")
                && !line.starts_with("<oai-")
        })
        .map(|line| compact_line(line.trim_start_matches(['-', '*', '•', ' ']), 180))
        .filter(|line| !line.is_empty())
        .take(3)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(compact_line(body, 180));
    }
    lines
}

fn beijing_time(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| {
            time.with_timezone(&FixedOffset::east_opt(8 * 60 * 60).expect("固定东八区偏移有效"))
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| value.to_string())
}

fn mail_content(delivery: &DeliveryMessage) -> (String, String) {
    let task = delivery
        .title
        .trim_start_matches("Codex 任务已完成：")
        .trim();
    let subject = format!("[Codex任务完成] {}", compact_line(task, 80));
    let summary = summary_lines(&delivery.body, &delivery.output)
        .into_iter()
        .map(|line| format!("- {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "Codex 任务已完成\n\n项目：{}\n任务：{}\n完成时间：{}\n会话ID：{}\n\n完成摘要：\n{}\n\nCodex输出：\n{}\n\n完整信息请在 AI 个人工作台的消息中心查看。",
        if delivery.project.trim().is_empty() { "AI个人工作台" } else { delivery.project.trim() },
        task,
        beijing_time(&delivery.completed_at),
        if delivery.source_id.is_empty() { "未记录" } else { &delivery.source_id },
        summary,
        output_excerpt(&delivery.output)
    );
    (subject, body)
}

fn smtp_send(credential: &QqEmailCredential, subject: &str, body: &str) -> Result<(), String> {
    let mailbox: Mailbox = credential
        .email
        .parse()
        .map_err(|_| "QQ 邮箱地址格式无效。".to_string())?;
    let message = Message::builder()
        .from(mailbox.clone())
        .to(mailbox)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|error| error.to_string())?;
    let mailer = SmtpTransport::relay(SMTP_HOST)
        .map_err(|error| error.to_string())?
        .port(SMTP_PORT)
        .credentials(Credentials::new(
            credential.email.clone(),
            credential.auth_code.clone(),
        ))
        .timeout(Some(StdDuration::from_secs(25)))
        .build();
    mailer.send(&message).map_err(|error| error.to_string())?;
    Ok(())
}

fn due_deliveries(state: &DatabaseState) -> Result<Vec<DeliveryMessage>, String> {
    let connection = state.connect()?;
    let now = Utc::now().to_rfc3339();
    let mut statement = connection
        .prepare(
            "SELECT d.notification_id,n.title,n.body,n.output,COALESCE(n.source_id,''),COALESCE((SELECT project FROM conversations c WHERE c.id=n.source_id LIMIT 1),''),n.created_at,d.attempts FROM email_deliveries d JOIN notifications n ON n.id=d.notification_id WHERE d.status IN ('pending','retrying') AND (d.next_attempt_at IS NULL OR d.next_attempt_at<=?1) ORDER BY d.created_at LIMIT 5",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([now], |row| {
            Ok(DeliveryMessage {
                notification_id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                output: row.get(3)?,
                source_id: row.get(4)?,
                project: row.get(5)?,
                completed_at: row.get(6)?,
                attempts: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn record_failure(
    state: &DatabaseState,
    delivery: &DeliveryMessage,
    error: &str,
) -> Result<(), String> {
    let attempts = delivery.attempts + 1;
    let friendly = friendly_send_error(error);
    let auth_error = is_authentication_error(error);
    let (status, delay_minutes) = retry_plan(attempts, auth_error);
    let next_attempt_at =
        delay_minutes.map(|minutes| (Utc::now() + Duration::minutes(minutes)).to_rfc3339());
    let now = Utc::now().to_rfc3339();
    state
        .connect()?
        .execute(
            "UPDATE email_deliveries SET status=?1,attempts=?2,next_attempt_at=?3,last_error=?4,updated_at=?5 WHERE notification_id=?6",
            params![status,attempts,next_attempt_at,friendly,now,delivery.notification_id],
        )
        .map_err(|error| error.to_string())?;
    set_meta(state, "codex_email_last_error", &friendly)?;
    if auth_error {
        set_meta(state, "codex_email_config_status", "error")?;
    }
    Ok(())
}

pub fn process_due_deliveries_for_state(state: &DatabaseState) -> Result<usize, String> {
    reconcile_notifications_for_state(state)?;
    if !enabled(state) {
        return Ok(0);
    }
    let credential = credential()?;
    let mut sent = 0;
    for delivery in due_deliveries(state)? {
        let (subject, body) = mail_content(&delivery);
        match smtp_send(&credential, &subject, &body) {
            Ok(()) => {
                let now = Utc::now().to_rfc3339();
                state
                    .connect()?
                    .execute(
                        "UPDATE email_deliveries SET status='sent',attempts=attempts+1,next_attempt_at=NULL,sent_at=?1,last_error='',updated_at=?1 WHERE notification_id=?2",
                        params![now,delivery.notification_id],
                    )
                    .map_err(|error| error.to_string())?;
                set_meta(state, "codex_email_config_status", "ready")?;
                set_meta(state, "codex_email_last_error", "")?;
                sent += 1;
            }
            Err(error) => record_failure(state, &delivery, &error)?,
        }
    }
    Ok(sent)
}

pub fn sync_tray_menu(app: &AppHandle, state: &DatabaseState) {
    let current = status_for_state(state);
    if let Some(item) = app.try_state::<EmailTrayMenuItem>() {
        let _ = item.0.set_checked(current.enabled);
        let text = match current.state.as_str() {
            "error" => "Codex完成邮件（异常）",
            "unconfigured" => "Codex完成邮件（未配置）",
            "unverified" => "Codex完成邮件（待验证）",
            _ => "Codex完成邮件",
        };
        let _ = item.0.set_text(text);
    }
    let _ = app.emit("codex-email-status-changed", current);
}

pub fn set_enabled_for_state(state: &DatabaseState, value: bool) -> Result<(), String> {
    if value {
        credential()?;
        if meta(state, "codex_email_config_status") != "ready" {
            return Err("请先在设置中发送测试邮件，验证 QQ 邮箱和 SMTP 授权码。".into());
        }
        let now = Utc::now().to_rfc3339();
        if meta(state, "codex_email_started_at").is_empty() {
            set_meta(state, "codex_email_started_at", &now)?;
        }
        set_meta(state, "codex_email_enabled_at", &now)?;
    } else {
        let now = Utc::now().to_rfc3339();
        state
            .connect()?
            .execute(
                "UPDATE email_deliveries SET status='skipped_disabled',next_attempt_at=NULL,last_error='邮件通知关闭，不再补发',updated_at=?1 WHERE status IN ('pending','retrying')",
                [&now],
            )
            .map_err(|error| error.to_string())?;
    }
    set_meta(state, "codex_email_enabled", if value { "1" } else { "0" })?;
    Ok(())
}

#[tauri::command]
pub fn email_notification_status(
    state: tauri::State<'_, DatabaseState>,
) -> EmailNotificationStatus {
    status_for_state(&state)
}

#[tauri::command]
pub fn save_qq_email_config(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
    email: String,
    auth_code: String,
) -> Result<(), String> {
    let email = email.trim().to_ascii_lowercase();
    let auth_code = auth_code.trim();
    if !valid_qq_email(&email) {
        return Err("请输入完整的数字 QQ 邮箱，例如 123456@qq.com。".into());
    }
    if auth_code.len() < 8 {
        return Err("请输入 QQ 邮箱生成的 SMTP 授权码，不是 QQ 登录密码。".into());
    }
    let raw = serde_json::to_string(&QqEmailCredential {
        email,
        auth_code: auth_code.into(),
    })
    .map_err(|error| error.to_string())?;
    credential_entry()?
        .set_password(&raw)
        .map_err(|error| error.to_string())?;
    set_enabled_for_state(&state, false)?;
    set_meta(&state, "codex_email_config_status", "unverified")?;
    set_meta(&state, "codex_email_last_error", "")?;
    sync_tray_menu(&app, &state);
    Ok(())
}

#[tauri::command]
pub fn delete_qq_email_config(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
) -> Result<(), String> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(error) => return Err(error.to_string()),
    }
    set_enabled_for_state(&state, false)?;
    set_meta(&state, "codex_email_config_status", "unconfigured")?;
    set_meta(&state, "codex_email_last_error", "")?;
    sync_tray_menu(&app, &state);
    Ok(())
}

#[tauri::command]
pub async fn test_qq_email(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
) -> Result<String, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let credential = credential()?;
        smtp_send(
            &credential,
            "[AI个人工作台] QQ邮件通知测试",
            &format!(
                "QQ 邮件通知连接测试成功。\n\n发件人/收件人：{}\nSMTP：smtp.qq.com:465（SSL/TLS）\n测试时间：{}\n\n验证通过后，可在工作台顶部开启 Codex 完成邮件通知。",
                masked_email(&credential.email),
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ),
        )
    })
    .await
    .map_err(|error| error.to_string())?;
    match result {
        Ok(()) => {
            set_meta(&state, "codex_email_config_status", "ready")?;
            set_meta(&state, "codex_email_last_error", "")?;
            sync_tray_menu(&app, &state);
            Ok("测试邮件已发送，请检查当前 QQ 邮箱收件箱。".into())
        }
        Err(error) => {
            let friendly = friendly_send_error(&error);
            set_meta(&state, "codex_email_config_status", "error")?;
            set_meta(&state, "codex_email_last_error", &friendly)?;
            sync_tray_menu(&app, &state);
            Err(friendly)
        }
    }
}

#[tauri::command]
pub fn set_codex_email_enabled(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
    enabled: bool,
) -> Result<EmailNotificationStatus, String> {
    set_enabled_for_state(&state, enabled)?;
    sync_tray_menu(&app, &state);
    Ok(status_for_state(&state))
}

#[tauri::command]
pub async fn retry_failed_emails(
    app: AppHandle,
    state: tauri::State<'_, DatabaseState>,
) -> Result<EmailNotificationStatus, String> {
    let database = state.inner().clone();
    database
        .connect()?
        .execute(
            "UPDATE email_deliveries SET status='pending',attempts=0,next_attempt_at=NULL,last_error='',updated_at=?1 WHERE status IN ('failed','retrying')",
            [Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    set_meta(&database, "codex_email_last_error", "")?;
    let task_state = database.clone();
    tauri::async_runtime::spawn_blocking(move || process_due_deliveries_for_state(&task_state))
        .await
        .map_err(|error| error.to_string())??;
    sync_tray_menu(&app, &database);
    Ok(status_for_state(&database))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state(name: &str) -> (std::path::PathBuf, DatabaseState) {
        let directory =
            std::env::temp_dir().join(format!("workbench-email-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let state = DatabaseState::new(directory.join("workbench.sqlite3")).unwrap();
        (directory, state)
    }

    #[test]
    fn threshold_uses_actual_beijing_completion_time() {
        assert!(!completion_is_after_threshold(
            "2026-08-07T09:39:59Z",
            "17:40"
        ));
        assert!(completion_is_after_threshold(
            "2026-08-07T09:40:00Z",
            "17:40"
        ));
    }

    #[test]
    fn qq_email_validation_and_masking_are_strict() {
        assert!(valid_qq_email("123456@qq.com"));
        assert!(!valid_qq_email("name@qq.com"));
        assert!(!valid_qq_email("123456@example.com"));
        assert_eq!(masked_email("123456@qq.com"), "12****@qq.com");
    }

    #[test]
    fn codex_output_is_capped_at_two_thousand_characters() {
        let source = "测".repeat(2400);
        assert_eq!(output_excerpt(&source).chars().count(), 2000);
    }

    #[test]
    fn mail_content_contains_project_task_session_and_truncated_output() {
        let delivery = DeliveryMessage {
            notification_id: "codex-complete-turn-1".into(),
            title: "Codex 任务已完成：修复测试中心文本溢出".into(),
            body: "完成页面修复".into(),
            output: "已".repeat(2200),
            source_id: "session-1".into(),
            project: "AI个人工作台".into(),
            completed_at: "2026-08-07T10:26:00Z".into(),
            attempts: 0,
        };
        let (subject, body) = mail_content(&delivery);
        assert_eq!(subject, "[Codex任务完成] 修复测试中心文本溢出");
        assert!(body.contains("项目：AI个人工作台"));
        assert!(body.contains("任务：修复测试中心文本溢出"));
        assert!(body.contains("完成时间：2026-08-07 18:26"));
        assert!(body.contains("会话ID：session-1"));
        let excerpt = body
            .split("Codex输出：\n")
            .nth(1)
            .unwrap()
            .split("\n\n完整信息")
            .next()
            .unwrap();
        assert_eq!(excerpt.chars().count(), 2000);
    }

    #[test]
    fn authentication_errors_do_not_enter_retry_loop() {
        assert!(is_authentication_error("535 Authentication failed"));
        assert_eq!(
            friendly_send_error("535 Authentication failed"),
            "QQ邮箱认证失败，请重新生成或保存 SMTP 授权码。"
        );
    }

    #[test]
    fn delivery_decision_respects_runtime_and_latest_enable_time() {
        assert_eq!(
            delivery_decision(
                "2026-08-07T09:39:59Z",
                "17:40",
                "2026-08-07T09:00:00Z",
                "2026-08-07T09:30:00Z",
                true,
                true,
            )
            .0,
            "skipped_before_time"
        );
        assert_eq!(
            delivery_decision(
                "2026-08-07T09:50:00Z",
                "17:40",
                "2026-08-07T10:00:00Z",
                "2026-08-07T09:30:00Z",
                true,
                true,
            )
            .0,
            "skipped_disabled"
        );
        assert_eq!(
            delivery_decision(
                "2026-08-07T10:00:00Z",
                "17:40",
                "2026-08-07T09:00:00Z",
                "2026-08-07T10:01:00Z",
                true,
                true,
            )
            .0,
            "skipped_disabled"
        );
        assert_eq!(
            delivery_decision(
                "2026-08-07T10:02:00Z",
                "17:40",
                "2026-08-07T09:00:00Z",
                "2026-08-07T10:01:00Z",
                true,
                true,
            )
            .0,
            "pending"
        );
    }

    #[test]
    fn retry_schedule_is_one_five_fifteen_then_failed() {
        assert_eq!(retry_plan(1, false), ("retrying", Some(1)));
        assert_eq!(retry_plan(2, false), ("retrying", Some(5)));
        assert_eq!(retry_plan(3, false), ("retrying", Some(15)));
        assert_eq!(retry_plan(4, false), ("failed", None));
        assert_eq!(retry_plan(1, true), ("failed", None));
    }

    #[test]
    fn disabling_marks_unsent_mail_as_skipped_without_deleting_history() {
        let (directory, state) = test_state("disable");
        let connection = state.connect().unwrap();
        connection.execute("INSERT INTO notifications(id,kind,title,created_at) VALUES('codex-complete-disable','codex_complete','测试','2026-08-07T10:00:00Z')", []).unwrap();
        connection.execute("INSERT INTO email_deliveries(notification_id,status,attempts,next_attempt_at,created_at,updated_at) VALUES('codex-complete-disable','retrying',1,'2026-08-07T10:01:00Z','2026-08-07T10:00:00Z','2026-08-07T10:00:00Z')", []).unwrap();
        drop(connection);

        set_enabled_for_state(&state, false).unwrap();
        let status: String = state
            .connect()
            .unwrap()
            .query_row(
                "SELECT status FROM email_deliveries WHERE notification_id='codex-complete-disable'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "skipped_disabled");

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reconciliation_only_tracks_codex_complete_once() {
        let (directory, state) = test_state("deduplicate");
        initialize_for_state(&state).unwrap();
        set_meta(&state, "codex_email_after_time", "00:00").unwrap();
        let completed_at = (Utc::now() + Duration::minutes(1)).to_rfc3339();
        let connection = state.connect().unwrap();
        connection.execute("INSERT INTO notifications(id,kind,title,created_at) VALUES('codex-complete-once','codex_complete','Codex 完成',?1)", [&completed_at]).unwrap();
        connection.execute("INSERT INTO notifications(id,kind,title,created_at) VALUES('report-ignore','report','报告完成',?1)", [&completed_at]).unwrap();
        drop(connection);

        assert_eq!(reconcile_notifications_for_state(&state).unwrap(), 1);
        assert_eq!(reconcile_notifications_for_state(&state).unwrap(), 0);
        let count: i64 = state
            .connect()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM email_deliveries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);

        std::fs::remove_dir_all(directory).unwrap();
    }
}
