use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use serde::Serialize;
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

fn creation_root() -> Result<PathBuf, String> {
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
            "发布信息.md" => 120,
            "发布文案.md" => 115,
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
}
