use crate::database::DatabaseState;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoItem {
    id: String,
    title: String,
    project: String,
    path: String,
    folder: String,
    source_root: String,
    file_name: String,
    extension: String,
    size_bytes: u64,
    modified_at: String,
    status: String,
    cover_path: Option<String>,
    collection: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDeliverable {
    kind: String,
    label: String,
    path: Option<String>,
    file_name: Option<String>,
    content: Option<String>,
    available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoProjectDetails {
    project_root: String,
    deliverables: Vec<VideoDeliverable>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoJobDeliverable {
    kind: String,
    path: String,
    status: String,
    quality_summary: String,
    checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoJob {
    id: String,
    title: String,
    video_type: String,
    status: String,
    current_stage: String,
    progress_percent: i64,
    progress_message: String,
    last_progress_at: Option<String>,
    project_root: String,
    failure_reason: String,
    manually_confirmed_type: bool,
    content_idea_id: Option<String>,
    skill_name: String,
    codex_thread_id: Option<String>,
    codex_output: String,
    cli_log_path: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
    deliverables: Vec<VideoJobDeliverable>,
}

impl VideoJob {
    pub(crate) fn status(&self) -> &str {
        &self.status
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoPipelineSummary {
    job_count: usize,
    complete_count: usize,
    needs_attention_count: usize,
    tech_samples: usize,
    reasoning_samples: usize,
    human_weakness_samples: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVideoType {
    id: String,
    video_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoPublishRecord {
    id: String,
    video_job_id: String,
    title: String,
    video_type: String,
    platform: String,
    status: String,
    publish_url: String,
    published_at: Option<String>,
    views: i64,
    likes: i64,
    comments: i64,
    favorites: i64,
    notes: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveVideoPublishRecord {
    video_job_id: String,
    platform: String,
    status: String,
    publish_url: String,
    published_at: Option<String>,
    views: i64,
    likes: i64,
    comments: i64,
    favorites: i64,
    notes: String,
}

pub(crate) fn creation_root() -> Result<PathBuf, String> {
    let profile =
        env::var_os("USERPROFILE").ok_or_else(|| "无法读取 Windows 用户目录。".to_string())?;
    Ok(PathBuf::from(profile).join("Documents").join("视频创作"))
}

fn source_roots() -> Result<Vec<(PathBuf, &'static str)>, String> {
    let root = creation_root()?;
    Ok(vec![
        (root.join("videos"), "视频创作 / videos"),
        (root, "视频创作"),
    ])
}

fn supported_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("mp4" | "mov" | "mkv" | "webm" | "avi")
    )
}

fn supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp")
    )
}

fn is_intermediate(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    matches!(
        name.as_str(),
        "node_modules"
            | ".tools"
            | ".media"
            | ".hyperframes"
            | "capture"
            | "assets"
            | "compositions"
    ) || name == "work"
        || name.starts_with(".work")
        || name == ".skill-test"
}

fn project_for(path: &Path, root: &Path) -> (String, PathBuf) {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let first = relative
        .components()
        .find_map(|value| match value {
            Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "未归类".to_string());
    (first.clone(), root.join(first))
}

fn display_title(file_name: &str, project: &str) -> String {
    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    if matches!(
        stem.to_ascii_lowercase().as_str(),
        "video" | "final" | "output"
    ) {
        project.to_string()
    } else {
        stem.replace('_', " ").replace('-', " ")
    }
}

fn video_status(file_name: &str, path: &Path) -> String {
    let value = file_name.to_ascii_lowercase();
    if value.contains("最终")
        || value.contains("成片")
        || value.contains("final")
        || value.contains("clean")
        || value.contains("完美")
    {
        "final".to_string()
    } else if path.components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("output")
    }) {
        "output".to_string()
    } else {
        "render".to_string()
    }
}

fn find_cover(project_root: &Path) -> Option<PathBuf> {
    let mut fallback = None;
    for entry in WalkDir::new(project_root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|entry| !is_intermediate(entry))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || !supported_image(path) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.contains("封面") || name.contains("cover") || name.contains("thumbnail") {
            return Some(path.to_path_buf());
        }
        if fallback.is_none() && (name.contains("preview") || name.contains("intro")) {
            fallback = Some(path.to_path_buf());
        }
    }
    fallback
}

fn find_final_video(project_root: &Path, selected: &Path) -> PathBuf {
    WalkDir::new(project_root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|entry| !is_intermediate(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && supported_video(entry.path()))
        .filter_map(|entry| {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|value| value.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let path = entry.into_path();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if name.starts_with("clip") || name == "silent.mp4" {
                return None;
            }
            let mut score = 0;
            if name.contains("最终") || name.contains("成片") || name.contains("final") {
                score += 100;
            }
            if name.contains("clean") || name.contains("完美") {
                score += 20;
            }
            if path.components().any(|part| {
                let value = part.as_os_str().to_string_lossy();
                value.eq_ignore_ascii_case("output") || value.eq_ignore_ascii_case("renders")
            }) {
                score += 10;
            }
            Some((score, modified, path))
        })
        .max_by(
            |(score_a, modified_a, path_a), (score_b, modified_b, path_b)| {
                score_a
                    .cmp(score_b)
                    .then_with(|| modified_a.cmp(modified_b))
                    .then_with(|| path_a.cmp(path_b))
            },
        )
        .map(|(_, _, path)| path)
        .unwrap_or_else(|| selected.to_path_buf())
}

fn project_root_for_video(video_path: &Path) -> Result<PathBuf, String> {
    let root =
        fs::canonicalize(creation_root()?).map_err(|_| "视频创作目录不存在。".to_string())?;
    let videos_root = root.join("videos");
    let base = if videos_root.exists() {
        let canonical_videos = fs::canonicalize(&videos_root).unwrap_or(videos_root);
        if video_path.starts_with(&canonical_videos) {
            canonical_videos
        } else {
            root
        }
    } else {
        root
    };
    let has_delivery_metadata = |directory: &Path| {
        fs::read_dir(directory).ok().is_some_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                    return false;
                }
                matches!(
                    entry
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .as_str(),
                    "publish.md"
                        | "publishing.md"
                        | "script.md"
                        | "完整脚本.md"
                        | "copy.md"
                        | "title.txt"
                )
            })
        })
    };
    let mut candidate = video_path.parent();
    while let Some(directory) = candidate {
        if !directory.starts_with(&base) {
            break;
        }
        if has_delivery_metadata(directory) {
            return Ok(directory.to_path_buf());
        }
        if directory == base {
            break;
        }
        candidate = directory.parent();
    }
    let relative = video_path.strip_prefix(&base).unwrap_or(video_path);
    let first = relative.components().find_map(|value| match value {
        Component::Normal(name) => Some(name),
        _ => None,
    });
    match first {
        Some(name) if relative.components().count() > 1 => Ok(base.join(name)),
        _ => Ok(video_path.parent().unwrap_or(&base).to_path_buf()),
    }
}

fn classify_video_type(title: &str, project_root: &Path) -> &'static str {
    let text = format!("{} {}", title, project_root.display()).to_lowercase();
    if text.contains("human-weakness")
        || text.contains("人性弱点")
        || text.contains("人性的弱点")
        || text.contains("海绵宝宝")
        || text.contains("spongebob")
    {
        "human-weakness"
    } else if text.contains("reasoning")
        || text.contains("推理")
        || text.contains("逻辑")
        || text.contains("who-lied")
        || text.contains("绳子")
    {
        "reasoning"
    } else {
        "tech"
    }
}

fn stage_for(deliverables: &[VideoDeliverable]) -> (&'static str, &'static str) {
    for (kind, stage) in [
        ("script", "script"),
        ("video", "render"),
        ("cover", "cover"),
        ("publish", "publish"),
    ] {
        if deliverables
            .iter()
            .find(|item| item.kind == kind)
            .is_none_or(|item| !item.available)
        {
            return ("needs-attention", stage);
        }
    }
    ("complete", "delivery")
}

fn deliverable_quality(item: &VideoDeliverable) -> String {
    if !item.available {
        return "未找到交付文件".into();
    }
    if let Some(content) = item.content.as_deref() {
        let chars = content.chars().count();
        return format!("文本可读 · {chars} 字符");
    }
    let size = item
        .path
        .as_deref()
        .and_then(|path| fs::metadata(path).ok())
        .map(|value| value.len())
        .unwrap_or_default();
    format!("文件可读 · {:.1} MB", size as f64 / 1_048_576_f64)
}

fn normalized_project_root(value: &str) -> String {
    value
        .trim_start_matches(r"\\?\")
        .replace('/', r"\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn content_job_for_root(
    state: &DatabaseState,
    project_root: &str,
) -> Result<Option<String>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT id,project_root FROM video_jobs WHERE content_idea_id IS NOT NULL")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;
    let expected = normalized_project_root(project_root);
    for row in rows {
        let (id, root) = row.map_err(|error| error.to_string())?;
        if normalized_project_root(&root) == expected {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

pub fn sync_video_pipeline_for_state(
    state: &DatabaseState,
) -> Result<VideoPipelineSummary, String> {
    state.connect()?.execute(
        "UPDATE video_jobs SET video_type='tech',manually_confirmed_type=0 WHERE video_type='product-demo'",
        [],
    ).map_err(|error| error.to_string())?;
    let stale_before = (Utc::now() - chrono::Duration::minutes(15)).to_rfc3339();
    let recovered_at = Utc::now().to_rfc3339();
    state.connect()?.execute(
        "UPDATE video_jobs SET status='needs-attention',current_stage='interrupted',progress_message='上次制作已中断，可以重新启动',failure_reason='应用退出后未检测到继续运行的制作进程。',updated_at=?1,last_progress_at=?1 WHERE content_idea_id IS NOT NULL AND status IN ('queued','running','finalizing') AND updated_at<?2",
        params![recovered_at,stale_before],
    ).map_err(|error| error.to_string())?;
    let videos = list_local_videos()?;
    let mut roots = HashSet::new();
    let now = Utc::now().to_rfc3339();
    for video in videos {
        let details = video_project_details(video.path.clone())?;
        if !roots.insert(details.project_root.clone()) {
            continue;
        }
        let (automatic_status, stage) = stage_for(&details.deliverables);
        let id = content_job_for_root(state, &details.project_root)?
            .unwrap_or_else(|| format!("video-job:{}", details.project_root.to_lowercase()));
        let previous = state
            .connect()?
            .query_row(
                "SELECT video_type,manually_confirmed_type,created_at FROM video_jobs WHERE id=?1",
                [&id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let automatic_type = classify_video_type(&video.project, Path::new(&details.project_root));
        let (video_type, manually_confirmed, created_at) = previous
            .map(|(value, confirmed, created)| {
                (
                    if confirmed != 0 {
                        value
                    } else {
                        automatic_type.into()
                    },
                    confirmed,
                    created,
                )
            })
            .unwrap_or_else(|| (automatic_type.into(), 0, now.clone()));
        let missing = details
            .deliverables
            .iter()
            .filter(|item| !item.available)
            .map(|item| item.label.clone())
            .collect::<Vec<_>>();
        let ready_count = details
            .deliverables
            .iter()
            .filter(|item| item.available)
            .count() as i64;
        let progress_percent = if automatic_status == "complete" {
            100
        } else {
            (ready_count * 25).clamp(0, 95)
        };
        let progress_message = if automatic_status == "complete" {
            "四项交付已验收".to_string()
        } else if ready_count > 0 {
            format!("已找到 {ready_count}/4 项交付")
        } else {
            "等待开始制作".to_string()
        };
        state.connect()?.execute(
            "INSERT INTO video_jobs(id,title,video_type,status,current_stage,progress_percent,progress_message,last_progress_at,project_root,failure_reason,manually_confirmed_type,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(id) DO UPDATE SET title=CASE WHEN video_jobs.content_idea_id IS NOT NULL THEN COALESCE((SELECT title FROM content_ideas WHERE id=video_jobs.content_idea_id),video_jobs.title) ELSE excluded.title END,video_type=?3,
             status=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.status ELSE excluded.status END,
             current_stage=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.current_stage ELSE excluded.current_stage END,
             progress_percent=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.progress_percent ELSE excluded.progress_percent END,
             progress_message=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.progress_message ELSE excluded.progress_message END,
             last_progress_at=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.last_progress_at ELSE excluded.last_progress_at END,
             project_root=excluded.project_root,
             failure_reason=CASE WHEN video_jobs.status IN ('queued','running') THEN video_jobs.failure_reason ELSE excluded.failure_reason END,
             manually_confirmed_type=?11,updated_at=excluded.updated_at",
            params![id,video.project,video_type,automatic_status,stage,progress_percent,progress_message,now,details.project_root,missing.join("、"),manually_confirmed,created_at,now],
        ).map_err(|error| error.to_string())?;
        state
            .connect()?
            .execute(
                "DELETE FROM video_deliverables WHERE video_job_id=?1",
                [&id],
            )
            .map_err(|error| error.to_string())?;
        for item in details.deliverables {
            let deliverable_id = format!("{}:{}", id, item.kind);
            let quality = deliverable_quality(&item);
            state.connect()?.execute(
                "INSERT INTO video_deliverables(id,video_job_id,kind,path,status,quality_summary,checked_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![deliverable_id,id,item.kind,item.path.unwrap_or_default(),if item.available {"ready"} else {"missing"},quality,now],
            ).map_err(|error| error.to_string())?;
        }
    }
    let jobs = list_video_jobs_for_state(state)?;
    let now = Utc::now().to_rfc3339();
    let connection = state.connect()?;
    for job in jobs.iter().filter(|item| item.status == "complete") {
        connection.execute(
            "INSERT OR IGNORE INTO video_publish_records(id,video_job_id,platform,status,publish_url,published_at,views,likes,comments,favorites,notes,created_at,updated_at)
             VALUES(?1,?2,'抖音','ready','',NULL,0,0,0,0,'',?3,?3)",
            params![format!("publish:{}:douyin",job.id),job.id,now],
        ).map_err(|error| error.to_string())?;
    }
    Ok(VideoPipelineSummary {
        job_count: jobs.len(),
        complete_count: jobs.iter().filter(|item| item.status == "complete").count(),
        needs_attention_count: jobs.iter().filter(|item| item.status != "complete").count(),
        tech_samples: jobs
            .iter()
            .filter(|item| item.video_type == "tech" && item.status == "complete")
            .count(),
        reasoning_samples: jobs
            .iter()
            .filter(|item| item.video_type == "reasoning" && item.status == "complete")
            .count(),
        human_weakness_samples: jobs
            .iter()
            .filter(|item| item.video_type == "human-weakness" && item.status == "complete")
            .count(),
    })
}

pub fn list_video_jobs_for_state(state: &DatabaseState) -> Result<Vec<VideoJob>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare("SELECT id,title,video_type,status,current_stage,progress_percent,progress_message,last_progress_at,project_root,failure_reason,manually_confirmed_type,content_idea_id,skill_name,codex_thread_id,codex_output,cli_log_path,started_at,completed_at,created_at,updated_at FROM video_jobs ORDER BY updated_at DESC,title").map_err(|error| error.to_string())?;
    let base = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, Option<String>>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, String>(18)?,
                row.get::<_, String>(19)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut jobs = Vec::new();
    for (
        id,
        title,
        video_type,
        status,
        current_stage,
        progress_percent,
        progress_message,
        last_progress_at,
        project_root,
        failure_reason,
        confirmed,
        content_idea_id,
        skill_name,
        codex_thread_id,
        codex_output,
        cli_log_path,
        started_at,
        completed_at,
        created_at,
        updated_at,
    ) in base
    {
        let mut deliverable_statement = connection.prepare("SELECT kind,path,status,quality_summary,checked_at FROM video_deliverables WHERE video_job_id=?1 ORDER BY CASE kind WHEN 'script' THEN 1 WHEN 'video' THEN 2 WHEN 'cover' THEN 3 ELSE 4 END").map_err(|error| error.to_string())?;
        let deliverables = deliverable_statement
            .query_map([&id], |row| {
                Ok(VideoJobDeliverable {
                    kind: row.get(0)?,
                    path: row.get(1)?,
                    status: row.get(2)?,
                    quality_summary: row.get(3)?,
                    checked_at: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        jobs.push(VideoJob {
            id,
            title,
            video_type,
            status,
            current_stage,
            progress_percent,
            progress_message,
            last_progress_at,
            project_root,
            failure_reason,
            manually_confirmed_type: confirmed != 0,
            content_idea_id,
            skill_name,
            codex_thread_id,
            codex_output,
            cli_log_path,
            started_at,
            completed_at,
            created_at,
            updated_at,
            deliverables,
        });
    }
    let content_roots = jobs
        .iter()
        .filter(|item| item.content_idea_id.is_some())
        .map(|item| normalized_project_root(&item.project_root))
        .collect::<HashSet<_>>();
    Ok(jobs
        .into_iter()
        .filter(|item| {
            item.content_idea_id.is_some()
                || !content_roots.contains(&normalized_project_root(&item.project_root))
        })
        .collect())
}

pub(crate) fn video_job_by_id_for_state(
    state: &DatabaseState,
    id: &str,
) -> Result<Option<VideoJob>, String> {
    Ok(list_video_jobs_for_state(state)?
        .into_iter()
        .find(|job| job.id == id))
}

#[tauri::command]
pub fn sync_video_pipeline(
    state: tauri::State<'_, DatabaseState>,
) -> Result<VideoPipelineSummary, String> {
    sync_video_pipeline_for_state(&state)
}

#[tauri::command]
pub fn list_video_jobs(state: tauri::State<'_, DatabaseState>) -> Result<Vec<VideoJob>, String> {
    list_video_jobs_for_state(&state)
}

#[tauri::command]
pub fn save_video_job_type(
    state: tauri::State<'_, DatabaseState>,
    selection: SaveVideoType,
) -> Result<(), String> {
    if !["human-weakness", "tech", "reasoning"].contains(&selection.video_type.as_str()) {
        return Err("视频合集仅支持人性的弱点、AI未来观察局和谜题推演社。".into());
    }
    state.connect()?.execute(
        "UPDATE video_jobs SET video_type=?2,manually_confirmed_type=1,updated_at=?3 WHERE id=?1",
        params![selection.id,selection.video_type,Utc::now().to_rfc3339()],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_video_publish_records(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<VideoPublishRecord>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT p.id,p.video_job_id,j.title,j.video_type,p.platform,p.status,p.publish_url,p.published_at,p.views,p.likes,p.comments,p.favorites,p.notes,p.updated_at
         FROM video_publish_records p JOIN video_jobs j ON j.id=p.video_job_id
         ORDER BY CASE p.status WHEN 'ready' THEN 0 WHEN 'published' THEN 1 ELSE 2 END,p.updated_at DESC",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(VideoPublishRecord {
                id: row.get(0)?,
                video_job_id: row.get(1)?,
                title: row.get(2)?,
                video_type: row.get(3)?,
                platform: row.get(4)?,
                status: row.get(5)?,
                publish_url: row.get(6)?,
                published_at: row.get(7)?,
                views: row.get(8)?,
                likes: row.get(9)?,
                comments: row.get(10)?,
                favorites: row.get(11)?,
                notes: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_video_publish_record(
    state: tauri::State<'_, DatabaseState>,
    record: SaveVideoPublishRecord,
) -> Result<(), String> {
    if !["ready", "published"].contains(&record.status.as_str()) {
        return Err("发布状态仅支持待发布或已发布。".to_string());
    }
    if [
        record.views,
        record.likes,
        record.comments,
        record.favorites,
    ]
    .iter()
    .any(|value| *value < 0)
    {
        return Err("播放、点赞、评论和收藏不能为负数。".to_string());
    }
    let platform = if record.platform.trim().is_empty() {
        "抖音"
    } else {
        record.platform.trim()
    };
    let now = Utc::now().to_rfc3339();
    let published_at = if record.status == "published" {
        record.published_at.or_else(|| Some(now.clone()))
    } else {
        None
    };
    let connection = state.connect()?;
    let job_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM video_jobs WHERE id=?1)",
            [&record.video_job_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !job_exists {
        return Err("视频生产任务不存在。".to_string());
    }
    connection.execute(
        "INSERT INTO video_publish_records(id,video_job_id,platform,status,publish_url,published_at,views,likes,comments,favorites,notes,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)
         ON CONFLICT(video_job_id,platform) DO UPDATE SET status=excluded.status,publish_url=excluded.publish_url,published_at=excluded.published_at,views=excluded.views,likes=excluded.likes,comments=excluded.comments,favorites=excluded.favorites,notes=excluded.notes,updated_at=excluded.updated_at",
        params![format!("publish:{}:{}",record.video_job_id,platform),record.video_job_id,platform,record.status,record.publish_url.trim(),published_at,record.views,record.likes,record.comments,record.favorites,record.notes.trim(),now],
    ).map_err(|error| error.to_string())?;
    if record.status == "published" {
        connection.execute("UPDATE content_ideas SET status='published',updated_at=?2 WHERE id=(SELECT content_idea_id FROM video_jobs WHERE id=?1)",params![record.video_job_id,now]).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn text_file_score(path: &Path, kind: &str) -> i32 {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match kind {
        "script" => match name.as_str() {
            "完整脚本.md" => 120,
            "script.md" => 115,
            "文案包.md" => 110,
            "旁白.txt" => 100,
            "user_script.txt" => 95,
            "分镜脚本.md" => 90,
            _ if name.contains("脚本") || name.contains("script") => 60,
            _ => 0,
        },
        "publish" => match name.as_str() {
            "publish.md" => 125,
            "发布信息.md" => 120,
            "发布文案.md" => 115,
            "publishing.md" => 115,
            "文案包.md" => 110,
            _ if name.contains("发布")
                || name.contains("配文")
                || name.contains("标题")
                || name.contains("置顶") =>
            {
                60
            }
            _ => 0,
        },
        _ => 0,
    }
}

fn find_text_file(project_root: &Path, kind: &str) -> Option<PathBuf> {
    WalkDir::new(project_root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|entry| !is_intermediate(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let path = entry.into_path();
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(extension.as_str(), "md" | "txt") {
                return None;
            }
            let score = text_file_score(&path, kind);
            (score > 0).then_some((score, path))
        })
        .max_by(|(score_a, path_a), (score_b, path_b)| {
            score_a
                .cmp(score_b)
                .then_with(|| {
                    path_b
                        .components()
                        .count()
                        .cmp(&path_a.components().count())
                })
                .then_with(|| path_b.cmp(path_a))
        })
        .map(|(_, path)| path)
}

fn markdown_from_heading(content: &str, heading: &str, through_end: bool) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|line| {
        let trimmed = line.trim();
        trimmed.starts_with('#') && trimmed.contains(heading)
    })?;
    let start_level = lines[start]
        .trim()
        .chars()
        .take_while(|value| *value == '#')
        .count();
    let end = if through_end {
        lines.len()
    } else {
        lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(index, line)| {
                let trimmed = line.trim();
                let level = trimmed.chars().take_while(|value| *value == '#').count();
                (level > 0 && level <= start_level).then_some(index)
            })
            .unwrap_or(lines.len())
    };
    Some(lines[start..end].join("\n").trim().to_string())
}

fn read_deliverable_text(path: &Path, kind: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取交付内容失败：{error}"))?;
    let mut content = String::from_utf8_lossy(&bytes).to_string();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name == "文案包.md" {
        if kind == "script" {
            if let Some(section) = markdown_from_heading(&content, "完整口播", false) {
                content = section;
            }
        } else if let Some(section) = markdown_from_heading(&content, "发布标题", true) {
            content = section;
        }
    }
    const MAX_CHARS: usize = 50_000;
    if content.chars().count() > MAX_CHARS {
        content = format!(
            "{}\n\n……内容过长，已截断显示。",
            content.chars().take(MAX_CHARS).collect::<String>()
        );
    }
    Ok(content.trim().to_string())
}

fn file_deliverable(kind: &str, label: &str, path: Option<PathBuf>) -> VideoDeliverable {
    VideoDeliverable {
        kind: kind.to_string(),
        label: label.to_string(),
        file_name: path
            .as_ref()
            .and_then(|value| value.file_name())
            .map(|value| value.to_string_lossy().to_string()),
        path: path
            .as_ref()
            .map(|value| value.to_string_lossy().to_string()),
        content: None,
        available: path.is_some(),
    }
}

fn text_deliverable(kind: &str, label: &str, path: Option<PathBuf>) -> VideoDeliverable {
    let content = path
        .as_ref()
        .and_then(|value| read_deliverable_text(value, kind).ok());
    VideoDeliverable {
        kind: kind.to_string(),
        label: label.to_string(),
        file_name: path
            .as_ref()
            .and_then(|value| value.file_name())
            .map(|value| value.to_string_lossy().to_string()),
        path: path
            .as_ref()
            .map(|value| value.to_string_lossy().to_string()),
        available: path.is_some() && content.is_some(),
        content,
    }
}

fn canonical_allowed(path: &str, require_video: bool) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path).map_err(|_| "本地文件不存在或已经移动。".to_string())?;
    let root =
        fs::canonicalize(creation_root()?).map_err(|_| "视频创作目录不存在。".to_string())?;
    if !canonical.starts_with(root) {
        return Err("只能访问视频创作目录中的文件。".to_string());
    }
    if require_video && !supported_video(&canonical) {
        return Err("该文件不是支持的视频格式。".to_string());
    }
    Ok(canonical)
}

#[tauri::command]
pub fn list_local_videos() -> Result<Vec<VideoItem>, String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut covers: HashMap<PathBuf, Option<PathBuf>> = HashMap::new();
    for (root, label) in source_roots()? {
        if !root.exists() {
            continue;
        }
        let canonical_root = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
        for entry in WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| !is_intermediate(entry))
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !entry.file_type().is_file() || !supported_video(path) {
                continue;
            }
            let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            if !seen.insert(canonical.clone()) {
                continue;
            }
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            let modified: DateTime<Utc> = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .into();
            let (project, project_root) = project_for(&canonical, &canonical_root);
            let collection = classify_video_type(&project, &project_root).to_string();
            let cover = covers
                .entry(project_root.clone())
                .or_insert_with(|| find_cover(&project_root))
                .clone();
            let file_name = entry.file_name().to_string_lossy().to_string();
            result.push(VideoItem {
                id: canonical.to_string_lossy().to_string(),
                title: display_title(&file_name, &project),
                project,
                path: canonical.to_string_lossy().to_string(),
                folder: canonical
                    .parent()
                    .unwrap_or(&canonical)
                    .to_string_lossy()
                    .to_string(),
                source_root: label.to_string(),
                extension: canonical
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_uppercase(),
                size_bytes: metadata.len(),
                modified_at: modified.to_rfc3339(),
                status: video_status(&file_name, &canonical),
                cover_path: cover.map(|value| value.to_string_lossy().to_string()),
                collection,
                file_name,
            });
        }
    }
    result.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Ok(result)
}

#[tauri::command]
pub fn read_video_cover(path: String) -> Result<String, String> {
    let path = canonical_allowed(&path, false)?;
    if !supported_image(&path) {
        return Err("封面格式不受支持。".to_string());
    }
    let bytes = fs::read(&path).map_err(|error| format!("读取封面失败：{error}"))?;
    let mime = match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
pub fn open_local_video(path: String) -> Result<(), String> {
    let path = canonical_allowed(&path, true)?;
    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|error| format!("无法打开视频：{error}"))?;
    Ok(())
}

#[tauri::command]
pub fn reveal_local_video(path: String) -> Result<(), String> {
    let path = canonical_allowed(&path, true)?;
    Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|error| format!("无法在资源管理器中定位：{error}"))?;
    Ok(())
}

#[tauri::command]
pub fn video_project_details(path: String) -> Result<VideoProjectDetails, String> {
    let video_path = canonical_allowed(&path, true)?;
    let project_root = project_root_for_video(&video_path)?;
    let final_video = find_final_video(&project_root, &video_path);
    let cover = find_cover(&project_root);
    let script = find_text_file(&project_root, "script");
    let publish = find_text_file(&project_root, "publish");
    Ok(VideoProjectDetails {
        project_root: project_root.to_string_lossy().to_string(),
        deliverables: vec![
            file_deliverable("video", "最终视频 MP4", Some(final_video)),
            file_deliverable("cover", "竖屏封面 PNG", cover),
            text_deliverable("script", "完整脚本", script),
            text_deliverable("publish", "标题、配文及置顶评论", publish),
        ],
    })
}

#[tauri::command]
pub fn reveal_local_file(path: String) -> Result<(), String> {
    let path = canonical_allowed(&path, false)?;
    if !path.is_file() {
        return Err("该交付文件不存在。".to_string());
    }
    Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|error| format!("无法在资源管理器中定位：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn scanner_excludes_intermediate_clips() {
        let videos = list_local_videos().expect("视频目录应当可以扫描");
        assert!(!videos.is_empty());
        assert!(videos.iter().all(|item| {
            let path = item.path.to_ascii_lowercase();
            let file = item.file_name.to_ascii_lowercase();
            !path.contains("\\work\\")
                && !path.contains("\\.work")
                && !file.starts_with("clip")
                && file != "silent.mp4"
        }));
    }

    #[test]
    fn windows_long_path_and_regular_path_share_one_project_key() {
        let regular = r"C:\Users\11429\Documents\视频创作\项目A";
        let long = r"\\?\C:\Users\11429\Documents\视频创作\项目A\";
        assert_eq!(
            normalized_project_root(regular),
            normalized_project_root(long)
        );
    }

    #[test]
    fn details_include_four_delivery_entries() {
        let videos = list_local_videos().expect("视频目录应当可以扫描");
        let item = videos.first().expect("至少应有一个本地视频");
        let details = video_project_details(item.path.clone()).expect("应当可以读取视频项目详情");
        assert_eq!(details.deliverables.len(), 4);
        assert_eq!(details.deliverables[0].kind, "video");
        assert!(details.deliverables[0].available);
        assert!(videos.iter().any(|item| {
            video_project_details(item.path.clone())
                .map(|value| {
                    value.deliverables.iter().all(|entry| entry.available)
                        && value.deliverables[2]
                            .content
                            .as_deref()
                            .is_some_and(|content| !content.trim().is_empty())
                        && value.deliverables[3]
                            .content
                            .as_deref()
                            .is_some_and(|content| !content.trim().is_empty())
                })
                .unwrap_or(false)
        }));
    }

    #[test]
    fn spongebob_project_uses_publish_markdown_and_human_weakness_collection() {
        let videos = list_local_videos().expect("视频目录应当可以扫描");
        let item = videos
            .iter()
            .find(|item| item.project == "human-weakness-spongebob-preview")
            .expect("应当找到海绵宝宝主题视频");
        assert_eq!(item.collection, "human-weakness");
        let details = video_project_details(item.path.clone()).expect("应当读取海绵宝宝项目详情");
        let publish = details
            .deliverables
            .iter()
            .find(|entry| entry.kind == "publish")
            .expect("应当包含发布内容");
        assert_eq!(publish.file_name.as_deref(), Some("PUBLISH.md"));
        assert!(publish
            .content
            .as_deref()
            .is_some_and(|content| content.contains("越批评，他为什么越不改")));
    }

    #[test]
    fn dated_series_video_uses_its_nearest_delivery_folder() {
        let videos = list_local_videos().expect("视频目录应当可以扫描");
        let item = videos
            .iter()
            .find(|item| {
                item.path.contains("daily-human-weakness-series")
                    && item.file_name.eq_ignore_ascii_case("final.mp4")
            })
            .expect("应当找到每日人性弱点系列成片");
        let details = video_project_details(item.path.clone()).expect("应当读取本期作品详情");
        assert!(details.project_root.ends_with(r"outputs\2026-08-07"));
        let publish = details
            .deliverables
            .iter()
            .find(|entry| entry.kind == "publish")
            .expect("应当包含发布内容");
        assert_eq!(publish.file_name.as_deref(), Some("PUBLISH.md"));
        assert!(publish
            .content
            .as_deref()
            .is_some_and(|content| content.contains("你越有道理，他越不想帮")));
    }

    #[test]
    fn pipeline_has_complete_samples_for_all_three_video_types() {
        let path = std::env::temp_dir().join(format!(
            "workbench-video-pipeline-{}.sqlite3",
            Uuid::new_v4()
        ));
        let state = DatabaseState::new(path.clone()).unwrap();
        let summary = sync_video_pipeline_for_state(&state).unwrap();
        assert!(summary.tech_samples > 0, "缺少完整科技探索样例");
        assert!(summary.reasoning_samples > 0, "缺少完整推理样例");
        assert!(summary.human_weakness_samples > 0, "缺少完整人性的弱点样例");
        assert!(summary.complete_count >= 3);
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
