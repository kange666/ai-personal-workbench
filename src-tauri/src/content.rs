use crate::{ai, database::DatabaseState};
use chrono::{Datelike, Local, NaiveDate, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdea {
    pub id: String,
    pub idea_date: String,
    pub content_type: String,
    pub category: String,
    pub title: String,
    pub hook: String,
    pub script: String,
    pub storyboard: String,
    pub visual_prompts: String,
    pub editing_guide: String,
    pub cover_title: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentDraft {
    category: String,
    title: String,
    hook: String,
    script: String,
    storyboard: String,
    visual_prompts: String,
    editing_guide: String,
    cover_title: String,
}

struct Concept {
    category: &'static str,
    subject: &'static str,
    title: &'static str,
    hook: &'static str,
    premise: &'static str,
    change: &'static str,
    caution: &'static str,
    conclusion: &'static str,
    cover: &'static str,
}

struct ReasoningConcept {
    category: &'static str,
    title: &'static str,
    hook: &'static str,
    premise: &'static str,
    options: &'static str,
    answer: &'static str,
    reasoning: &'static str,
    scene: &'static str,
    cover: &'static str,
}

fn parse_date(value: Option<String>) -> Result<NaiveDate, String> {
    value
        .map(|date| NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string()))
        .transpose()?
        .map_or_else(|| Ok(Local::now().date_naive()), Ok)
}

fn concepts() -> [[Concept; 2]; 5] {
    [
        [
            Concept { category: "AI未来", subject: "AI个人助理", title: "如果 AI 开始替你管理一天，会发生什么？", hook: "未来最懂你的人，可能不是人。", premise: "AI 助理正在从回答问题，变成理解日程、文件和工作目标的行动中枢。", change: "它会在你起床前整理信息，工作时拆解任务，晚上自动生成复盘，让工具开始围绕人主动协作。", caution: "真正的难点不是 AI 会不会做，而是隐私边界、错误决策和人是否仍保留最终控制权。", conclusion: "未来的效率差距，可能不在于谁更忙，而在于谁更早学会与一套可靠的 AI 系统协作。", cover: "AI 管理你的一天" },
            Concept { category: "AI未来", subject: "数字分身", title: "数字分身会先替你工作，还是先替你生活？", hook: "如果另一个你，可以一天工作 24 小时呢？", premise: "数字分身不只是一个会说话的虚拟形象，它可能记住你的表达方式、知识和判断习惯。", change: "它能先处理资料、回复常见问题、整理会议，甚至在多个空间里同时代表你完成低风险沟通。", caution: "但声音和形象一旦可以复制，身份授权、内容责任和被冒用的风险也会一起出现。", conclusion: "真正有价值的数字分身，不是复制一个外表，而是把可验证的能力交给一个受控的代理。", cover: "另一个你正在上班" },
        ],
        [
            Concept { category: "智能硬件", subject: "AI眼镜", title: "AI 眼镜真的会成为下一部手机吗？", hook: "下一块屏幕，可能根本不在你手里。", premise: "手机把信息装进一块屏幕，而 AI 眼镜试图让信息直接出现在你看向的真实世界里。", change: "导航、翻译、提醒和拍摄可以从低头操作，变成抬眼就能获得的实时帮助。", caution: "续航、重量、隐私和全天佩戴舒适度，仍决定它是每天都戴的工具，还是偶尔拿出的玩具。", conclusion: "AI 眼镜短期不会让手机消失，但它可能先拿走手机最频繁、最碎片化的那些操作。", cover: "手机的下一位替代者" },
            Concept { category: "智能硬件", subject: "家用机器人", title: "家用机器人距离普通家庭还有多远？", hook: "机器人进家门，最后一米比想象中更难。", premise: "工厂里的机器人面对固定环境，家庭却充满台阶、杂物、老人、孩子和每天都在变化的任务。", change: "视觉模型和灵巧手正在让机器人从只会移动，走向看懂物体并完成简单整理。", caution: "安全、价格、维修和稳定性必须同时过关，家庭不会容忍一台经常犯错的大型机器。", conclusion: "第一批普及的家用机器人，可能不是万能管家，而是能稳定完成一两件高频家务的专用助手。", cover: "机器人多久能进家门" },
        ],
        [
            Concept { category: "未来生活", subject: "未来家庭", title: "2035 年的普通家庭，可能已经不需要开关了", hook: "未来的家，不会等你下命令。", premise: "今天的智能家居仍依赖 App 和口令，未来的系统会根据时间、环境和你的习惯主动判断。", change: "灯光、温度、安防和能源会像一个整体协同，在人察觉之前完成调整。", caution: "主动服务必须可解释、可关闭，也不能把家庭生活变成被持续收集的数据。", conclusion: "真正聪明的家，不是设备更多，而是设备安静地配合，让人几乎感觉不到它们存在。", cover: "未来的家没有开关" },
            Concept { category: "未来生活", subject: "无感计算", title: "当屏幕消失以后，我们会怎样使用科技？", hook: "未来最先进的设备，可能看不见。", premise: "从键盘到触屏，每次交互升级都在减少人与信息之间的距离。", change: "语音、目光、手势和环境传感会让计算融入空间，信息在需要时出现，用完后自动退场。", caution: "越无感的系统越容易让人忘记它正在收集什么，因此透明提示和物理关闭能力更重要。", conclusion: "屏幕不会突然消失，但科技的终点可能不是占据注意力，而是在不打扰的前提下提供帮助。", cover: "屏幕会消失吗" },
        ],
        [
            Concept { category: "科技趋势", subject: "具身智能", title: "为什么所有科技公司突然都在做机器人？", hook: "AI 的下一站，不在聊天框里。", premise: "大模型已经能理解语言和图像，下一步是让它通过身体感知并改变真实世界。", change: "当算法、传感器和硬件成本同时下降，机器人开始从预设动作走向理解任务再行动。", caution: "演示视频里的成功一次，不等于现实环境中能稳定运行一万次，可靠性仍是最大门槛。", conclusion: "机器人热潮背后，是 AI 从信息工具走向现实执行者的一次关键迁移。", cover: "AI 为什么需要身体" },
            Concept { category: "科技趋势", subject: "个人AI设备", title: "未来 5 年，最容易被低估的科技变化是什么？", hook: "真正改变生活的科技，开始时往往很普通。", premise: "人们常关注更大的模型，却容易忽略 AI 正在进入耳机、眼镜、汽车和家庭设备。", change: "当模型在本地运行，设备会更快、更私密，也能在没有网络时理解环境并提供帮助。", caution: "本地 AI 仍受算力、功耗和更新速度限制，云端与端侧会长期配合而不是彼此取代。", conclusion: "未来五年的关键变化，也许不是又多一个聊天工具，而是身边每台设备都获得一点理解能力。", cover: "被低估的科技变化" },
        ],
        [
            Concept { category: "个人科技升级", subject: "未来办公桌", title: "2030 年的办公桌，会变成什么样？", hook: "未来的办公桌，可能不再以电脑为中心。", premise: "现在的桌面由多个孤立设备组成，未来的工作空间会围绕任务、注意力和身体状态协同。", change: "AI 自动整理资料，屏幕按场景重组，灯光和声音帮助专注，语音与手势承担更多操作。", caution: "设备堆叠不等于效率升级，通知更多、自动化不可控，反而会制造新的负担。", conclusion: "真正的未来工作台不是设备更多，而是让重要信息更早出现，让无关设备主动退场。", cover: "2030 的办公桌" },
            Concept { category: "个人科技升级", subject: "个人工作系统", title: "一个人的工作，为什么也需要一套 AI 中枢？", hook: "你缺的可能不是工具，而是一个会整理工具的系统。", premise: "任务、对话、代码、日历和笔记分散在不同地方，真正浪费时间的是反复切换和重新理解上下文。", change: "AI 中枢可以自动汇总当天工作、连接任务与报告，并把重复经验沉淀成可搜索的知识。", caution: "自动总结必须保留来源，关键结论要能人工确认，否则错误会被不断复制。", conclusion: "个人工作系统的价值不是替你决定，而是把散落的信息整理好，让你更快做出自己的决定。", cover: "给自己装一个 AI 中枢" },
        ],
    ]
}

fn local_drafts(date: NaiveDate) -> Vec<ContentDraft> {
    let parity = date.ordinal() as usize % 2;
    concepts()
        .into_iter()
        .enumerate()
        .map(|(index, variants)| {
            let concept = &variants[(parity + index) % 2];
            ContentDraft {
                category: concept.category.to_string(),
                title: concept.title.to_string(),
                hook: concept.hook.to_string(),
                script: format!(
                    "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n如果你也想提前看看科技会怎样改变普通人的生活，关注我，继续探索那些正在靠近的未来。",
                    concept.hook, concept.premise, concept.change, concept.caution, concept.conclusion
                ),
                storyboard: format!(
                    "| 时间 | 画面 | 字幕 | 旁白重点 |\n|---|---|---|---|\n| 0-3秒 | 黑场中浮现{}的未来轮廓，快速推近 | {} | {} |\n| 3-10秒 | 当下生活与未来界面对比 | 变化已经开始 | {} |\n| 10-25秒 | 三个连续使用场景，界面信息克制 | 工具开始主动配合人 | {} |\n| 25-40秒 | 现实限制以警示图形出现 | 未来并不只剩想象 | {} |\n| 40-53秒 | 普通人的真实生活场景，光线逐渐变暖 | 真正的价值是什么 | {} |\n| 53-60秒 | 画面定格并出现账号统一标识 | 提前看见未来 | 关注并继续探索 |",
                    concept.subject, concept.cover, concept.hook, concept.premise, concept.change, concept.caution, concept.conclusion
                ),
                visual_prompts: format!(
                    "统一风格：写实电影感、近未来但可信、冷灰与蓝紫主色、真实材质、竖屏 9:16、无品牌 Logo、画面内不生成文字。\n\n1. {}的未来轮廓，黑色空间，微弱体积光，镜头快速推进，cinematic realistic, vertical 9:16\n2. 普通人的日常空间与克制的透明界面对比，自然光，真实生活细节，near future technology\n3. {}在三个真实场景中提供帮助，人物动作自然，界面只做信息点缀\n4. 隐私、安全和可靠性风险的抽象可视化，红橙警示光，不恐怖\n5. 温暖的普通生活结尾，科技设备退到背景，人物成为视觉中心，hopeful cinematic ending",
                    concept.subject, concept.subject
                ),
                editing_guide: "总时长 55—60 秒；前 3 秒使用快速推进和低频冲击音；正文每 3—5 秒切换一次景别；关键词字幕每行不超过 12 个字；限制与风险段降低音乐并短暂停顿；结尾使用 1 秒定格。配乐选择克制的未来氛围电子乐，避免过强赛博朋克。".to_string(),
                cover_title: concept.cover.to_string(),
            }
        })
        .collect()
}

fn reasoning_concepts() -> [ReasoningConcept; 5] {
    [
        ReasoningConcept { category: "真假话推理", title: "三个人里，谁在说谎？", hook: "只有一个人说谎，你能在十秒内找出他吗？", premise: "深夜的旧校舍里，A、B、C 被带进推理审判室。馆主说明：三人中只有一个人在说谎，另外两人都说真话。", options: "A：B 在说谎。\nB：C 在说谎。\nC：A 和 B 之中，只有一个人在说谎。", answer: "B 在说谎。", reasoning: "假设 A 说谎，那么 B 说真话会推出 C 说谎，出现两个说谎者。假设 C 说谎，也会与 A、B 的证词冲突。只有假设 B 说谎时，A 的话为真、C 的话也为真，满足唯一说谎者条件。", scene: "谜题审判室，三名学生并排站立，馆主在前景侧面，证词卡片位置清晰。", cover: "谁在说谎？" },
        ReasoningConcept { category: "生死门", title: "三扇门，你会选哪一扇？", hook: "三扇门只有一扇能活，你会选哪扇？", premise: "主角被困在古堡门厅，必须从三扇门中选一扇离开。", options: "左门：门后是毒蛇房。\n中门：门后是燃烧的火海。\n右门：门后是三个月没吃饭的猛兽。", answer: "选择右门。", reasoning: "三个月没有进食的猛兽无法继续存活，因此右门描述的危险已经失效。左门和中门仍是即时危险。", scene: "古堡门厅，三扇门横向展开，蛇影、火焰和猛兽剪影分别对应三个选项。", cover: "哪扇门能活？" },
        ReasoningConcept { category: "犯罪推理", title: "三个人中，谁偷了宝石？", hook: "只有一句是真话，宝石到底是谁偷的？", premise: "博物馆的蓝宝石失窃，现场只有学生 A、女仆 B 和商人 C。已知三句证词中只有一句是真话。", options: "A：不是我偷的。\nB：是 C 偷的。\nC：B 在冤枉我。", answer: "A 偷了宝石。", reasoning: "如果 A 是窃贼，A 的否认是假话，B 指认 C 也是假话，C 说自己被冤枉是真话，恰好只有一句真话。换成 B 或 C 都会产生两句真话。", scene: "证词展示板，三人半身像围绕蓝宝石，逐条证词用卡片呈现。", cover: "谁偷了宝石？" },
        ReasoningConcept { category: "身份判断", title: "两位公主，谁才是真的？", hook: "她们长得一模一样，但只有一位公主会说真话。", premise: "王座前站着两位外表相同的公主。真公主一定说真话，冒牌货一定说假话。", options: "左边：右边是假的。\n右边：我们的身份相同。", answer: "左边是真公主。", reasoning: "如果左边是真公主，那么右边确实是假冒者；右边所说“身份相同”是假的，条件完全成立。反过来假设右边是真公主，她的证词就要求两人身份相同，与只有一位真公主矛盾。", scene: "宫廷审问室，两位公主左右站立，服装一明一暗，馆主在前方观察。", cover: "谁是真公主？" },
        ReasoningConcept { category: "逻辑排除", title: "三杯水，哪一杯有毒？", hook: "三句话只有一句是真的，你敢选哪杯？", premise: "桌上有 A、B、C 三杯水，其中只有一杯有毒。杯前的三句话只有一句是真话。", options: "A 杯前：毒在 B 杯。\nB 杯前：毒不在 B 杯。\nC 杯前：毒不在 A 杯。", answer: "A 杯有毒。", reasoning: "毒在 A 杯时，A 的提示是假、B 的提示是真、C 的提示是假，恰好一句真话。毒在 B 或 C 杯都会让两句提示同时为真。", scene: "暗色长桌上的三杯水，A、B、C 标记清晰，背后是低饱和证据墙。", cover: "哪杯水有毒？" },
    ]
}

fn local_reasoning_drafts() -> Vec<ContentDraft> {
    reasoning_concepts().into_iter().map(|concept| ContentDraft {
        category: concept.category.to_string(),
        title: concept.title.to_string(),
        hook: concept.hook.to_string(),
        script: format!("{}\n\n{}\n\n{}\n\n给你十秒钟思考。\n\n答案是：{}\n\n{}\n\n你答对了吗？把你的答案留在评论区。", concept.hook, concept.premise, concept.options, concept.answer, concept.reasoning),
        storyboard: format!("| 时间 | 画面 | 字幕 | 旁白重点 |\n|---|---|---|---|\n| 0-3秒 | 馆主从暗处抬眼，题目快速出现 | {} | {} |\n| 3-10秒 | {} | 规则只出现一次 | {} |\n| 10-22秒 | 选项依次亮起，人物或道具保持固定站位 | A / B / C | {} |\n| 22-29秒 | 倒计时环与轻微心跳动效 | 10 秒思考 | 暂停口播留给观众判断 |\n| 29-43秒 | 正确选项用绿色标记，矛盾项用红线排除 | 答案：{} | {} |\n| 43-50秒 | 馆主合上文件，画面定格 | 你答对了吗？ | 评论区留下答案 |", concept.cover, concept.hook, concept.scene, concept.premise, concept.options.replace('\n', "；"), concept.answer, concept.reasoning),
        visual_prompts: format!("统一视觉：二次元悬疑学院风、日系推理插画、轻暗黑、低饱和灰蓝与暗紫、戏剧光影、人物站位明确、竖屏 9:16。答案阶段仅使用红色排除线和绿色正确标记。\n\n1. 固定馆主：冷静的深蓝黑短发少年，改良学院制服，手持文件板，冷蓝眼睛高光。\n2. 题目场景：{}\n3. 选项画面：角色与道具分区清晰，给字幕和证词卡片留出安全区。\n4. 思考阶段：暗色背景、金色倒计时环、轻微悬疑光影。\n5. 揭晓阶段：错误逻辑红色、正确答案绿色，构图不改变，方便观众对照。", concept.scene),
        editing_guide: "总时长 45—50 秒；前 3 秒直接抛出问题；证词逐条出现并配短促提示音；保留 6—8 秒无口播思考时间；揭晓时先给答案，再用两步排除解释；固定使用深灰蓝背景、神秘紫强调、红色错误和绿色正确；结尾提问“你答对了吗”。".to_string(),
        cover_title: concept.cover.to_string(),
    }).collect()
}

fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        &trimmed[start..=end]
    } else {
        trimmed
    }
}

async fn ai_drafts(date: NaiveDate) -> Result<Vec<ContentDraft>, String> {
    let prompt = format!(
        "为日期 {} 的中文短视频账号生成 5 条候选内容。账号定位是“小众科技探索”，没有实物，不做虚假开箱或亲身体验，内容方向覆盖 AI未来、智能硬件、未来生活、科技趋势、个人科技升级。每条约 60 秒，既有想象力，也必须说明现实限制。只返回 JSON 数组，必须恰好 5 项。每项字段：category、title、hook、script、storyboard、visualPrompts、editingGuide、coverTitle。storyboard 使用 Markdown 表格且至少 6 镜；visualPrompts 至少 5 条，统一写实近未来竖屏 9:16 风格；不要使用代码围栏。",
        date.format("%Y-%m-%d")
    );
    let raw = ai::complete_with_limit(
        "你是严谨的中文科技短视频策划。禁止伪造产品体验、参数和新闻事实，必须清楚区分趋势、推测与已知事实。",
        &prompt,
        8000,
    )
    .await?;
    let drafts: Vec<ContentDraft> = serde_json::from_str(extract_json(&raw))
        .map_err(|e| format!("AI 内容格式解析失败：{e}"))?;
    if drafts.len() != 5 {
        return Err(format!(
            "AI 返回了 {} 条内容，需要恰好 5 条。",
            drafts.len()
        ));
    }
    Ok(drafts)
}

fn save_drafts(
    state: &DatabaseState,
    date: NaiveDate,
    drafts: Vec<ContentDraft>,
    source: &str,
    content_type: &str,
    force: bool,
) -> Result<Vec<ContentIdea>, String> {
    let date_text = date.format("%Y-%m-%d").to_string();
    let mut connection = state.connect()?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    if force {
        transaction
            .execute(
                "DELETE FROM content_ideas WHERE idea_date=?1 AND content_type=?2 AND status IN ('candidate','rejected')",
                params![date_text, content_type],
            )
            .map_err(|e| e.to_string())?;
    }
    let existing_titles = {
        let mut statement = transaction
            .prepare("SELECT title FROM content_ideas WHERE idea_date=?1 AND content_type=?2")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map(params![date_text, content_type], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<HashSet<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    let missing = 5usize.saturating_sub(existing_titles.len());
    let now = Utc::now().to_rfc3339();
    for draft in drafts
        .into_iter()
        .filter(|draft| !existing_titles.contains(&draft.title))
        .take(missing)
    {
        transaction
            .execute(
                "INSERT OR IGNORE INTO content_ideas(id,idea_date,content_type,category,title,hook,script,storyboard,visual_prompts,editing_guide,cover_title,status,source,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'candidate',?12,?13,?13)",
                params![Uuid::new_v4().to_string(),date_text,content_type,draft.category,draft.title,draft.hook,draft.script,draft.storyboard,draft.visual_prompts,draft.editing_guide,draft.cover_title,source,now],
            )
            .map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    list_for_date(state, date, content_type)
}

fn list_for_date(
    state: &DatabaseState,
    date: NaiveDate,
    content_type: &str,
) -> Result<Vec<ContentIdea>, String> {
    let connection = state.connect()?;
    let mut statement = connection
        .prepare("SELECT id,idea_date,content_type,category,title,hook,script,storyboard,visual_prompts,editing_guide,cover_title,status,source,created_at,updated_at FROM content_ideas WHERE idea_date=?1 AND content_type=?2 ORDER BY CASE status WHEN 'selected' THEN 0 WHEN 'candidate' THEN 1 WHEN 'published' THEN 2 ELSE 3 END,created_at")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(
            params![date.format("%Y-%m-%d").to_string(), content_type],
            |row| {
                Ok(ContentIdea {
                    id: row.get(0)?,
                    idea_date: row.get(1)?,
                    content_type: row.get(2)?,
                    category: row.get(3)?,
                    title: row.get(4)?,
                    hook: row.get(5)?,
                    script: row.get(6)?,
                    storyboard: row.get(7)?,
                    visual_prompts: row.get(8)?,
                    editing_guide: row.get(9)?,
                    cover_title: row.get(10)?,
                    status: row.get(11)?,
                    source: row.get(12)?,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub async fn ensure_today_content(state: DatabaseState) -> Result<Vec<ContentIdea>, String> {
    let today = Local::now().date_naive();
    ensure_content_for_date(state, today).await
}

fn normalize_content_type(value: Option<String>) -> Result<String, String> {
    let value = value.unwrap_or_else(|| "tech".to_string());
    if matches!(value.as_str(), "tech" | "reasoning") {
        Ok(value)
    } else {
        Err("内容栏目无效。".to_string())
    }
}

async fn ensure_content_type(
    state: &DatabaseState,
    date: NaiveDate,
    content_type: &str,
) -> Result<Vec<ContentIdea>, String> {
    if list_for_date(state, date, content_type)?.len() >= 5 {
        return list_for_date(state, date, content_type);
    }
    let (drafts, source) = if content_type == "reasoning" {
        (local_reasoning_drafts(), "local")
    } else {
        match ai_drafts(date).await {
            Ok(drafts) => (drafts, "deepseek"),
            Err(_) => (local_drafts(date), "local"),
        }
    };
    save_drafts(state, date, drafts, source, content_type, false)
}

pub async fn ensure_content_for_date(
    state: DatabaseState,
    date: NaiveDate,
) -> Result<Vec<ContentIdea>, String> {
    let tech = ensure_content_type(&state, date, "tech").await?;
    ensure_content_type(&state, date, "reasoning").await?;
    Ok(tech)
}

#[tauri::command(rename_all = "camelCase")]
pub fn list_content_ideas(
    state: tauri::State<'_, DatabaseState>,
    date: Option<String>,
    content_type: Option<String>,
) -> Result<Vec<ContentIdea>, String> {
    let content_type = normalize_content_type(content_type)?;
    list_for_date(&state, parse_date(date)?, &content_type)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn generate_daily_content(
    state: tauri::State<'_, DatabaseState>,
    date: Option<String>,
    force: Option<bool>,
    use_ai: Option<bool>,
    content_type: Option<String>,
) -> Result<Vec<ContentIdea>, String> {
    let date = parse_date(date)?;
    let force = force.unwrap_or(false);
    let content_type = normalize_content_type(content_type)?;
    let database = state.inner().clone();
    if !force && list_for_date(&database, date, &content_type)?.len() >= 5 {
        return list_for_date(&database, date, &content_type);
    }
    let (drafts, source) = if content_type == "reasoning" {
        (local_reasoning_drafts(), "local")
    } else if use_ai.unwrap_or(true) {
        match ai_drafts(date).await {
            Ok(drafts) => (drafts, "deepseek"),
            Err(_) => (local_drafts(date), "local"),
        }
    } else {
        (local_drafts(date), "local")
    };
    save_drafts(&database, date, drafts, source, &content_type, force)
}

#[tauri::command(rename_all = "camelCase")]
pub fn update_content_status(
    state: tauri::State<'_, DatabaseState>,
    id: String,
    status: String,
) -> Result<(), String> {
    if !matches!(
        status.as_str(),
        "candidate" | "selected" | "rejected" | "published"
    ) {
        return Err("内容状态无效。".to_string());
    }
    state
        .connect()?
        .execute(
            "UPDATE content_ideas SET status=?1,updated_at=?2 WHERE id=?3",
            params![status, Utc::now().to_rfc3339(), id],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_cases_are_complete_and_unique() {
        let drafts = local_reasoning_drafts();
        assert_eq!(5, drafts.len());
        let titles: HashSet<_> = drafts.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(5, titles.len());
        assert!(drafts
            .iter()
            .all(|item| item.script.contains("答案是：") && item.storyboard.contains("10 秒思考")));
    }
}
