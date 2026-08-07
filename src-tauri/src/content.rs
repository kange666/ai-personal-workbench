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

fn local_drafts(date: NaiveDate, generation_round: usize) -> Vec<ContentDraft> {
    let parity = (date.ordinal() as usize + generation_round) % 2;
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

const REASONING_BATCH_COUNT: usize = 5;

fn reasoning_concept_batches() -> [[ReasoningConcept; 5]; REASONING_BATCH_COUNT] {
    [
        [
            ReasoningConcept { category: "数字规律", title: "2、6、12、20，下一项是多少？", hook: "这不是简单加法，你能看出数字背后的结构吗？", premise: "馆主在黑板上写下数列：2、6、12、20、？。每一项都由它所在的位置决定。", options: "A：28\nB：30\nC：32", answer: "B，下一项是 30。", reasoning: "这组数依次是 1×2、2×3、3×4、4×5，因此下一项是 5×6，也就是 30。也可以观察相邻差值为 4、6、8，下一次应增加 10。", scene: "谜题教室黑板，数字逐项亮起，下面同步出现乘法结构与差值箭头。", cover: "下一项是多少？" },
            ReasoningConcept { category: "称重逻辑", title: "9枚硬币中有1枚较轻，最少称几次？", hook: "没有砝码，天平只能用两次，你能找出那枚轻硬币吗？", premise: "9 枚外观相同的硬币中有 1 枚较轻。你有一架没有砝码的天平，需要保证找出它。", options: "A：2 次\nB：3 次\nC：4 次", answer: "A，最少称 2 次。", reasoning: "先把硬币分成三组，每组 3 枚，称其中两组。平衡则轻币在第三组，不平衡则在较轻的一组。再从目标组三枚中取两枚相称，一次就能确定轻币。", scene: "九枚硬币分成三组摆在天平前，两次称重路径用清晰分支图展示。", cover: "两次找出轻硬币" },
            ReasoningConcept { category: "空间想象", title: "涂色立方体切成27块，几块有两面颜色？", hook: "只看边，不看角，这道空间题很容易多数或少数。", premise: "一个大立方体六个面全部涂色，再平均切成 3×3×3 共 27 个小立方体。", options: "A：8 块\nB：12 块\nC：18 块", answer: "B，有 12 块。", reasoning: "恰好两面涂色的小块只在大立方体的棱上，但不能是八个角。每条棱中间有 1 块，共有 12 条棱，因此答案是 12。", scene: "透明三维立方体逐层拆开，十二条棱的中间小方块用金色高亮。", cover: "到底有几块？" },
            ReasoningConcept { category: "策略博弈", title: "21颗棋子轮流拿，怎样保证自己拿到最后一颗？", hook: "每次只能拿1到3颗，先手真的一定能赢吗？", premise: "桌上有 21 颗棋子，两人轮流拿，每次只能拿 1、2 或 3 颗，拿到最后一颗的人获胜，你是先手。", options: "A：第一次拿 1 颗\nB：第一次拿 2 颗\nC：第一次拿 3 颗", answer: "A，第一次拿 1 颗。", reasoning: "先拿 1 颗后剩下 20 颗。此后对方拿几颗，你就补到两人这一轮合计拿 4 颗。这样会依次留下 16、12、8、4，最后一轮由你拿完。", scene: "二十一颗棋子排成一列，每四颗组成一组，先手与对手的拿取过程分色显示。", cover: "先手怎样稳赢？" },
            ReasoningConcept { category: "实验推理", title: "三个开关控制一盏灯，只进房间一次怎么判断？", hook: "你只能进房间一次，但灯泡会留下第二种线索。", premise: "门外有 A、B、C 三个开关，房间内只有一盏灯，三个开关中只有一个控制它。你在门外可以任意操作，但只能进房间一次。", options: "A：依次快速开关三个\nB：开 A 一会儿后关掉，再开 B\nC：同时打开 A 和 B", answer: "B，利用亮度和温度判断。", reasoning: "先打开 A 几分钟再关掉，然后打开 B 并进入房间。灯亮说明 B 控制；灯灭但灯泡温热说明 A 控制；灯灭且冰凉说明 C 控制。", scene: "门外三个开关与门内灯泡分屏展示，亮、热、冷三种结果形成判断表。", cover: "只进门一次" },
        ],
        [
            ReasoningConcept { category: "经典过桥", title: "四个人过桥，最快需要多少分钟？", hook: "手电筒只有一支，最慢的人会决定答案吗？", premise: "四个人过桥分别需要 1、2、7、10 分钟。桥上最多两人，必须带手电筒，同行按较慢者计时。", options: "A：17 分钟\nB：19 分钟\nC：21 分钟", answer: "A，最快 17 分钟。", reasoning: "1和2先过桥用2分钟，1返回用1分钟；7和10一起过桥用10分钟，2返回用2分钟；最后1和2一起过桥用2分钟，总计17分钟。", scene: "夜间窄桥与四名角色，手电筒往返路线按五个步骤依次点亮。", cover: "最快几分钟？" },
            ReasoningConcept { category: "概率选择", title: "三扇门选中一扇后，主持人开空门，要换吗？", hook: "剩下两扇门看似各一半，其实概率并没有重新开始。", premise: "三扇门后只有一辆车。你先选一扇，知道答案的主持人从另外两扇中打开一扇空门，并允许你换到最后一扇。", options: "A：换门\nB：不换\nC：概率完全一样", answer: "A，换门胜率是三分之二。", reasoning: "第一次选中车的概率只有三分之一，不换就保持三分之一。第一次选错的概率是三分之二，主持人排除另一扇空门后，这三分之二全部集中到剩下那扇门，所以换门更有利。", scene: "三扇门与一辆车的概率分支图，第一次选择和主持人排除过程分步展示。", cover: "到底要不要换？" },
            ReasoningConcept { category: "分类逻辑", title: "三个水果箱标签全错，先开哪一箱？", hook: "只允许拿出一个水果，却能修正三个错误标签。", premise: "三个箱子分别贴着“苹果”“橙子”“混合”，但三个标签全部贴错。你只能从一个箱子中拿出一个水果。", options: "A：苹果箱\nB：橙子箱\nC：混合箱", answer: "C，先开标着“混合”的箱子。", reasoning: "因为标签全错，“混合”箱一定只装一种水果。拿出一个就能确定它的真实类别，再结合另外两个标签也必然错误，便能依次确定剩余箱子。", scene: "三只木箱并排放置，错误标签、苹果与橙子用逻辑连线逐步交换位置。", cover: "先开哪一箱？" },
            ReasoningConcept { category: "状态规划", title: "狼、羊和菜怎样安全渡河？", hook: "船每次只能带一样东西，任何一步错了都会出事。", premise: "农夫要把狼、羊和菜运过河。船一次只能带农夫和一样东西；农夫不在时，狼会吃羊，羊会吃菜。", options: "A：先带狼\nB：先带羊\nC：先带菜", answer: "B，必须先带羊。", reasoning: "先带羊过河，空船回来；再带狼过去，把羊带回；然后带菜过去，空船回来；最后带羊过去。每一步都不会让不能单独相处的两样东西留在一起。", scene: "河流两岸俯视图，狼、羊、菜和小船按七个状态逐格移动。", cover: "第一步带谁？" },
            ReasoningConcept { category: "组合计数", title: "10个人每两人握一次手，一共握几次？", hook: "不是10乘9，因为每次握手都被你数了两遍。", premise: "房间里有 10 个人，每两个人之间都恰好握手一次，没有人和自己握手。", options: "A：45 次\nB：90 次\nC：100 次", answer: "A，一共 45 次。", reasoning: "每个人可以和另外 9 人握手，先得到 10×9=90。但每次握手会分别从两个人的角度被计算一次，所以要除以2，结果是45。", scene: "十个人围成圆形，人物之间的连线逐步出现，并用成对计数动画消除重复。", cover: "一共握手几次？" },
        ],
        [
            ReasoningConcept { category: "序列观察", title: "1、11、21、1211、111221，下一项是什么？", hook: "不要计算大小，只要把上一行读出来。", premise: "黑板上出现数列：1、11、21、1211、111221、？。后一个数字是在描述前一个数字。", options: "A：312211\nB：122211\nC：311221", answer: "A，下一项是 312211。", reasoning: "111221 可以读作“三个1、两个2、一个1”，依次写成 31、22、11，合起来就是312211。", scene: "数字被按连续相同字符分组，三个1、两个2、一个1分别用不同颜色框选。", cover: "你会读这串数字吗？" },
            ReasoningConcept { category: "时间测量", title: "两根燃烧不均的绳子，怎样量出45分钟？", hook: "每根都烧60分钟，但不能从长度判断时间。", premise: "两根绳子从一端烧完都需要 60 分钟，但燃烧速度处处不均匀。没有钟表，怎样准确量出 45 分钟？", options: "A：一根折成四段\nB：一根两头点燃，另一根先点一头\nC：两根都只点一头", answer: "B，同时利用两头燃烧。", reasoning: "同时点燃第一根的两端和第二根的一端。第一根30分钟烧完时，立刻点燃第二根的另一端；第二根剩余部分从两端燃烧，15分钟后烧完，总计45分钟。", scene: "两根粗细不均的绳子分上下摆放，点火位置和30加15分钟时间轴清晰显示。", cover: "怎样量出45分钟？" },
            ReasoningConcept { category: "天平推理", title: "8个球中有1个较重，两次称重能找出吗？", hook: "关键不是四对四，而是先分成三组。", premise: "8 个外观相同的球中有 1 个更重。你有一架没有砝码的天平，最多称两次。", options: "A：可以\nB：不可以\nC：至少需要三次", answer: "A，可以在两次内找到。", reasoning: "先取3个对3个。若不平衡，重球在较重的3个中，再取其中2个相称即可确定；若平衡，重球在剩下2个中，第二次把它们相称即可。", scene: "八个球按3、3、2分组，第一次称重后分成平衡与不平衡两条路径。", cover: "两次能找出来吗？" },
            ReasoningConcept { category: "年龄代数", title: "父亲年龄是儿子4倍，20年后是2倍，现在多大？", hook: "只需要一个未知数，就能算出两个人的年龄。", premise: "现在父亲的年龄是儿子的 4 倍；20 年后，父亲的年龄将是儿子的 2 倍。", options: "A：父40岁、子10岁\nB：父36岁、子9岁\nC：父48岁、子12岁", answer: "A，父亲40岁，儿子10岁。", reasoning: "设儿子现在x岁，父亲就是4x岁。20年后有4x+20=2(x+20)，解得x=10，因此父亲40岁。", scene: "父子两条年龄时间轴从现在延伸到20年后，倍数关系在两端分别标注。", cover: "父子现在几岁？" },
            ReasoningConcept { category: "任务排程", title: "两个人完成三项工作，最快需要多久？", hook: "最后一项必须等前两项完成，谁先空闲并不重要。", premise: "工作 A 需要2小时，B需要1小时，C需要3小时；C必须等A和B都完成后才能开始。两个人可以同时工作。", options: "A：4小时\nB：5小时\nC：6小时", answer: "B，最快需要5小时。", reasoning: "开始时两人分别做A和B。B在1小时后完成，但C仍要等待A；第2小时A完成后立即开始C，再用3小时，所以总时间是5小时。", scene: "双泳道甘特图展示两个人的任务安排，C的起点与A、B完成节点相连。", cover: "最快需要多久？" },
        ],
        [
            ReasoningConcept { category: "位置推理", title: "A、B、C坐三个位，谁坐中间？", hook: "三个条件只用到两个，就能锁定全部位置。", premise: "三个座位从左到右编号1、2、3。A坐在B左边，C不坐两端，每人恰好一个座位。", options: "A：A坐中间\nB：B坐中间\nC：C坐中间", answer: "C，C坐在中间。", reasoning: "C不坐两端，所以只能坐2号位。剩下1号和3号给A、B，又因为A在B左边，因此A坐1号、B坐3号。", scene: "三个编号座位横向排列，人物卡片根据条件逐步落位。", cover: "谁坐在中间？" },
            ReasoningConcept { category: "日历推理", title: "如果后天是星期日，今天星期几？", hook: "不要被“后天”绕进去，沿时间线倒推两格。", premise: "馆主只告诉你一句话：后天是星期日。需要判断今天是星期几。", options: "A：星期四\nB：星期五\nC：星期六", answer: "B，今天是星期五。", reasoning: "后天是星期日，那么明天是星期六，再往前一天，今天就是星期五。", scene: "一周日历卡片横向排开，从星期日向左回退两格并高亮星期五。", cover: "今天星期几？" },
            ReasoningConcept { category: "容积规划", title: "只有5升和3升水壶，怎样量出4升水？", hook: "先留下2升，再用它控制第二次倒水。", premise: "水源无限，但只有一个5升壶和一个3升壶，壶上没有刻度，需要准确量出4升水。", options: "A：先装满5升壶\nB：先装满3升壶\nC：无法完成", answer: "A，先装满5升壶。", reasoning: "5升壶倒满3升壶后剩2升；清空3升壶，把这2升倒进去；再装满5升壶，向3升壶补1升，此时5升壶中恰好剩4升。", scene: "两个透明水壶按四个步骤排列，水位与每次剩余容量用数字标注。", cover: "怎样量出4升？" },
            ReasoningConcept { category: "开关规律", title: "100扇门反复开关，最后几扇开着？", hook: "只有因数个数为奇数的门，最终状态会改变。", premise: "100扇门开始都关闭。第1轮切换所有门，第2轮切换编号为2倍数的门，依此类推，第100轮只切换第100扇门。", options: "A：10扇\nB：25扇\nC：50扇", answer: "A，最后有10扇门开着。", reasoning: "第n扇门会在n的每个因数对应轮次被切换。因数通常成对出现，只有完全平方数有一个重复的平方根因数，切换次数为奇数。1到100共有1²到10²十个平方数。", scene: "一百扇门组成网格，因数成对消除，最后十个完全平方数编号亮起。", cover: "最后开着几扇？" },
            ReasoningConcept { category: "真假话推理", title: "A说B在说谎，B说C在说谎，谁是唯一说谎者？", hook: "三个人中只有一个说谎，第三句话决定答案。", premise: "A、B、C三人中只有一个说谎。A说“B在说谎”，B说“C在说谎”，C说“A和B中只有一个说谎”。", options: "A：A\nB：B\nC：C", answer: "B，B是唯一说谎者。", reasoning: "若B说谎，则C说真话，说明A和B中恰有一个说谎；A说B在说谎也是真话，条件成立。假设A或C说谎都会额外推出第二个说谎者。", scene: "审判室内三名角色与证词卡片，真假状态通过逐项假设表验证。", cover: "谁是说谎者？" },
        ],
        [
            ReasoningConcept { category: "时间角度", title: "3点30分时，时针和分针夹角是多少？", hook: "时针不会停在3上，它已经走过了半格。", premise: "一只正常运行的钟表显示3点30分，需要计算时针与分针之间较小的夹角。", options: "A：75度\nB：90度\nC：105度", answer: "A，夹角是75度。", reasoning: "分针在6的位置，也就是180度；时针每小时走30度，3点30分时位于3和4中间，即105度。两者相差75度。", scene: "钟面刻度清晰，时针从3向4移动半格，两条半径与角度扇形同步出现。", cover: "夹角真是90度吗？" },
            ReasoningConcept { category: "数阵规律", title: "2和3得到8，4和5得到24，6和7得到多少？", hook: "每一行都先相乘，再补上第一个数。", premise: "数阵前两行分别是“2、3、8”和“4、5、24”，第三行是“6、7、？”，三行遵循同一规则。", options: "A：42\nB：48\nC：54", answer: "B，答案是48。", reasoning: "每行第三个数等于前两个数相乘后再加第一个数：2×3+2=8，4×5+4=24，因此6×7+6=48。", scene: "三行数阵写在发光方格中，乘号和加号按步骤浮现。", cover: "问号应该填几？" },
            ReasoningConcept { category: "递归策略", title: "4层汉诺塔最少移动多少次？", hook: "每增加一层，移动次数都会翻倍再加一。", premise: "三根柱子上有4个大小不同的圆盘，需要把整塔移到另一根柱子；每次只能移动一个盘，且大盘不能放在小盘上。", options: "A：12次\nB：15次\nC：16次", answer: "B，最少15次。", reasoning: "先用7次把上面3个盘移到辅助柱，再移动最大盘1次，最后再用7次把3个盘移到目标柱，共15次。规律是2的层数次方减1。", scene: "四层圆盘在三根柱子之间移动，7加1加7的三个阶段分色呈现。", cover: "最少移动几次？" },
            ReasoningConcept { category: "概率计算", title: "掷两枚骰子，点数和为7的概率是多少？", hook: "一共有36种结果，其中只有6种刚好凑成7。", premise: "同时掷两枚公平的六面骰子，计算两个点数之和等于7的概率。", options: "A：六分之一\nB：七分之一\nC：十二分之一", answer: "A，概率是六分之一。", reasoning: "两枚骰子共有6×6=36种等可能结果。和为7的组合是1+6、2+5、3+4、4+3、5+2、6+1，共6种，因此概率是6/36，也就是1/6。", scene: "六乘六结果方格矩阵中，和为7的六条对角组合被高亮。", cover: "和为7有多难？" },
            ReasoningConcept { category: "逻辑排序", title: "甲比乙快，丙比甲慢但比乙快，谁排第二？", hook: "把每句话变成箭头，顺序立刻清楚。", premise: "三人比赛，已知甲比乙快；丙比甲慢，但丙又比乙快。没有并列。", options: "A：甲\nB：乙\nC：丙", answer: "C，丙排第二。", reasoning: "甲比乙快得到甲在乙前；丙比甲慢得到甲在丙前；丙比乙快得到丙在乙前。合并就是甲、丙、乙，所以丙第二。", scene: "三名选手与速度箭头逐条出现，最终排列在一条终点线上。", cover: "谁排在第二？" },
        ],
    ]
}

fn local_reasoning_drafts(date: NaiveDate, generation_round: usize) -> Vec<ContentDraft> {
    let batch_index = (date.ordinal() as usize + generation_round) % REASONING_BATCH_COUNT;
    let concepts = reasoning_concept_batches()
        .into_iter()
        .nth(batch_index)
        .expect("逻辑思维题库批次必须存在");
    concepts.into_iter().map(|concept| ContentDraft {
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

async fn ai_drafts(date: NaiveDate, generation_round: usize) -> Result<Vec<ContentDraft>, String> {
    let prompt = format!(
        "为日期 {} 的中文短视频账号生成第 {} 批 5 条全新候选内容，不要重复此前批次的常见标题。账号定位是“小众科技探索”，没有实物，不做虚假开箱或亲身体验，内容方向覆盖 AI未来、智能硬件、未来生活、科技趋势、个人科技升级。每条约 60 秒，既有想象力，也必须说明现实限制。只返回 JSON 数组，必须恰好 5 项。每项字段：category、title、hook、script、storyboard、visualPrompts、editingGuide、coverTitle。storyboard 使用 Markdown 表格且至少 6 镜；visualPrompts 至少 5 条，统一写实近未来竖屏 9:16 风格；不要使用代码围栏。",
        date.format("%Y-%m-%d"),
        generation_round + 1
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
    let missing = if force {
        5
    } else {
        5usize.saturating_sub(existing_titles.len())
    };
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

fn generation_round(
    state: &DatabaseState,
    date: NaiveDate,
    content_type: &str,
) -> Result<usize, String> {
    let key = format!(
        "content_generation_round:{}:{}",
        date.format("%Y-%m-%d"),
        content_type
    );
    let connection = state.connect()?;
    let value = connection
        .query_row("SELECT value FROM app_meta WHERE key=?1", [&key], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|_| "0".to_string());
    Ok(value.parse::<usize>().unwrap_or(0))
}

fn local_titles_for_round(date: NaiveDate, content_type: &str, round: usize) -> HashSet<String> {
    let drafts = if content_type == "reasoning" {
        local_reasoning_drafts(date, round)
    } else {
        local_drafts(date, round)
    };
    drafts.into_iter().map(|draft| draft.title).collect()
}

fn next_generation_round(
    state: &DatabaseState,
    date: NaiveDate,
    content_type: &str,
) -> Result<usize, String> {
    let key = format!(
        "content_generation_round:{}:{}",
        date.format("%Y-%m-%d"),
        content_type
    );
    let connection = state.connect()?;
    let stored_round = connection
        .query_row("SELECT value FROM app_meta WHERE key=?1", [&key], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .and_then(|value| value.parse::<usize>().ok());
    let next_round = if let Some(round) = stored_round {
        round + 1
    } else {
        let existing_titles = {
            let mut statement = connection
                .prepare("SELECT title FROM content_ideas WHERE idea_date=?1 AND content_type=?2")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map(
                    params![date.format("%Y-%m-%d").to_string(), content_type],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|error| error.to_string())?;
            rows.collect::<Result<HashSet<_>, _>>()
                .map_err(|error| error.to_string())?
        };
        let search_rounds = if content_type == "reasoning" {
            REASONING_BATCH_COUNT
        } else {
            2
        };
        let current_round = (0..search_rounds)
            .max_by_key(|round| {
                local_titles_for_round(date, content_type, *round)
                    .intersection(&existing_titles)
                    .count()
            })
            .unwrap_or(0);
        current_round + 1
    };
    connection
        .execute(
            "INSERT INTO app_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, next_round.to_string()],
        )
        .map_err(|error| error.to_string())?;
    Ok(next_round)
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
    let round = generation_round(state, date, content_type)?;
    let (drafts, source) = if content_type == "reasoning" {
        (local_reasoning_drafts(date, round), "local")
    } else {
        match ai_drafts(date, round).await {
            Ok(drafts) => (drafts, "deepseek"),
            Err(_) => (local_drafts(date, round), "local"),
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
    let round = if force {
        next_generation_round(&database, date, &content_type)?
    } else {
        generation_round(&database, date, &content_type)?
    };
    let (drafts, source) = if content_type == "reasoning" {
        (local_reasoning_drafts(date, round), "local")
    } else if use_ai.unwrap_or(true) {
        match ai_drafts(date, round).await {
            Ok(drafts) => (drafts, "deepseek"),
            Err(_) => (local_drafts(date, round), "local"),
        }
    } else {
        (local_drafts(date, round), "local")
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
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let drafts = local_reasoning_drafts(date, 0);
        assert_eq!(5, drafts.len());
        let titles: HashSet<_> = drafts.iter().map(|item| item.title.as_str()).collect();
        let categories: HashSet<_> = drafts.iter().map(|item| item.category.as_str()).collect();
        assert_eq!(5, titles.len());
        assert_eq!(5, categories.len());
        assert!(drafts
            .iter()
            .all(|item| item.script.contains("答案是：") && item.storyboard.contains("10 秒思考")));
    }

    #[test]
    fn reasoning_batches_cover_multiple_logic_dimensions_without_title_repeats() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let mut all_titles = HashSet::new();
        let mut all_categories = HashSet::new();
        for round in 0..REASONING_BATCH_COUNT {
            let drafts = local_reasoning_drafts(date, round);
            assert_eq!(5, drafts.len());
            assert_eq!(
                5,
                drafts
                    .iter()
                    .map(|item| item.category.as_str())
                    .collect::<HashSet<_>>()
                    .len()
            );
            for draft in drafts {
                assert!(all_titles.insert(draft.title));
                all_categories.insert(draft.category);
            }
        }
        assert_eq!(25, all_titles.len());
        assert!(all_categories.len() >= 20);
    }

    #[test]
    fn manual_generation_rotates_to_a_fresh_batch() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let first_reasoning: HashSet<_> = local_reasoning_drafts(date, 0)
            .into_iter()
            .map(|item| item.title)
            .collect();
        let next_reasoning: HashSet<_> = local_reasoning_drafts(date, 1)
            .into_iter()
            .map(|item| item.title)
            .collect();
        let first_tech: HashSet<_> = local_drafts(date, 0)
            .into_iter()
            .map(|item| item.title)
            .collect();
        let next_tech: HashSet<_> = local_drafts(date, 1)
            .into_iter()
            .map(|item| item.title)
            .collect();

        assert!(first_reasoning.is_disjoint(&next_reasoning));
        assert!(first_tech.is_disjoint(&next_tech));
    }

    #[test]
    fn force_generation_keeps_selected_and_adds_five_candidates() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let path = std::env::temp_dir().join(format!("content-test-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let initial = save_drafts(
            &state,
            date,
            local_reasoning_drafts(date, 0),
            "local",
            "reasoning",
            false,
        )
        .unwrap();
        state
            .connect()
            .unwrap()
            .execute(
                "UPDATE content_ideas SET status='selected' WHERE id=?1",
                [&initial[0].id],
            )
            .unwrap();

        let regenerated = save_drafts(
            &state,
            date,
            local_reasoning_drafts(date, 1),
            "local",
            "reasoning",
            true,
        )
        .unwrap();
        assert_eq!(
            1,
            regenerated
                .iter()
                .filter(|item| item.status == "selected")
                .count()
        );
        assert_eq!(
            5,
            regenerated
                .iter()
                .filter(|item| item.status == "candidate")
                .count()
        );

        drop(state);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }

    #[test]
    fn first_generation_after_upgrade_skips_the_existing_legacy_batch() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 5).unwrap();
        let path = std::env::temp_dir().join(format!("content-test-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let legacy_titles: HashSet<_> = local_reasoning_drafts(date, 1)
            .into_iter()
            .map(|item| item.title)
            .collect();
        save_drafts(
            &state,
            date,
            local_reasoning_drafts(date, 1),
            "local",
            "reasoning",
            false,
        )
        .unwrap();

        let next_round = next_generation_round(&state, date, "reasoning").unwrap();
        let next_titles = local_titles_for_round(date, "reasoning", next_round);
        assert!(legacy_titles.is_disjoint(&next_titles));

        drop(state);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}
