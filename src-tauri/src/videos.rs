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
}
