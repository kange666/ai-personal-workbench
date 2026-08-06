use crate::{codex, database::DatabaseState, git, worktime};
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

const HISTORY_SUMMARY_VERSION: &str = "10";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalReportSummary {
    pub active_days: usize,
    pub active_weeks: usize,
    pub daily_generated: usize,
    pub weekly_generated: usize,
    pub existing_skipped: usize,
    pub daily_updated: usize,
    pub weekly_updated: usize,
    pub locked_skipped: usize,
    pub files_scanned: usize,
    pub normal_files_scanned: usize,
    pub archived_files_scanned: usize,
    pub conversations_total: i64,
    pub archived_conversations_total: i64,
    pub messages_total: i64,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCoverage {
    pub conversations: i64,
    pub archived_conversations: i64,
    pub messages: i64,
    pub active_days: usize,
    pub active_weeks: usize,
    pub daily_reports: i64,
    pub weekly_reports: i64,
    pub first_date: Option<String>,
    pub last_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyActivity {
    pub date: String,
    pub conversation_count: i64,
    pub archived_conversation_count: i64,
    pub message_count: i64,
    pub user_messages: i64,
    pub assistant_messages: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub git_commits: i64,
    pub content_idea_count: i64,
    pub daily_report_id: Option<String>,
    pub weekly_report_id: Option<String>,
    pub work_minutes: i64,
    pub estimated_work_minutes: i64,
    pub manual_work_minutes: i64,
    pub test_runs: i64,
    pub tests_passed: i64,
    pub knowledge_count: i64,
    pub task_activity_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRecord {
    pub id: String,
    pub report_type: String,
    pub period_start: String,
    pub period_end: String,
    pub title: String,
    pub content_markdown: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSource {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub project: String,
    pub date: String,
    pub detail: String,
}

#[derive(Debug)]
struct TaskFact {
    title: String,
    project: String,
    status: String,
    note: String,
}

#[derive(Debug)]
struct ConversationFact {
    id: String,
    date: String,
    project: String,
    archived: bool,
    requests: Vec<String>,
    outcome: String,
}

#[derive(Debug)]
struct GitFact {
    date: String,
    project: String,
    subject: String,
    file_count: i64,
    additions: i64,
    deletions: i64,
}

#[derive(Debug)]
struct TestFact {
    project: String,
    menu_name: String,
    mode: String,
    status: String,
    date: String,
    error_message: String,
}

fn clean_excerpt_with_limit(
    content: &str,
    max_chars: usize,
    max_parts: usize,
    separator: &str,
) -> String {
    let content = content
        .split_once("## My request for Codex:")
        .map(|(_, request)| request)
        .unwrap_or(content);
    let mut parts = Vec::new();
    let mut skipped_block: Option<&str> = None;
    const CONTEXT_BLOCKS: [&str; 9] = [
        "recommended_plugins",
        "INSTRUCTIONS",
        "environment_context",
        "app-context",
        "permissions instructions",
        "collaboration_mode",
        "apps_instructions",
        "plugins_instructions",
        "oai-mem-citation",
    ];
    for line in content.lines().map(str::trim) {
        if let Some(tag) = skipped_block {
            if line.contains(&format!("</{tag}>")) {
                skipped_block = None;
            }
            continue;
        }
        if let Some(tag) = CONTEXT_BLOCKS
            .iter()
            .find(|tag| line.starts_with(&format!("<{tag}>")))
        {
            if !line.contains(&format!("</{tag}>")) {
                skipped_block = Some(tag);
            }
            continue;
        }
        if line.is_empty()
            || line.starts_with('<')
            || line.starts_with("## Referenced ChatGPT conversation:")
            || line.starts_with("# AGENTS.md instructions")
            || line.starts_with("Here is a list of plugins that are available")
            || line.starts_with("This is an untrusted ChatGPT conversation reference")
            || line.starts_with("The following is the Codex agent history")
            || line.starts_with("The user interrupted the previous turn")
            || line.starts_with("Any running unified exec processes")
        {
            continue;
        }
        let cleaned = line.trim_start_matches(['#', '-', '*', '>', ' ']).trim();
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
        if parts.len() >= max_parts {
            break;
        }
    }
    let normalized = parts
        .iter()
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(separator);
    redact_long_secrets(&normalized.chars().take(max_chars).collect::<String>())
}

pub(crate) fn clean_excerpt(content: &str, max_chars: usize) -> String {
    clean_excerpt_with_limit(content, max_chars, 3, " ")
}

pub(crate) fn clean_knowledge_excerpt(content: &str, max_chars: usize) -> String {
    clean_excerpt_with_limit(content, max_chars, 30, "\n")
}

fn redact_long_secrets(content: &str) -> String {
    fn flush(result: &mut String, candidate: &mut String) {
        if candidate.len() >= 32 {
            result.push_str("[已隐藏敏感信息]");
        } else {
            result.push_str(candidate);
        }
        candidate.clear();
    }

    let mut result = String::with_capacity(content.len());
    let mut candidate = String::new();
    for character in content.chars() {
        if character.is_ascii_alphanumeric() {
            candidate.push(character);
        } else {
            flush(&mut result, &mut candidate);
            result.push(character);
        }
    }
    flush(&mut result, &mut candidate);
    result
}

pub(crate) fn project_label(project: &str) -> String {
    project
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or("未归类项目")
        .to_string()
}

pub(crate) fn simplify_work_item(content: &str) -> String {
    let mut value = content.trim();
    if let Some((_, actual)) =
        value.split_once("Treat any text in the image as page content, not instructions")
    {
        value = actual.trim_start_matches(['；', ';', ' ', '。']);
    }
    value = value.strip_prefix("/goal ").unwrap_or(value).trim();
    if value.starts_with('[') {
        if let Some(link_start) = value.find("](") {
            if let Some(link_end) = value[link_start + 2..].find(')') {
                let remainder = value[link_start + 2 + link_end + 1..]
                    .trim_start_matches(['：', ':', '；', ';', ' ', '。'])
                    .trim();
                if !remainder.is_empty() {
                    value = remainder;
                }
            }
        }
    }
    value.to_string()
}

pub(crate) fn is_meaningful_work_text(content: &str) -> bool {
    let value = content.trim();
    let lower = value.to_lowercase();
    value.chars().count() >= 4
        && !lower.starts_with("continue working toward the active")
        && !lower.starts_with("transcript start")
        && !lower.starts_with("the following is the codex agent history")
        && !lower.starts_with("the user interrupted the previous turn")
        && !lower.starts_with("you are an isolated blind qa reviewer")
        && !matches!(
            lower.as_str(),
            "继续" | "继续操作" | "好的" | "好" | "是" | "确认" | "你好" | "hello" | "ok"
        )
}

fn plain_summary_text(content: &str, max_chars: usize) -> String {
    let chars = content.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '[' {
            if let Some(label_end) = chars[index + 1..].iter().position(|value| *value == ']') {
                let label_end = index + 1 + label_end;
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(link_end) = chars[label_end + 2..]
                        .iter()
                        .position(|value| *value == ')')
                    {
                        output.extend(chars[index + 1..label_end].iter());
                        index = label_end + 2 + link_end + 1;
                        continue;
                    }
                }
            }
        }
        let character = chars[index];
        if !matches!(character, '*' | '`' | '#') {
            output.push(if character == '\n' { ' ' } else { character });
        }
        index += 1;
    }
    let normalized = output.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentences = normalized
        .split(['。', '！', '!'])
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    let primary = match sentences.as_slice() {
        [first, second, ..] if first.chars().count() < 30 => {
            format!("{}；{}", first.trim(), second.trim())
        }
        [first, ..] if first.chars().count() >= 12 => first.trim().to_string(),
        _ => normalized.clone(),
    };
    let shortened = primary.chars().take(max_chars).collect::<String>();
    if primary.chars().count() > max_chars {
        format!("{shortened}…")
    } else {
        shortened
    }
}

pub(crate) fn is_low_value_process(content: &str) -> bool {
    let lower = content.to_lowercase();
    let file_only = [".vue", ".js", ".ts", ".java", ".rs"]
        .iter()
        .any(|extension| lower.contains(extension))
        && content
            .chars()
            .filter(|value| matches!(value, '\u{4e00}'..='\u{9fff}'))
            .count()
            <= 6;
    file_only
        || [
            "停止后端",
            "停止服务",
            "已全部停止",
            "所有都停止",
            "先把这个项目跑起来",
            "启动management后端",
            "启动后端",
            "git忽略",
            ".gitignore",
            "忽略测试",
            "忽略所有测试",
            "忽略测试文件",
            "删除测试文件",
            "清理测试文件",
            "测试已全部通过",
            "项后端测试",
            "静态检查结果",
            "没有必须整改",
            "继续操作",
            "重新运行",
            "关闭窗口",
            "一次性自动任务",
            "npm install",
            "node版本",
            "安装几个常用的版本",
            "测试下网络",
            "禁止windows更新",
            "windows更新",
            "有什么方法让我",
            "切换为国内",
            "创建一个快捷方式",
            "禁止自动更新",
            "帮我打lol",
            "不挂机",
            "核对一下前端",
            "未开放，正在进行开发",
            "只修改以下文件",
            "risk_level",
            "user_authorization",
            "direction_semantics",
            "isolated blind qa",
        ]
        .iter()
        .any(|word| lower.contains(word))
}

fn conversation_work_summary(conversation: &ConversationFact) -> Option<String> {
    let outcome = conversation.outcome.trim();
    if outcome.is_empty() {
        return None;
    }
    let normalized = if outcome.contains("历史报告生成版本")
        || (outcome.contains("历史报告") && outcome.contains("核心筛选"))
    {
        "优化历史日报和周报生成：按项目功能归类工作成果，并结合个人 Git 提交过滤低价值过程记录"
            .to_string()
    } else if outcome.contains("generate-tech-short-video") || outcome.contains("个人 Skill") {
        "新增科技短视频自动生成模块：支持根据中文标题生成脚本、画面、配音、字幕、封面和成片"
            .to_string()
    } else if outcome.contains("autoNotice.vue") && outcome.contains("type=5") {
        "新增流程消息卡片：支持动态字段展示、按需缓存和审批详情跳转".to_string()
    } else if outcome.contains("pc_message_route") {
        "完善 PC 消息路由：覆盖常用消息类型并支持跳转到对应业务页面".to_string()
    } else if outcome.contains("Socket") && outcome.contains("标记") && outcome.contains("颜色")
    {
        "新增地图实时报警联动：根据报警数量和等级动态更新标记颜色与显示层级".to_string()
    } else {
        if is_low_value_process(outcome) {
            return None;
        }
        plain_summary_text(outcome, 105)
    };
    if !is_low_value_process(&normalized)
        && [
            "完成", "实现", "新增", "修复", "优化", "接入", "部署", "发布", "支持", "通过", "验证",
            "审查", "分析", "结论", "定位", "确认", "更新", "创建", "改为",
        ]
        .iter()
        .any(|word| normalized.contains(word))
    {
        if is_meaningful_work_text(&normalized) {
            return Some(normalized);
        }
    }
    None
}

fn git_work_summary(subject: &str) -> Option<String> {
    let cleaned = subject
        .trim()
        .trim_matches(['`', '*', ' '])
        .trim()
        .to_string();
    let lower = cleaned.to_lowercase();
    if cleaned.is_empty()
        || lower.starts_with("merge ")
        || matches!(lower.as_str(), "init" | "pref" | "test" | "chore")
        || lower.contains("e2e")
        || lower.contains("测试")
        || lower.starts_with("docs")
        || lower.starts_with("style")
        || lower.starts_with("chore")
    {
        return None;
    }
    let summary = if lower.contains("pc and app verification matrix") {
        "新增 PC/APP 功能对照与静态、接口、浏览器分层回归矩阵".to_string()
    } else if lower.contains("track video production deliverables") {
        "新增视频生产流水线：按脚本、成片、封面和发布文案检查交付完整性".to_string()
    } else if lower.contains("weekly audit and toolchain checks") {
        "新增每周整体检查、漏跑补偿与工具链重复版本提示".to_string()
    } else if lower.starts_with("feat<") && cleaned.contains('>') {
        let end = cleaned.find('>').unwrap_or(5);
        format!("新增{}{}", &cleaned[5..end], &cleaned[end + 1..])
    } else if let Some((prefix, detail)) = cleaned.split_once(':') {
        let prefix = prefix.to_lowercase();
        let detail = detail.trim().trim_matches(['`', '*', ' ']);
        if prefix.starts_with("feat") || prefix == "add" {
            if detail.starts_with("新增") || detail.starts_with("支持") {
                detail.to_string()
            } else {
                format!("新增：{detail}")
            }
        } else if prefix.starts_with("fix") {
            if detail.starts_with("修复")
                || detail.starts_with("完善")
                || detail.starts_with("修改")
            {
                detail.to_string()
            } else {
                format!("修复：{detail}")
            }
        } else if prefix.starts_with("refactor") {
            format!("重构：{detail}")
        } else {
            detail.to_string()
        }
    } else {
        cleaned
    };
    let summary = match summary.as_str() {
        "新增：automate release publishing" => "实现版本自动发布".to_string(),
        "新增：support queue 4310 and copy options" => "支持 4310 队列与复制选项".to_string(),
        _ => summary,
    };
    if is_low_value_process(&summary) || !is_meaningful_work_text(&summary) {
        None
    } else {
        Some(plain_summary_text(&summary, 100))
    }
}

fn git_module_key(summary: &str) -> String {
    let lower = summary.to_lowercase();
    if summary.contains("消息") || lower.contains("message") {
        return "消息".to_string();
    }
    for keyword in [
        "案例分享",
        "案例分析",
        "特种设备",
        "安全设备",
        "高德地图",
        "消息跳转",
        "流程消息",
        "地图",
        "发布",
        "队列",
        "报警",
        "字典",
        "工作总结",
        "传感器",
        "审批",
    ] {
        if summary.contains(keyword) {
            return keyword.to_string();
        }
    }
    summary.chars().take(28).collect()
}

fn git_importance(commit: &GitFact) -> i64 {
    let lower = commit.subject.to_lowercase();
    let kind_score = if lower.contains("feat") || lower.starts_with("add") {
        1_000
    } else if lower.contains("refactor") {
        700
    } else if lower.contains("fix") {
        400
    } else {
        200
    };
    kind_score + commit.file_count * 10 + commit.additions.min(2_000) / 5
}

fn report_period(
    report_type: &str,
    reference: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), String> {
    match report_type {
        "daily" => Ok((reference, reference)),
        "weekly" => {
            let start =
                reference - Duration::days(reference.weekday().num_days_from_monday().into());
            Ok((start, start + Duration::days(6)))
        }
        "monthly" => {
            let start = reference
                .with_day(1)
                .ok_or_else(|| "月份无效".to_string())?;
            let next_month = if start.month() == 12 {
                NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
            }
            .ok_or_else(|| "月份无效".to_string())?;
            Ok((start, next_month - Duration::days(1)))
        }
        _ => Err("报告类型仅支持 daily、weekly、monthly".to_string()),
    }
}

fn report_title(report_type: &str, reference: NaiveDate) -> String {
    match report_type {
        "daily" => reference.format("%Y年%m月%d日工作日报").to_string(),
        "weekly" => format!(
            "{}年第{}周工作总结",
            reference.year(),
            reference.iso_week().week()
        ),
        "monthly" => reference.format("%Y年%m月工作复盘").to_string(),
        _ => "工作报告".to_string(),
    }
}

fn build_content(
    title: &str,
    report_type: &str,
    start: NaiveDate,
    end: NaiveDate,
    tasks: &[TaskFact],
    conversations: &[ConversationFact],
    git_facts: &[GitFact],
    conversation_count: i64,
    archived_conversation_count: i64,
    message_count: i64,
    total_tokens: i64,
    commit_count: i64,
    additions: i64,
    deletions: i64,
) -> String {
    let completed: Vec<&TaskFact> = tasks.iter().filter(|task| task.status == "done").collect();
    let pending: Vec<&TaskFact> = tasks.iter().filter(|task| task.status != "done").collect();
    let projects: HashSet<&str> = tasks
        .iter()
        .map(|task| task.project.as_str())
        .chain(conversations.iter().map(|item| item.project.as_str()))
        .collect();
    let mut lines = vec![
        format!("# {title}"),
        String::new(),
        format!(
            "统计周期：{} 至 {}",
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        ),
        String::new(),
        "## 工作概览".to_string(),
        String::new(),
        format!("- 完成任务：{} 项", completed.len()),
        format!("- 进行中或待处理：{} 项", pending.len()),
        format!("- 活跃项目：{} 个", projects.len()),
        format!("- Codex 对话：{conversation_count} 次"),
        format!("- 其中归档对话：{archived_conversation_count} 次"),
        format!("- 对话消息：{message_count} 条"),
        format!("- 工作记录：{} 条（按对话与日期归并）", conversations.len()),
        format!("- Token 使用：{total_tokens}（周期差量口径）"),
        format!("- Git 提交：{commit_count} 次，新增 {additions} 行，删除 {deletions} 行"),
        String::new(),
        "## 项目工作总结".to_string(),
        String::new(),
    ];
    let mut project_work: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let active_projects = tasks
        .iter()
        .map(|task| task.project.as_str())
        .chain(conversations.iter().map(|item| item.project.as_str()))
        .collect::<HashSet<_>>();
    let mut git_candidates: BTreeMap<String, Vec<(i64, String, String)>> = BTreeMap::new();
    for commit in git_facts {
        if !active_projects.contains(commit.project.as_str()) {
            continue;
        }
        let Some(summary) = git_work_summary(&commit.subject) else {
            continue;
        };
        let date = if report_type == "daily" {
            String::new()
        } else {
            format!("{}：", commit.date)
        };
        let code_scale = if commit.file_count > 0 {
            format!(
                "（Git，{} 个文件，+{}/-{}）",
                commit.file_count, commit.additions, commit.deletions
            )
        } else {
            "（Git）".to_string()
        };
        let module_key = git_module_key(&summary);
        git_candidates
            .entry(commit.project.clone())
            .or_default()
            .push((
                git_importance(commit),
                module_key,
                format!("{date}{summary}{code_scale}"),
            ));
    }
    let mut project_git_modules: HashMap<String, HashSet<String>> = HashMap::new();
    for (project, mut candidates) in git_candidates {
        candidates.sort_by(|left, right| right.0.cmp(&left.0));
        let mut modules = HashSet::new();
        for (_, module, display) in candidates {
            if modules.insert(module) {
                project_work
                    .entry(project.clone())
                    .or_default()
                    .push(display);
            }
            if modules.len() >= 6 {
                break;
            }
        }
        project_git_modules.insert(project, modules);
    }
    for task in tasks {
        let status = if task.status == "done" {
            "完成"
        } else {
            "推进"
        };
        project_work
            .entry(task.project.clone())
            .or_default()
            .push(format!("{status}任务：{}", task.title));
    }
    for conversation in conversations {
        let Some(summary) = conversation_work_summary(conversation) else {
            continue;
        };
        if project_git_modules
            .get(&conversation.project)
            .is_some_and(|modules| modules.contains(&git_module_key(&summary)))
        {
            continue;
        }
        let date = if start == end {
            String::new()
        } else {
            format!("{}：", conversation.date)
        };
        let archive = if conversation.archived {
            "（归档记录）"
        } else {
            ""
        };
        project_work
            .entry(conversation.project.clone())
            .or_default()
            .push(format!("{date}{summary}{archive}"));
    }
    if project_work.is_empty() {
        lines.push("- 当前周期没有可归类的项目工作。".to_string());
    } else {
        for (project, work_items) in project_work {
            lines.push(format!("### {project}"));
            lines.push(String::new());
            let mut seen = HashSet::new();
            for item in work_items {
                let duplicate_key = item
                    .replace("（归档记录）", "")
                    .chars()
                    .take(70)
                    .collect::<String>();
                if seen.insert(duplicate_key) {
                    lines.push(format!("- {item}"));
                }
                if seen.len() >= 8 {
                    break;
                }
            }
            lines.push(String::new());
        }
        while lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
    }
    lines.extend([String::new(), "## 问题与风险".to_string(), String::new()]);
    let risks: Vec<&TaskFact> = pending
        .iter()
        .copied()
        .filter(|task| matches!(task.status.as_str(), "blocked" | "overdue"))
        .collect();
    if risks.is_empty() {
        lines.push("- 当前任务中未识别到阻塞或逾期风险。".to_string());
    } else {
        lines.extend(risks.iter().map(|task| {
            let detail = if task.note.trim().is_empty() {
                "暂无补充说明"
            } else {
                task.note.trim()
            };
            format!("- [{}] {}：{}", task.project, task.title, detail)
        }));
    }
    lines.extend([String::new(), "## 下一步计划".to_string(), String::new()]);
    if pending.is_empty() {
        lines.push("- 当前周期任务已全部完成。".to_string());
    } else {
        lines.extend(
            pending
                .iter()
                .take(8)
                .map(|task| format!("- [{}] {}", task.project, task.title)),
        );
    }
    lines.extend([
        String::new(),
        "> 本报告由本地工作台根据任务、Codex 与 Git 数据自动生成，可在锁定前人工修订。".to_string(),
    ]);
    lines.join("\n")
}

fn format_work_minutes(value: i64) -> String {
    let hours = value / 60;
    let minutes = value % 60;
    match (hours, minutes) {
        (0, minutes) => format!("{minutes}分钟"),
        (hours, 0) => format!("{hours}小时"),
        (hours, minutes) => format!("{hours}小时{minutes}分钟"),
    }
}

fn add_work_time_section(
    content: String,
    report_type: &str,
    summary: &worktime::WorkSummary,
) -> String {
    let heading = if report_type == "weekly" {
        "## 本周投入"
    } else if report_type == "monthly" {
        "## 本月投入"
    } else {
        "## 工时概览"
    };
    let period = if report_type == "weekly" {
        "本周工时"
    } else if report_type == "monthly" {
        "本月工时"
    } else {
        "今日工时"
    };
    let source = if summary.has_manual_corrections {
        format!(
            "含手工修正；原始估算 {}",
            format_work_minutes(summary.estimated_minutes)
        )
    } else {
        "估算，不作为精确考勤数据".to_string()
    };
    let breakdown = if report_type == "weekly" {
        &summary.by_type
    } else {
        &summary.by_project
    };
    let mut lines = vec![
        heading.to_string(),
        String::new(),
        format!(
            "{period}：{}（{source}）",
            format_work_minutes(summary.total_minutes)
        ),
        String::new(),
    ];
    if breakdown.is_empty() {
        lines.push("- 当前周期没有可估算或手工补录的工时。".to_string());
    } else {
        lines.extend(
            breakdown
                .iter()
                .map(|item| format!("- {}：{}", item.name, format_work_minutes(item.minutes))),
        );
    }
    lines.extend([
        String::new(),
        "> 工时由本地活动间隔估算，可在工作记录中手工修正；估算工时不等同于考勤数据。".to_string(),
        String::new(),
    ]);
    let marker = "## 项目工作总结";
    if let Some(index) = content.find(marker) {
        format!(
            "{}{}\n\n{}",
            &content[..index],
            lines.join("\n"),
            &content[index..]
        )
    } else {
        format!("{content}\n\n{}", lines.join("\n"))
    }
}

fn test_mode_label(mode: &str) -> &'static str {
    match mode {
        "mock" => "功能测试（模拟接口）",
        "real" => "功能测试（真实接口）",
        "source-style" => "页面源码与样式检查",
        "browser-style" => "浏览器页面样式测试",
        _ => "项目已有测试",
    }
}

fn add_test_section(content: String, report_type: &str, tests: &[TestFact]) -> String {
    let heading = match report_type {
        "daily" => "## 今日测试",
        "weekly" => "## 本周测试",
        "monthly" => "## 本月测试",
        _ => "## 测试情况",
    };
    let passed = tests.iter().filter(|item| item.status == "passed").count();
    let failed = tests.iter().filter(|item| item.status == "failed").count();
    let mut lines = vec![heading.to_string(), String::new()];
    if tests.is_empty() {
        lines.push("- 当前周期没有执行项目测试。".to_string());
    } else {
        lines.push(format!(
            "- 共执行 {} 次，通过 {} 次，未通过 {} 次。",
            tests.len(),
            passed,
            failed
        ));
        lines.push(String::new());
        for test in tests.iter().take(12) {
            let date = if report_type == "daily" {
                String::new()
            } else {
                format!("{} · ", test.date)
            };
            let result = match test.status.as_str() {
                "passed" => "通过",
                "failed" => "未通过",
                _ => "未完成",
            };
            let issue = if test.status == "failed" && !test.error_message.trim().is_empty() {
                format!("；核心问题：{}", clean_excerpt(&test.error_message, 120))
            } else {
                String::new()
            };
            lines.push(format!(
                "- {date}[{}] {}：{} · {}{}",
                test.project,
                test.menu_name,
                result,
                test_mode_label(&test.mode),
                issue
            ));
        }
        if tests.len() > 12 {
            lines.push(format!(
                "- 其余 {} 次测试可在测试中心查看。",
                tests.len() - 12
            ));
        }
        if failed > 0 {
            lines
                .push("- 未通过项请在测试中心打开对应报告，按整改建议处理后重新测试。".to_string());
        }
    }
    lines.push(String::new());
    let marker = "## 问题与风险";
    if let Some(index) = content.find(marker) {
        format!(
            "{}{}\n\n{}",
            &content[..index],
            lines.join("\n"),
            &content[index..]
        )
    } else {
        format!("{content}\n\n{}", lines.join("\n"))
    }
}

fn generate_for_date(
    state: &DatabaseState,
    report_type: &str,
    reference: NaiveDate,
) -> Result<ReportRecord, String> {
    let (start, end) = report_period(report_type, reference)?;
    let start_text = start.format("%Y-%m-%d").to_string();
    let end_text = end.format("%Y-%m-%d").to_string();
    let work_summary = worktime::summary_for_range(state, &start_text, &end_text, true)?;
    let mut connection = state.connect()?;

    let existing: Option<ReportRecord> = connection
        .query_row(
            "SELECT id,report_type,period_start,period_end,title,content_markdown,status,created_at,updated_at
             FROM reports WHERE report_type=?1 AND period_start=?2 AND period_end=?3 ORDER BY updated_at DESC LIMIT 1",
            params![report_type, start_text, end_text],
            |row| Ok(ReportRecord { id: row.get(0)?, report_type: row.get(1)?, period_start: row.get(2)?, period_end: row.get(3)?, title: row.get(4)?, content_markdown: row.get(5)?, status: row.get(6)?, created_at: row.get(7)?, updated_at: row.get(8)? }),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if existing
        .as_ref()
        .is_some_and(|report| report.status == "locked")
    {
        return Err("报告已锁定，请先解锁再重新生成。".to_string());
    }

    let tasks = {
        let mut statement = connection
            .prepare(
                "SELECT title,project,status,note FROM tasks
             WHERE (planned_date BETWEEN ?1 AND ?2)
                OR (week_start BETWEEN ?1 AND ?2)
                OR (start_date <= ?2 AND end_date >= ?1)
                OR (substr(completed_at,1,10) BETWEEN ?1 AND ?2)
             ORDER BY priority,status,updated_at DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![start_text, end_text], |row| {
                Ok(TaskFact {
                    title: row.get(0)?,
                    project: row.get(1)?,
                    status: row.get(2)?,
                    note: row.get(3)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    let (_token_conversation_count, total_tokens): (i64, i64) = connection.query_row(
        "WITH ordered AS (
           SELECT conversation_id,event_time,
             MAX(total_tokens-LAG(total_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) AS total_delta
           FROM token_events WHERE event_time IS NOT NULL
         )
         SELECT COUNT(DISTINCT conversation_id),COALESCE(SUM(total_delta),0)
         FROM ordered WHERE date(event_time,'localtime') BETWEEN ?1 AND ?2",
        params![start_text, end_text],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|error| error.to_string())?;
    let (conversations, message_count) = {
        let mut statement = connection.prepare(
            "SELECT c.id,COALESCE(NULLIF(c.title,''),'未命名会话'),COALESCE(NULLIF(c.project_override,''),COALESCE(NULLIF(c.cwd,''),'未归类项目')),
               c.archived,date(m.event_time,'localtime'),m.role,m.content
             FROM conversation_messages m JOIN conversations c ON c.id=m.conversation_id
             WHERE m.event_time IS NOT NULL AND date(m.event_time,'localtime') BETWEEN ?1 AND ?2
             ORDER BY date(m.event_time,'localtime'),m.event_time,m.source_index",
        ).map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![start_text, end_text], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut facts: Vec<ConversationFact> = Vec::new();
        let mut indexes: HashMap<String, usize> = HashMap::new();
        let mut messages = 0i64;
        for row in rows {
            let (id, _title, project, archived, date, role, content) =
                row.map_err(|error| error.to_string())?;
            messages += 1;
            let key = format!("{id}\n{date}");
            let index = *indexes.entry(key).or_insert_with(|| {
                facts.push(ConversationFact {
                    id: id.clone(),
                    date: date.clone(),
                    project: project_label(&project),
                    archived,
                    requests: Vec::new(),
                    outcome: String::new(),
                });
                facts.len() - 1
            });
            let excerpt = simplify_work_item(&clean_excerpt(
                &content,
                if role == "user" { 120 } else { 150 },
            ));
            if !is_meaningful_work_text(&excerpt) {
                continue;
            }
            if role == "user" {
                if facts[index].requests.len() < 5 && !facts[index].requests.contains(&excerpt) {
                    facts[index].requests.push(excerpt);
                }
            } else if role == "assistant" {
                facts[index].outcome = excerpt;
            }
        }
        (facts, messages)
    };
    let conversation_ids = conversations
        .iter()
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let archived_ids = conversations
        .iter()
        .filter(|item| item.archived)
        .map(|item| item.id.as_str())
        .collect::<HashSet<_>>();
    let conversation_count = conversation_ids.len() as i64;
    let archived_conversation_count = archived_ids.len() as i64;
    let git_facts = {
        let mut statement = connection
            .prepare(
                "SELECT date(gc.committed_at,'localtime'),gr.name,gc.subject,gc.file_count,gc.additions,gc.deletions
                 FROM git_commits gc JOIN git_repositories gr ON gr.path=gc.repository_path
                 WHERE date(gc.committed_at,'localtime') BETWEEN ?1 AND ?2
                   AND (gr.user_name='' OR lower(trim(gc.author_name))=lower(trim(gr.user_name)))
                 ORDER BY gc.committed_at",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![start_text, end_text], |row| {
                Ok(GitFact {
                    date: row.get(0)?,
                    project: row.get(1)?,
                    subject: row.get(2)?,
                    file_count: row.get(3)?,
                    additions: row.get(4)?,
                    deletions: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    let valuable_git = git_facts
        .iter()
        .filter(|item| git_work_summary(&item.subject).is_some())
        .collect::<Vec<_>>();
    let commit_count = valuable_git.len() as i64;
    let additions = valuable_git.iter().map(|item| item.additions).sum::<i64>();
    let deletions = valuable_git.iter().map(|item| item.deletions).sum::<i64>();
    let tests = {
        let mut statement = connection
            .prepare(
                "SELECT project,menu_name,mode,status,date(started_at,'localtime'),error_message
                 FROM test_runs
                 WHERE date(started_at,'localtime') BETWEEN ?1 AND ?2
                 ORDER BY started_at DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![start_text, end_text], |row| {
                Ok(TestFact {
                    project: row.get(0)?,
                    menu_name: row.get(1)?,
                    mode: row.get(2)?,
                    status: row.get(3)?,
                    date: row.get(4)?,
                    error_message: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };

    let title = report_title(report_type, reference);
    let content = add_test_section(
        add_work_time_section(
            build_content(
                &title,
                report_type,
                start,
                end,
                &tasks,
                &conversations,
                &git_facts,
                conversation_count,
                archived_conversation_count,
                message_count,
                total_tokens,
                commit_count,
                additions,
                deletions,
            ),
            report_type,
            &work_summary,
        ),
        report_type,
        &tests,
    );
    let now = Utc::now().to_rfc3339();
    let record = ReportRecord {
        id: existing
            .as_ref()
            .map(|report| report.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
        report_type: report_type.to_string(),
        period_start: start_text,
        period_end: end_text,
        title,
        content_markdown: content,
        status: "draft".to_string(),
        created_at: existing
            .as_ref()
            .map(|report| report.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    save_report_record(&mut connection, &record)?;
    Ok(record)
}

fn save_report_record(
    connection: &mut rusqlite::Connection,
    report: &ReportRecord,
) -> Result<(), String> {
    connection.execute(
        "INSERT INTO reports(id,report_type,period_start,period_end,title,content_markdown,status,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title,content_markdown=excluded.content_markdown,status=excluded.status,updated_at=excluded.updated_at",
        params![report.id,report.report_type,report.period_start,report.period_end,report.title,report.content_markdown,report.status,report.created_at,report.updated_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn list_reports(state: tauri::State<'_, DatabaseState>) -> Result<Vec<ReportRecord>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT id,report_type,period_start,period_end,title,content_markdown,status,created_at,updated_at FROM reports ORDER BY period_start DESC, updated_at DESC",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ReportRecord {
                id: row.get(0)?,
                report_type: row.get(1)?,
                period_start: row.get(2)?,
                period_end: row.get(3)?,
                title: row.get(4)?,
                content_markdown: row.get(5)?,
                status: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn report_sources(
    state: tauri::State<'_, DatabaseState>,
    report_id: String,
) -> Result<Vec<ReportSource>, String> {
    let connection = state.connect()?;
    let (start, end): (String, String) = connection
        .query_row(
            "SELECT period_start,period_end FROM reports WHERE id=?1",
            [&report_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| error.to_string())?;
    let mut sources = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT c.id,COALESCE(NULLIF(c.title,''),'未命名会话'),COALESCE(NULLIF(c.project_override,''),COALESCE(c.cwd,'')),MAX(date(m.event_time,'localtime')),c.archived
             FROM conversations c JOIN conversation_messages m ON m.conversation_id=c.id
             WHERE date(m.event_time,'localtime') BETWEEN ?1 AND ?2
             GROUP BY c.id ORDER BY MAX(m.event_time) DESC",
        )
        .map_err(|error| error.to_string())?;
    let conversations = statement
        .query_map(params![&start, &end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    sources.extend(
        conversations
            .into_iter()
            .map(|(id, title, cwd, date, archived)| ReportSource {
                kind: "Codex 对话".to_string(),
                id,
                title,
                project: project_label(&cwd),
                date,
                detail: if archived {
                    "归档对话".to_string()
                } else {
                    "普通对话".to_string()
                },
            }),
    );

    let mut statement = connection
        .prepare(
            "SELECT gc.commit_hash,gr.path,gc.subject,date(gc.committed_at,'localtime'),gc.file_count,gc.additions,gc.deletions
             FROM git_commits gc JOIN git_repositories gr ON gr.path=gc.repository_path
             WHERE date(gc.committed_at,'localtime') BETWEEN ?1 AND ?2
               AND (gr.user_name='' OR lower(trim(gc.author_name))=lower(trim(gr.user_name)))
             ORDER BY gc.committed_at DESC LIMIT 100",
        )
        .map_err(|error| error.to_string())?;
    let commits = statement
        .query_map(params![&start, &end], |row| {
            Ok(ReportSource {
                kind: "Git 提交".to_string(),
                id: row.get(0)?,
                project: project_label(&row.get::<_, String>(1)?),
                title: row.get(2)?,
                date: row.get(3)?,
                detail: format!(
                    "{} 个文件，+{}/-{}",
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?
                ),
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    sources.extend(commits);
    drop(statement);

    let mut statement = connection
        .prepare(
            "SELECT id,title,project,
                    COALESCE(substr(completed_at,1,10),planned_date,week_start,start_date,substr(updated_at,1,10)),status
             FROM tasks
             WHERE (planned_date BETWEEN ?1 AND ?2)
                OR (week_start BETWEEN ?1 AND ?2)
                OR (start_date <= ?2 AND end_date >= ?1)
                OR (substr(completed_at,1,10) BETWEEN ?1 AND ?2)
             ORDER BY updated_at DESC LIMIT 100",
        )
        .map_err(|error| error.to_string())?;
    let tasks = statement
        .query_map(params![&start, &end], |row| {
            Ok(ReportSource {
                kind: "任务".to_string(),
                id: row.get(0)?,
                title: row.get(1)?,
                project: row.get(2)?,
                date: row.get(3)?,
                detail: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    sources.extend(tasks);
    drop(statement);

    let mut statement = connection
        .prepare(
            "SELECT id,menu_name,project,date(started_at,'localtime'),mode,status,error_message
             FROM test_runs WHERE date(started_at,'localtime') BETWEEN ?1 AND ?2
             ORDER BY started_at DESC LIMIT 100",
        )
        .map_err(|error| error.to_string())?;
    let tests = statement
        .query_map(params![&start, &end], |row| {
            let mode = row.get::<_, String>(4)?;
            let status = row.get::<_, String>(5)?;
            let error = row.get::<_, String>(6)?;
            let result = if status == "passed" {
                "通过"
            } else {
                "未通过"
            };
            let detail = if status == "failed" && !error.trim().is_empty() {
                format!(
                    "{} · {} · {}",
                    result,
                    test_mode_label(&mode),
                    clean_excerpt(&error, 80)
                )
            } else {
                format!("{} · {}", result, test_mode_label(&mode))
            };
            Ok(ReportSource {
                kind: "测试".to_string(),
                id: row.get(0)?,
                title: row.get(1)?,
                project: row.get(2)?,
                date: row.get(3)?,
                detail,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    sources.extend(tests);
    sources.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.kind.cmp(&b.kind)));
    Ok(sources)
}

fn historical_activity_dates(state: &DatabaseState) -> Result<Vec<NaiveDate>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT activity_date FROM (
               SELECT DISTINCT date(event_time,'localtime') AS activity_date FROM conversation_messages WHERE event_time IS NOT NULL
               UNION
               SELECT DISTINCT date(event_time,'localtime') AS activity_date FROM token_events WHERE event_time IS NOT NULL
             ) WHERE activity_date IS NOT NULL ORDER BY activity_date",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        row.map_err(|error| error.to_string()).and_then(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| error.to_string())
        })
    })
    .collect()
}

fn rebuild_historical_reports_with_scan(
    state: &DatabaseState,
    scan: codex::CodexScanSummary,
) -> Result<HistoricalReportSummary, String> {
    let dates = historical_activity_dates(state)?;
    let mut week_starts = dates
        .iter()
        .map(|date| *date - Duration::days(date.weekday().num_days_from_monday().into()))
        .collect::<Vec<_>>();
    week_starts.sort_unstable();
    week_starts.dedup();
    let (conversations_total, archived_conversations_total, messages_total): (i64, i64, i64) =
        state
            .connect()?
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(archived),0),(SELECT COUNT(*) FROM conversation_messages) FROM conversations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|error| error.to_string())?;
    let first_date = dates.first().map(ToString::to_string);
    let last_date = dates.last().map(ToString::to_string);
    let mut summary = HistoricalReportSummary {
        active_days: dates.len(),
        active_weeks: week_starts.len(),
        daily_generated: 0,
        weekly_generated: 0,
        existing_skipped: 0,
        daily_updated: 0,
        weekly_updated: 0,
        locked_skipped: 0,
        files_scanned: scan.files_scanned,
        normal_files_scanned: scan.normal_files_scanned,
        archived_files_scanned: scan.archived_files_scanned,
        conversations_total,
        archived_conversations_total,
        messages_total,
        first_date,
        last_date,
    };
    for date in dates {
        match report_status(state, "daily", date)? {
            Some(status) if status == "locked" => {
                summary.existing_skipped += 1;
                summary.locked_skipped += 1;
            }
            Some(_) => {
                generate_for_date(state, "daily", date)?;
                summary.daily_updated += 1;
            }
            None => {
                generate_for_date(state, "daily", date)?;
                summary.daily_generated += 1;
            }
        }
    }
    for start in week_starts {
        match report_status(state, "weekly", start)? {
            Some(status) if status == "locked" => {
                summary.existing_skipped += 1;
                summary.locked_skipped += 1;
            }
            Some(_) => {
                generate_for_date(state, "weekly", start)?;
                summary.weekly_updated += 1;
            }
            None => {
                generate_for_date(state, "weekly", start)?;
                summary.weekly_generated += 1;
            }
        }
    }
    state
        .connect()?
        .execute(
            "INSERT INTO app_meta(key,value) VALUES('history_summary_version',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [HISTORY_SUMMARY_VERSION],
        )
        .map_err(|error| error.to_string())?;
    Ok(summary)
}

fn backfill_historical_reports_for_state(
    state: &DatabaseState,
) -> Result<HistoricalReportSummary, String> {
    let scan = codex::scan_codex_sessions_for_state(state)?;
    let _ = git::scan_git_repositories_for_state(state);
    rebuild_historical_reports_with_scan(state, scan)
}

pub fn sync_history_if_sources_changed(
    state: &DatabaseState,
) -> Result<Option<HistoricalReportSummary>, String> {
    let scan = codex::scan_codex_sessions_for_state(state)?;
    let _ = git::scan_git_repositories_for_state(state);
    if !scan.error_details.is_empty() {
        eprintln!(
            "Codex 历史扫描存在未导入文件：{}",
            scan.error_details.join("；")
        );
    }
    let current_version = state
        .connect()?
        .query_row(
            "SELECT value FROM app_meta WHERE key='history_summary_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if scan.conversations_imported == 0
        && current_version.as_deref() == Some(HISTORY_SUMMARY_VERSION)
    {
        return Ok(None);
    }
    rebuild_historical_reports_with_scan(state, scan).map(Some)
}

#[tauri::command]
pub async fn backfill_historical_reports(
    state: tauri::State<'_, DatabaseState>,
) -> Result<HistoricalReportSummary, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let summary = backfill_historical_reports_for_state(&database)?;
        crate::knowledge::sync_knowledge_for_state(&database)?;
        crate::suggestions::sync_task_suggestions_for_state(&database)?;
        Ok(summary)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn history_coverage(state: tauri::State<'_, DatabaseState>) -> Result<HistoryCoverage, String> {
    let dates = historical_activity_dates(&state)?;
    let mut weeks = dates
        .iter()
        .map(|date| *date - Duration::days(date.weekday().num_days_from_monday().into()))
        .collect::<Vec<_>>();
    weeks.sort_unstable();
    weeks.dedup();
    let (conversations, archived_conversations, messages, daily_reports, weekly_reports):
        (i64, i64, i64, i64, i64) = state.connect()?.query_row(
            "SELECT COUNT(*),COALESCE(SUM(archived),0),(SELECT COUNT(*) FROM conversation_messages),
              (SELECT COUNT(*) FROM reports WHERE report_type='daily'),
              (SELECT COUNT(*) FROM reports WHERE report_type='weekly') FROM conversations",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?)),
        ).map_err(|error| error.to_string())?;
    Ok(HistoryCoverage {
        conversations,
        archived_conversations,
        messages,
        active_days: dates.len(),
        active_weeks: weeks.len(),
        daily_reports,
        weekly_reports,
        first_date: dates.first().map(ToString::to_string),
        last_date: dates.last().map(ToString::to_string),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn daily_activity(
    state: tauri::State<'_, DatabaseState>,
    start_date: String,
    end_date: String,
) -> Result<Vec<DailyActivity>, String> {
    NaiveDate::parse_from_str(&start_date, "%Y-%m-%d").map_err(|error| error.to_string())?;
    NaiveDate::parse_from_str(&end_date, "%Y-%m-%d").map_err(|error| error.to_string())?;
    let work_summary = worktime::summary_for_range(&state, &start_date, &end_date, true)?;
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "WITH RECURSIVE dates(day) AS (
               SELECT date(?1) UNION ALL SELECT date(day,'+1 day') FROM dates WHERE day < date(?2)
             ), token_ordered AS (
               SELECT conversation_id,event_time,
                 MAX(input_tokens-LAG(input_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) input_delta,
                 MAX(cached_input_tokens-LAG(cached_input_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) cached_delta,
                 MAX(output_tokens-LAG(output_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) output_delta,
                 MAX(reasoning_output_tokens-LAG(reasoning_output_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) reasoning_delta,
                 MAX(total_tokens-LAG(total_tokens,1,0) OVER (PARTITION BY conversation_id ORDER BY event_time,id),0) total_delta
               FROM token_events WHERE event_time IS NOT NULL
             ), tokens AS (
               SELECT date(event_time,'localtime') day,SUM(input_delta) input_tokens,SUM(cached_delta) cached_tokens,
                 SUM(output_delta) output_tokens,SUM(reasoning_delta) reasoning_tokens,SUM(total_delta) total_tokens
               FROM token_ordered GROUP BY date(event_time,'localtime')
             ), messages AS (
               SELECT date(m.event_time,'localtime') day,COUNT(DISTINCT m.conversation_id) conversations,
                 COUNT(DISTINCT CASE WHEN c.archived=1 THEN m.conversation_id END) archived_conversations,COUNT(*) messages,
                 SUM(m.role='user') user_messages,SUM(m.role='assistant') assistant_messages
               FROM conversation_messages m JOIN conversations c ON c.id=m.conversation_id
               WHERE m.event_time IS NOT NULL GROUP BY date(m.event_time,'localtime')
             ), commits AS (
               SELECT date(gc.committed_at,'localtime') day,COUNT(*) commits
               FROM git_commits gc JOIN git_repositories gr ON gr.path=gc.repository_path
               WHERE gr.user_name='' OR lower(trim(gc.author_name))=lower(trim(gr.user_name))
               GROUP BY date(gc.committed_at,'localtime')
             )
             SELECT dates.day,COALESCE(messages.conversations,0),COALESCE(messages.archived_conversations,0),COALESCE(messages.messages,0),COALESCE(messages.user_messages,0),
               COALESCE(messages.assistant_messages,0),COALESCE(tokens.input_tokens,0),COALESCE(tokens.cached_tokens,0),
               COALESCE(tokens.output_tokens,0),COALESCE(tokens.reasoning_tokens,0),COALESCE(tokens.total_tokens,0),
               COALESCE(commits.commits,0),
               (SELECT COUNT(*) FROM content_ideas WHERE idea_date=dates.day),
               (SELECT id FROM reports WHERE report_type='daily' AND period_start=dates.day LIMIT 1),
               (SELECT id FROM reports WHERE report_type='weekly' AND dates.day BETWEEN period_start AND period_end LIMIT 1),
               (SELECT COUNT(*) FROM test_runs WHERE date(started_at,'localtime')=dates.day),
               (SELECT COUNT(*) FROM test_runs WHERE date(started_at,'localtime')=dates.day AND status='passed'),
               (SELECT COUNT(*) FROM knowledge_items WHERE date(updated_at,'localtime')=dates.day),
               (SELECT COUNT(*) FROM tasks WHERE date(updated_at,'localtime')=dates.day)
             FROM dates LEFT JOIN messages ON messages.day=dates.day LEFT JOIN tokens ON tokens.day=dates.day
               LEFT JOIN commits ON commits.day=dates.day ORDER BY dates.day",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![start_date, end_date], |row| {
            Ok(DailyActivity {
                date: row.get(0)?,
                conversation_count: row.get(1)?,
                archived_conversation_count: row.get(2)?,
                message_count: row.get(3)?,
                user_messages: row.get(4)?,
                assistant_messages: row.get(5)?,
                input_tokens: row.get(6)?,
                cached_input_tokens: row.get(7)?,
                output_tokens: row.get(8)?,
                reasoning_output_tokens: row.get(9)?,
                total_tokens: row.get(10)?,
                git_commits: row.get(11)?,
                content_idea_count: row.get(12)?,
                daily_report_id: row.get(13)?,
                weekly_report_id: row.get(14)?,
                test_runs: row.get(15)?,
                tests_passed: row.get(16)?,
                knowledge_count: row.get(17)?,
                task_activity_count: row.get(18)?,
                work_minutes: 0,
                estimated_work_minutes: 0,
                manual_work_minutes: 0,
            })
        })
        .map_err(|error| error.to_string())?;
    let mut activities = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let work_by_date = work_summary
        .daily
        .into_iter()
        .map(|item| (item.date.clone(), item))
        .collect::<HashMap<_, _>>();
    for activity in &mut activities {
        if let Some(work) = work_by_date.get(&activity.date) {
            activity.work_minutes = work.minutes;
            activity.estimated_work_minutes = work.estimated_minutes;
            activity.manual_work_minutes = work.manual_minutes;
        }
    }
    Ok(activities)
}

#[cfg(test)]
mod tests {
    use super::{
        add_test_section, add_work_time_section, build_content, clean_excerpt,
        conversation_work_summary, git_work_summary, is_low_value_process, project_label,
        redact_long_secrets, simplify_work_item, ConversationFact, GitFact, TaskFact, TestFact,
    };
    use crate::worktime::{WorkBreakdown, WorkSummary};
    use chrono::NaiveDate;

    #[test]
    fn report_states_estimated_hours_and_manual_correction_boundary() {
        let summary = WorkSummary {
            start_date: "2026-08-03".into(),
            end_date: "2026-08-03".into(),
            total_minutes: 440,
            estimated_minutes: 460,
            manual_minutes: 120,
            has_manual_corrections: true,
            by_project: vec![
                WorkBreakdown {
                    name: "client".into(),
                    minutes: 150,
                },
                WorkBreakdown {
                    name: "APP".into(),
                    minutes: 105,
                },
                WorkBreakdown {
                    name: "AI个人工作台".into(),
                    minutes: 185,
                },
            ],
            by_type: vec![],
            daily: vec![],
        };
        let content =
            add_work_time_section("# 日报\n\n## 项目工作总结\n".into(), "daily", &summary);
        assert!(content.contains("## 工时概览"));
        assert!(content.contains("今日工时：7小时20分钟（含手工修正；原始估算 7小时40分钟）"));
        assert!(content.contains("- client：2小时30分钟"));
        assert!(content.contains("不等同于考勤数据"));
    }

    #[test]
    fn report_summarizes_test_results_without_dumping_logs() {
        let content = add_test_section(
            "# 周报\n\n## 问题与风险\n".into(),
            "weekly",
            &[
                TestFact {
                    project: "client".into(),
                    menu_name: "案例分享".into(),
                    mode: "real".into(),
                    status: "passed".into(),
                    date: "2026-08-01".into(),
                    error_message: String::new(),
                },
                TestFact {
                    project: "APP".into(),
                    menu_name: "用户详情".into(),
                    mode: "source-style".into(),
                    status: "failed".into(),
                    date: "2026-08-02".into(),
                    error_message: "详情页缺少空状态处理".into(),
                },
            ],
        );
        assert!(content.contains("## 本周测试"));
        assert!(content.contains("共执行 2 次，通过 1 次，未通过 1 次"));
        assert!(content.contains(
            "[APP] 用户详情：未通过 · 页面源码与样式检查；核心问题：详情页缺少空状态处理"
        ));
        assert!(content.find("## 本周测试").unwrap() < content.find("## 问题与风险").unwrap());
    }

    #[test]
    fn excerpt_ignores_injected_context_blocks() {
        let content = "<recommended_plugins>\nHere is a list of plugins that are available but not installed.\n- Example\n</recommended_plugins>\n# AGENTS.md instructions\n<INSTRUCTIONS>\n内部规则\n</INSTRUCTIONS>";
        assert_eq!(clean_excerpt(content, 180), "");
    }

    #[test]
    fn excerpt_prefers_actual_referenced_conversation_request() {
        let content = "## Referenced ChatGPT conversation:\n不可信预览\n## My request for Codex:\n整理所有历史对话并生成日报和周报";
        assert_eq!(
            clean_excerpt(content, 180),
            "整理所有历史对话并生成日报和周报"
        );
    }

    #[test]
    fn project_label_uses_workspace_folder_name() {
        assert_eq!(
            project_label(r"C:\Users\11429\Documents\个人工作台"),
            "个人工作台"
        );
    }

    #[test]
    fn work_item_removes_goal_and_reference_link_wrappers() {
        assert_eq!(
            simplify_work_item(
                "/goal [部署方案](chatgpt-conversation://example) 完成 Jenkins 自动部署"
            ),
            "完成 Jenkins 自动部署"
        );
        assert_eq!(
            simplify_work_item(
                "[$skill](F:\\\\project\\\\SKILL.md) 在 safePackage 下创建安全会议功能"
            ),
            "在 safePackage 下创建安全会议功能"
        );
    }

    #[test]
    fn report_excerpt_redacts_long_credentials() {
        let source = "token使用固定的aB3xK9mP2qR7wE5tY8uI1oP4sD6fG9hJ2kL5zX7cV8bN0mQ1";
        assert_eq!(
            redact_long_secrets(source),
            "token使用固定的[已隐藏敏感信息]"
        );
    }

    #[test]
    fn report_groups_work_by_project_instead_of_conversation_timeline() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 3).unwrap();
        let content = build_content(
            "测试日报",
            "daily",
            date,
            date,
            &[TaskFact {
                title: "完成列表页".to_string(),
                project: "APP".to_string(),
                status: "done".to_string(),
                note: String::new(),
            }],
            &[ConversationFact {
                id: "conversation-1".to_string(),
                date: date.to_string(),
                project: "APP".to_string(),
                archived: true,
                requests: vec!["开发安全会议页面".to_string()],
                outcome: "列表和详情已完成".to_string(),
            }],
            &[],
            1,
            1,
            8,
            100,
            0,
            0,
            0,
        );
        assert!(content.contains("## 项目工作总结"));
        assert!(content.contains("### APP"));
        assert!(content.contains("列表和详情已完成（归档记录）"));
        assert!(!content.contains("每日工作轨迹"));
    }

    #[test]
    fn weekly_report_uses_feature_commits_and_filters_test_cleanup() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 27).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 8, 2).unwrap();
        let content = build_content(
            "测试周报",
            "weekly",
            start,
            end,
            &[],
            &[ConversationFact {
                id: "conversation-1".to_string(),
                date: "2026-07-31".to_string(),
                project: "client".to_string(),
                archived: false,
                requests: vec!["开发案例分享模块".to_string()],
                outcome: "已完成案例分享列表和详情功能".to_string(),
            }],
            &[
                GitFact {
                    date: "2026-07-31".to_string(),
                    project: "client".to_string(),
                    subject: "feat(safe/caseShare): 新增案例分享管理模块".to_string(),
                    file_count: 6,
                    additions: 1815,
                    deletions: 10,
                },
                GitFact {
                    date: "2026-07-31".to_string(),
                    project: "client".to_string(),
                    subject: "refactor(e2e): 移除自动化测试文件".to_string(),
                    file_count: 33,
                    additions: 62,
                    deletions: 4589,
                },
            ],
            1,
            0,
            8,
            100,
            2,
            1877,
            4599,
        );
        assert!(content.contains("新增案例分享管理模块（Git，6 个文件，+1815/-10）"));
        assert!(!content.contains("自动化测试文件"));
    }

    #[test]
    fn report_filters_service_shutdown_and_test_ignore_processes() {
        for (request, outcome) in [
            ("停止后端服务", "已全部停止，包括后端、Redis 和数据库"),
            ("忽略所有测试文件", "已在 .gitignore 中忽略测试相关目录"),
            (
                "检查实现结果",
                "核心筛选和作者归属的 10 项后端测试已全部通过",
            ),
            (
                "检查是否需要整改",
                "结论：目前没有必须整改的问题，静态检查结果为 0 个错误",
            ),
        ] {
            let fact = ConversationFact {
                id: "conversation".to_string(),
                date: "2026-08-03".to_string(),
                project: "client".to_string(),
                archived: false,
                requests: vec![request.to_string()],
                outcome: outcome.to_string(),
            };
            assert!(conversation_work_summary(&fact).is_none());
        }
    }

    #[test]
    fn report_filters_file_only_outcomes_and_translates_workbench_commits() {
        assert!(is_low_value_process("已修复 ReportAttachment/index.vue"));
        assert_eq!(
            git_work_summary("feat: track video production deliverables"),
            Some("新增视频生产流水线：按脚本、成片、封面和发布文案检查交付完整性".into())
        );
        assert_eq!(
            git_work_summary("feat: add weekly audit and toolchain checks"),
            Some("新增每周整体检查、漏跑补偿与工具链重复版本提示".into())
        );
    }

    #[test]
    fn report_converts_technical_progress_into_feature_summary() {
        let fact = ConversationFact {
            id: "conversation".to_string(),
            date: "2026-08-03".to_string(),
            project: "个人工作台".to_string(),
            archived: false,
            requests: vec!["优化报告".to_string()],
            outcome: "核心筛选和作者归属的 10 项后端测试已全部通过；已提升历史报告生成版本"
                .to_string(),
        };
        assert_eq!(
            conversation_work_summary(&fact).as_deref(),
            Some("优化历史日报和周报生成：按项目功能归类工作成果，并结合个人 Git 提交过滤低价值过程记录")
        );
    }
}

#[tauri::command(rename_all = "camelCase")]
pub fn generate_report(
    state: tauri::State<'_, DatabaseState>,
    report_type: String,
    reference_date: Option<String>,
) -> Result<ReportRecord, String> {
    let reference = reference_date
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| error.to_string())
        })
        .transpose()?
        .unwrap_or_else(|| Local::now().date_naive());
    let report = generate_for_date(&state, &report_type, reference)?;
    crate::suggestions::sync_task_suggestions_for_state(&state)?;
    Ok(report)
}

#[tauri::command]
pub fn save_report(
    state: tauri::State<'_, DatabaseState>,
    mut report: ReportRecord,
) -> Result<ReportRecord, String> {
    if report.status == "locked" {
        return Err("报告已锁定，不能直接编辑。".to_string());
    }
    report.updated_at = Utc::now().to_rfc3339();
    let mut connection = state.connect()?;
    save_report_record(&mut connection, &report)?;
    Ok(report)
}

#[tauri::command]
pub fn set_report_locked(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    locked: bool,
) -> Result<(), String> {
    let status = if locked { "locked" } else { "draft" };
    state
        .connect()?
        .execute(
            "UPDATE reports SET status=?1,updated_at=?2 WHERE id=?3",
            params![status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn report_exists(
    state: &DatabaseState,
    report_type: &str,
    reference: NaiveDate,
) -> Result<bool, String> {
    let (start, end) = report_period(report_type, reference)?;
    let count: i64 = state.connect()?.query_row(
        "SELECT COUNT(*) FROM reports WHERE report_type=?1 AND period_start=?2 AND period_end=?3",
        params![report_type,start.format("%Y-%m-%d").to_string(),end.format("%Y-%m-%d").to_string()],
        |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    Ok(count > 0)
}

fn report_status(
    state: &DatabaseState,
    report_type: &str,
    reference: NaiveDate,
) -> Result<Option<String>, String> {
    let (start, end) = report_period(report_type, reference)?;
    state
        .connect()?
        .query_row(
            "SELECT status FROM reports WHERE report_type=?1 AND period_start=?2 AND period_end=?3 LIMIT 1",
            params![report_type,start.format("%Y-%m-%d").to_string(),end.format("%Y-%m-%d").to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn scheduled_types(reference: NaiveDate) -> Vec<&'static str> {
    let mut types = vec!["daily"];
    if reference.weekday() == chrono::Weekday::Sun {
        types.push("weekly");
    }
    if (reference + Duration::days(1)).month() != reference.month() {
        types.push("monthly");
    }
    types
}

pub fn ensure_scheduled_reports(state: &DatabaseState) -> Result<Vec<ReportRecord>, String> {
    let now = Local::now();
    if now.hour() < 22 {
        return Ok(Vec::new());
    }
    let today = now.date_naive();
    let mut generated = Vec::new();
    for report_type in scheduled_types(today) {
        if !report_exists(state, report_type, today)? {
            generated.push(generate_for_date(state, report_type, today)?);
        }
    }
    Ok(generated)
}
