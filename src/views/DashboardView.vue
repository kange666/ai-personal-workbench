<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import ActivityTrendChart from "../components/ActivityTrendChart.vue";
import BrandWordmark from "../components/BrandWordmark.vue";
import { APP_BRAND } from "../utils/brand";
import WorkTimeDrawer from "../components/WorkTimeDrawer.vue";
import { getDailyActivity, getDailyCheckin, getHistoryCoverage, getTokenTrend, getWorkSummary, isTauriRuntime, listInboxItems, listReports, listTestRuns, saveDailyCheckin, type DailyActivity, type DailyCheckin, type HistoryCoverage, type InboxItem, type ReportRecord, type TestRun, type TokenTrendPoint, type WorkSummary } from "../services/backend";

const router = useRouter();
const todayIso = new Date().toLocaleDateString("sv-SE");
const tokenTrend = ref<TokenTrendPoint[]>([]);
const activityTrend = ref<DailyActivity[]>([]);
const reports = ref<ReportRecord[]>([]);
const recentTests = ref<TestRun[]>([]);
const history = ref<HistoryCoverage | null>(null);
const inboxItems = ref<InboxItem[]>([]);
const emptyWorkSummary = (): WorkSummary => ({ startDate: todayIso, endDate: todayIso, totalMinutes: 0, estimatedMinutes: 0, manualMinutes: 0, hasManualCorrections: false, byProject: [], byType: [], daily: [] });
const todayWork = ref<WorkSummary>(emptyWorkSummary());
const weekWork = ref<WorkSummary>(emptyWorkSummary());
const selectedTimePeriod = ref<{ start: string; end: string; title: string } | null>(null);
const focusSeconds = ref(0);
const focusing = ref(false);
const checkin = ref<DailyCheckin>({ date:todayIso, mood:"", exerciseMinutes:0, note:"", createdAt:"", updatedAt:"" });
const checkinOpen = ref(false);
const savingCheckin = ref(false);
let focusTimer = 0;
const todayLabel = new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(new Date());
const todayReport = computed(() => reports.value.find(report => report.reportType === "daily" && report.periodStart === todayIso));
const weekReport = computed(() => reports.value.find(report => report.reportType === "weekly" && report.periodStart <= todayIso && report.periodEnd >= todayIso));
const todayToken = computed(() => tokenTrend.value.at(-1)?.totalTokens ?? (isTauriRuntime() ? 0 : 86_400));
const projectProgress = computed(() => weekWork.value.byProject.slice(0, 4).map(item => ({ name: item.name, progress: weekWork.value.totalMinutes ? Math.round(item.minutes / weekWork.value.totalMinutes * 100) : 0, minutes: item.minutes })));
const todayActivity = computed(() => activityTrend.value.find(item=>item.date===todayIso));
const todaySignals = computed(() => { const item=todayActivity.value; return item ? item.conversationCount+item.gitCommits+item.testRuns+item.knowledgeCount+item.quickCaptureCount+item.completedVideoCount : 0; });
const actionItems = computed(() => inboxItems.value.filter(item=>["needs_decision","in_progress"].includes(item.workflowStatus)).slice(0,5));
const topPriorities = computed(() => actionItems.value.slice(0,3));
const blockerCount = computed(() => actionItems.value.filter(item=>item.priority==="high").length);
function reportFeatures(report?: ReportRecord) { if (!report) return []; const lines=report.contentMarkdown.split("\n"); const start=lines.findIndex(line=>line.trim()==="## 项目工作总结"); if(start<0)return[]; const end=lines.findIndex((line,index)=>index>start&&line.startsWith("## ")); return lines.slice(start+1,end<0?undefined:end).filter(line=>line.startsWith("### ")||line.startsWith("- ")).slice(0,6).map(line=>line.replace(/^### /,"项目：").replace(/^- /,"")); }
const todayFeatures = computed(() => reportFeatures(todayReport.value));
const weekFeatures = computed(() => reportFeatures(weekReport.value));
function compactToken(value: number) {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(value);
}
function compactHours(value: number) { const hours = Math.floor(value / 60); const minutes = value % 60; return `${hours ? `${hours}h` : ""}${minutes ? `${minutes}m` : !hours ? "0m" : ""}`; }
function openSearch() { window.dispatchEvent(new CustomEvent("open-workbench-search")); }
function openQuickCapture() { window.dispatchEvent(new CustomEvent("open-quick-capture")); }
function openInboxItem(item:InboxItem) { void router.push({path:"/inbox",query:{item:item.id}}); }
function notificationTime(value:string) { const date=new Date(value); return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("zh-CN", {month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(date); }
function toggleFocus() {
  focusing.value = !focusing.value;
  window.clearInterval(focusTimer);
  if (focusing.value) focusTimer = window.setInterval(() => focusSeconds.value += 1, 1000);
}
const focusText = computed(() => `${String(Math.floor(focusSeconds.value / 60)).padStart(2, "0")}:${String(focusSeconds.value % 60).padStart(2, "0")}`);
async function persistCheckin() {
  if (!isTauriRuntime()) return;
  savingCheckin.value=true;
  try { checkin.value=await saveDailyCheckin(checkin.value); checkinOpen.value=false; }
  finally { savingCheckin.value=false; }
}

async function loadDashboardData() {
  if (!isTauriRuntime()) return;
  try {
    const activityStart = new Date(); activityStart.setDate(activityStart.getDate() - 6);
    [tokenTrend.value, activityTrend.value, reports.value, history.value, recentTests.value, inboxItems.value] = await Promise.all([getTokenTrend(7), getDailyActivity(activityStart.toLocaleDateString("sv-SE"), todayIso), listReports(), getHistoryCoverage(), listTestRuns(), listInboxItems(undefined,30)]);
    const todayDate = new Date(); const monday = new Date(todayDate); monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7));
    weekWork.value = await getWorkSummary(monday.toLocaleDateString("sv-SE"), todayIso, true);
    todayWork.value = await getWorkSummary(todayIso, todayIso, false);
    checkin.value = await getDailyCheckin(todayIso) || checkin.value;
  }
  catch (error) { console.error("首页统计读取失败", error); }
}
function refreshAfterCodexScan() { void loadDashboardData(); }
onMounted(() => {
  window.addEventListener("workbench-notifications-updated",refreshAfterCodexScan);
  window.addEventListener("workbench-codex-data-updated",refreshAfterCodexScan);
  void loadDashboardData();
});
onBeforeUnmount(() => {
  window.clearInterval(focusTimer);
  window.removeEventListener("workbench-notifications-updated",refreshAfterCodexScan);
  window.removeEventListener("workbench-codex-data-updated",refreshAfterCodexScan);
});
</script>

<template>
  <div class="view dashboard-view">
    <header class="page-header dashboard-brand-header">
      <div class="dashboard-brand-intro">
        <h1 :aria-label="APP_BRAND.name"><BrandWordmark variant="hero" /></h1>
        <p>一个整合项目、知识、自动化和工作状态的个人数字空间</p>
      </div>
      <div class="dashboard-brand-actions"><button class="button secondary" @click="openSearch">⌕ 搜索</button><button class="button primary" @click="openQuickCapture">＋ 快速记录</button></div>
    </header>
    <section class="welcome-strip action-overview"><div><b>{{ actionItems.length ? `当前有 ${actionItems.length} 项需要行动` : '当前没有阻塞事项' }}</b><p>{{ blockerCount ? `${blockerCount} 项高优先级 · 优先确认失败、缺陷和项目风险` : `今天已自动捕捉 ${todaySignals} 条工作信号` }}</p></div><div><RouterLink class="button primary link-button" to="/inbox">打开待处理收件箱</RouterLink><button class="button secondary" @click="toggleFocus">{{ focusing ? `暂停 ${focusText}` : '开始专注' }}</button></div></section>
    <section class="metric-grid"><article class="clickable-card" tabindex="0" @click="selectedTimePeriod={start:todayIso,end:todayIso,title:'今日工时明细'}"><span>今日工时</span><b>{{ compactHours(todayWork.totalMinutes) }}</b><p>{{ todayWork.hasManualCorrections ? '手工修正优先' : '估算工时' }} · 查看明细</p></article><article class="clickable-card" tabindex="0" @click="selectedTimePeriod={start:weekWork.startDate,end:weekWork.endDate,title:'本周工时明细'}"><span>本周工时</span><b>{{ compactHours(weekWork.totalMinutes) }}</b><p>{{ weekWork.byProject.length }} 个项目 · 查看分布</p></article><article class="clickable-card" tabindex="0" @click="router.push('/work-records')"><span>今日活动</span><b>{{ todaySignals }}<small> 条</small></b><p>{{ todayActivity?.conversationCount || 0 }} 个 Codex 对话 · {{ todayActivity?.gitCommits || 0 }} 次提交</p></article><article class="clickable-card" tabindex="0" @click="router.push('/tokens')"><span>今日 Token</span><b>{{ compactToken(todayToken) }}</b><p>只反映 AI 使用量</p></article></section>
    <section class="dashboard-notifications panel dashboard-actions">
      <div class="panel-head"><div><h2>今天优先处理</h2><p>按风险和来源状态排序，只展示需要决策或正在处理的事项</p></div><RouterLink to="/inbox">查看全部 →</RouterLink></div>
      <div class="dashboard-notification-list">
        <button v-for="item in topPriorities" :key="item.id" :class="[`priority-${item.priority}`,{unread:item.workflowStatus==='needs_decision'}]" @click="openInboxItem(item)"><i></i><span><b>{{ item.title }}</b><small>{{ item.project }} · {{ item.summary || item.detail }}</small></span><em>{{ notificationTime(item.updatedAt) }}</em></button>
        <p v-if="!topPriorities.length" class="panel-empty">当前没有需要你决策的事项。新的 Codex 结果、TAPD 缺陷、测试失败和项目风险会显示在这里。</p>
      </div>
    </section>
    <section class="dashboard-grid">
      <article class="panel today-panel passive-digest-panel"><div class="panel-head"><div><h2>今天自动记录了什么</h2><p>{{ todayLabel }} · 不依赖手工添加任务</p></div><RouterLink to="/work-records">查看明细 →</RouterLink></div><div class="passive-sources"><RouterLink to="/tokens"><b>{{ todayActivity?.conversationCount || 0 }}</b><span>Codex 对话</span><small>{{ todayActivity?.messageCount || 0 }} 条消息</small></RouterLink><RouterLink to="/projects"><b>{{ todayActivity?.gitCommits || 0 }}</b><span>Git 提交</span><small>按项目归类</small></RouterLink><RouterLink to="/testing"><b>{{ todayActivity?.testRuns || 0 }}</b><span>测试执行</span><small>{{ todayActivity?.testsPassed || 0 }} 次通过</small></RouterLink><RouterLink to="/knowledge"><b>{{ todayActivity?.knowledgeCount || 0 }}</b><span>知识更新</span><small>自动去重留版本</small></RouterLink></div><footer><span><b>可选生活状态</b><small>{{ checkin.energy ? `精力 ${checkin.energy}/5` : '尚未记录' }}<template v-if="checkin.exerciseMinutes"> · 运动 {{ checkin.exerciseMinutes }} 分钟</template></small></span><button class="button secondary small" @click="checkinOpen=true">{{ checkin.updatedAt ? '修改状态' : '顺手记一下' }}</button></footer></article>
      <article class="panel project-panel"><div class="panel-head"><div><h2>本周项目投入</h2><p>{{ projectProgress.length }} 个活跃项目 · 工时为估算/修正口径</p></div><RouterLink to="/work-records">工作记录 →</RouterLink></div><div v-for="item in projectProgress" :key="item.name" class="project-progress"><span><b>{{ item.name }}</b><em>{{ compactHours(item.minutes) }} · {{ item.progress }}%</em></span><div><i :style="{ width: `${item.progress}%` }"></i></div></div><p v-if="!projectProgress.length" class="panel-empty">本周暂无可估算的本地活动。</p></article>
      <article class="panel chart-panel clickable-card" tabindex="0" @click="router.push('/work-records')"><div class="panel-head"><div><h2>Codex 活跃趋势</h2><p>近 7 天对话次数 · 悬停查看数据</p></div><b>{{ activityTrend.reduce((sum,item) => sum + item.conversationCount, 0) }} 次</b></div><ActivityTrendChart :points="activityTrend" @select="router.push(`/calendar?date=${$event.date}`)" /></article>
      <article class="panel recent-panel"><div class="panel-head"><div><h2>最近报告</h2><p>日报与周报均可按历史周期查看</p></div><RouterLink to="/reports">全部报告 →</RouterLink></div><RouterLink v-for="report in reports.slice(0,3)" :key="report.id" class="dashboard-report" :to="`/reports?report=${report.id}`"><span>▤</span><div><b>{{ report.title }}</b><small>{{ report.status === 'locked' ? '已锁定' : '可编辑' }} · {{ report.periodStart }}</small></div></RouterLink><p v-if="!reports.length" class="panel-empty">尚无真实报告，可前往报告中心生成。</p></article>
      <article class="panel risk-panel"><div class="panel-head"><div><h2>最近测试与需要关注</h2><p>这里只展示真实测试结果，不混入未维护的任务</p></div><RouterLink to="/testing">测试中心 →</RouterLink></div><div v-for="test in recentTests.slice(0,4)" :key="test.id" class="dashboard-risk test" @click="router.push('/testing')"><i></i><span><b>{{ test.menuName }}</b><small>{{ test.project }} · {{ test.status === 'passed' ? '测试通过' : '测试失败' }} · {{ test.startedAt.slice(0,10) }}</small></span></div><p v-if="!recentTests.length" class="panel-empty">当前没有测试记录。</p></article>
      <article class="panel history-panel"><div class="panel-head"><div><h2>今天和本周做了什么</h2><p>{{ history?.firstDate || '尚无数据' }}—{{ history?.lastDate || '尚无数据' }} · 普通与归档会话统一统计</p></div><RouterLink to="/work-records">全部工作记录 →</RouterLink></div><div class="dashboard-result-columns"><RouterLink :to="todayReport ? `/reports?report=${todayReport.id}` : '/reports'"><b>今天</b><span v-for="item in todayFeatures.slice(0,3)" :key="item">{{ item }}</span><em v-if="!todayFeatures.length">尚未生成今天的项目成果总结</em></RouterLink><RouterLink :to="weekReport ? `/reports?report=${weekReport.id}` : '/reports'"><b>本周</b><span v-for="item in weekFeatures.slice(0,3)" :key="item">{{ item }}</span><em v-if="!weekFeatures.length">尚未生成本周的项目成果总结</em></RouterLink></div></article>
    </section>
    <WorkTimeDrawer :open="Boolean(selectedTimePeriod)" :start-date="selectedTimePeriod?.start || ''" :end-date="selectedTimePeriod?.end || ''" :title="selectedTimePeriod?.title" @close="selectedTimePeriod=null" @changed="router.go(0)" />
    <div v-if="checkinOpen" class="editor-backdrop" @click.self="checkinOpen=false"><aside class="task-editor daily-checkin-editor"><header><div><h2>今日状态（可选）</h2><p>只做个人回顾，不形成任务、打卡或绩效评分</p></div><button class="icon-button" @click="checkinOpen=false">×</button></header><label>精力<select v-model.number="checkin.energy"><option :value="undefined">不记录</option><option v-for="value in 5" :key="value" :value="value">{{ value }}/5</option></select></label><label>心情<input v-model="checkin.mood" placeholder="例如：平稳、专注、疲惫"></label><label>运动分钟<input v-model.number="checkin.exerciseMinutes" min="0" max="1440" type="number"></label><label>一句话备注<textarea v-model="checkin.note" rows="3" placeholder="今天身体或生活上值得记住的事"></textarea></label><footer><span></span><button class="button secondary" @click="checkinOpen=false">取消</button><button class="button primary" :disabled="savingCheckin" @click="persistCheckin">{{ savingCheckin ? '保存中…' : '保存' }}</button></footer></aside></div>
  </div>
</template>

<style scoped>
.dashboard-brand-header{height:auto;min-height:84px;gap:20px;align-items:center;padding:2px 0 18px}
.dashboard-brand-intro{min-width:0;flex:1}
.dashboard-brand-header h1{margin:0 0 10px;line-height:1}
.dashboard-brand-intro p{font-size:11px;line-height:1.65;letter-spacing:.04em;max-width:48em}
.dashboard-brand-actions{flex:0 0 auto}
.dashboard-task-panel{overflow:hidden}.week-task-progress{height:3px;background:var(--surface-2)}.week-task-progress i{display:block;height:100%;background:var(--success)}.dashboard-task-columns{display:grid;grid-template-columns:1fr 1fr;height:180px}.dashboard-task-columns>section{min-width:0;border-right:1px solid var(--line);overflow:hidden}.dashboard-task-columns>section:last-child{border-right:0}.dashboard-task-columns h3{height:28px;margin:0;padding:8px 14px 0;color:var(--muted);font-size:10px}.dashboard-task-columns p{padding:12px 14px;color:var(--muted)}
.dashboard-result-columns{display:grid;grid-template-columns:1fr 1fr;gap:10px;padding:0 16px 14px}.dashboard-result-columns>a{min-height:130px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:10px;color:inherit;text-decoration:none;display:flex;flex-direction:column;gap:6px}.dashboard-result-columns b{color:var(--primary)}.dashboard-result-columns span{font-size:10px;line-height:1.45}.dashboard-result-columns em{font-style:normal;color:var(--muted);font-size:10px}.dashboard-risk{cursor:pointer}.dashboard-risk.test>i{background:var(--primary)}
.dashboard-notifications{margin-bottom:12px;overflow:hidden}.dashboard-notifications .panel-head{height:54px}.dashboard-notification-list{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));border-top:1px solid var(--line)}.dashboard-notification-list>button{min-width:0;min-height:76px;border:0;border-right:1px solid var(--line);background:transparent;color:inherit;padding:11px 14px;display:grid;grid-template-columns:8px minmax(0,1fr);gap:9px;text-align:left;position:relative}.dashboard-notification-list>button:last-child{border-right:0}.dashboard-notification-list>button:hover{background:var(--primary-soft)}.dashboard-notification-list>button>i{width:7px;height:7px;margin-top:5px;border-radius:50%;background:var(--line)}.dashboard-notification-list>button.unread>i{background:var(--primary);box-shadow:0 0 0 3px var(--primary-soft)}.dashboard-notification-list span{min-width:0;display:flex;flex-direction:column;gap:6px}.dashboard-notification-list b,.dashboard-notification-list small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.dashboard-notification-list small{color:var(--muted)}.dashboard-notification-list em{position:absolute;right:12px;bottom:8px;color:var(--muted);font-size:9px;font-style:normal}.dashboard-notification-list>.panel-empty{grid-column:1/4;margin:0;padding:22px 16px}
.dashboard-notification-list>button.priority-high>i{background:var(--danger);box-shadow:0 0 0 3px color-mix(in srgb,var(--danger) 10%,transparent)}.dashboard-actions .panel-head>a{color:var(--primary);text-decoration:none}.action-overview{border-color:color-mix(in srgb,var(--primary) 28%,var(--line));background:color-mix(in srgb,var(--primary) 4%,var(--surface))}
.passive-digest-panel{overflow:hidden}.passive-sources{display:grid;grid-template-columns:repeat(4,1fr);border-top:1px solid var(--line);border-bottom:1px solid var(--line)}.passive-sources>a{min-width:0;padding:13px;color:inherit;text-decoration:none;display:grid;gap:4px;border-right:1px solid var(--line)}.passive-sources>a:last-child{border-right:0}.passive-sources>a:hover{background:var(--primary-soft)}.passive-sources b{font-size:22px}.passive-sources span{font-weight:700}.passive-sources small{color:var(--muted)}.passive-digest-panel>footer{min-height:58px;padding:10px 14px;display:flex;align-items:center;justify-content:space-between;gap:12px}.passive-digest-panel>footer>span{display:grid;gap:4px}.passive-digest-panel>footer small{color:var(--muted)}.daily-checkin-editor{width:min(520px,calc(100vw - 40px))}
@media(max-width:1050px){.dashboard-notification-list{grid-template-columns:1fr}.dashboard-notification-list>button{border-right:0;border-bottom:1px solid var(--line)}}
</style>
