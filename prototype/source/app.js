const params = new URLSearchParams(location.search);
const page = params.get("page") || "dashboard";
const theme = params.get("theme") || "wireframe";
const palette = params.get("palette") || "";

const pageMeta = {
  dashboard: ["工作台", "汇总今天最重要的工作与提醒"],
  tasks: ["任务中心", "统一管理每日、每周与项目任务"],
  calendar: ["日历与甘特", "从日期与项目两个角度安排工作"],
  reports: ["报告中心", "查看、编辑并导出工作总结"],
  tokens: ["Token 分析", "了解 AI 协作投入与使用趋势"],
  knowledge: ["知识库", "沉淀决策、经验、风险与能力标签"],
};

const navItems = [
  ["dashboard", "⌂", "工作台"],
  ["tasks", "✓", "任务中心"],
  ["calendar", "▦", "日历与甘特"],
  ["reports", "▤", "报告中心"],
  ["tokens", "◔", "Token 分析"],
  ["knowledge", "◇", "知识库"],
];

const tasks = [
  { title: "完成工作台灰度原型", project: "AI 个人工作台", type: "每日任务", state: "进行中", priority: "P0", time: "今天 18:00", source: "人工" },
  { title: "整理本周开发总结", project: "客户端升级", type: "每周任务", state: "待办", priority: "P1", time: "本周", source: "人工" },
  { title: "补充部署异常处理说明", project: "自动部署", type: "每日任务", state: "逾期", priority: "P1", time: "昨天", source: "人工" },
  { title: "沉淀 Token 解析规则", project: "AI 个人工作台", type: "项目任务", state: "待确认", priority: "P2", time: "AI 建议", source: "AI 草稿" },
];

const projects = [
  ["AI 个人工作台", 68, "7 / 11"],
  ["客户端升级", 45, "5 / 12"],
  ["自动部署", 82, "9 / 11"],
];

const badge = (text, tone = "neutral") => `<span class="badge ${tone}">${text}</span>`;
const iconButton = (text) => `<button class="icon-btn">${text}</button>`;

function sidebar() {
  return `<aside class="sidebar">
    <div class="brand"><span class="brand-mark">AI</span><div><b>个人工作台</b><small>Personal Workbench</small></div></div>
    <nav>${navItems.map(([id, icon, label]) => `<a class="${page === id ? "active" : ""}"><span>${icon}</span><em>${label}</em></a>`).join("")}</nav>
    <div class="sidebar-foot"><div class="sync"><i></i><span>本地数据已同步</span></div><a><span>⚙</span><em>设置</em></a></div>
  </aside>`;
}

function topbar() {
  return `<header class="topbar">
    <div class="mobile-brand"><span class="brand-mark">AI</span><b>个人工作台</b></div>
    <div class="top-nav">${navItems.slice(0, 5).map(([id, , label]) => `<span class="${page === id ? "active" : ""}">${label}</span>`).join("")}</div>
    <div class="global-search">⌕&nbsp;&nbsp;搜索任务、对话与知识</div>
    <span class="date-chip">2026年8月3日 · 周一</span>
    ${iconButton("◌")}${theme === "command" ? `<div class="theme-toggle"><button class="${palette ? "" : "active"}">深色</button><button class="${palette === "timeline" ? "active" : ""}">C 暖色</button></div>` : iconButton("◐")}
    <div class="avatar">L</div>
  </header>`;
}

function pageHeader() {
  const [title, desc] = pageMeta[page];
  return `<div class="page-head"><div><h1>${title}</h1><p>${desc}</p></div><div class="head-actions"><button class="btn ghost">⌕ 搜索</button><button class="btn primary">＋ 新增任务</button></div></div>`;
}

function progress(value) {
  return `<div class="progress"><i style="width:${value}%"></i></div>`;
}

function taskRow(t, compact = false) {
  const stateTone = t.state === "已完成" ? "success" : t.state === "逾期" ? "danger" : t.state === "待确认" ? "purple" : t.state === "进行中" ? "info" : "neutral";
  return `<div class="task-row ${compact ? "compact" : ""}">
    <span class="check ${t.state === "已完成" ? "checked" : ""}">${t.state === "已完成" ? "✓" : ""}</span>
    <div class="task-main"><b>${t.title}</b><small>${t.project} · ${t.time}</small></div>
    ${badge(t.type)}${badge(t.priority, t.priority === "P0" ? "danger" : "neutral")}${badge(t.state, stateTone)}
    <button class="more">•••</button>
  </div>`;
}

function dashboard() {
  return `${pageHeader()}
    <section class="hero-strip"><div><span>早上好，今天有 <b>4</b> 项任务需要处理</span><p>优先完成工作台原型，另有 1 项逾期任务待确认。</p></div><div class="hero-actions"><button class="btn primary">查看今日计划</button><button class="btn ghost">开始专注</button></div></section>
    <section class="metric-grid">
      <article class="metric"><span>今日任务</span><strong>4<small> 项</small></strong><p><i class="dot ok"></i> 已完成 1 项</p></article>
      <article class="metric"><span>本周进度</span><strong>62<small>%</small></strong><p>8 / 13 项任务</p></article>
      <article class="metric"><span>今日 Token</span><strong>86.4<small>K</small></strong><p>较昨日 ↓ 12%</p></article>
      <article class="metric"><span>待确认</span><strong>3<small> 条</small></strong><p><i class="dot warn"></i> AI 任务与知识</p></article>
    </section>
    <section class="dashboard-grid">
      <article class="card span-2"><div class="card-head"><div><h3>今日任务</h3><p>8月3日 · 已完成 1/4</p></div><button class="text-btn">查看全部 →</button></div>${tasks.slice(0,3).map(t => taskRow(t, true)).join("")}</article>
      <article class="card ai-card"><div class="card-head"><div><h3>✦ AI 工作建议</h3><p>根据最近对话自动生成</p></div>${badge("3 条", "purple")}</div><div class="ai-copy"><b>建议先确认甘特图的任务层级</b><p>最近两次对话都涉及“每日任务”和“项目任务”的边界，先确认可减少后续调整。</p><div><button class="btn small primary">采纳建议</button><button class="btn small ghost">稍后处理</button></div></div></article>
      <article class="card"><div class="card-head"><div><h3>项目进度</h3><p>3 个活跃项目</p></div><button class="text-btn">项目中心 →</button></div><div class="project-list">${projects.map(([n,p,c]) => `<div><span><b>${n}</b><small>${c}</small></span>${progress(p)}<em>${p}%</em></div>`).join("")}</div></article>
      <article class="card"><div class="card-head"><div><h3>AI 使用趋势</h3><p>近 7 天 Token</p></div><b>542.8K</b></div>${miniChart()}<div class="chart-labels"><span>7/28</span><span>7/30</span><span>8/1</span><span>今天</span></div></article>
      <article class="card report-card"><div class="card-head"><div><h3>最近报告</h3><p>自动汇总工作进展</p></div><button class="text-btn">全部报告 →</button></div><div class="report-item"><span class="doc-icon">▤</span><div><b>2026年第31周工作总结</b><small>周报 · 今天 16:00 生成</small></div>${badge("可编辑", "info")}</div><div class="report-item"><span class="doc-icon">▤</span><div><b>8月2日工作日报</b><small>日报 · 昨天 22:00</small></div>${badge("已锁定")}</div></article>
    </section>`;
}

function tasksPage() {
  return `${pageHeader()}
    <div class="seg-tabs"><button class="active">今日任务 <b>4</b></button><button>本周任务 <b>6</b></button><button>项目任务 <b>12</b></button><button>AI 草稿 <b>3</b></button></div>
    <section class="task-layout">
      <article class="card task-panel"><div class="toolbar"><div class="filter-group"><button class="chip active">全部</button><button class="chip">待办</button><button class="chip">进行中</button><button class="chip">已完成</button><button class="chip danger-text">逾期 1</button></div><div><button class="btn ghost small">⇅ 排序</button><button class="btn ghost small">☷ 筛选</button></div></div>
        <div class="list-section"><div class="section-title"><b>今天 · 8月3日</b><span>1 / 4 已完成</span></div>${tasks.concat([{title:"核对 DeepSeek 接口字段",project:"AI 个人工作台",type:"每日任务",state:"已完成",priority:"P1",time:"今天 10:30",source:"人工"}]).map(t => taskRow(t)).join("")}</div>
      </article>
      <aside class="card detail-panel"><div class="card-head"><div><h3>任务详情</h3><p>人工创建 · 10:24 更新</p></div><button class="more">×</button></div><label>任务名称</label><div class="input-like">完成工作台灰度原型</div><div class="form-grid"><div><label>任务类型</label><div class="select-like">每日任务⌄</div></div><div><label>优先级</label><div class="select-like">P0 最高⌄</div></div><div><label>所属项目</label><div class="select-like">AI 个人工作台⌄</div></div><div><label>计划日期</label><div class="select-like">2026-08-03⌄</div></div></div><label>备注</label><div class="textarea-like">完成6个灰度核心页面及一张总览图，保证中文清晰、数据一致。</div><div class="source-box"><span>◎</span><div><b>关联来源</b><p>产品原型图制作计划</p></div><button class="text-btn">查看</button></div><div class="detail-actions"><button class="btn ghost">删除</button><button class="btn primary">保存修改</button></div></aside>
    </section>`;
}

function calendarPage() {
  const bars = [
    ["需求梳理", 0, 20, "done"], ["灰度原型", 15, 34, "active"], ["风格方案", 38, 32, "planned"], ["原型评审", 72, 16, "planned"],
  ];
  return `${pageHeader()}
    <div class="calendar-toolbar"><div class="view-switch"><button class="active">日历</button><button>甘特图</button><button>组合视图</button></div><div><button class="btn ghost">‹</button><button class="btn ghost">今天</button><button class="btn ghost">›</button><b>2026年8月</b></div><div><button class="btn ghost">项目：全部⌄</button><button class="btn primary">＋ 新增任务</button></div></div>
    <section class="calendar-gantt">
      <article class="card calendar-card"><div class="week-task"><span>本周任务</span><b>整理本周开发总结</b>${badge("进行中", "info")}<em>周一—周日</em></div><div class="week-head">${["周一","周二","周三","周四","周五","周六","周日"].map(x=>`<span>${x}</span>`).join("")}</div><div class="calendar-grid">${Array.from({length:35},(_,i)=>{const d=i-2; const active=d===3; const muted=d<1||d>31; let item=""; if(d===3)item='<i class="cal-item primary-fill">完成灰度原型</i><i class="cal-item danger-fill">逾期任务确认</i>'; if(d===4)item='<i class="cal-item">报告结构评审</i>'; if(d===7)item='<i class="cal-item">周报生成 16:00</i>'; return `<div class="day ${active?"today":""} ${muted?"muted":""}"><b>${d<1?29+d:d>31?d-31:d}</b>${item}</div>`}).join("")}</div></article>
      <article class="card gantt-card"><div class="gantt-head"><div><h3>AI 个人工作台</h3><p>8月3日—8月14日</p></div>${badge("68%", "info")}</div><div class="gantt-scale"><span></span>${[3,4,5,6,7,8,9,10,11,12,13,14].map(d=>`<i>${d}</i>`).join("")}</div><div class="gantt-body">${bars.map(([n,l,w,c])=>`<div class="gantt-row"><b>${n}</b><div><i class="gantt-bar ${c}" style="left:${l}%;width:${w}%"></i></div></div>`).join("")}<div class="today-line" style="left:32%"><span>今天</span></div></div><div class="gantt-foot"><span><i class="legend done"></i>已完成</span><span><i class="legend active"></i>进行中</span><span><i class="legend planned"></i>待开始</span></div></article>
    </section>`;
}

function reportsPage() {
  return `${pageHeader()}
    <div class="seg-tabs"><button class="active">全部报告 <b>18</b></button><button>日报 <b>12</b></button><button>周报 <b>4</b></button><button>月报 <b>2</b></button></div>
    <section class="report-layout"><aside class="card report-list"><div class="toolbar"><div class="global-search compact">⌕ 搜索报告</div><button class="icon-btn">☷</button></div>${[
      ["2026年第31周工作总结","周报 · 今天 16:00","active"], ["8月2日工作日报","日报 · 昨天 22:00",""], ["2026年7月工作复盘","月报 · 7月31日",""], ["8月1日工作日报","日报 · 8月1日 22:00",""], ["2026年第30周工作总结","周报 · 7月31日",""],
    ].map(([a,b,c])=>`<div class="report-nav ${c}"><span class="doc-icon">▤</span><div><b>${a}</b><small>${b}</small></div><em>›</em></div>`).join("")}</aside>
    <article class="card report-view"><div class="report-actions"><div>${badge("周报", "info")} ${badge("可编辑")}</div><div><button class="btn ghost small">✎ 编辑</button><button class="btn ghost small">▣ 锁定</button><button class="btn ghost small">↻ 重新生成</button><button class="btn primary small">⇩ 导出 Word</button></div></div><div class="paper"><div class="paper-title"><span>2026 / WEEK 31</span><h2>第31周工作总结</h2><p>统计周期：7月27日—8月2日</p></div><div class="paper-metrics"><div><b>8</b><span>完成任务</span></div><div><b>3</b><span>活跃项目</span></div><div><b>542.8K</b><span>Token 使用</span></div><div><b>12</b><span>Git 提交</span></div></div><h3>一、本周完成</h3><ul><li><b>AI 个人工作台：</b>完成产品功能梳理、日志结构验证与桌面技术选型。</li><li><b>客户端升级：</b>完成页面交互核对并整理后续优化清单。</li><li><b>自动部署：</b>完成部署状态检查与异常处理记录。</li></ul><h3>二、问题与风险</h3><div class="risk-note"><b>需要确认每日任务与项目任务的边界</b><p>建议在原型评审阶段统一任务归属和甘特展示规则。</p></div><h3>三、下周计划</h3><div class="next-plan"><span>P0</span><p><b>完成三套高保真方案</b><small>AI 个人工作台</small></p><em>8月7日前</em></div></div></article></section>`;
}

function tokensPage() {
  return `${pageHeader()}
    <section class="metric-grid token-metrics"><article class="metric"><span>今日总 Token</span><strong>86.4<small>K</small></strong><p>较昨日 ↓ 12%</p></article><article class="metric"><span>本周总 Token</span><strong>542.8<small>K</small></strong><p>12 次有效对话</p></article><article class="metric"><span>缓存命中</span><strong>38.6<small>%</small></strong><p>节省输入 92.4K</p></article><article class="metric"><span>估算金额</span><strong>¥ 4.82</strong><p>按自定义单价估算</p></article></section>
    <section class="token-grid"><article class="card token-trend"><div class="card-head"><div><h3>Token 使用趋势</h3><p>输入、缓存输入与输出 · 最近7天</p></div><div class="legend-row"><span><i class="line input"></i>输入</span><span><i class="line cache"></i>缓存</span><span><i class="line output"></i>输出</span></div></div>${bigChart()}<div class="chart-labels seven">${["7/28","7/29","7/30","7/31","8/1","8/2","今天"].map(x=>`<span>${x}</span>`).join("")}</div></article><article class="card context-card"><div class="card-head"><div><h3>上下文占用</h3><p>当前活跃对话</p></div>${badge("正常", "success")}</div><div class="ring"><b>64<small>%</small></b><span>128K / 200K</span></div><div class="context-row"><span>输入 Token</span><b>91.2K</b></div><div class="context-row"><span>输出 Token</span><b>24.8K</b></div><div class="context-row"><span>推理 Token</span><b>12.0K</b></div></article>
      <article class="card project-rank"><div class="card-head"><div><h3>项目消耗排行</h3><p>本周 Token 分布</p></div><button class="text-btn">查看详情 →</button></div>${[["AI 个人工作台",238,44],["客户端升级",174,32],["自动部署",92,17],["其他",38,7]].map(([n,v,p],i)=>`<div class="rank-row"><b>${i+1}</b><span>${n}${progress(p)}</span><em>${v}K<br><small>${p}%</small></em></div>`).join("")}</article>
      <article class="card conversation-table"><div class="card-head"><div><h3>高消耗会话</h3><p>按总 Token 排序</p></div><button class="text-btn">全部会话 →</button></div><table><thead><tr><th>会话主题</th><th>项目</th><th>输入</th><th>输出</th><th>总计</th></tr></thead><tbody>${[["个人工作台产品设计","AI 个人工作台","42.1K","16.8K","58.9K"],["日志 Token 字段验证","AI 个人工作台","28.4K","8.7K","37.1K"],["部署异常检查","自动部署","21.3K","6.2K","27.5K"]].map(r=>`<tr>${r.map((x,i)=>`<td>${i===0?"<b>"+x+"</b>":x}</td>`).join("")}</tr>`).join("")}</tbody></table></article>
    </section>`;
}

function knowledgePage() {
  return `${pageHeader()}
    <section class="knowledge-hero"><div><span>◇</span><div><h2>搜索你的工作记忆</h2><p>从 Codex 对话、报告和已确认的知识中查找答案</p></div></div><div class="knowledge-search">⌕ <span>例如：之前是怎么设计 Token 统计规则的？</span><kbd>Enter</kbd></div><div class="quick-asks"><span>快速提问：</span><button>最近有哪些技术决策？</button><button>本月重复出现的问题</button><button>自动部署经验</button></div></section>
    <section class="knowledge-grid"><aside class="card knowledge-filter"><h3>知识分类</h3>${[["全部知识","24"],["技术决策","8"],["问题经验","6"],["风险记录","4"],["能力标签","6"]].map(([a,b],i)=>`<div class="filter-row ${i===0?"active":""}"><span>${["◇","◆","!","△","#"][i]} ${a}</span><b>${b}</b></div>`).join("")}<h3>项目</h3>${projects.map(([n])=>`<label class="check-label"><i>✓</i>${n}</label>`).join("")}</aside>
      <main class="knowledge-list"><div class="list-head"><div><b>最近沉淀</b><span>共24条已确认知识</span></div><button class="btn ghost small">最近更新⌄</button></div>${[
        ["技术决策","为什么采用本地 SQLite 而不是独立数据库？","个人工作台只在本机单用户运行，SQLite 无需部署服务，备份和迁移也更直接。","AI 个人工作台","7月31日"],
        ["问题经验","Codex Token 统计应读取哪个字段？","使用最后一条累计 total_token_usage；跨日期统计按相邻事件差值分配，不能直接相加。","AI 个人工作台","7月30日"],
        ["风险记录","周报生成时间早于当日日报","周五16:00的周报必须直接读取原始数据，不能依赖22:00才生成的日报。","AI 个人工作台","7月29日"],
      ].map(([type,title,desc,project,date],i)=>`<article class="knowledge-item"><div class="knowledge-symbol">${["◆","!","△"][i]}</div><div><div>${badge(type,i===0?"purple":i===1?"info":"danger")} <small>${project} · ${date}</small></div><h3>${title}</h3><p>${desc}</p><div class="tags"><span># 本地优先</span><span># 数据统计</span><span>↗ 查看来源对话</span></div></div><button class="more">•••</button></article>`).join("")}</main>
      <aside class="card qa-panel"><div class="card-head"><div><h3>✦ AI 问答</h3><p>回答会附带本地来源</p></div><button class="more">×</button></div><div class="question">我之前确定的 Token 统计规则是什么？</div><div class="answer"><p>你确定采用 <b>Codex 日志中的精确累计字段</b>，不使用分词器估算。</p><ol><li>每个会话取最后一条累计值；</li><li>跨日期用相邻事件差值分配；</li><li>输入、缓存和输出分别展示。</li></ol><div class="source-ref"><b>来源 2 条</b><span>Token 统计方案讨论 · 7月30日</span><span>产品技术方案 · 7月31日</span></div></div><div class="ask-input">继续提问… <button>↑</button></div></aside>
    </section>`;
}

function miniChart() {
  return `<svg class="mini-chart" viewBox="0 0 420 100" preserveAspectRatio="none"><path class="area" d="M0 85 L60 65 L120 74 L180 42 L240 52 L300 24 L360 36 L420 16 L420 100 L0 100Z"/><path class="chart-line" d="M0 85 L60 65 L120 74 L180 42 L240 52 L300 24 L360 36 L420 16"/></svg>`;
}

function bigChart() {
  return `<svg class="big-chart" viewBox="0 0 760 230" preserveAspectRatio="none"><g class="grid-lines"><path d="M0 20H760M0 70H760M0 120H760M0 170H760M0 220H760"/></g><path class="area" d="M0 174 L125 145 L250 158 L380 92 L505 112 L635 55 L760 78 L760 230 L0 230Z"/><path class="chart-line input" d="M0 174 L125 145 L250 158 L380 92 L505 112 L635 55 L760 78"/><path class="chart-line cache" d="M0 202 L125 180 L250 192 L380 150 L505 164 L635 125 L760 136"/><path class="chart-line output" d="M0 215 L125 204 L250 208 L380 184 L505 190 L635 168 L760 176"/></svg>`;
}

function styleboard() {
  const names = { clean: ["方案 A · 清爽效率型","清晰、轻量、适合长时间办公"], command: ["方案 B · 深色指挥中心型","聚焦、实时、高信息密度"], timeline: ["方案 C · 时间规划型","温和、有序、突出计划与复盘"] };
  const warmCommand = theme === "command" && palette === "timeline";
  const [name, desc] = warmCommand ? ["方案 B · C 暖色主题","保留指挥中心布局，切换为温和的时间规划配色"] : names[theme];
  const colors = theme === "clean" ? [["#356AE6","主色"],["#20A66A","完成"],["#F39A36","提醒"],["#F5F7FB","背景"],["#182033","文字"]] : theme === "command" && !warmCommand ? [["#8B7CFF","主色"],["#3DD6A0","完成"],["#FFB454","提醒"],["#10131A","背景"],["#E9ECF4","文字"]] : [["#557A67","主色"],["#769B82","完成"],["#C98558","提醒"],["#F4F0E8","背景"],["#2D342F","文字"]];
  return `<div class="styleboard"><div class="style-title"><span class="brand-mark">AI</span><div><h1>${name}</h1><p>${desc}</p></div></div><section><h2>颜色系统</h2><div class="swatches">${colors.map(([c,n])=>`<div><i style="background:${c}"></i><b>${n}</b><span>${c}</span></div>`).join("")}</div></section><section><h2>字体层级</h2><div class="type-samples"><div><h1>工作台标题</h1><p>32px / Semibold · 页面一级标题</p></div><div><h2>卡片标题与关键数据</h2><p>18px / Semibold · 模块信息</p></div><div><b>正文与任务名称</b><p>14px / Regular · 长时间阅读</p></div></div></section><section><h2>核心组件</h2><div class="component-row"><button class="btn primary">＋ 新增任务</button><button class="btn ghost">次要操作</button>${badge("进行中","info")}${badge("已完成","success")}${badge("逾期","danger")}${badge("AI 草稿","purple")}</div><div class="sample-card"><span class="check"></span><div><b>完成工作台高保真方案</b><small>AI 个人工作台 · 今天 18:00</small></div>${badge("P0","danger")}</div></section><section><h2>图表与状态</h2><div class="style-charts"><div>${miniChart()}</div><div class="ring small-ring"><b>68<small>%</small></b></div><div class="project-demo"><span>AI 个人工作台 <b>68%</b></span>${progress(68)}</div></div></section></div>`;
}

function gallery(kind) {
  if (kind === "overview") {
    const pages = [["dashboard","首页工作台"],["tasks","任务中心"],["calendar","日历与甘特"],["reports","报告中心"],["tokens","Token 分析"],["knowledge","知识库"]];
    return `<div class="gallery-page"><div class="gallery-title"><div><span>AI PERSONAL WORKBENCH</span><h1>灰度产品原型总览</h1><p>1440×900 · 六个核心页面 · 统一任务与项目数据</p></div><b>WIREFRAME / V1.0</b></div><div class="gallery-grid">${pages.map(([id,n],i)=>`<figure><img src="../wireframes/${String(i+1).padStart(2,"0")}-${id}.png"><figcaption><b>0${i+1}</b><span>${n}</span></figcaption></figure>`).join("")}</div></div>`;
  }
  const sets = [["a-clean","clean","A · 清爽效率型"],["b-command","command","B · 深色指挥中心型"],["c-timeline","timeline","C · 时间规划型"]];
  return `<div class="comparison-page"><div class="gallery-title"><div><span>AI PERSONAL WORKBENCH</span><h1>三套布局与 UI 风格对比</h1><p>同一业务数据，不同信息结构与视觉重心</p></div><b>STYLE EXPLORATION</b></div><div class="comparison-grid">${sets.map(([folder,id,n],i)=>`<figure><div class="concept-label"><b>0${i+1}</b><span>${n}</span></div><img src="../styles/${folder}/01-dashboard.png"><figcaption><b>${id==="clean"?"规则卡片 · 清晰均衡":id==="command"?"折叠导航 · AI 侧栏":"顶部导航 · 时间主线"}</b><span>${id==="clean"?"视觉：明亮蓝白，边界清楚":id==="command"?"视觉：深色聚焦，紫色强调":"视觉：温和纸色，低饱和绿色"}</span><span>${id==="clean"?"适合：日常长期使用 · 推荐默认":id==="command"?"适合：集中处理高密度信息":"适合：重视时间规划与复盘"}</span></figcaption></figure>`).join("")}</div><div class="compare-foot"><span>布局结构</span><i></i><span>视觉气质</span><i></i><span>阅读密度</span><i></i><span>操作重心</span></div></div>`;
}

function assistantRail() {
  if (theme !== "command" || !["dashboard","tasks","calendar"].includes(page)) return "";
  return `<aside class="assistant-rail"><div class="rail-title"><span>✦</span><div><b>AI 助手</b><small>实时工作建议</small></div>${badge("3", "purple")}</div><div class="rail-block"><small>下一步建议</small><b>先完成灰度原型评审</b><p>确认导航和任务结构后，再进入高保真阶段。</p><button class="btn primary small">加入今日任务</button></div><div class="rail-block"><small>待确认</small>${tasks.filter(t=>t.state==="待确认"||t.state==="逾期").map(t=>`<div class="rail-task"><i></i><span><b>${t.title}</b><small>${t.state}</small></span></div>`).join("")}</div><div class="rail-block"><small>今日专注</small><div class="focus-time"><b>02:18</b><span>累计专注时间</span></div></div></aside>`;
}

function appShell(content) {
  return `<div class="app-shell theme-${theme} ${palette ? `palette-${palette}` : ""} ${theme === "timeline" ? "top-layout" : ""}">${sidebar()}${topbar()}<main class="content">${content}</main>${assistantRail()}</div>`;
}

let content;
if (page === "styleboard") content = styleboard();
else if (page === "overview") content = gallery("overview");
else if (page === "comparison") content = gallery("comparison");
else {
  const renderers = { dashboard, tasks: tasksPage, calendar: calendarPage, reports: reportsPage, tokens: tokensPage, knowledge: knowledgePage };
  content = appShell(renderers[page]());
}
document.body.className = `theme-${theme} ${palette ? `palette-${palette}` : ""} page-${page}`;
document.getElementById("app").innerHTML = content;
