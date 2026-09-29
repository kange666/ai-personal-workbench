//! Codex 账户只读额度适配。绝不调用 consume/reset，也不读取或转存认证令牌。
use super::{CodexQuotaSnapshot, CodexQuotaWindow};
use chrono::Utc;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::{mpsc, Mutex},
    time::{Duration, Instant, SystemTime},
};

// 头部、健康检查与托盘共享快照，避免同时启动多个查询进程和重复扫描历史。
const CACHE_TTL: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_LINE: u64 = 1024 * 1024;
static CACHE: Mutex<Option<CachedQuota>> = Mutex::new(None);

struct CachedQuota {
    home: PathBuf,
    auth_stamp: Option<(u64, SystemTime)>,
    loaded_at: Instant,
    snapshot: CodexQuotaSnapshot,
}

struct QueryProcess(Child);
impl Drop for QueryProcess {
    fn drop(&mut self) {
        // 仅回收本次查询创建的进程；绝不结束用户已有的 Codex 实例。
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            crate::git::terminate_command_tree(&mut self.0);
        }
    }
}

fn finish_reader(reader: std::thread::JoinHandle<()>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !reader.is_finished() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    // 极端情况下后代进程仍占用管道，也不能再次无期限等待并阻塞额度共享缓存。
    if reader.is_finished() {
        let _ = reader.join();
    }
}

fn live_window(value: &Value, now: i64) -> Option<CodexQuotaWindow> {
    let used_percent = value.get("usedPercent")?.as_f64()?;
    let resets_at = value.get("resetsAt")?.as_i64()?;
    let window_minutes = value.get("windowDurationMins")?.as_i64()?;
    if !used_percent.is_finite() || resets_at <= now || window_minutes <= 0 {
        return None;
    }
    Some(CodexQuotaWindow {
        used_percent: used_percent.clamp(0.0, 100.0),
        remaining_percent: (100.0 - used_percent).clamp(0.0, 100.0),
        window_minutes,
        resets_at,
    })
}

fn snapshot_from_response(response: &Value, now: i64) -> CodexQuotaSnapshot {
    let limits = response.pointer("/rateLimitsByLimitId/codex").or_else(|| {
        response
            .get("rateLimits")
            .filter(|limits| limits.get("limitId").and_then(Value::as_str) == Some("codex"))
    });
    let primary = limits
        .and_then(|value| value.get("primary"))
        .and_then(|value| live_window(value, now));
    let secondary = limits
        .and_then(|value| value.get("secondary"))
        .and_then(|value| live_window(value, now));
    let reset_credits_available = response
        .pointer("/rateLimitResetCredits/availableCount")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    CodexQuotaSnapshot {
        available: primary.is_some() || secondary.is_some(),
        captured_at: Some(Utc::now().to_rfc3339()),
        plan_type: limits
            .and_then(|value| value.get("planType"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        reset_credits_available,
        reset_credits_status: if reset_credits_available.is_some() {
            "available"
        } else {
            "unavailable"
        }
        .into(),
        primary,
        secondary,
        source_file: Some("Codex 账户接口".into()),
        source_modified_at: None,
        freshness: "fresh".into(),
        selection_reason: "通过 Codex 账户只读接口获取；只展示通用 codex 使用限额".into(),
    }
}

fn response_for_id(
    receiver: &mpsc::Receiver<Value>,
    id: u64,
    deadline: Instant,
) -> Result<Value, String> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("Codex 额度读取超时")?;
        let message = receiver
            .recv_timeout(remaining)
            .map_err(|_| "Codex 额度读取超时或服务已退出")?;
        if message.get("id").and_then(Value::as_u64) != Some(id) {
            continue;
        }
        if message.get("error").is_some() {
            // 上游错误可能含 URL、账户和令牌。界面仅返回固定类别说明。
            return Err("Codex 账户接口暂不可用，请检查登录状态和 CLI 版本".into());
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(|| "Codex 额度响应不完整".into());
    }
}

fn read_account_limits(executable: &Path, home: &Path) -> Result<Value, String> {
    let mut child = QueryProcess(
        crate::codex_video::hidden_command(executable)
            .arg("app-server")
            .env("CODEX_HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "无法启动 Codex 额度只读查询")?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("Codex 额度查询未提供响应管道")?;
    let mut stdin = child.0.stdin.take().ok_or("Codex 额度查询未提供输入管道")?;
    let (sender, receiver) = mpsc::sync_channel(8);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let length = (&mut reader).take(MAX_RESPONSE_LINE).read_line(&mut line);
            match length {
                Ok(0) | Err(_) => break,
                Ok(length) if length as u64 >= MAX_RESPONSE_LINE => break,
                _ => {}
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if sender.send(value).is_err() {
                    break;
                }
            }
        }
    });
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let result = (|| {
        writeln!(stdin, "{}", json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"astrion","title":"星枢","version":env!("CARGO_PKG_VERSION")}}}))
            .map_err(|_| "Codex 查询初始化失败")?;
        stdin.flush().map_err(|_| "Codex 查询初始化失败")?;
        response_for_id(&receiver, 1, deadline)?;
        writeln!(stdin, "{}", json!({"method":"initialized"}))
            .map_err(|_| "Codex 查询初始化失败")?;
        writeln!(
            stdin,
            "{}",
            json!({"id":2,"method":"account/rateLimits/read"})
        )
        .map_err(|_| "Codex 额度请求失败")?;
        stdin.flush().map_err(|_| "Codex 额度请求失败")?;
        response_for_id(&receiver, 2, deadline)
    })();
    drop(stdin);
    drop(receiver);
    drop(child);
    finish_reader(reader, Duration::from_millis(200));
    result
}

pub(super) fn latest_quota_snapshot() -> CodexQuotaSnapshot {
    let home = super::codex_home();
    let auth_stamp = std::fs::metadata(home.join("auth.json"))
        .ok()
        .and_then(|metadata| Some((metadata.len(), metadata.modified().ok()?)));
    let mut cache = match CACHE.lock() {
        Ok(cache) => cache,
        Err(_) => {
            return CodexQuotaSnapshot {
                reset_credits_status: "error".into(),
                ..Default::default()
            }
        }
    };
    if let Some(cached) = cache.as_ref().filter(|cached| {
        cached.home == home
            && cached.auth_stamp == auth_stamp
            && cached.loaded_at.elapsed() < CACHE_TTL
    }) {
        let mut snapshot = cached.snapshot.clone();
        let now = Utc::now().timestamp();
        snapshot.primary = snapshot.primary.filter(|window| window.resets_at > now);
        snapshot.secondary = snapshot.secondary.filter(|window| window.resets_at > now);
        snapshot.available = snapshot.primary.is_some() || snapshot.secondary.is_some();
        return snapshot;
    }
    // 直接选独立可执行文件，不额外执行 --version；避免每次刷新多启动一轮进程。
    let executable = crate::codex_video::cli_candidates()
        .into_iter()
        .find(|path| path.is_file());
    let response = executable
        .ok_or_else(|| "未找到独立 Codex CLI".to_string())
        .and_then(|path| read_account_limits(&path, &home));
    let snapshot = match response {
        Ok(response) => snapshot_from_response(&response, Utc::now().timestamp()),
        Err(_) => {
            let mut snapshot = super::latest_log_quota_snapshot();
            snapshot.reset_credits_available = None;
            snapshot.reset_credits_status = "error".into();
            snapshot.selection_reason =
                "账户接口读取失败，通用使用限额降级为本地日志快照；重置次数暂不可用".into();
            snapshot
        }
    };
    *cache = Some(CachedQuota {
        home,
        auth_stamp,
        loaded_at: Instant::now(),
        snapshot: snapshot.clone(),
    });
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_general_bucket_and_authoritative_reset_count() {
        let response = json!({"rateLimits":{"limitId":"codex_bengalfox","primary":{"usedPercent":0,"windowDurationMins":300,"resetsAt":3000}},"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":45,"windowDurationMins":10080,"resetsAt":3000}}},"rateLimitResetCredits":{"availableCount":3,"credits":[]}});
        let snapshot = snapshot_from_response(&response, 1000);
        assert_eq!(snapshot.primary.unwrap().remaining_percent, 55.0);
        assert_eq!(snapshot.reset_credits_available, Some(3));
    }

    #[test]
    fn distinguishes_zero_missing_and_expired_quota() {
        let zero =
            snapshot_from_response(&json!({"rateLimitResetCredits":{"availableCount":0}}), 1000);
        assert_eq!(zero.reset_credits_available, Some(0));
        assert_eq!(zero.reset_credits_status, "available");
        let missing = snapshot_from_response(&json!({"rateLimitResetCredits":null}), 1000);
        assert_eq!(missing.reset_credits_available, None);
        assert_eq!(missing.reset_credits_status, "unavailable");
        let expired = snapshot_from_response(
            &json!({"rateLimits":{"limitId":"codex","primary":{"usedPercent":20,"windowDurationMins":300,"resetsAt":999}}}),
            1000,
        );
        assert!(!expired.available);
        assert!(
            !snapshot_from_response(&json!({"rateLimits":{"limitId":"codex_bengalfox"}}), 1000)
                .available
        );
    }

    #[test]
    fn dismissing_an_announcement_is_never_a_quota_source() {
        let value = json!({"electron-persisted-atom-state":{"rate-limit-reset-home-announcement-dismissal-by-account-id":{"account":{"dismissedAtMs":123,"availableCount":9}}}});
        assert_eq!(
            snapshot_from_response(&value, 1000).reset_credits_available,
            None
        );
    }

    #[test]
    fn response_reader_ignores_notifications_and_sanitizes_errors() {
        let (sender, receiver) = mpsc::channel();
        sender.send(json!({"method":"account/updated"})).unwrap();
        sender
            .send(json!({"id":2,"result":{"rateLimitResetCredits":{"availableCount":0}}}))
            .unwrap();
        assert!(response_for_id(&receiver, 2, Instant::now() + Duration::from_secs(1)).is_ok());
        sender
            .send(json!({"id":3,"error":{"message":"sensitive upstream response"}}))
            .unwrap();
        let error =
            response_for_id(&receiver, 3, Instant::now() + Duration::from_secs(1)).unwrap_err();
        assert!(!error.contains("sensitive"));
        assert!(response_for_id(&receiver, 4, Instant::now())
            .unwrap_err()
            .contains("超时"));
    }

    #[test]
    fn inherited_pipe_cannot_block_query_cleanup() {
        let (sender, receiver) = mpsc::channel::<()>();
        let reader = std::thread::spawn(move || {
            let _ = receiver.recv();
        });
        let started = Instant::now();
        finish_reader(reader, Duration::from_millis(20));
        let elapsed = started.elapsed();
        drop(sender);
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    #[ignore = "显式运行的本机只读账户接口检查；不重置额度，不输出账户或令牌"]
    fn live_account_quota_smoke() {
        let path = crate::codex_video::cli_candidates()
            .into_iter()
            .find(|path| path.is_file())
            .expect("Codex CLI");
        let response = read_account_limits(&path, &super::super::codex_home()).expect("read quota");
        let snapshot = snapshot_from_response(&response, Utc::now().timestamp());
        assert!(snapshot.available, "需要可用的通用额度");
        println!(
            "live quota verified; reset count present={}",
            snapshot.reset_credits_available.is_some()
        );
    }
}
