use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::{DirEntry, WalkDir};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitScanSummary {
    pub repositories_found: usize,
    pub commits_imported: usize,
    pub snapshots_created: usize,
    pub errors: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitScanConfiguration {
    pub roots: Vec<String>,
    pub max_depth: usize,
}

#[derive(Debug)]
struct CommitRecord {
    hash: String,
    committed_at: String,
    subject: String,
    author_name: String,
    author_email: String,
    file_count: i64,
    additions: i64,
    deletions: i64,
}

fn git_output(arguments: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(arguments);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn repository_root(cwd: &str) -> Result<String, String> {
    git_output(&["-C", cwd, "rev-parse", "--show-toplevel"])
}

fn default_scan_roots() -> Vec<String> {
    let mut roots = Vec::new();
    let drive = PathBuf::from(r"F:\");
    if drive.exists() {
        roots.push(drive.display().to_string());
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let documents = PathBuf::from(profile).join("Documents");
        if documents.exists() {
            roots.push(documents.display().to_string());
        }
    }
    roots
}

fn load_scan_configuration(connection: &Connection) -> Result<GitScanConfiguration, String> {
    let stored = connection
        .query_row(
            "SELECT value FROM app_meta WHERE key='git_scan_configuration'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut configuration = stored
        .as_deref()
        .and_then(|value| serde_json::from_str::<GitScanConfiguration>(value).ok())
        .unwrap_or_else(|| GitScanConfiguration {
            roots: default_scan_roots(),
            max_depth: 3,
        });
    configuration.max_depth = configuration.max_depth.clamp(1, 6);
    configuration.roots.retain(|root| !root.trim().is_empty());
    Ok(configuration)
}

fn should_descend(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !matches!(
        name.as_ref(),
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | ".vite"
            | "coverage"
            | "$RECYCLE.BIN"
            | "System Volume Information"
    )
}

fn discover_repository_candidates(configuration: &GitScanConfiguration) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut repositories = Vec::new();
    for root in &configuration.roots {
        let root = PathBuf::from(root);
        if !root.exists() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .max_depth(configuration.max_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(should_descend)
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_dir())
        {
            let candidate = entry.path();
            if !candidate.join(".git").exists() {
                continue;
            }
            let path = candidate.display().to_string();
            let key = path.replace('/', "\\").to_lowercase();
            if seen.insert(key) {
                repositories.push(path);
            }
        }
    }
    repositories
}

fn merge_repository(repositories: &mut Vec<String>, seen: &mut HashSet<String>, candidate: String) {
    let key = candidate.replace('/', "\\").to_lowercase();
    if seen.insert(key) {
        repositories.push(candidate);
    }
}

fn parse_commits(output: &str) -> Vec<CommitRecord> {
    let mut commits = Vec::new();
    let mut current: Option<CommitRecord> = None;
    for line in output.lines() {
        if let Some(header) = line.strip_prefix("@@@") {
            if let Some(commit) = current.take() {
                commits.push(commit);
            }
            let mut fields = header.splitn(5, '\u{1f}');
            current = Some(CommitRecord {
                hash: fields.next().unwrap_or_default().to_string(),
                committed_at: fields.next().unwrap_or_default().to_string(),
                subject: fields.next().unwrap_or_default().to_string(),
                author_name: fields.next().unwrap_or_default().to_string(),
                author_email: fields.next().unwrap_or_default().to_string(),
                file_count: 0,
                additions: 0,
                deletions: 0,
            });
            continue;
        }
        let Some(commit) = current.as_mut() else {
            continue;
        };
        let fields: Vec<&str> = line.splitn(3, '\t').collect();
        if fields.len() != 3 {
            continue;
        }
        commit.file_count += 1;
        commit.additions += fields[0].parse::<i64>().unwrap_or(0);
        commit.deletions += fields[1].parse::<i64>().unwrap_or(0);
    }
    if let Some(commit) = current {
        commits.push(commit);
    }
    commits
}

fn diff_totals(repository: &str, cached: bool) -> Result<(i64, i64), String> {
    let mut arguments = vec!["-C", repository, "diff"];
    if cached {
        arguments.push("--cached");
    }
    arguments.extend(["--numstat", "--no-renames"]);
    let output = git_output(&arguments)?;
    let mut additions = 0;
    let mut deletions = 0;
    for line in output.lines() {
        let fields: Vec<&str> = line.splitn(3, '\t').collect();
        if fields.len() == 3 {
            additions += fields[0].parse::<i64>().unwrap_or(0);
            deletions += fields[1].parse::<i64>().unwrap_or(0);
        }
    }
    Ok((additions, deletions))
}

pub fn scan_git_repositories_for_state(state: &DatabaseState) -> Result<GitScanSummary, String> {
    git_output(&["--version"])
        .map_err(|error| format!("无法运行 Git，请确认已安装并加入 PATH：{error}"))?;

    let mut connection = state.connect()?;
    let workspaces = {
        let mut statement = connection
            .prepare("SELECT DISTINCT cwd FROM conversations WHERE cwd IS NOT NULL AND cwd <> ''")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    let configuration = load_scan_configuration(&connection)?;

    let mut seen = HashSet::new();
    let mut repositories = discover_repository_candidates(&configuration)
        .into_iter()
        .map(|candidate| repository_root(&candidate).unwrap_or(candidate))
        .collect::<Vec<_>>();
    repositories.sort_by_key(|path| path.to_lowercase());
    repositories.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    for repository in &repositories {
        seen.insert(repository.replace('/', "\\").to_lowercase());
    }
    for workspace in workspaces {
        if let Ok(root) = repository_root(&workspace) {
            merge_repository(&mut repositories, &mut seen, root);
        }
    }

    let mut summary = GitScanSummary {
        repositories_found: repositories.len(),
        commits_imported: 0,
        snapshots_created: 0,
        errors: 0,
    };

    for repository in repositories {
        let branch = git_output(&["-C", &repository, "branch", "--show-current"])
            .unwrap_or_else(|_| "HEAD".to_string());
        let user_name = git_output(&["-C", &repository, "config", "user.name"]).unwrap_or_default();
        let user_email =
            git_output(&["-C", &repository, "config", "user.email"]).unwrap_or_default();
        let missing_authors: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM git_commits WHERE repository_path=?1 AND author_name=''",
                [&repository],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let latest_commit = if missing_authors > 0 {
            None
        } else {
            connection
                .query_row(
                    "SELECT MAX(committed_at) FROM git_commits WHERE repository_path=?1",
                    [&repository],
                    |row| row.get::<_, Option<String>>(0),
                )
                .map_err(|error| error.to_string())?
        };
        let since_argument = latest_commit.map(|value| format!("--since={value}"));
        let mut history_arguments = vec![
            "-C",
            repository.as_str(),
            "log",
            "--all",
            "--format=@@@%H%x1f%cI%x1f%s%x1f%an%x1f%ae",
            "--numstat",
            "--no-renames",
            "--max-count=500",
        ];
        if let Some(argument) = since_argument.as_deref() {
            history_arguments.push(argument);
        }
        let history = match git_output(&history_arguments) {
            Ok(output) => parse_commits(&output),
            Err(_) => {
                summary.errors += 1;
                Vec::new()
            }
        };
        let (status, status_failed) = match git_output(&[
            "-C",
            &repository,
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
        ]) {
            Ok(output) => (output, false),
            Err(_) => {
                summary.errors += 1;
                (String::new(), true)
            }
        };
        let (unstaged_additions, unstaged_deletions) =
            diff_totals(&repository, false).unwrap_or((0, 0));
        let (staged_additions, staged_deletions) = diff_totals(&repository, true).unwrap_or((0, 0));
        let captured_at = Utc::now().to_rfc3339();
        let repository_name = Path::new(&repository)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&repository)
            .to_string();
        let has_uncommitted_changes = !status.trim().is_empty();
        let health_level = if status_failed {
            "失败"
        } else if has_uncommitted_changes {
            "警告"
        } else {
            "健康"
        };

        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO git_repositories(path,name,current_branch,user_name,user_email,last_scanned_at)
                 VALUES(?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(path) DO UPDATE SET name=excluded.name,current_branch=excluded.current_branch,user_name=excluded.user_name,user_email=excluded.user_email,last_scanned_at=excluded.last_scanned_at",
                params![repository, repository_name, branch, user_name, user_email, captured_at],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO repository_assets(path,name,default_branch,has_uncommitted_changes,last_scanned_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?5)
                 ON CONFLICT(path) DO UPDATE SET name=excluded.name,default_branch=excluded.default_branch,has_uncommitted_changes=excluded.has_uncommitted_changes,last_scanned_at=excluded.last_scanned_at,updated_at=excluded.updated_at",
                params![repository, repository_name, branch, has_uncommitted_changes as i64, captured_at],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO repository_health_snapshots(id,repository_path,health_level,has_uncommitted_changes,summary,failure_reason,verified_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    repository,
                    health_level,
                    has_uncommitted_changes as i64,
                    if has_uncommitted_changes { "工作区存在未提交修改" } else { "Git 状态正常" },
                    if status_failed { "无法读取 Git 工作区状态" } else { "" },
                    captured_at
                ],
            )
            .map_err(|error| error.to_string())?;
        for commit in history {
            summary.commits_imported += transaction
                .execute(
                    "INSERT INTO git_commits(repository_path,commit_hash,committed_at,subject,author_name,author_email,file_count,additions,deletions)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
                     ON CONFLICT(repository_path,commit_hash) DO UPDATE SET committed_at=excluded.committed_at,subject=excluded.subject,author_name=excluded.author_name,author_email=excluded.author_email,file_count=excluded.file_count,additions=excluded.additions,deletions=excluded.deletions",
                    params![repository, commit.hash, commit.committed_at, commit.subject, commit.author_name, commit.author_email, commit.file_count, commit.additions, commit.deletions],
                )
                .map_err(|error| error.to_string())?;
        }

        let mut modified_count = 0;
        let mut added_count = 0;
        let mut deleted_count = 0;
        let mut untracked_count = 0;
        for line in status.lines() {
            let code = line.get(0..2).unwrap_or_default();
            if code == "??" {
                untracked_count += 1;
            } else {
                if code.contains('M') {
                    modified_count += 1;
                }
                if code.contains('A') {
                    added_count += 1;
                }
                if code.contains('D') {
                    deleted_count += 1;
                }
            }
        }
        transaction
            .execute(
                "INSERT INTO git_worktree_snapshots(repository_path,captured_at,modified_count,added_count,deleted_count,untracked_count,additions,deletions)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![repository, captured_at, modified_count, added_count, deleted_count, untracked_count, unstaged_additions + staged_additions, unstaged_deletions + staged_deletions],
            )
            .map_err(|error| error.to_string())?;
        summary.snapshots_created += 1;
        transaction.commit().map_err(|error| error.to_string())?;
    }

    Ok(summary)
}

#[tauri::command]
pub fn git_scan_configuration(
    state: tauri::State<'_, DatabaseState>,
) -> Result<GitScanConfiguration, String> {
    load_scan_configuration(&state.connect()?)
}

#[tauri::command]
pub fn save_git_scan_configuration(
    state: tauri::State<'_, DatabaseState>,
    configuration: GitScanConfiguration,
) -> Result<(), String> {
    if configuration.roots.is_empty() {
        return Err("至少保留一个 Git 扫描根目录".to_string());
    }
    if !(1..=6).contains(&configuration.max_depth) {
        return Err("扫描深度必须在 1 到 6 之间".to_string());
    }
    let value = serde_json::to_string(&configuration).map_err(|error| error.to_string())?;
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES('git_scan_configuration',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn scan_git_repositories(
    state: tauri::State<'_, DatabaseState>,
) -> Result<GitScanSummary, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || scan_git_repositories_for_state(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{discover_repository_candidates, parse_commits, GitScanConfiguration};
    use std::path::PathBuf;

    #[test]
    fn commit_parser_keeps_author_identity_for_personal_reports() {
        let commits = parse_commits(
            "@@@abc123\u{1f}2026-08-03T10:00:00+08:00\u{1f}feat: 新增工作台\u{1f}lzsk\u{1f}user@example.com\n10\t2\tsrc/main.rs",
        );
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].author_name, "lzsk");
        assert_eq!(commits[0].author_email, "user@example.com");
        assert_eq!(commits[0].additions, 10);
        assert_eq!(commits[0].deletions, 2);
    }

    #[test]
    fn root_scanner_finds_nested_repositories_and_skips_dependencies() {
        let directory =
            std::env::temp_dir().join(format!("workbench-git-scan-{}", uuid::Uuid::new_v4()));
        let project = directory.join("group").join("project");
        let dependency = directory.join("node_modules").join("ignored");
        std::fs::create_dir_all(project.join(".git")).unwrap();
        std::fs::create_dir_all(dependency.join(".git")).unwrap();
        let configuration = GitScanConfiguration {
            roots: vec![directory.display().to_string()],
            max_depth: 3,
        };
        let repositories = discover_repository_candidates(&configuration)
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        assert_eq!(repositories, vec![project]);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
