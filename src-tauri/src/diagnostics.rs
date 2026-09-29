//! 本机后台诊断：只记录固定任务名、时间、耗时和错误类别，不保存业务正文或认证信息。
use chrono::Utc;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Instant,
};

const MAX_LOG_BYTES: u64 = 1024 * 1024;
static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static TASKS: Mutex<BTreeMap<String, BackgroundTaskStatus>> = Mutex::new(BTreeMap::new());
static LOG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundTaskStatus {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_success_at: Option<String>,
    pub duration_ms: u64,
    pub error_message: String,
}

pub fn initialize(directory: PathBuf) {
    let _ = DIRECTORY.set(directory);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // panic正文可能含私有对话/URL，因此日志不转存payload，只留固定故障类别。
        append(&BackgroundTaskStatus {
            id: "runtime".into(),
            name: "程序运行".into(),
            finished_at: Some(Utc::now().to_rfc3339()),
            error_message: "程序发生未处理异常".into(),
            ..Default::default()
        });
        previous(info);
    }));
}

fn safe_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("timeout") || error.contains("超时") {
        "执行超时，请检查网络或稍后重试"
    } else if lower.contains("locked") || lower.contains("busy") || error.contains("占用") {
        "资源忙，请稍后重试"
    } else if lower.contains("permission") || lower.contains("denied") || error.contains("权限") {
        "访问被拒绝，请检查目录权限或服务授权"
    } else if lower.contains("401") || lower.contains("unauthorized") || error.contains("登录") {
        "登录状态不可用，请重新检查连接"
    } else if error.contains("重启") {
        "数据恢复后等待重新启动"
    } else {
        "后台任务失败，请在对应模块重试并检查连接状态"
    }
    .into()
}

fn append_to(directory: &Path, status: &BackgroundTaskStatus) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;
    let path = directory.join("background.jsonl");
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() >= MAX_LOG_BYTES) {
        let previous = directory.join("background.previous.jsonl");
        if previous.is_file() {
            fs::remove_file(&previous)?;
        }
        fs::rename(&path, previous)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", serde_json::to_string(status)?)
}

fn append(status: &BackgroundTaskStatus) {
    if let (Some(directory), Ok(_guard)) = (DIRECTORY.get(), LOG_LOCK.lock()) {
        // 日志失败不改变原业务结果，也不让后台线程panic。
        let _ = append_to(directory, status);
    }
}

pub fn run_task<T>(
    id: &str,
    name: &str,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let started = Instant::now();
    if let Ok(mut tasks) = TASKS.lock() {
        let task = tasks.entry(id.into()).or_default();
        task.id = id.into();
        task.name = name.into();
        task.running = true;
        task.started_at = Some(Utc::now().to_rfc3339());
    }
    // 线程异常也要结束“运行中”状态；记录固定说明后继续上抛，不吞掉程序错误。
    let execution = std::panic::catch_unwind(std::panic::AssertUnwindSafe(action));
    let (result, panic) = match execution {
        Ok(result) => (result, None),
        Err(payload) => (Err("后台任务发生未处理异常".into()), Some(payload)),
    };
    let status = TASKS.lock().ok().map(|mut tasks| {
        let task = tasks.entry(id.into()).or_default();
        task.running = false;
        task.finished_at = Some(Utc::now().to_rfc3339());
        task.duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        task.error_message = result
            .as_ref()
            .err()
            .map(|error| safe_error(error))
            .unwrap_or_default();
        if result.is_ok() {
            task.last_success_at = task.finished_at.clone();
        }
        task.clone()
    });
    if let Some(status) = status {
        append(&status);
    }
    if let Some(payload) = panic {
        std::panic::resume_unwind(payload);
    }
    result
}

#[tauri::command]
pub fn list_background_task_status() -> Vec<BackgroundTaskStatus> {
    TASKS
        .lock()
        .map(|tasks| tasks.values().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_never_persist_credentials_urls_or_private_text() {
        for error in [
            "https://user:secret@example.invalid/?token=private network error",
            "Bearer example-key secret conversation",
            "database locked at C:/private/data.db",
            "401 account private@example.invalid",
        ] {
            let output = safe_error(error);
            assert!(
                !output.contains("secret")
                    && !output.contains("private")
                    && !output.contains("example")
            );
        }
    }

    #[test]
    fn log_rotation_is_bounded_and_valid_json() {
        let root =
            std::env::temp_dir().join(format!("astrion-diagnostics-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("background.jsonl"),
            vec![b' '; MAX_LOG_BYTES as usize],
        )
        .unwrap();
        append_to(
            &root,
            &BackgroundTaskStatus {
                id: "test".into(),
                name: "匿名测试".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(root.join("background.previous.jsonl").is_file());
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("background.jsonl")).unwrap())
                .unwrap();
        assert_eq!(value["id"], "test");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn task_failure_recovers_without_losing_last_success_time() {
        run_task("anonymous-test", "测试任务", || Ok(())).unwrap();
        let first = list_background_task_status()
            .into_iter()
            .find(|task| task.id == "anonymous-test")
            .unwrap();
        let _ = run_task("anonymous-test", "测试任务", || {
            Err::<(), _>("timeout secret".into())
        });
        let failure = list_background_task_status()
            .into_iter()
            .find(|task| task.id == "anonymous-test")
            .unwrap();
        assert_eq!(failure.last_success_at, first.last_success_at);
        assert!(!failure.error_message.is_empty());
        run_task("anonymous-test", "测试任务", || Ok(())).unwrap();
        assert!(list_background_task_status()
            .into_iter()
            .find(|task| task.id == "anonymous-test")
            .unwrap()
            .error_message
            .is_empty());
    }

    #[test]
    fn panicked_worker_is_not_left_running_and_does_not_store_payload() {
        let result = std::panic::catch_unwind(|| {
            let _ = run_task::<()>("anonymous-panic", "测试异常", || {
                panic!("private token")
            });
        });
        assert!(result.is_err());
        let status = list_background_task_status()
            .into_iter()
            .find(|task| task.id == "anonymous-panic")
            .unwrap();
        assert!(!status.running);
        assert!(status.finished_at.is_some());
        assert!(!status.error_message.is_empty());
        assert!(!status.error_message.contains("private"));
    }
}
