use crate::{database::DatabaseState, notifications, videos};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use walkdir::WalkDir;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexCliStatus {
    installed: bool,
    authenticated: bool,
    version: String,
    executable_path: String,
    message: String,
}

#[derive(Debug)]
struct IdeaPayload {
    id: String,
    content_type: String,
    category: String,
    title: String,
    hook: String,
    script: String,
    storyboard: String,
    visual_prompts: String,
    editing_guide: String,
    cover_title: String,
}

#[derive(Debug)]
struct PreparedJob {
    id: String,
    project_root: PathBuf,
    prompt: String,
    existing_thread_id: Option<String>,
}

pub(crate) fn hidden_command(path: &Path) -> Command {
    let mut command = Command::new(path);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn cli_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("CODEX_CLI_PATH") {
        candidates.push(PathBuf::from(path));
    }
    let mut prefixes = Vec::new();
    if let Some(path) = env::var_os("NVM_SYMLINK") {
        prefixes.push(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("APPDATA") {
        prefixes.push(PathBuf::from(path).join("npm"));
    }
    if let Some(path) = env::var_os("PATH") {
        prefixes.extend(env::split_paths(&path));
    }
    prefixes.push(PathBuf::from(r"C:\nvm4w\nodejs"));
    for prefix in prefixes {
        candidates.push(
            prefix
                .join("node_modules")
                .join("@openai")
                .join("codex")
                .join("node_modules")
                .join("@openai")
                .join("codex-win32-x64")
                .join("vendor")
                .join("x86_64-pc-windows-msvc")
                .join("bin")
                .join("codex.exe"),
        );
        let direct = prefix.join("codex.exe");
        if !direct
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("windowsapps")
        {
            candidates.push(direct);
        }
    }
    let mut unique = Vec::new();
    for candidate in candidates {
        if !unique.contains(&candidate) {
            unique.push(candidate);
        }
    }
    unique
}

pub(crate) fn resolve_codex_cli() -> Result<(PathBuf, String), String> {
    for candidate in cli_candidates().into_iter().filter(|path| path.is_file()) {
        let output = hidden_command(&candidate).arg("--version").output();
        if let Ok(output) = output {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok((candidate, version));
            }
        }
    }
    Err("未找到可调用的独立 Codex CLI。请重新安装 @openai/codex。".into())
}

#[tauri::command]
pub fn codex_cli_status() -> CodexCliStatus {
    let Ok((path, version)) = resolve_codex_cli() else {
        return CodexCliStatus {
            installed: false,
            authenticated: false,
            version: String::new(),
            executable_path: String::new(),
            message: "未找到独立 Codex CLI。".into(),
        };
    };
    let login = hidden_command(&path).args(["login", "status"]).output();
    let authenticated = login.as_ref().is_ok_and(|output| output.status.success());
    CodexCliStatus {
        installed: true,
        authenticated,
        version,
        executable_path: path.display().to_string(),
        message: if authenticated {
            "Codex CLI 已安装并登录。".into()
        } else {
            "Codex CLI 已安装，但尚未登录。请先在终端运行 codex login。".into()
        },
    }
}

fn safe_folder_name(title: &str, idea_id: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut name = title
        .chars()
        .filter(|character| !character.is_control() && !invalid.contains(character))
        .take(52)
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_string();
    if name.is_empty() {
        name = "Codex视频任务".into();
    }
    let suffix = idea_id
        .chars()
        .filter(|value| value.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    if suffix.is_empty() {
        name
    } else {
        format!("{name}-{suffix}")
    }
}

fn read_idea(state: &DatabaseState, idea_id: &str) -> Result<IdeaPayload, String> {
    state
        .connect()?
        .query_row(
            "SELECT id,content_type,category,title,hook,script,storyboard,visual_prompts,editing_guide,cover_title
             FROM content_ideas WHERE id=?1 AND status IN ('selected','published')",
            [idea_id],
            |row| {
                Ok(IdeaPayload {
                    id: row.get(0)?, content_type: row.get(1)?, category: row.get(2)?, title: row.get(3)?,
                    hook: row.get(4)?, script: row.get(5)?, storyboard: row.get(6)?, visual_prompts: row.get(7)?,
                    editing_guide: row.get(8)?, cover_title: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "请先在内容工坊选择这条内容，再启动视频制作。".to_string())
}

fn build_prompt(idea: &IdeaPayload, skill_name: &str, project_root: &Path) -> String {
    format!(
        "标题：《{}》\n\n请使用 ${} 完成这条视频。你由星枢 ASTRION 启动，请读取并严格执行该 Skill 的完整 SKILL.md。下面的选题、脚本、分镜和视觉方向已经由用户在内容工坊选定，不要重新做选题发散或重复输出方案；仅在逻辑错误或硬性质检要求需要时修正文案，然后直接进入素材生成、合成、渲染和验收。\n\n固定输出目录：{}\n所有脚本、素材、工程文件和最终交付都必须保存在该目录中；不要修改其他项目。完成真实制作与质检，不要只给方案。工作台已获得用户授权，可在本任务中启动本地 Chrome、FFmpeg、配音和截图子进程。\n\n内容类型：{}\n分类：{}\n封面标题：{}\n\n## 3秒钩子\n{}\n\n## 完整口播\n{}\n\n## 分镜脚本\n{}\n\n## AI画面提示词\n{}\n\n## 剪辑指导\n{}\n\n最终回复必须列出：最终 MP4、竖屏封面、完整脚本、发布标题/配文/置顶评论和质检报告的绝对路径；若任何交付未完成，明确写出失败原因和可继续执行的下一步。",
        idea.title,
        skill_name,
        project_root.display(),
        if idea.content_type == "reasoning" { "推理案例" } else { "科技探索" },
        idea.category,
        idea.cover_title,
        idea.hook,
        idea.script,
        idea.storyboard,
        idea.visual_prompts,
        idea.editing_guide,
    )
}

fn prepare_job(state: &DatabaseState, idea_id: &str) -> Result<PreparedJob, String> {
    let (cli_path, _) = resolve_codex_cli()?;
    let login = hidden_command(&cli_path)
        .args(["login", "status"])
        .output()
        .map_err(|error| error.to_string())?;
    if !login.status.success() {
        return Err("Codex CLI 尚未登录，请先运行 codex login。".into());
    }
    let idea = read_idea(state, idea_id)?;
    let skill_name = if idea.content_type == "reasoning" {
        "generate-reasoning-short-video"
    } else {
        "generate-tech-short-video"
    }
    .to_string();
    let previous = state.connect()?.query_row(
        "SELECT id,status,project_root,codex_thread_id FROM video_jobs WHERE content_idea_id=?1 LIMIT 1",
        [idea_id],
        |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,Option<String>>(3)?)),
    ).optional().map_err(|error| error.to_string())?;
    if previous.as_ref().is_some_and(|(_, status, _, _)| {
        matches!(status.as_str(), "queued" | "running" | "finalizing")
    }) {
        return Err("该内容的 Codex 视频任务正在执行，请勿重复启动。".into());
    }
    let project_root = previous
        .as_ref()
        .map(|(_, _, root, _)| PathBuf::from(root))
        .unwrap_or_else(|| {
            videos::creation_root()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(safe_folder_name(&idea.title, &idea.id))
        });
    fs::create_dir_all(&project_root).map_err(|error| format!("创建视频项目目录失败：{error}"))?;
    let job_id = previous
        .as_ref()
        .map(|(id, _, _, _)| id.clone())
        .unwrap_or_else(|| {
            format!(
                "video-job:{}",
                project_root.to_string_lossy().to_lowercase()
            )
        });
    let existing_thread_id = previous.and_then(|(_, _, _, thread)| thread);
    let prompt = build_prompt(&idea, &skill_name, &project_root);
    fs::write(project_root.join("工作台制作任务.md"), &prompt)
        .map_err(|error| format!("写入制作任务失败：{error}"))?;
    let now = Utc::now().to_rfc3339();
    state.connect()?.execute(
        "INSERT INTO video_jobs(id,title,video_type,status,current_stage,progress_percent,progress_message,last_progress_at,project_root,failure_reason,manually_confirmed_type,content_idea_id,skill_name,codex_thread_id,codex_output,cli_log_path,started_at,completed_at,created_at,updated_at)
         VALUES(?1,?2,?3,'queued','selection',0,'等待 Codex 启动',?8,?4,'',1,?5,?6,?7,'','',NULL,NULL,?8,?8)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title,video_type=excluded.video_type,status='queued',current_stage='selection',progress_percent=0,progress_message='等待 Codex 启动',last_progress_at=excluded.last_progress_at,project_root=excluded.project_root,failure_reason='',manually_confirmed_type=1,content_idea_id=excluded.content_idea_id,skill_name=excluded.skill_name,codex_output='',cli_log_path='',started_at=NULL,completed_at=NULL,updated_at=excluded.updated_at",
        params![job_id,idea.title,idea.content_type,project_root.display().to_string(),idea.id,skill_name,existing_thread_id,now],
    ).map_err(|error| error.to_string())?;
    Ok(PreparedJob {
        id: job_id,
        project_root,
        prompt,
        existing_thread_id,
    })
}

fn update_job_failure(state: &DatabaseState, job_id: &str, message: &str) {
    let now = Utc::now().to_rfc3339();
    let _ = state.connect().and_then(|connection| connection.execute(
        "UPDATE video_jobs SET status='failed',current_stage='failed',progress_message='执行失败，请查看原因',failure_reason=?1,completed_at=?2,updated_at=?2,last_progress_at=?2 WHERE id=?3",
        params![message,now,job_id],
    ).map(|_| ()).map_err(|error| error.to_string()));
}

fn update_progress(state: &DatabaseState, job_id: &str, stage: &str, percent: i64, message: &str) {
    let now = Utc::now().to_rfc3339();
    let _ = state.connect().and_then(|connection| {
        connection
            .execute(
                "UPDATE video_jobs SET current_stage=?1,progress_percent=?2,progress_message=?3,last_progress_at=?4,updated_at=?4 WHERE id=?5 AND progress_percent<=?2",
                params![stage, percent.clamp(0, 100), message, now, job_id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    });
}

fn progress_from_project(project_root: &Path) -> (&'static str, i64, &'static str) {
    let mut has_script = false;
    let mut has_image = false;
    let mut has_voice = false;
    let mut has_composition = false;
    let mut has_quality = false;
    let mut has_video = false;
    for entry in WalkDir::new(project_root)
        .max_depth(7)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .as_str(),
                "node_modules" | ".git" | ".workbench"
            )
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        has_script |= matches!(name.as_str(), "script.md" | "storyboard.md" | "brief.md");
        has_image |= matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp");
        has_voice |= matches!(extension.as_str(), "mp3" | "wav" | "srt");
        has_composition |= name == "hyperframes.json" || name == "index.html";
        has_quality |= name.contains("qa_report")
            || name.contains("quality-report")
            || name.contains("质检报告");
        has_video |= extension == "mp4";
    }
    if has_video {
        ("render", 96, "最终视频已生成，正在验收")
    } else if has_quality {
        ("quality", 84, "正在执行画面与结构质检")
    } else if has_composition {
        ("composition", 70, "正在搭建并检查视频工程")
    } else if has_voice {
        ("voice", 58, "正在生成配音与字幕")
    } else if has_image {
        ("assets", 42, "正在生成画面与素材")
    } else if has_script {
        ("script", 25, "正在校验题目与整理脚本")
    } else {
        ("codex", 10, "正在读取制作规范")
    }
}

fn text_tail(value: &str, max_chars: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    chars[chars.len().saturating_sub(max_chars)..]
        .iter()
        .collect()
}

fn npm_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("NVM_SYMLINK") {
        candidates.push(PathBuf::from(path).join("npm.cmd"));
    }
    if let Some(path) = env::var_os("PATH") {
        candidates.extend(env::split_paths(&path).map(|prefix| prefix.join("npm.cmd")));
    }
    candidates.push(PathBuf::from(r"C:\nvm4w\nodejs\npm.cmd"));
    candidates.push(PathBuf::from(r"C:\Program Files\nodejs\npm.cmd"));
    candidates.into_iter().fold(Vec::new(), |mut result, item| {
        if !result.contains(&item) {
            result.push(item);
        }
        result
    })
}

fn resolve_npm_cmd() -> Result<PathBuf, String> {
    npm_candidates()
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "未找到 npm.cmd，无法在工作台主进程中完成 HyperFrames 渲染。".to_string())
}

fn find_hyperframes_project(root: &Path) -> Option<PathBuf> {
    WalkDir::new(root)
        .max_depth(7)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .as_str(),
                "node_modules" | ".git" | ".workbench"
            )
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.join("hyperframes.json").is_file()
                && path.join("package.json").is_file()
                && path.join("index.html").is_file()
        })
        .max_by_key(|path| path.components().count())
}

fn find_final_video(root: &Path) -> Option<PathBuf> {
    WalkDir::new(root)
        .max_depth(8)
        .into_iter()
        .filter_entry(|entry| {
            !matches!(
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .as_str(),
                "node_modules" | ".git" | ".workbench" | "assets"
            )
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("mp4"))
                && entry.metadata().is_ok_and(|value| value.len() > 1_000_000)
        })
        .map(|entry| entry.into_path())
        .max_by_key(|path| {
            let value = path.to_string_lossy().to_ascii_lowercase();
            usize::from(
                value.contains("video-final")
                    || value.contains("final-video")
                    || value.contains("最终")
                    || value.contains("成片"),
            ) * 10
                + usize::from(value.contains("render") || value.contains("deliver")) * 5
        })
}

fn run_npm_step(
    npm: &Path,
    project: &Path,
    args: &[&str],
    log_path: &Path,
    label: &str,
) -> Result<(), String> {
    let output = hidden_command(npm)
        .current_dir(project)
        .args(args)
        .output()
        .map_err(|error| format!("启动 {label} 失败：{error}"))?;
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|error| error.to_string())?;
    writeln!(
        log,
        "\n===== {label} =====\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Err(format!(
            "{label} 未通过：{}",
            text_tail(detail.trim(), 1800)
        ))
    }
}

fn finalize_hyperframes_on_host(
    state: &DatabaseState,
    job_id: &str,
    root: &Path,
    workbench_dir: &Path,
) -> Result<Option<PathBuf>, String> {
    if let Some(video) = find_final_video(root) {
        return Ok(Some(video));
    }
    let Some(project) = find_hyperframes_project(root) else {
        return Ok(None);
    };
    let npm = resolve_npm_cmd()?;
    let log_path = workbench_dir.join("host-hyperframes.log");
    update_progress(
        state,
        job_id,
        "quality",
        84,
        "主进程正在检查 HyperFrames 工程",
    );
    run_npm_step(
        &npm,
        &project,
        &["run", "check", "--", "--snapshots", "--frame-check"],
        &log_path,
        "HyperFrames 质量检查",
    )?;
    update_progress(
        state,
        job_id,
        "render",
        91,
        "检查通过，主进程正在渲染 1080×1920 成片",
    );
    let render_dir = project.join("renders");
    fs::create_dir_all(&render_dir).map_err(|error| format!("创建渲染目录失败：{error}"))?;
    let output_path = render_dir.join("video-final.mp4");
    let output_text = output_path.to_string_lossy().to_string();
    run_npm_step(
        &npm,
        &project,
        &[
            "run",
            "render",
            "--",
            "--quality",
            "high",
            "--fps",
            "30",
            "--workers",
            "1",
            "--output",
            &output_text,
        ],
        &log_path,
        "HyperFrames 成片渲染",
    )?;
    if !output_path.is_file()
        || fs::metadata(&output_path)
            .map(|value| value.len())
            .unwrap_or(0)
            <= 1_000_000
    {
        return Err("HyperFrames 返回成功，但未生成有效的最终 MP4。".into());
    }
    update_progress(
        state,
        job_id,
        "render",
        96,
        "最终视频已生成，正在同步视频中心",
    );
    Ok(Some(output_path))
}

fn run_job(state: DatabaseState, job: PreparedJob) -> Result<(), String> {
    let (cli_path, _) = resolve_codex_cli()?;
    let workbench_dir = job.project_root.join(".workbench");
    fs::create_dir_all(&workbench_dir).map_err(|error| error.to_string())?;
    let jsonl_path = workbench_dir.join("codex-run.jsonl");
    let stderr_path = workbench_dir.join("codex-stderr.log");
    let last_message_path = workbench_dir.join("codex-last-message.md");
    let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    state.connect()?.execute(
        "UPDATE video_jobs SET status='running',current_stage='codex',progress_percent=8,progress_message='Codex 已启动',last_progress_at=?2,cli_log_path=?1,started_at=?2,updated_at=?2 WHERE id=?3",
        params![jsonl_path.display().to_string(),now,job.id],
    ).map_err(|error| error.to_string())?;

    let mut command = hidden_command(&cli_path);
    command
        .args([
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "never",
            "--cd",
        ])
        .arg(&job.project_root)
        .arg("exec");
    if let Some(thread_id) = job.existing_thread_id.as_deref() {
        command
            .args(["resume", "--all", "--json", "--skip-git-repo-check"])
            .arg("--output-last-message")
            .arg(&last_message_path)
            .arg(thread_id)
            .arg("-");
    } else {
        command
            .args(["--json", "--skip-git-repo-check"])
            .arg("--output-last-message")
            .arg(&last_message_path)
            .arg("-");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|error| format!("启动 Codex CLI 失败：{error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(job.prompt.as_bytes())
            .map_err(|error| format!("发送任务给 Codex 失败：{error}"))?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Codex CLI 输出。".to_string())?;
    let mut log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&jsonl_path)
        .map_err(|error| error.to_string())?;
    let mut thread_id = job.existing_thread_id.clone();
    let mut streamed_output = String::new();
    let mut last_progress_check = Instant::now() - Duration::from_secs(2);
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        writeln!(log, "{line}").map_err(|error| error.to_string())?;
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("type").and_then(Value::as_str) == Some("thread.started") {
                if let Some(value) = value.get("thread_id").and_then(Value::as_str) {
                    thread_id = Some(value.to_string());
                    let _ = state.connect()?.execute(
                        "UPDATE video_jobs SET codex_thread_id=?1,updated_at=?2 WHERE id=?3",
                        params![value, Utc::now().to_rfc3339(), job.id],
                    );
                }
            }
            if value.get("type").and_then(Value::as_str) == Some("item.completed")
                && value.pointer("/item/type").and_then(Value::as_str) == Some("agent_message")
            {
                if let Some(text) = value.pointer("/item/text").and_then(Value::as_str) {
                    streamed_output = text.to_string();
                }
            }
        }
        if last_progress_check.elapsed() >= Duration::from_secs(2) {
            let (stage, percent, message) = progress_from_project(&job.project_root);
            update_progress(&state, &job.id, stage, percent, message);
            last_progress_check = Instant::now();
        }
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    let final_output = fs::read_to_string(&last_message_path)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(streamed_output);
    let finished_at = Utc::now().to_rfc3339();
    if !status.success() {
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        let detail = text_tail(stderr.trim(), 1800);
        let codex_message = if detail.is_empty() {
            format!(
                "Codex CLI 执行失败，退出码：{}",
                status.code().unwrap_or(-1)
            )
        } else {
            detail
        };
        match finalize_hyperframes_on_host(&state, &job.id, &job.project_root, &workbench_dir) {
            Ok(Some(_)) => {
                state.connect()?.execute(
                    "UPDATE video_jobs SET status='finalizing',current_stage='render',progress_percent=96,progress_message='主进程已补全成片，正在同步视频中心',failure_reason='',codex_output=?1,codex_thread_id=COALESCE(?2,codex_thread_id),updated_at=?3,last_progress_at=?3 WHERE id=?4",
                    params![final_output,thread_id,finished_at,job.id],
                ).map_err(|error| error.to_string())?;
                let _ = videos::sync_video_pipeline_for_state(&state);
                let _ = notifications::sync_codex_notifications_for_state(&state);
                return Ok(());
            }
            Ok(None) => {}
            Err(host_error) => {
                let message = format!("{codex_message}\n\n工作台主进程补全失败：{host_error}");
                state.connect()?.execute(
                    "UPDATE video_jobs SET status='failed',current_stage='failed',progress_message='执行失败，请查看原因',failure_reason=?1,codex_output=?2,codex_thread_id=COALESCE(?3,codex_thread_id),completed_at=?4,updated_at=?4,last_progress_at=?4 WHERE id=?5",
                    params![message,final_output,thread_id,finished_at,job.id],
                ).map_err(|error| error.to_string())?;
                let _ = notifications::sync_codex_notifications_for_state(&state);
                return Err(message);
            }
        }
        state.connect()?.execute(
            "UPDATE video_jobs SET status='failed',current_stage='failed',progress_message='执行失败，请查看原因',failure_reason=?1,codex_output=?2,codex_thread_id=COALESCE(?3,codex_thread_id),completed_at=?4,updated_at=?4,last_progress_at=?4 WHERE id=?5",
            params![codex_message,final_output,thread_id,finished_at,job.id],
        ).map_err(|error| error.to_string())?;
        let _ = notifications::sync_codex_notifications_for_state(&state);
        return Err(codex_message);
    }
    state.connect()?.execute(
        "UPDATE video_jobs SET status='finalizing',current_stage='finalizing',progress_percent=82,progress_message='Codex 已完成，正在由主进程验收交付',last_progress_at=?3,failure_reason='',codex_output=?1,codex_thread_id=COALESCE(?2,codex_thread_id),completed_at=NULL,updated_at=?3 WHERE id=?4",
        params![final_output,thread_id,finished_at,job.id],
    ).map_err(|error| error.to_string())?;
    let rendered =
        finalize_hyperframes_on_host(&state, &job.id, &job.project_root, &workbench_dir)?;
    let _ = videos::sync_video_pipeline_for_state(&state);
    if let Some(current) = videos::video_job_by_id_for_state(&state, &job.id)? {
        if matches!(current.status(), "queued" | "running" | "finalizing") {
            let reason = if rendered.is_some() {
                "主进程已完成渲染，但视频中心未能识别成片，请查看主进程渲染日志。"
            } else {
                "Codex 已结束，但未发现可由工作台主进程渲染的 HyperFrames 工程或最终成片。"
            };
            state.connect()?.execute(
                "UPDATE video_jobs SET status='needs-attention',current_stage='render',progress_percent=97,progress_message='成片未完成识别，请查看详情',failure_reason=?1,last_progress_at=?2,updated_at=?2 WHERE id=?3",
                params![reason,Utc::now().to_rfc3339(),job.id],
            ).map_err(|error| error.to_string())?;
        }
    }
    let _ = notifications::sync_codex_notifications_for_state(&state);
    Ok(())
}

#[tauri::command]
pub fn content_video_job(
    state: tauri::State<'_, DatabaseState>,
    idea_id: String,
) -> Result<Option<videos::VideoJob>, String> {
    let id = state
        .connect()?
        .query_row(
            "SELECT id FROM video_jobs WHERE content_idea_id=?1 LIMIT 1",
            [&idea_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    id.map(|value| videos::video_job_by_id_for_state(&state, &value))
        .transpose()
        .map(|value| value.flatten())
}

#[tauri::command]
pub fn start_content_video_job(
    state: tauri::State<'_, DatabaseState>,
    idea_id: String,
) -> Result<videos::VideoJob, String> {
    let database = state.inner().clone();
    let job = prepare_job(&database, &idea_id)?;
    let job_id = job.id.clone();
    let task_job_id = job_id.clone();
    let task_state = database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_job(task_state.clone(), job) {
            update_job_failure(&task_state, &task_job_id, &error);
        }
    });
    videos::video_job_by_id_for_state(&database, &job_id)?
        .ok_or_else(|| "视频任务创建后未能读取。".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_name_removes_windows_invalid_characters() {
        let value = safe_folder_name("AI:未来/测试？*", "idea-12345678");
        assert!(!value.contains([':', '/', '?', '*']));
        assert!(value.contains("idea1234"));
    }

    #[test]
    fn prompt_explicitly_invokes_the_selected_skill() {
        let idea = IdeaPayload {
            id: "1".into(),
            content_type: "tech".into(),
            category: "AI".into(),
            title: "测试".into(),
            hook: "钩子".into(),
            script: "脚本".into(),
            storyboard: "分镜".into(),
            visual_prompts: "画面".into(),
            editing_guide: "剪辑".into(),
            cover_title: "封面".into(),
        };
        let prompt = build_prompt(&idea, "generate-tech-short-video", Path::new(r"C:\video"));
        assert!(prompt.contains("$generate-tech-short-video"));
        assert!(prompt.contains("不要只给方案"));
        assert!(prompt.contains("不要重新做选题发散"));
    }

    #[test]
    fn project_files_drive_real_progress_stages() {
        let root =
            std::env::temp_dir().join(format!("workbench-video-progress-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("output")).unwrap();
        fs::write(root.join("SCRIPT.md"), "脚本").unwrap();
        assert_eq!(progress_from_project(&root).1, 25);
        fs::write(root.join("index.html"), "<html></html>").unwrap();
        assert_eq!(progress_from_project(&root).1, 70);
        fs::write(root.join("output").join("final.mp4"), b"video").unwrap();
        assert_eq!(progress_from_project(&root).1, 96);
        fs::remove_dir_all(root).unwrap();
    }
}
