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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAsset {
    pub path: String,
    pub name: String,
    pub category: String,
    pub purpose: String,
    pub technology_stack: String,
    pub main_modules: String,
    pub install_command: String,
    pub start_command: String,
    pub test_command: String,
    pub build_command: String,
    pub command_source: String,
    pub remote_url: String,
    pub default_branch: String,
    pub has_uncommitted_changes: bool,
    pub inference_status: String,
    pub manually_confirmed: bool,
    pub last_scanned_at: String,
    pub health_level: String,
    pub health_summary: String,
    pub commit_count: i64,
    pub conversation_count: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAssetUpdate {
    pub path: String,
    pub category: String,
    pub purpose: String,
    pub technology_stack: String,
    pub main_modules: String,
    pub install_command: String,
    pub start_command: String,
    pub test_command: String,
    pub build_command: String,
    pub command_source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConversation {
    pub id: String,
    pub title: String,
    pub updated_at: String,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryCommit {
    pub hash: String,
    pub subject: String,
    pub committed_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAssetDetails {
    pub conversations: Vec<RepositoryConversation>,
    pub commits: Vec<RepositoryCommit>,
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

#[tauri::command]
pub fn list_repository_assets(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<RepositoryAsset>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT a.path,a.name,a.category,a.purpose,a.technology_stack,a.main_modules,
                    a.install_command,a.start_command,a.test_command,a.build_command,a.command_source,
                    a.remote_url,a.default_branch,a.has_uncommitted_changes,a.inference_status,
                    a.manually_confirmed,a.last_scanned_at,
                    COALESCE((SELECT h.health_level FROM repository_health_snapshots h WHERE h.repository_path=a.path ORDER BY h.verified_at DESC LIMIT 1),'未验证'),
                    COALESCE((SELECT h.summary FROM repository_health_snapshots h WHERE h.repository_path=a.path ORDER BY h.verified_at DESC LIMIT 1),'尚未执行健康检查'),
                    (SELECT COUNT(*) FROM git_commits c WHERE c.repository_path=a.path),
                    (SELECT COUNT(*) FROM conversations c WHERE lower(replace(COALESCE(c.cwd,''),'/','\')) LIKE lower(replace(a.path,'/','\')) || '%')
             FROM repository_assets a ORDER BY a.has_uncommitted_changes DESC,a.name COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(RepositoryAsset {
                path: row.get(0)?,
                name: row.get(1)?,
                category: row.get(2)?,
                purpose: row.get(3)?,
                technology_stack: row.get(4)?,
                main_modules: row.get(5)?,
                install_command: row.get(6)?,
                start_command: row.get(7)?,
                test_command: row.get(8)?,
                build_command: row.get(9)?,
                command_source: row.get(10)?,
                remote_url: row.get(11)?,
                default_branch: row.get(12)?,
                has_uncommitted_changes: row.get::<_, i64>(13)? != 0,
                inference_status: row.get(14)?,
                manually_confirmed: row.get::<_, i64>(15)? != 0,
                last_scanned_at: row.get(16)?,
                health_level: row.get(17)?,
                health_summary: row.get(18)?,
                commit_count: row.get(19)?,
                conversation_count: row.get(20)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn repository_asset_details(
    state: tauri::State<'_, DatabaseState>,
    path: String,
) -> Result<RepositoryAssetDetails, String> {
    let connection = state.connect()?;
    let normalized = path.replace('/', "\\").to_lowercase();
    let mut conversation_statement = connection
        .prepare(
            "SELECT id,COALESCE(NULLIF(title,''),'未命名 Codex 任务'),COALESCE(updated_at,started_at,''),archived
             FROM conversations
             WHERE lower(replace(COALESCE(cwd,''),'/','\')) LIKE ?1 || '%'
             ORDER BY COALESCE(updated_at,started_at) DESC LIMIT 30",
        )
        .map_err(|error| error.to_string())?;
    let conversations = conversation_statement
        .query_map([&normalized], |row| {
            Ok(RepositoryConversation {
                id: row.get(0)?,
                title: row.get(1)?,
                updated_at: row.get(2)?,
                archived: row.get::<_, i64>(3)? != 0,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut commit_statement = connection
        .prepare(
            "SELECT commit_hash,subject,committed_at FROM git_commits
             WHERE repository_path=?1 ORDER BY committed_at DESC LIMIT 30",
        )
        .map_err(|error| error.to_string())?;
    let commits = commit_statement
        .query_map([&path], |row| {
            Ok(RepositoryCommit {
                hash: row.get(0)?,
                subject: row.get(1)?,
                committed_at: row.get(2)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(RepositoryAssetDetails {
        conversations,
        commits,
    })
}

#[tauri::command]
pub fn save_repository_asset(
    state: tauri::State<'_, DatabaseState>,
    asset: RepositoryAssetUpdate,
) -> Result<(), String> {
    if asset.path.trim().is_empty() {
        return Err("项目路径不能为空".to_string());
    }
    let changed = state
        .connect()?
        .execute(
            "UPDATE repository_assets SET category=?2,purpose=?3,technology_stack=?4,main_modules=?5,
                    install_command=?6,start_command=?7,test_command=?8,build_command=?9,command_source=?10,
                    inference_status='confirmed',manually_confirmed=1,updated_at=?11 WHERE path=?1",
            params![asset.path,asset.category,asset.purpose,asset.technology_stack,asset.main_modules,
                asset.install_command,asset.start_command,asset.test_command,asset.build_command,asset.command_source,Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("项目资产不存在，请先重新扫描 Git 仓库".to_string());
    }
    Ok(())
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
