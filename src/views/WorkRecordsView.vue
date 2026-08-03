<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import WorkTimeDrawer from "../components/WorkTimeDrawer.vue";
import { useWorkbenchStore } from "../stores/workbench";
import { getDailyActivity, getHistoryCoverage, getReportSources, getWorkSummary, isTauriRuntime, listConversationMetrics, listKnowledge, listReports, listTestMenus, listTestRuns, listWorkSessions, type ConversationMetric, type DailyActivity, type KnowledgeItem, type ReportRecord, type ReportSource, type TestMenu, type TestRun, type WorkSession, type WorkSummary } from "../services/backend";

const router = useRouter();
const store = useWorkbenchStore();
const today = new Date();
const defaultStart = new Date(today); defaultStart.setDate(defaultStart.getDate() - 30);
const iso = (date: Date) => date.toLocaleDateString("sv-SE");
const startDate = ref(iso(defaultStart));
const endDate = ref(iso(today));
const mode = ref<"day" | "week">("day");
const project = ref("全部");
const archive = ref<"all" | "normal" | "archived">("all");
const keyword = ref("");
const gitOnly = ref(false);
const reportOnly = ref(false);
const activities = ref<DailyActivity[]>([]);
const reports = ref<ReportRecord[]>([]);
const sessions = ref<WorkSession[]>([]);
const conversations = ref<ConversationMetric[]>([]);
const knowledge = ref<KnowledgeItem[]>([]);
const testMenus = ref<TestMenu[]>([]);
const testRuns = ref<TestRun[]>([]);
const workSummary = ref<WorkSummary>({ startDate: startDate.value, endDate: endDate.value, totalMinutes: 0, estimatedMinutes: 0, manualMinutes: 0, hasManualCorrections: false, byProject: [], byType: [], daily: [] });
const selectedPeriod = ref<{ start: string; end: string; title: string } | null>(null);
const recordSources = ref<ReportSource[]>([]);
const sourceTitle = ref("");
const loading = ref(false);
const error = ref("");
const message = ref("");

const projectDisplay = (value?: string) => (value || "").split(/[\\/]/).filter(Boolean).at(-1) || "未归类项目";
const projects = computed(() => ["全部", ...new Set([...sessions.value.map(item => item.project),...conversations.value.map(item=>projectDisplay(item.project))])]);
const reportByDay = computed(() => new Map(reports.value.filter(item => item.reportType === "daily").map(item => [item.periodStart, item])));
const activityByDay = computed(() => new Map(activities.value.map(item => [item.date, item])));
const sessionsByDay = computed(() => {
  const map = new Map<string, WorkSession[]>();
  for (const item of sessions.value) map.set(item.date, [...(map.get(item.date) || []), item]);
  return map;
});
const selectedProjectMinutes = computed(() => workSummary.value.byProject.find(item => item.name === project.value)?.minutes || 0);
const selectedProjectSessions = computed(() => sessions.value.filter(item => item.project === project.value));
const selectedProjectTypes = computed(() => [...new Set(selectedProjectSessions.value.map(item => item.workType))]);
const projectKey = (value?: string) => (value || "").split(/[\\/]/).filter(Boolean).at(-1)?.replace(/\s+/g, "").toLowerCase() || "";
const sameProject = (value?: string) => projectKey(value) === projectKey(project.value);
function projectReportLines(report: ReportRecord) {
  const lines = report.contentMarkdown.split("\n");
  const start = lines.findIndex(line => line.startsWith("### ") && sameProject(line.slice(4)));
  if (start < 0) return [];
  const end = lines.findIndex((line,index) => index > start && (line.startsWith("### ") || line.startsWith("## ")));
  return lines.slice(start + 1, end < 0 ? undefined : end).filter(line => line.startsWith("- ")).map(line => line.slice(2).trim()).filter(Boolean);
}
const projectOverview = computed(() => {
  if (project.value === "全部") return null;
  const projectConversations = conversations.value.filter(item => sameProject(item.project));
  const paths = [...new Set(projectConversations.map(item => item.cwd).filter((value): value is string => Boolean(value)))];
  const projectReports = reports.value.filter(item => projectReportLines(item).length).sort((a,b) => b.periodEnd.localeCompare(a.periodEnd));
  const features = [...new Set(projectReports.flatMap(projectReportLines))].slice(0,6);
  const tasks = store.tasks.filter(item => sameProject(item.project) && !["done","cancelled"].includes(item.status)).slice(0,5);
  const tests = testRuns.value.filter(item => sameProject(item.project)).sort((a,b) => b.startedAt.localeCompare(a.startedAt));
  const relatedKnowledge = knowledge.value.filter(item => sameProject(item.project)).sort((a,b) => b.updatedAt.localeCompare(a.updatedAt)).slice(0,5);
  const relatedMenus = testMenus.value.filter(item => sameProject(item.project));
  const activityDates = [
    ...projectConversations.map(item => item.updatedAt || ""), ...selectedProjectSessions.value.map(item => item.updatedAt),
    ...projectReports.map(item => item.updatedAt), ...tests.map(item => item.startedAt), ...relatedKnowledge.map(item => item.updatedAt),
  ].filter(Boolean).sort().reverse();
  return { paths, features, tasks, recentReport: projectReports[0], recentTest: tests[0], knowledge: relatedKnowledge, keyFiles: relatedMenus.map(item => item.sourcePath).slice(0,6), lastActivity: activityDates[0], next: tasks[0]?.title || "结合最近报告确认下一项可交付功能" };
});

function minutesText(value: number) { const h = Math.floor(value / 60); const m = value % 60; return `${h ? `${h}小时` : ""}${m || !h ? `${m}分钟` : ""}`; }
function reportSummary(report?: ReportRecord) {
  if (!report) return [];
  const lines = report.contentMarkdown.split("\n");
  const start = lines.findIndex(line => line.trim() === "## 项目工作总结");
  if (start < 0) return [];
  const end = lines.findIndex((line,index) => index > start && line.startsWith("## "));
  const selected = lines.slice(start + 1, end < 0 ? undefined : end).filter(line => line.startsWith("### ") || line.startsWith("- "));
  return selected.slice(0, 8).map(line => line.replace(/^### /, "项目：").replace(/^- /, ""));
}
function reportSection(report: ReportRecord | undefined, heading: string) { if (!report) return []; const lines=report.contentMarkdown.split("\n"); const start=lines.findIndex(line=>line.trim()===heading); if(start<0)return[]; const end=lines.findIndex((line,index)=>index>start&&line.startsWith("## ")); return lines.slice(start+1,end<0?undefined:end).filter(line=>line.startsWith("- ")).map(line=>line.slice(2).trim()).slice(0,4); }
function effectiveMinutes(date: string) { return workSummary.value.daily.find(item => item.date === date)?.minutes || 0; }
function matches(date: string) {
  const activity = activityByDay.value.get(date); const report = reportByDay.value.get(date); const daySessions = sessionsByDay.value.get(date) || [];
  if (project.value !== "全部" && !daySessions.some(item => item.project === project.value)) return false;
  if (archive.value === "archived" && !activity?.archivedConversationCount) return false;
  if (archive.value === "normal" && activity && activity.conversationCount <= activity.archivedConversationCount) return false;
  if (gitOnly.value && !activity?.gitCommits) return false;
  if (reportOnly.value && !report) return false;
  if (keyword.value.trim()) { const text = `${report?.title || ""} ${report?.contentMarkdown || ""} ${daySessions.map(item => `${item.project} ${item.note}`).join(" ")}`.toLowerCase(); if (!text.includes(keyword.value.trim().toLowerCase())) return false; }
  return Boolean(activity?.messageCount || report || daySessions.length);
}
const dayRecords = computed(() => activities.value.map(item => ({ ...item, report: reportByDay.value.get(item.date), sessions: sessionsByDay.value.get(item.date) || [], workMinutes: effectiveMinutes(item.date) })).filter(item => matches(item.date)).sort((a,b) => b.date.localeCompare(a.date)));
function monday(dateText: string) { const date = new Date(`${dateText}T00:00:00`); date.setDate(date.getDate() - ((date.getDay() + 6) % 7)); return iso(date); }
const weekRecords = computed(() => {
  const groups = new Map<string, typeof dayRecords.value>();
  for (const item of dayRecords.value) { const key = monday(item.date); groups.set(key, [...(groups.get(key) || []), item]); }
  return [...groups].map(([start,days]) => { const end = new Date(`${start}T00:00:00`); end.setDate(end.getDate()+6); const report = reports.value.find(item => item.reportType === "weekly" && item.periodStart === start); return { start, end:iso(end), days, report, workMinutes:days.reduce((sum,item)=>sum+item.workMinutes,0), tokens:days.reduce((sum,item)=>sum+item.totalTokens,0), commits:days.reduce((sum,item)=>sum+item.gitCommits,0), conversations:days.reduce((sum,item)=>sum+item.conversationCount,0), testRuns:days.reduce((sum,item)=>sum+item.testRuns,0), testsPassed:days.reduce((sum,item)=>sum+item.testsPassed,0), knowledgeCount:days.reduce((sum,item)=>sum+item.knowledgeCount,0), taskActivityCount:days.reduce((sum,item)=>sum+item.taskActivityCount,0) }; }).sort((a,b)=>b.start.localeCompare(a.start));
});

async function load() {
  if (!isTauriRuntime()) return;
  loading.value = true; error.value = "";
  try { workSummary.value = await getWorkSummary(startDate.value,endDate.value,true); [activities.value,reports.value,sessions.value,conversations.value,knowledge.value,testMenus.value,testRuns.value] = await Promise.all([getDailyActivity(startDate.value,endDate.value),listReports(),listWorkSessions(startDate.value,endDate.value,false),listConversationMetrics(1000),listKnowledge(),listTestMenus(),listTestRuns()]); }
  catch (cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function initialize() {
  if (!isTauriRuntime()) return;
  try {
    const coverage = await getHistoryCoverage();
    if (coverage.firstDate && coverage.firstDate < startDate.value) { startDate.value = coverage.firstDate; return; }
  } catch { /* 历史覆盖读取失败时仍加载默认近 30 天 */ }
  await load();
}
function openTime(start:string,end=start,title="工时明细") { selectedPeriod.value={start,end,title}; }
function continuationText() {
  const overview = projectOverview.value;
  if (!overview) return "";
  const list = (values: string[], empty: string) => values.length ? values.map(value => `- ${value}`).join("\n") : `- ${empty}`;
  return `# ${project.value} 项目续接说明\n\n## 本地路径\n${list(overview.paths,"尚未识别，请先扫描 Codex 和 Git")}\n\n## 上次做到哪里\n${list(overview.features,"尚未生成包含该项目的工作报告")}\n\n## 关键文件\n${list(overview.keyFiles,"尚未从现有测试菜单识别关键文件")}\n\n## 未完成事项\n${list(overview.tasks.map(item => `${item.title}（${item.status === "draft" ? "建议待确认" : item.status}）`),"当前没有已记录的未完成任务")}\n\n## 下一步\n- ${overview.next}\n\n## 可复用知识\n${list(overview.knowledge.map(item => item.title),"暂无已沉淀知识")}\n\n请先核对上述本地文件和当前代码状态，再继续实现；不要重复已完成的功能。`;
}
async function copyContinuation() {
  const text = continuationText();
  try { await navigator.clipboard.writeText(text); message.value = "项目续接说明已复制，可直接粘贴到新的 Codex 任务。"; }
  catch { const area=document.createElement("textarea"); area.value=text; document.body.appendChild(area); area.select(); document.execCommand("copy"); area.remove(); message.value="项目续接说明已复制。"; }
}
async function openRecordSources(report: ReportRecord) { loading.value=true; error.value=""; try { recordSources.value=await getReportSources(report.id); sourceTitle.value=`${report.title} · Codex、Git 与任务来源`; } catch(cause){error.value=String(cause);} finally{loading.value=false;} }
function openRecordSource(item: ReportSource) { if(item.kind==="Codex 对话") void router.push(`/tokens?conversation=${item.id}`); else if(item.kind==="任务") void router.push(`/tasks?task=${item.id}`); else if(item.kind==="测试") void router.push(`/testing?run=${item.id}`); }
watch([startDate,endDate],load);
onMounted(initialize);
</script>

<template><div class="view work-records-view"><header class="page-header"><div><h1>工作记录</h1><p>按天和周查看整理后的工作成果、工时、Codex、Git、测试与报告</p></div><div><button class="button secondary" :disabled="loading" @click="load">↻ 重新整理</button></div></header>
  <div v-if="error || message" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
  <section class="panel work-record-filter"><div class="mode-switch"><button :class="{active:mode==='day'}" @click="mode='day'">日维度</button><button :class="{active:mode==='week'}" @click="mode='week'">周维度</button></div><input v-model="startDate" type="date"><span>至</span><input v-model="endDate" type="date"><select v-model="project"><option v-for="item in projects" :key="item">{{ item }}</option></select><select v-model="archive"><option value="all">全部对话</option><option value="normal">普通对话</option><option value="archived">含归档对话</option></select><label>⌕<input v-model="keyword" placeholder="搜索功能、项目或备注"></label><button class="filter-chip" :class="{active:gitOnly}" @click="gitOnly=!gitOnly">有 Git</button><button class="filter-chip" :class="{active:reportOnly}" @click="reportOnly=!reportOnly">有报告</button></section>
  <section class="work-record-summary"><button class="panel" @click="openTime(startDate,endDate,'当前筛选周期工时')"><small>有效工时</small><b>{{ minutesText(workSummary.totalMinutes) }}</b><span>{{ workSummary.hasManualCorrections ? '含手工修正' : '估算工时' }} · 点击查看</span></button><div class="panel"><small>原始估算</small><b>{{ minutesText(workSummary.estimatedMinutes) }}</b><span>始终保留用于对比</span></div><div class="panel"><small>手工记录</small><b>{{ minutesText(workSummary.manualMinutes) }}</b><span>重叠区间优先</span></div><div class="panel"><small>活跃项目</small><b>{{ workSummary.byProject.length }}</b><span>{{ workSummary.byProject.slice(0,3).map(item=>item.name).join('、') || '暂无' }}</span></div></section>
  <section v-if="project !== '全部'" class="panel project-worktime-summary"><div><small>项目累计工时 · {{ startDate }}—{{ endDate }}</small><h2>{{ project }}</h2><p>{{ selectedProjectSessions.length }} 个工作区间 · {{ selectedProjectTypes.join('、') || '暂无工作类型' }}</p></div><button @click="openTime(startDate,endDate,`${project} 项目累计工时`)"><b>{{ minutesText(selectedProjectMinutes) }}</b><span>{{ selectedProjectSessions.some(item=>item.source==='manual') ? '含手工修正' : '估算工时' }} · 查看明细 →</span></button></section>
  <section v-if="projectOverview" class="panel project-overview"><header><div><small>轻量项目概览</small><h2>{{ project }}</h2><p>{{ projectOverview.paths.join(' · ') || '尚未识别本地路径' }} · 最近工作 {{ projectOverview.lastActivity?.slice(0,10) || '暂无' }}</p></div><button class="button primary" @click="copyContinuation">复制项目续接说明</button></header><div class="project-overview-grid"><article><h3>已完成的主要功能</h3><ul><li v-for="item in projectOverview.features" :key="item">{{ item }}</li></ul><p v-if="!projectOverview.features.length">生成或重建报告后显示整理后的功能成果。</p></article><article><h3>当前任务</h3><button v-for="item in projectOverview.tasks" :key="item.id" @click="router.push(`/tasks?task=${item.id}`)">{{ item.title }}<span>{{ item.status === 'draft' ? '建议待确认' : item.status }}</span></button><p v-if="!projectOverview.tasks.length">当前没有未完成任务。</p></article><article><h3>最近报告与测试</h3><button v-if="projectOverview.recentReport" @click="router.push(`/reports?report=${projectOverview.recentReport.id}`)">▤ {{ projectOverview.recentReport.title }}</button><button v-if="projectOverview.recentTest" @click="router.push('/testing')">✓ {{ projectOverview.recentTest.menuName }} · {{ projectOverview.recentTest.status === 'passed' ? '通过' : '失败' }}</button><p v-if="!projectOverview.recentReport && !projectOverview.recentTest">暂无项目报告或测试。</p></article><article><h3>相关知识与下一步</h3><button v-for="item in projectOverview.knowledge" :key="item.id" @click="router.push(`/knowledge?item=${item.id}`)">◇ {{ item.title }}</button><p><b>下一步：</b>{{ projectOverview.next }}</p></article></div></section>
  <section v-if="mode==='day'" class="work-record-list"><article v-for="item in dayRecords" :key="item.date" class="panel work-record-card"><header><div><b>{{ item.date }}</b><span>{{ item.sessions.map(session=>session.project).filter((v,i,a)=>a.indexOf(v)===i).join('、') || '未归类项目' }}</span></div><button @click="openTime(item.date,item.date,`${item.date} 工时明细`)"><strong>{{ minutesText(item.workMinutes) }}</strong><small>估算/修正工时 →</small></button></header><div class="record-metrics"><span>Codex {{ item.conversationCount }} 次</span><span>归档 {{ item.archivedConversationCount }}</span><span>Git {{ item.gitCommits }} 次</span><span>测试 {{ item.testsPassed }}/{{ item.testRuns }}</span><span>任务操作 {{ item.taskActivityCount }}</span><span>知识 {{ item.knowledgeCount }}</span><span>Token {{ Math.round(item.totalTokens/1000) }}K</span><span>{{ item.report ? '已有日报' : '暂无日报' }}</span></div><ul v-if="reportSummary(item.report).length"><li v-for="line in reportSummary(item.report)" :key="line">{{ line }}</li></ul><p v-else>当天已有活动，但尚未生成可读的项目成果总结。</p><footer><button v-if="item.report" class="button primary small" @click="router.push(`/reports?report=${item.report.id}`)">查看当天做了什么</button><button v-if="item.report" class="button secondary small" @click="openRecordSources(item.report)">查看 Codex / Git 来源</button><button class="button secondary small" @click="router.push(`/calendar?date=${item.date}`)">打开日历</button><button v-if="item.testRuns" class="button secondary small" @click="router.push('/testing')">查看测试报告</button></footer></article><p v-if="!dayRecords.length" class="panel empty-state">没有符合筛选条件的工作记录。</p></section>
  <section v-else class="work-record-list"><article v-for="item in weekRecords" :key="item.start" class="panel work-record-card weekly"><header><div><b>{{ item.start }}—{{ item.end }}</b><span>{{ item.days.length }} 个活跃日</span></div><button @click="openTime(item.start,item.end,`${item.start} 本周工时`)"><strong>{{ minutesText(item.workMinutes) }}</strong><small>本周有效工时 →</small></button></header><div class="record-metrics"><span>Codex {{ item.conversations }} 次</span><span>Git {{ item.commits }} 次</span><span>测试 {{ item.testsPassed }}/{{ item.testRuns }}</span><span>任务操作 {{ item.taskActivityCount }}</span><span>知识 {{ item.knowledgeCount }}</span><span>Token {{ Math.round(item.tokens/1000) }}K</span><span>{{ item.report ? '已有周报' : '暂无周报' }}</span></div><ul v-if="reportSummary(item.report).length"><li v-for="line in reportSummary(item.report)" :key="line">{{ line }}</li></ul><div v-if="reportSection(item.report,'## 下一步计划').length" class="record-next"><b>尚未完成 / 下周建议</b><span v-for="line in reportSection(item.report,'## 下一步计划')" :key="line">{{ line }}</span></div><p v-else-if="!reportSummary(item.report).length">本周有活动记录，生成周报后可查看按项目归类的核心成果。</p><footer><button v-if="item.report" class="button primary small" @click="router.push(`/reports?report=${item.report.id}`)">查看本周做了什么</button><button v-if="item.report" class="button secondary small" @click="openRecordSources(item.report)">重要 Git / Codex 来源</button><button class="button secondary small" @click="openTime(item.start,item.end,'本周工时与项目分布')">工时与项目分布</button><button v-if="item.testRuns" class="button secondary small" @click="router.push('/testing')">查看测试报告</button></footer></article></section>
  <div v-if="sourceTitle" class="activity-backdrop" @click.self="sourceTitle=''"><aside class="activity-drawer panel"><header><div><h2>工作来源</h2><p>{{ sourceTitle }}</p></div><button class="icon-button" @click="sourceTitle=''">×</button></header><div class="report-source-list"><button v-for="item in recordSources" :key="`${item.kind}:${item.id}`" @click="openRecordSource(item)"><i>{{ item.kind }}</i><span><b>{{ item.title }}</b><small>{{ item.project }} · {{ item.date }} · {{ item.detail }}</small></span><em>{{ item.kind === 'Git 提交' ? '来源记录' : '查看 →' }}</em></button></div></aside></div>
  <WorkTimeDrawer :open="Boolean(selectedPeriod)" :start-date="selectedPeriod?.start || ''" :end-date="selectedPeriod?.end || ''" :title="selectedPeriod?.title" @close="selectedPeriod=null" @changed="load" />
</div></template>

<style scoped>
.record-next{margin:0 0 12px;padding:10px 12px;border-left:3px solid var(--warning);background:color-mix(in srgb,var(--warning) 7%,transparent);display:flex;flex-direction:column;gap:5px}.record-next span{font-size:10px;color:var(--muted)}
</style>
