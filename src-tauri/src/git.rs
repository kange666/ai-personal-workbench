use crate::database::DatabaseState;
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

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

    let mut seen = HashSet::new();
    let mut repositories = Vec::new();
    for workspace in workspaces {
        if let Ok(root) = repository_root(&workspace) {
            let key = root.to_lowercase();
            if seen.insert(key) {
                repositories.push(root);
            }
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
        ];
        if let Some(argument) = since_argument.as_deref() {
            history_arguments.push(argument);
        }
        let history = match git_output(&history_arguments) {
            Ok(output) => parse_commits(&output),
            Err(_) => {
                summary.errors += 1;
                continue;
            }
        };
        let status = match git_output(&[
            "-C",
            &repository,
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
        ]) {
            Ok(output) => output,
            Err(_) => {
                summary.errors += 1;
                continue;
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
    use super::parse_commits;

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
}
