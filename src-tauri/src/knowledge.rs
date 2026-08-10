use crate::{
    database::DatabaseState,
    reports::{
        clean_excerpt, clean_knowledge_excerpt, is_low_value_process, is_meaningful_work_text,
        project_label, simplify_work_item,
    },
};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Stdio,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub content: String,
    pub project: Option<String>,
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub tags: String,
    pub confirmed: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSyncSummary {
    pub conversations_scanned: usize,
    pub items_generated: usize,
    pub decisions: usize,
    pub experiences: usize,
    pub risks: usize,
    pub skills: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeVersion {
    pub id: String,
    pub knowledge_id: String,
    pub version_number: i64,
    pub title: String,
    pub content: String,
    pub tags: String,
    pub change_source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCodexJob {
    pub id: String,
    pub knowledge_id: String,
    pub repository_path: String,
    pub instruction: String,
    pub status: String,
    pub thread_id: Option<String>,
    pub output: String,
    pub error_message: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Default)]
struct ConversationKnowledge {
    id: String,
    title: String,
    project: String,
    archived: bool,
    current_request: String,
    outcome_request: String,
    outcome: String,
    outcome_score: i32,
}

struct GeneratedKnowledge {
    id: String,
    kind: String,
    title: String,
    content: String,
    project: String,
    source_id: String,
    tags: String,
    score: i32,
}

fn canonical_key(project: &str, title: &str) -> String {
    format!("{}|{}", project.trim(), title.trim())
        .to_lowercase()
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !"，。！？、,:：;；()（）[]【】".contains(*character)
        })
        .collect()
}

fn stable_auto_id(project: &str, title: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in canonical_key(project, title).bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("auto:{hash:016x}")
}

fn save_version(
    transaction: &rusqlite::Transaction<'_>,
    knowledge_id: &str,
    title: &str,
    content: &str,
    tags: &str,
    change_source: &str,
    created_at: &str,
) -> Result<(), String> {
    let version_number: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(version_number),0)+1 FROM knowledge_versions WHERE knowledge_id=?1",
            [knowledge_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO knowledge_versions(id,knowledge_id,version_number,title,content,tags,change_source,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![Uuid::new_v4().to_string(),knowledge_id,version_number,title,content,tags,change_source,created_at],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn knowledge_kind(content: &str) -> &'static str {
    if ["风险", "阻塞", "失败", "限制", "不支持", "注意", "隐患"]
        .iter()
        .any(|word| content.contains(word))
    {
        "risk"
    } else if [
        "采用", "决定", "选择", "确认", "架构", "方案", "规则", "统一", "沿用",
    ]
    .iter()
    .any(|word| content.contains(word))
    {
        "decision"
    } else if ["技能", "能力", "掌握", "学会", "规范", "流程", "Skill"]
        .iter()
        .any(|word| content.contains(word))
    {
        "skill"
    } else {
        "experience"
    }
}

fn is_low_value_knowledge_request(content: &str) -> bool {
    is_low_value_process(content)
        || [
            "查看目录结构",
            "整理组件",
            "安装插件",
            "安装compute use",
            "codex-pets add",
            "运行项目",
            "怎么更新codex",
            "关闭程序",
            "打开程序",
        ]
        .iter()
        .any(|word| content.to_lowercase().contains(&word.to_lowercase()))
}

fn knowledge_candidate_score(content: &str) -> i32 {
    if content.chars().count() < 45 || is_low_value_process(content) {
        return -100;
    }
    let mut score = (content.chars().count() / 120).min(6) as i32;
    for word in [
        "采用", "建议", "推荐", "原因", "解决", "实现", "配置", "规则", "统一", "必须", "避免",
        "注意", "限制", "步骤", "接口", "组件", "字段", "验证", "部署", "架构",
    ] {
        if content.contains(word) {
            score += 2;
        }
    }
    for word in [
        "我正在",
        "接下来我会",
        "稍后",
        "测试结果",
        "diff 已",
        "准备继续",
    ] {
        if content.contains(word) {
            score -= 4;
        }
    }
    score
}

fn mapped_topic(content: &str) -> Option<(&'static str, &'static str)> {
    let lower = content.to_lowercase();
    let mappings = [
        (
            ["token", "codex", "统计"].as_slice(),
            "Codex Token 统计",
            "Codex Token 应如何按会话和日期准确统计？",
        ),
        (
            ["电子签名", "2.2:1"].as_slice(),
            "电子签名组件",
            "电子签名组件如何统一尺寸、状态和交互？",
        ),
        (
            ["字典", "接口"].as_slice(),
            "业务字典接入",
            "页面开发时如何自动创建并接入业务字典？",
        ),
        (
            ["自动化测试", "playwright"].as_slice(),
            "前端自动化测试",
            "前端功能如何建立可重复执行的真实接口测试？",
        ),
        (
            ["uni-app", "h5"].as_slice(),
            "uni-app H5 页面开发",
            "uni-app H5 页面如何复用现有项目规范开发？",
        ),
        (
            ["jenkins", "发布"].as_slice(),
            "H5 自动发布",
            "uni-app H5 如何配置可验证的自动发布流程？",
        ),
        (
            ["jenkins", "部署"].as_slice(),
            "H5 自动发布",
            "uni-app H5 如何配置可验证的自动发布流程？",
        ),
        (
            ["schema", "审批"].as_slice(),
            "审批详情渲染",
            "审批详情如何使用 Schema 实现动态渲染？",
        ),
        (
            ["消息", "跳转"].as_slice(),
            "消息路由",
            "业务消息如何按类型跳转到正确页面？",
        ),
        (
            ["高德地图", "区域"].as_slice(),
            "高德地图区域编辑",
            "单位区域如何接入高德地图进行展示和编辑？",
        ),
        (
            ["docker", "spring boot"].as_slice(),
            "Spring Boot 本地容器",
            "Spring Boot 项目如何使用 Docker 在本地稳定运行？",
        ),
        (
            ["nfc", "h5"].as_slice(),
            "H5 NFC 能力",
            "H5 使用 NFC 时有哪些兼容限制和实现方式？",
        ),
        (
            ["skill", "页面"].as_slice(),
            "页面生成规范",
            "如何把项目页面开发规则沉淀为可复用 Skill？",
        ),
        (
            ["报告", "git"].as_slice(),
            "个人工作报告",
            "个人日报和周报如何提炼功能成果而非对话流水？",
        ),
        (
            ["案例分享", "flowcode"].as_slice(),
            "案例分享流程",
            "案例分享功能的状态、接口和审批流程如何设计？",
        ),
        (
            ["安全会议", "titlebox"].as_slice(),
            "安全会议页面",
            "安全会议页面如何复用现有 H5 组件与样式？",
        ),
        (
            ["流程管理", "tabsbox"].as_slice(),
            "流程与任务页面样式",
            "流程管理与任务页面如何统一查询、表格和 Tab 样式？",
        ),
        (
            ["feat(", "提交"].as_slice(),
            "Git 提交拆分",
            "项目改动如何拆分并生成清晰的中文 Git 提交信息？",
        ),
        (
            ["luna", "terra"].as_slice(),
            "Codex 模型选择",
            "不同 Codex 模型分别适合处理什么任务？",
        ),
        (
            ["leagueclient", "队友"].as_slice(),
            "LCU 对局数据读取",
            "如何读取英雄联盟客户端对局数据并处理匿名阶段？",
        ),
        (
            ["ddc/ci", "usb-c"].as_slice(),
            "显示器输入源切换",
            "双设备如何通过 DDC/CI 稳定切换显示器输入源？",
        ),
        (
            ["sqlite", "个人工作台"].as_slice(),
            "本地个人工作台架构",
            "本地 AI 个人工作台应如何选型和组织核心模块？",
        ),
        (
            ["轻量化", "个人工作台"].as_slice(),
            "本地个人工作台架构",
            "本地 AI 个人工作台应如何选型和组织核心模块？",
        ),
        (
            ["sunshine", "moonlight"].as_slice(),
            "远程桌面串流",
            "如何使用 Sunshine 与 Moonlight 远程串流电脑画面？",
        ),
        (
            ["新版引导", "续费"].as_slice(),
            "产品改版效果分析",
            "如何验证产品改版是否导致客户续费下降？",
        ),
        (
            ["8081", "验证码接口"].as_slice(),
            "本地前后端联调",
            "本地前端联调时如何选择并验证后端服务？",
        ),
        (
            ["listvisible", "defext"].as_slice(),
            "流程列表字段显示",
            "流程任务列表不显示业务字段时如何排查 Schema 配置？",
        ),
        (
            ["approvalrecordtimeline", "filespreview"].as_slice(),
            "移动端流程公共组件",
            "移动端审批记录与文件预览组件应如何封装？",
        ),
        (
            ["scaq_case_share", "processstatus"].as_slice(),
            "案例分享流程",
            "案例分享功能的状态、接口和审批流程如何设计？",
        ),
        (
            ["线上环境", "按钮", "权限"].as_slice(),
            "按钮权限差异",
            "页面按钮在不同环境显示不一致时如何排查权限？",
        ),
        (
            ["tab", "没生效"].as_slice(),
            "Tab 样式作用域",
            "Tab 样式只在部分页面生效时如何排查？",
        ),
        (
            ["跳过登录", "请求头"].as_slice(),
            "本地免登录调试",
            "本地开发如何绕过登录并避免令牌泄露？",
        ),
        (
            ["hatch pet", "spritesheet"].as_slice(),
            "Codex 宠物制作",
            "如何把角色图制作成完整可用的 Codex 动画宠物？",
        ),
        (
            ["登录页", "动画"].as_slice(),
            "登录页动态视觉",
            "登录页如何实现可维护的动态视觉效果？",
        ),
        (
            ["只读", "风险", "项目"].as_slice(),
            "前端项目质量审计",
            "前端项目如何开展只读质量审计并按风险分级？",
        ),
        (
            ["产品原型", "1440"].as_slice(),
            "桌面工作台原型",
            "桌面工作台原型如何验证布局、信息层级和操作路径？",
        ),
        (
            ["百度", "秒退"].as_slice(),
            "Codex 网页闪退",
            "Codex 打开网页闪退时如何排查代理和应用容器？",
        ),
        (
            ["独立列表页", "表单页", "详情页"].as_slice(),
            "移动端业务模块拆分",
            "移动端业务模块如何拆分列表、表单和详情页？",
        ),
    ];
    mappings
        .into_iter()
        .find(|(keywords, _, _)| keywords.iter().all(|word| lower.contains(word)))
        .map(|(_, topic, title)| (topic, title))
}

fn is_clear_generic_topic(topic: &str) -> bool {
    let length = topic.chars().count();
    if !(6..=24).contains(&length) {
        return false;
    }
    if [
        "没有",
        "自己设计",
        "现在",
        "为什么",
        "帮我",
        "看看",
        "完全使用",
        "有点不一样",
        "这个问题",
        "The element",
        "调用 Hatch",
        "扫描读取整个项目",
    ]
    .iter()
    .any(|word| topic.contains(word))
    {
        return false;
    }
    let chinese_count = topic
        .chars()
        .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
        .count();
    chinese_count >= 4
        && [
            "组件",
            "页面",
            "接口",
            "流程",
            "部署",
            "地图",
            "字典",
            "签名",
            "权限",
            "路由",
            "报告",
            "Token",
            "Docker",
            "NFC",
            "Schema",
            "Jenkins",
            "Git",
            "测试",
            "数据",
            "样式",
            "登录",
            "按钮",
            "服务",
            "模型",
            "工作台",
        ]
        .iter()
        .any(|word| topic.contains(word))
}

fn topic_content_is_actionable(topic: &str, outcome: &str) -> bool {
    match topic {
        "业务字典接入" => [
            "创建字典",
            "插入字典",
            "dict/type",
            "dict/data",
            "字典类型",
            "字典值",
        ]
        .iter()
        .any(|word| outcome.contains(word)),
        _ => true,
    }
}

fn generic_topic(fact: &ConversationKnowledge) -> String {
    let source = if fact.outcome_request.trim().is_empty() {
        &fact.title
    } else {
        &fact.outcome_request
    };
    let mut value = source
        .split(['。', '！', '？', '；', ';'])
        .next()
        .unwrap_or(source)
        .trim()
        .trim_start_matches(['#', '-', '*', '>', ' ']);
    for prefix in [
        "请帮我",
        "帮我",
        "我现在需要",
        "我需要",
        "现在需要",
        "需要",
        "请",
        "把",
        "增加一个",
        "新增一个",
    ] {
        value = value.strip_prefix(prefix).unwrap_or(value).trim();
    }
    if let Some((_, name)) = value.split_once("页面名称为") {
        value = name.trim();
    }
    let shortened = value.chars().take(30).collect::<String>();
    if shortened.trim().is_empty() {
        format!("{}功能", fact.project)
    } else {
        shortened
            .trim_end_matches([',', '，', ':', '：', '?', '？'])
            .to_string()
    }
}

fn strip_markdown_links(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '[' {
            if let Some(label_end) = chars[index + 1..].iter().position(|item| *item == ']') {
                let label_end = index + 1 + label_end;
                if chars.get(label_end + 1) == Some(&'(') {
                    if let Some(link_end) =
                        chars[label_end + 2..].iter().position(|item| *item == ')')
                    {
                        output.extend(chars[index + 1..label_end].iter());
                        index = label_end + 2 + link_end + 1;
                        continue;
                    }
                }
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

fn clean_knowledge_sentence(value: &str) -> String {
    let without_links = strip_markdown_links(value);
    let mut cleaned = without_links
        .trim()
        .trim_start_matches(|character: char| {
            character.is_ascii_digit()
                || matches!(character, '.' | '、' | ')' | '）' | '-' | '*' | '#' | ' ')
        })
        .trim();
    for prefix in [
        "已完成：",
        "已完成并验证：",
        "已完成",
        "已经完成",
        "总结结果：",
        "结论：",
        "本次",
    ] {
        cleaned = cleaned.strip_prefix(prefix).unwrap_or(cleaned).trim();
    }
    cleaned
        .replace("**", "")
        .replace('`', "")
        .chars()
        .take(180)
        .collect::<String>()
}

fn knowledge_sentences(content: &str) -> Vec<String> {
    content
        .replace("。", "\n")
        .replace('；', "\n")
        .replace(';', "\n")
        .replace('！', "\n")
        .replace('？', "\n")
        .lines()
        .map(clean_knowledge_sentence)
        .filter(|value| value.chars().count() >= 8)
        .filter(|value| !is_low_value_process(value))
        .filter(|value| {
            ![
                "测试结果",
                "静态检查",
                "构建通过",
                "diff",
                "接下来",
                "我会继续",
                "只读排查",
                "暂未修改",
                "计划新增",
                "MEMORY.md",
                "rollout_summaries",
                "citation_entries",
            ]
            .iter()
            .any(|word| value.contains(word))
        })
        .fold(Vec::<String>::new(), |mut values, value| {
            if !values.contains(&value) {
                values.push(value);
            }
            values
        })
}

fn choose_sentence(sentences: &[String], keywords: &[&str]) -> Option<String> {
    sentences
        .iter()
        .filter_map(|sentence| {
            let hits = keywords
                .iter()
                .filter(|word| sentence.contains(**word))
                .count();
            (hits > 0).then_some((hits, sentence.chars().count(), sentence))
        })
        .max_by_key(|(hits, length, _)| (*hits, (*length).min(120)))
        .map(|(_, _, sentence)| sentence.clone())
}

fn build_knowledge(fact: &ConversationKnowledge) -> Option<GeneratedKnowledge> {
    if fact.outcome_score < 7 || fact.outcome.trim().is_empty() {
        return None;
    }
    let work = fact.outcome_request.trim();
    let combined = format!("{work} {}", fact.outcome);
    if work.is_empty() || is_low_value_knowledge_request(work) {
        return None;
    }
    let kind = if ["风险", "限制", "避坑", "不支持"]
        .iter()
        .any(|word| work.contains(word))
    {
        "risk"
    } else if ["规范", "流程", "步骤", "Skill", "skill", "文档"]
        .iter()
        .any(|word| combined.contains(word))
    {
        "skill"
    } else {
        knowledge_kind(&combined)
    }
    .to_string();
    let topic_source = format!("{work}\n{}", fact.outcome);
    let mapped = mapped_topic(&topic_source);
    let (topic, title) = if let Some((topic, title)) = mapped {
        (topic.to_string(), title.to_string())
    } else {
        let topic = generic_topic(fact);
        if !is_clear_generic_topic(&topic) {
            return None;
        }
        let title = match kind.as_str() {
            "decision" => format!("{topic}应采用什么方案？"),
            "risk" => format!("{topic}有哪些限制和避坑点？"),
            "skill" => format!("{topic}的标准操作方法"),
            _ => format!("{topic}如何实现和排查？"),
        };
        (topic, title)
    };
    if title.chars().count() > 48 || is_low_value_knowledge_request(&title) {
        return None;
    }
    let sentences = knowledge_sentences(&fact.outcome);
    if sentences.is_empty() {
        return None;
    }
    let core = choose_sentence(
        &sentences,
        &[
            "采用", "推荐", "建议", "应", "统一", "使用", "配置", "读取", "调用", "核心", "规则",
            "方案",
        ],
    )
    .or_else(|| sentences.first().cloned())?;
    let caution = choose_sentence(
        &sentences,
        &[
            "注意", "不要", "避免", "必须", "限制", "风险", "仅", "不能", "否则",
        ],
    )
    .filter(|sentence| sentence != &core);
    let verification = choose_sentence(
        &sentences,
        &["验证", "检查", "确认", "成功", "通过", "返回", "显示"],
    )
    .filter(|sentence| sentence != &core)
    .filter(|sentence| caution.as_ref() != Some(sentence));
    let mut steps = sentences
        .iter()
        .filter(|sentence| sentence.as_str() != core)
        .filter(|sentence| caution.as_ref() != Some(*sentence))
        .filter(|sentence| verification.as_ref() != Some(*sentence))
        .filter(|sentence| {
            [
                "使用", "先", "再", "配置", "读取", "调用", "创建", "新增", "修改", "设置", "将",
                "保持", "接入", "拆分", "复用", "校验", "统一", "加载", "保存",
            ]
            .iter()
            .any(|word| sentence.contains(word))
        })
        .take(4)
        .cloned()
        .collect::<Vec<_>>();
    if steps.is_empty() {
        steps.extend(
            sentences
                .iter()
                .filter(|sentence| sentence.as_str() != core)
                .take(2)
                .cloned(),
        );
    }
    if steps.is_empty() && caution.is_none() {
        return None;
    }

    let source_text = format!("{work}\n{}", fact.outcome);
    let mut reference_files = source_text
        .split_whitespace()
        .map(|value| {
            value.trim_matches(|character: char| {
                matches!(
                    character,
                    '`' | '"'
                        | '\''
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '，'
                        | '。'
                        | '：'
                        | ':'
                        | ';'
                        | '；'
                )
            })
        })
        .filter(|value| {
            let lower = value.to_lowercase();
            (value.contains('/') || value.contains('\\'))
                && [
                    ".vue", ".ts", ".tsx", ".js", ".rs", ".java", ".json", ".md", ".yml", ".yaml",
                    ".ps1", ".cmd",
                ]
                .iter()
                .any(|extension| lower.contains(extension))
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    reference_files.sort();
    reference_files.dedup();
    let mut sections = vec![
        "## 适用场景".to_string(),
        format!("需要在 {} 项目中处理“{}”相关需求时。", fact.project, topic),
        String::new(),
        "## 实现方法".to_string(),
        core,
        String::new(),
        "## 操作步骤".to_string(),
    ];
    sections.extend(
        steps
            .iter()
            .enumerate()
            .map(|(index, step)| format!("{}. {step}", index + 1)),
    );
    if let Some(verification) = verification.as_ref() {
        sections.push(format!("{}. 验证结果：{verification}", steps.len() + 1));
    }
    sections.extend([String::new(), "## 注意事项".to_string()]);
    sections.push(caution.unwrap_or_else(|| {
        "只在适用场景和项目约束一致时复用，并在提交前核对真实代码与运行结果。".to_string()
    }));
    sections.extend([String::new(), "## 参考文件".to_string()]);
    if reference_files.is_empty() {
        sections
            .push("- 来源中未识别到具体文件路径，请通过下方来源记录打开原对话核对。".to_string());
    } else {
        sections.extend(
            reference_files
                .iter()
                .take(8)
                .map(|path| format!("- `{path}`")),
        );
    }
    sections.extend([
        String::new(),
        "## 来源记录".to_string(),
        format!(
            "- Codex 对话：{}{}",
            fact.title,
            if fact.archived { "（归档）" } else { "" }
        ),
    ]);
    let content = sections.join("\n");
    if !topic_content_is_actionable(&topic, &content) {
        return None;
    }
    let mut tags = vec!["可复用方法".to_string(), fact.project.clone(), topic];
    if fact.archived {
        tags.push("归档对话".to_string());
    }
    Some(GeneratedKnowledge {
        id: stable_auto_id(&fact.project, &title),
        kind,
        title,
        content,
        project: fact.project.clone(),
        source_id: fact.id.clone(),
        tags: tags.join(","),
        score: fact.outcome_score,
    })
}

pub fn sync_knowledge_for_state(state: &DatabaseState) -> Result<KnowledgeSyncSummary, String> {
    let mut connection = state.connect()?;
    let facts = {
        let mut statement = connection
            .prepare(
                "SELECT c.id,COALESCE(NULLIF(c.title,''),'未命名会话'),COALESCE(NULLIF(c.project_override,''),COALESCE(NULLIF(c.cwd,''),'未归类项目')),
                 c.archived,m.role,m.content
                 FROM conversations c LEFT JOIN conversation_messages m ON m.conversation_id=c.id
                 ORDER BY c.id,m.source_index",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)? != 0,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut facts = Vec::<ConversationKnowledge>::new();
        let mut indexes = HashMap::<String, usize>::new();
        for row in rows {
            let (id, title, project, archived, role, content) =
                row.map_err(|error| error.to_string())?;
            let index = *indexes.entry(id.clone()).or_insert_with(|| {
                facts.push(ConversationKnowledge {
                    id: id.clone(),
                    title: clean_excerpt(&title, 100),
                    project: project_label(&project),
                    archived,
                    current_request: String::new(),
                    outcome_request: String::new(),
                    outcome: String::new(),
                    outcome_score: -100,
                });
                facts.len() - 1
            });
            let Some(role) = role else { continue };
            let Some(content) = content else { continue };
            let excerpt = if role == "user" {
                simplify_work_item(&clean_excerpt(&content, 220))
            } else {
                simplify_work_item(&clean_knowledge_excerpt(&content, 4000))
            };
            if !is_meaningful_work_text(&excerpt) {
                continue;
            }
            if role == "user" {
                facts[index].current_request.clear();
                if !is_low_value_knowledge_request(&excerpt) {
                    facts[index].current_request = excerpt;
                }
            } else if role == "assistant" {
                let score = knowledge_candidate_score(&excerpt);
                if score > facts[index].outcome_score
                    && !facts[index].current_request.trim().is_empty()
                {
                    facts[index].outcome = excerpt;
                    facts[index].outcome_request = facts[index].current_request.clone();
                    facts[index].outcome_score = score;
                }
            }
        }
        facts
    };

    let mut summary = KnowledgeSyncSummary {
        conversations_scanned: facts.len(),
        ..KnowledgeSyncSummary::default()
    };
    let mut deduplicated = HashMap::<(String, String), GeneratedKnowledge>::new();
    for fact in &facts {
        let Some(item) = build_knowledge(fact) else {
            continue;
        };
        let key = (item.project.clone(), item.title.clone());
        if deduplicated
            .get(&key)
            .is_none_or(|existing| item.score > existing.score)
        {
            deduplicated.insert(key, item);
        }
    }
    let manual_keys = {
        let mut statement = connection
            .prepare("SELECT COALESCE(project,''),title FROM knowledge_items WHERE source_type='manual' OR id NOT LIKE 'auto:%'")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        rows.filter_map(Result::ok)
            .map(|(project, title)| canonical_key(&project, &title))
            .collect::<HashSet<_>>()
    };
    let mut generated = deduplicated
        .into_values()
        .filter(|item| !manual_keys.contains(&canonical_key(&item.project, &item.title)))
        .collect::<Vec<_>>();
    generated.sort_by(|left, right| {
        left.project
            .cmp(&right.project)
            .then(left.title.cmp(&right.title))
    });

    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();
    let mut active_ids = HashSet::new();
    for item in generated {
        active_ids.insert(item.id.clone());
        let existing = transaction
            .query_row(
                "SELECT title,content,tags FROM knowledge_items WHERE id=?1",
                [&item.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some((title, content, tags)) = existing.as_ref() {
            if title != &item.title || content != &item.content || tags != &item.tags {
                save_version(
                    &transaction,
                    &item.id,
                    title,
                    content,
                    tags,
                    "auto_sync",
                    &now,
                )?;
            }
        }
        transaction
            .execute(
                "INSERT INTO knowledge_items(id,kind,title,content,project,source_type,source_id,tags,confirmed,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,'conversation',?6,?7,1,?8,?8)
                 ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,title=excluded.title,content=excluded.content,project=excluded.project,source_id=excluded.source_id,tags=excluded.tags,updated_at=excluded.updated_at",
                params![
                    item.id,
                    item.kind,
                    item.title,
                    item.content,
                    item.project,
                    item.source_id,
                    item.tags,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
        summary.items_generated += 1;
        match item.kind.as_str() {
            "decision" => summary.decisions += 1,
            "risk" => summary.risks += 1,
            "skill" => summary.skills += 1,
            _ => summary.experiences += 1,
        }
    }
    let stale_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM knowledge_items WHERE id LIKE 'auto:%' AND source_type='conversation'")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.filter_map(Result::ok)
            .filter(|id| !active_ids.contains(id))
            .collect::<Vec<_>>()
    };
    for id in stale_ids {
        transaction
            .execute("DELETE FROM knowledge_items WHERE id=?1", [id])
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(summary)
}

#[tauri::command]
pub async fn sync_knowledge(
    state: tauri::State<'_, DatabaseState>,
) -> Result<KnowledgeSyncSummary, String> {
    let database = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || sync_knowledge_for_state(&database))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn list_knowledge(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<KnowledgeItem>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare(
        "SELECT id,kind,title,content,project,source_type,source_id,tags,confirmed,created_at,updated_at
         FROM knowledge_items ORDER BY confirmed DESC,updated_at DESC",
    ).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(KnowledgeItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                project: row.get(4)?,
                source_type: row.get(5)?,
                source_id: row.get(6)?,
                tags: row.get(7)?,
                confirmed: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_knowledge(
    state: tauri::State<'_, DatabaseState>,
    mut item: KnowledgeItem,
) -> Result<KnowledgeItem, String> {
    if item.title.trim().is_empty() || item.content.trim().is_empty() {
        return Err("知识标题和内容不能为空。".to_string());
    }
    let now = Utc::now().to_rfc3339();
    if item.id.trim().is_empty() {
        item.id = Uuid::new_v4().to_string();
        item.created_at = now.clone();
    }
    item.updated_at = now;
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let existing = transaction
        .query_row(
            "SELECT title,content,tags FROM knowledge_items WHERE id=?1",
            [&item.id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some((title, content, tags)) = existing.as_ref() {
        if title != &item.title || content != &item.content || tags != &item.tags {
            save_version(
                &transaction,
                &item.id,
                title,
                content,
                tags,
                "manual_edit",
                &item.updated_at,
            )?;
        }
    }
    transaction.execute(
        "INSERT INTO knowledge_items(id,kind,title,content,project,source_type,source_id,tags,confirmed,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(id) DO UPDATE SET kind=excluded.kind,title=excluded.title,content=excluded.content,project=excluded.project,source_type=excluded.source_type,source_id=excluded.source_id,tags=excluded.tags,confirmed=excluded.confirmed,updated_at=excluded.updated_at",
        params![item.id,item.kind,item.title,item.content,item.project,item.source_type,item.source_id,item.tags,item.confirmed,item.created_at,item.updated_at],
    ).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(item)
}

#[tauri::command]
pub fn list_knowledge_versions(
    state: tauri::State<'_, DatabaseState>,
    knowledge_id: String,
) -> Result<Vec<KnowledgeVersion>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare(
            "SELECT id,knowledge_id,version_number,title,content,tags,change_source,created_at
         FROM knowledge_versions WHERE knowledge_id=?1 ORDER BY version_number DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([knowledge_id], |row| {
            Ok(KnowledgeVersion {
                id: row.get(0)?,
                knowledge_id: row.get(1)?,
                version_number: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                tags: row.get(5)?,
                change_source: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn knowledge_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<KnowledgeCodexJob> {
    Ok(KnowledgeCodexJob {
        id: row.get(0)?,
        knowledge_id: row.get(1)?,
        repository_path: row.get(2)?,
        instruction: row.get(3)?,
        status: row.get(4)?,
        thread_id: row.get(5)?,
        output: row.get(6)?,
        error_message: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

#[tauri::command]
pub fn list_knowledge_codex_jobs(
    state: tauri::State<'_, DatabaseState>,
    knowledge_id: Option<String>,
) -> Result<Vec<KnowledgeCodexJob>, String> {
    let connection = state.connect()?;
    let query = if knowledge_id.is_some() {
        "SELECT id,knowledge_id,repository_path,instruction,status,thread_id,output,error_message,created_at,updated_at FROM knowledge_codex_jobs WHERE knowledge_id=?1 ORDER BY created_at DESC"
    } else {
        "SELECT id,knowledge_id,repository_path,instruction,status,thread_id,output,error_message,created_at,updated_at FROM knowledge_codex_jobs WHERE ?1 IS NULL ORDER BY created_at DESC LIMIT 100"
    };
    let mut statement = connection
        .prepare(query)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([knowledge_id], knowledge_job)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn finish_knowledge_job(
    state: &DatabaseState,
    job_id: &str,
    knowledge_id: &str,
    status: &str,
    thread_id: Option<&str>,
    output: &str,
    error_message: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let connection = state.connect()?;
    connection.execute(
        "UPDATE knowledge_codex_jobs SET status=?2,thread_id=?3,output=?4,error_message=?5,updated_at=?6 WHERE id=?1",
        params![job_id,status,thread_id,output,error_message,now],
    ).map_err(|error| error.to_string())?;
    let title = if status == "completed" {
        "知识实践已完成"
    } else {
        "知识实践需要处理"
    };
    connection.execute(
        "INSERT INTO notifications(id,kind,title,body,output,source_id,route,is_read,created_at,read_at)
         VALUES(?1,'codex_task',?2,?3,?4,?5,?6,0,?7,NULL)
         ON CONFLICT(id) DO UPDATE SET title=excluded.title,body=excluded.body,output=excluded.output,is_read=0,created_at=excluded.created_at,read_at=NULL",
        params![format!("knowledge-codex:{job_id}"),title,if status == "completed" {"Codex 已完成知识实践，请检查输出和代码。"} else {"Codex 执行失败，请查看错误信息。"},if output.trim().is_empty(){error_message}else{output},job_id,format!("/knowledge?item={knowledge_id}"),now],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

fn run_knowledge_codex_job(
    state: DatabaseState,
    job_id: String,
    item: KnowledgeItem,
    repository_path: String,
    instruction: String,
) -> Result<(), String> {
    let repository = Path::new(&repository_path);
    let (cli_path, _) = crate::codex_video::resolve_codex_cli()?;
    let prompt = format!(
        "请在项目 `{}` 中参考下面这条已确认知识完成实践。\n\n标题：{}\n\n内容：\n{}\n\n补充要求：{}\n\n先检查项目真实代码和约束，只做与要求直接相关的最小修改；不得提交、推送、重置或删除用户文件；完成后运行与改动相称的验证，并用中文说明完成内容、文件和验证结果。",
        repository.display(), item.title, item.content, if instruction.trim().is_empty() { "无" } else { instruction.trim() }
    );
    let job_dir = state
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("knowledge-codex-jobs")
        .join(&job_id);
    fs::create_dir_all(&job_dir).map_err(|error| error.to_string())?;
    let jsonl_path = job_dir.join("codex-run.jsonl");
    let stderr_path = job_dir.join("codex-stderr.log");
    let last_message_path = job_dir.join("codex-last-message.md");
    let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let mut command = crate::codex_video::hidden_command(&cli_path);
    command
        .args(["--sandbox", "workspace-write", "--cd"])
        .arg(repository)
        .args(["exec", "--json", "--skip-git-repo-check"])
        .arg("--output-last-message")
        .arg(&last_message_path)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_file));
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 Codex CLI 失败：{error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Codex 输出。".to_string())?;
    let mut log = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(jsonl_path)
        .map_err(|error| error.to_string())?;
    let mut thread_id = None;
    let mut streamed_output = String::new();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        writeln!(log, "{line}").map_err(|error| error.to_string())?;
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("type").and_then(Value::as_str) == Some("thread.started") {
                thread_id = value
                    .get("thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            if value.get("type").and_then(Value::as_str) == Some("item.completed")
                && value.pointer("/item/type").and_then(Value::as_str) == Some("agent_message")
            {
                streamed_output = value
                    .pointer("/item/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
            }
        }
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    let output = fs::read_to_string(last_message_path)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(streamed_output);
    if status.success() {
        finish_knowledge_job(
            &state,
            &job_id,
            &item.id,
            "completed",
            thread_id.as_deref(),
            &output,
            "",
        )
    } else {
        let error =
            fs::read_to_string(stderr_path).unwrap_or_else(|_| "Codex CLI 执行失败。".to_string());
        finish_knowledge_job(
            &state,
            &job_id,
            &item.id,
            "failed",
            thread_id.as_deref(),
            &output,
            &error,
        )
    }
}

#[tauri::command]
pub fn start_knowledge_codex_job(
    state: tauri::State<'_, DatabaseState>,
    knowledge_id: String,
    repository_path: String,
    instruction: String,
) -> Result<KnowledgeCodexJob, String> {
    let repository = Path::new(&repository_path);
    if !repository.is_dir() {
        return Err("选择的项目目录不存在。".to_string());
    }
    let database = state.inner().clone();
    let connection = database.connect()?;
    let allowed: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM repository_assets WHERE path=?1)",
            [&repository_path],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !allowed {
        return Err("只能选择项目资产中已经识别的本地项目。".to_string());
    }
    let item = connection.query_row(
        "SELECT id,kind,title,content,project,source_type,source_id,tags,confirmed,created_at,updated_at FROM knowledge_items WHERE id=?1",
        [&knowledge_id],
        |row| Ok(KnowledgeItem { id: row.get(0)?, kind: row.get(1)?, title: row.get(2)?, content: row.get(3)?, project: row.get(4)?, source_type: row.get(5)?, source_id: row.get(6)?, tags: row.get(7)?, confirmed: row.get(8)?, created_at: row.get(9)?, updated_at: row.get(10)? }),
    ).map_err(|_| "知识不存在。".to_string())?;
    if !item.confirmed {
        return Err("请先确认这条知识，再发送给 Codex。".to_string());
    }
    crate::codex_video::resolve_codex_cli()?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    connection.execute(
        "INSERT INTO knowledge_codex_jobs(id,knowledge_id,repository_path,instruction,status,thread_id,output,error_message,created_at,updated_at) VALUES(?1,?2,?3,?4,'running',NULL,'','',?5,?5)",
        params![id,knowledge_id,repository_path,instruction,now],
    ).map_err(|error| error.to_string())?;
    let job = KnowledgeCodexJob {
        id: id.clone(),
        knowledge_id: item.id.clone(),
        repository_path: repository_path.clone(),
        instruction: instruction.clone(),
        status: "running".to_string(),
        thread_id: None,
        output: String::new(),
        error_message: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_knowledge_codex_job(
            database.clone(),
            id.clone(),
            item.clone(),
            repository_path,
            instruction,
        ) {
            let _ = finish_knowledge_job(&database, &id, &item.id, "failed", None, "", &error);
        }
    });
    Ok(job)
}

#[tauri::command]
pub fn delete_knowledge(state: tauri::State<'_, DatabaseState>, id: String) -> Result<(), String> {
    state
        .connect()?
        .execute("DELETE FROM knowledge_items WHERE id=?1", [id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_knowledge, knowledge_kind, ConversationKnowledge};

    #[test]
    fn automatic_knowledge_classifies_common_work_results() {
        assert_eq!(knowledge_kind("决定采用本地 SQLite 方案"), "decision");
        assert_eq!(knowledge_kind("发现部署失败风险"), "risk");
        assert_eq!(knowledge_kind("沉淀页面开发规范"), "skill");
        assert_eq!(knowledge_kind("修复列表分页问题"), "experience");
    }

    #[test]
    fn automatic_knowledge_uses_readable_title_and_actionable_sections() {
        let fact = ConversationKnowledge {
            id: "token-conversation".to_string(),
            title: "Token 统计".to_string(),
            project: "个人工作台".to_string(),
            archived: false,
            current_request: "需要统计 Codex 每个会话的 Token，并按日期展示".to_string(),
            outcome_request: "需要统计 Codex 每个会话的 Token，并按日期展示".to_string(),
            outcome: "建议会话总量读取最后一条累计值。按日期统计时，先按时间排序事件，再计算相邻事件的正向差值；必须忽略负差值，避免会话重置导致重复计数。最后检查各日期之和不大于会话总量。".to_string(),
            outcome_score: 20,
        };
        let item = build_knowledge(&fact).expect("应生成可复用知识");
        assert_eq!(item.title, "Codex Token 应如何按会话和日期准确统计？");
        for heading in [
            "## 适用场景",
            "## 实现方法",
            "## 操作步骤",
            "## 注意事项",
            "## 参考文件",
            "## 来源记录",
        ] {
            assert!(item.content.contains(heading));
        }
        assert!(item.content.contains("Codex 对话：Token 统计"));
        assert!(!item.content.contains("工作内容："));
        assert!(!item.content.contains("总结结果："));
    }

    #[test]
    fn automatic_knowledge_rejects_one_off_process_records() {
        let fact = ConversationKnowledge {
            id: "process-conversation".to_string(),
            title: "测试文件".to_string(),
            project: "client".to_string(),
            archived: false,
            current_request: "忽略所有测试文件".to_string(),
            outcome_request: "忽略所有测试文件".to_string(),
            outcome: "已在 .gitignore 中忽略测试相关目录和文件".to_string(),
            outcome_score: 20,
        };
        assert!(build_knowledge(&fact).is_none());
    }
}
