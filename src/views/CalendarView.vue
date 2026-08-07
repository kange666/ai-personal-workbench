<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import TaskEditor from "../components/TaskEditor.vue";
import TasksView from "./TasksView.vue";
import WorkTimeDrawer from "../components/WorkTimeDrawer.vue";
import { useWorkbenchStore } from "../stores/workbench";
import type { WorkTask } from "../types/workbench";
import { getDailyActivity, isTauriRuntime, listReports, listWorkSessions, type DailyActivity, type ReportRecord, type WorkSession } from "../services/backend";
import { getAlmanac, lunarDayLabel } from "../utils/almanac";

defineEmits<{ "new-task": [] }>();
const store = useWorkbenchStore();
const router = useRouter();
const route = useRoute();
const activeSection = ref<"planning" | "tasks">(route.query.tab === "tasks" ? "tasks" : "planning");
const viewMode = ref<"calendar" | "gantt" | "combined">("combined");
const activeMonth = ref(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
const referenceToday = new Date();
const activities = ref<DailyActivity[]>([]);
const reports = ref<ReportRecord[]>([]);
const workSessions = ref<WorkSession[]>([]);
const selectedDate = ref("");
const selectedTask = ref<WorkTask | null>(null);
const projectFilter = ref("全部");
const loadingActivity = ref(false);
const selectedTimeDate = ref("");
const selectedAlmanacDate = ref("");
function minutesText(value: number) { const hours=Math.floor(value/60); const minutes=value%60; return `${hours ? `${hours}h` : ''}${minutes ? `${minutes}m` : !hours ? '0m' : ''}`; }

function iso(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function parse(value: string) { return new Date(`${value}T00:00:00`); }
function addDays(date: Date, count: number) { const next = new Date(date); next.setDate(next.getDate() + count); return next; }
function dayDiff(a: Date, b: Date) { return Math.round((a.getTime() - b.getTime()) / 86_400_000); }
function weekStart(date: Date) { const start = new Date(date); start.setDate(start.getDate() - ((start.getDay() + 6) % 7)); return start; }

const monthTitle = computed(() => `${activeMonth.value.getFullYear()}年${activeMonth.value.getMonth() + 1}月`);
const calendarDays = computed(() => {
  const first = new Date(activeMonth.value.getFullYear(), activeMonth.value.getMonth(), 1);
  const start = addDays(first, -((first.getDay() + 6) % 7));
  return Array.from({ length: 42 }, (_, index) => {
    const date = addDays(start, index);
    return { date, iso: iso(date), value: date.getDate(), current: date.getMonth() === activeMonth.value.getMonth(), today: iso(date) === iso(referenceToday) };
  });
});

const ganttStart = computed(() => weekStart(referenceToday));
const ganttDays = computed(() => Array.from({ length: 14 }, (_, index) => addDays(ganttStart.value, index)));
const ganttEnd = computed(() => ganttDays.value[13]);
const projects = computed(() => ["全部", ...new Set(store.tasks.map(task => task.project))]);
const activityByDate = computed(() => new Map(activities.value.map(item => [item.date, item])));
const selectedActivity = computed(() => activityByDate.value.get(selectedDate.value));
const selectedDateTasks = computed(() => store.tasks.filter(task => {
  if (task.scope === "day") return task.plannedDate === selectedDate.value;
  if (task.scope === "week" && task.weekStart) { const end=iso(addDays(parse(task.weekStart),6)); return selectedDate.value >= task.weekStart && selectedDate.value <= end; }
  return task.scope === "project" && Boolean(task.startDate && task.endDate && selectedDate.value >= task.startDate && selectedDate.value <= task.endDate);
}));
const selectedDateProjects = computed(() => [...new Set([
  ...workSessions.value.filter(item => item.date === selectedDate.value).map(item => item.project),
  ...selectedDateTasks.value.map(item => item.project),
])]);
const selectedDateReport = computed(() => reports.value.find(item => item.id === selectedActivity.value?.dailyReportId));
const selectedDateFeatures = computed(() => {
  const content=selectedDateReport.value?.contentMarkdown || ""; const lines=content.split("\n"); const start=lines.findIndex(line=>line.trim()==="## 项目工作总结");
  if(start<0)return[]; const end=lines.findIndex((line,index)=>index>start&&line.startsWith("## "));
  return lines.slice(start+1,end<0?undefined:end).filter(line=>line.startsWith("### ") || line.startsWith("- ")).slice(0,8).map(line=>line.replace(/^### /,"项目：").replace(/^- /,""));
});
const todayAlmanac = computed(() => getAlmanac(iso(referenceToday)));
const selectedAlmanac = computed(() => selectedAlmanacDate.value ? getAlmanac(selectedAlmanacDate.value) : null);

function taskRange(task: WorkTask): [Date, Date] | null {
  if (task.scope === "day" && task.plannedDate) { const date = parse(task.plannedDate); return [date, date]; }
  if (task.scope === "week" && task.weekStart) { const start = parse(task.weekStart); return [start, addDays(start, 6)]; }
  if (task.scope === "project" && task.startDate && task.endDate) return [parse(task.startDate), parse(task.endDate)];
  return null;
}

const ganttItems = computed(() => store.tasks.filter(task => projectFilter.value === "全部" || task.project === projectFilter.value).map((task) => {
  const range = taskRange(task);
  if (!range || range[1] < ganttStart.value || range[0] > ganttEnd.value) return null;
  const visibleStart = range[0] < ganttStart.value ? ganttStart.value : range[0];
  const visibleEnd = range[1] > ganttEnd.value ? ganttEnd.value : range[1];
  return {
    task,
    left: dayDiff(visibleStart, ganttStart.value) / 14 * 100,
    width: (dayDiff(visibleEnd, visibleStart) + 1) / 14 * 100,
    state: task.status === "done" ? "done" : task.status === "doing" ? "active" : "planned",
  };
}).filter((item): item is NonNullable<typeof item> => Boolean(item)).slice(0, 7));

function tasksForDate(date: string) { return store.tasks.filter((task) => task.scope === "day" && task.plannedDate === date && (projectFilter.value === "全部" || task.project === projectFilter.value)); }
function previousMonth() { activeMonth.value = new Date(activeMonth.value.getFullYear(), activeMonth.value.getMonth() - 1, 1); }
function nextMonth() { activeMonth.value = new Date(activeMonth.value.getFullYear(), activeMonth.value.getMonth() + 1, 1); }
function goToday() { activeMonth.value = new Date(referenceToday.getFullYear(), referenceToday.getMonth(), 1); }
async function loadActivity() {
  if (!isTauriRuntime()) return;
  const days = calendarDays.value;
  loadingActivity.value = true;
  try { [activities.value,reports.value,workSessions.value] = await Promise.all([getDailyActivity(days[0].iso, days.at(-1)!.iso),listReports(),listWorkSessions(days[0].iso,days.at(-1)!.iso,false)]); }
  finally { loadingActivity.value = false; }
}
function openDay(date: string) { selectedDate.value = date; }
function openReport(id?: string) { if (id) void router.push(`/reports?report=${id}`); }
function switchSection(value: "planning" | "tasks") {
  activeSection.value = value;
  const query = { ...route.query };
  if (value === "tasks") query.tab = "tasks"; else delete query.tab;
  void router.replace({ path: "/calendar", query });
}
watch(() => route.query.tab, value => { activeSection.value = value === "tasks" ? "tasks" : "planning"; });
watch(() => `${activeMonth.value.getFullYear()}-${activeMonth.value.getMonth()}`, loadActivity);
onMounted(() => { const date=String(route.query.date || ""); if (/^\d{4}-\d{2}-\d{2}$/.test(date)) { selectedDate.value=date; const parsed=parse(date); activeMonth.value=new Date(parsed.getFullYear(),parsed.getMonth(),1); } void loadActivity(); });
</script>

<template>
  <div class="view calendar-view">
    <header class="page-header"><div><h1>工作日历</h1><p>日历、甘特与任务在同一处管理，Codex 活动、工时、测试和报告按日期统一展示</p></div><div v-if="activeSection === 'planning'"><select v-model="projectFilter" class="button secondary" title="按项目筛选"><option v-for="project in projects" :key="project" :value="project">项目：{{ project }}</option></select><button class="button primary" @click="$emit('new-task')">＋ 新增任务</button></div></header>
    <nav class="calendar-main-tabs"><button :class="{ active:activeSection==='planning' }" @click="switchSection('planning')">日历与甘特</button><button :class="{ active:activeSection==='tasks' }" @click="switchSection('tasks')">任务管理</button></nav>
    <template v-if="activeSection === 'planning'">
    <div class="calendar-toolbar"><div class="mode-switch"><button :class="{ active:viewMode==='calendar' }" @click="viewMode='calendar'">日历</button><button :class="{ active:viewMode==='gantt' }" @click="viewMode='gantt'">甘特图</button><button :class="{ active:viewMode==='combined' }" @click="viewMode='combined'">组合视图</button></div><div><button class="icon-button" @click="previousMonth">‹</button><button class="button secondary" @click="goToday">今天</button><button class="icon-button" @click="nextMonth">›</button><b>{{ monthTitle }}</b></div></div>
    <section class="panel almanac-today"><div class="almanac-date-mark"><span>{{ referenceToday.getDate() }}</span><small>{{ todayAlmanac.week }}</small></div><div><small>今日黄历 · {{ todayAlmanac.lunarDate }}</small><b>{{ todayAlmanac.ganZhi }}</b><p>{{ todayAlmanac.duty }} · {{ todayAlmanac.heavenlyGod }}（{{ todayAlmanac.luck }}） · {{ todayAlmanac.clash }} {{ todayAlmanac.sha }}</p></div><div class="almanac-brief good"><i>宜</i><span>{{ todayAlmanac.yi.slice(0,5).join(' · ') || '诸事不宜' }}</span></div><div class="almanac-brief avoid"><i>忌</i><span>{{ todayAlmanac.ji.slice(0,5).join(' · ') || '诸事不忌' }}</span></div><button class="button secondary" @click="selectedAlmanacDate=iso(referenceToday)">详细黄历</button></section>
    <section class="calendar-layout" :class="`mode-${viewMode}`">
      <article v-if="viewMode !== 'gantt'" class="panel calendar-panel">
        <div class="week-banner"><span>本周任务</span><b>{{ store.weekTasks[0]?.title || '暂无本周任务' }}</b><strong v-if="store.weekTasks.length > 1">+{{ store.weekTasks.length - 1 }}</strong><i>周一—周日</i></div>
        <div class="week-days"><span v-for="name in ['周一','周二','周三','周四','周五','周六','周日']" :key="name">{{ name }}</span></div>
        <div class="month-grid"><div v-for="day in calendarDays" :key="day.iso" class="calendar-day" :class="{ muted:!day.current, today:day.today, active: selectedDate === day.iso }" @click="openDay(day.iso)"><b>{{ day.value }}</b><button class="lunar-day-label" title="查看该日黄历" @click.stop="selectedAlmanacDate=day.iso">{{ lunarDayLabel(day.iso) }}</button><button v-for="task in tasksForDate(day.iso).slice(0,2)" :key="task.id" class="calendar-task" :class="task.status" :title="task.title" @click.stop="selectedTask = task">{{ task.title }}</button><button v-if="activityByDate.get(day.iso)?.workMinutes" class="calendar-worktime" title="估算工时，可点击查看和手工修正" @click.stop="selectedTimeDate=day.iso">◷ {{ minutesText(activityByDate.get(day.iso)?.workMinutes || 0) }} {{ activityByDate.get(day.iso)?.manualWorkMinutes ? '已修正' : '估算' }}</button><button v-if="activityByDate.get(day.iso)?.conversationCount" class="calendar-activity" title="查看当天 Codex 与 Token 数据" @click.stop="openDay(day.iso)">◔ {{ activityByDate.get(day.iso)?.conversationCount }} · {{ Math.round((activityByDate.get(day.iso)?.totalTokens || 0) / 1000) }}K</button><button v-if="activityByDate.get(day.iso)?.testRuns" class="calendar-test" title="打开当天测试结果" @click.stop="router.push(`/testing?date=${day.iso}`)">✓ {{ activityByDate.get(day.iso)?.testsPassed }}/{{ activityByDate.get(day.iso)?.testRuns }} 测试</button><button v-if="activityByDate.get(day.iso)?.contentIdeaCount" class="calendar-content" title="打开当天内容候选" @click.stop="router.push(`/content?date=${day.iso}`)">✦ {{ activityByDate.get(day.iso)?.contentIdeaCount }} 个选题</button><button v-if="activityByDate.get(day.iso)?.dailyReportId" class="calendar-report" title="打开当日日报" @click.stop="openReport(activityByDate.get(day.iso)?.dailyReportId)">▤ 日报</button><small v-if="tasksForDate(day.iso).length > 2">还有 {{ tasksForDate(day.iso).length - 2 }} 项任务</small></div></div>
      </article>
      <article v-if="viewMode !== 'calendar'" class="panel gantt-panel">
        <div class="gantt-title"><div><h2>全部任务时间线</h2><p>{{ iso(ganttStart) }}—{{ iso(ganttEnd) }}</p></div><span>{{ ganttItems.length }} 项</span></div>
        <div class="gantt-scale"><span></span><i v-for="day in ganttDays" :key="iso(day)">{{ day.getMonth() + 1 }}/{{ day.getDate() }}</i></div>
        <div class="gantt-rows"><div v-for="item in ganttItems" :key="item.task.id" class="gantt-row"><button :title="item.task.title" @click="selectedTask = item.task">{{ item.task.title }}</button><div><button class="gantt-bar" :class="item.state" :style="{ left:`${item.left}%`, width:`${item.width}%` }" title="点击查看任务详情" @click="selectedTask = item.task"><span :style="{ width:`${item.task.progress}%` }"></span></button></div></div><span class="today-marker">今天</span><div v-if="!ganttItems.length" class="gantt-empty">当前时间范围没有已排期任务</div></div>
        <footer><span>■ 已完成</span><span>■ 进行中</span><span>□ 待开始</span></footer>
      </article>
    </section>
      <div v-if="selectedDate" class="activity-backdrop" @click.self="selectedDate = ''"><aside class="activity-drawer panel"><header><div><h2>{{ selectedDate }} 工作记录</h2><p>当天做了什么、涉及项目、任务、Git、测试、Token 与报告</p></div><button class="icon-button" @click="selectedDate = ''">×</button></header><template v-if="selectedActivity"><div class="activity-metrics"><button @click="selectedTimeDate=selectedDate"><b>◷ {{ minutesText(selectedActivity.workMinutes) }}</b><span>{{ selectedActivity.manualWorkMinutes ? '有效工时 · 含手工修正' : '估算工时' }}</span></button><button @click="router.push(`/tokens?date=${selectedDate}`)"><b>◔ {{ selectedActivity.conversationCount }}</b><span>Codex 对话</span></button><button @click="router.push('/testing')"><b>✓ {{ selectedActivity.testsPassed }}/{{ selectedActivity.testRuns }}</b><span>测试通过 / 执行</span></button><button @click="router.push('/calendar?tab=tasks')"><b>{{ selectedDateTasks.filter(item=>item.status==='done').length }}/{{ selectedDateTasks.length }}</b><span>任务完成</span></button></div><dl><div><dt>涉及项目</dt><dd>{{ selectedDateProjects.join('、') || '未归类' }}</dd></div><div><dt>Git 提交 / 内容候选</dt><dd>{{ selectedActivity.gitCommits }} / {{ selectedActivity.contentIdeaCount }}</dd></div><div><dt>原始估算 / 手工记录</dt><dd>{{ minutesText(selectedActivity.estimatedWorkMinutes) }} / {{ minutesText(selectedActivity.manualWorkMinutes) }}</dd></div><div><dt>普通与归档对话</dt><dd>{{ selectedActivity.conversationCount - selectedActivity.archivedConversationCount }} / {{ selectedActivity.archivedConversationCount }}</dd></div><div><dt>Token / 知识沉淀</dt><dd>{{ selectedActivity.totalTokens.toLocaleString() }} / {{ selectedActivity.knowledgeCount }}</dd></div><div><dt>用户消息 / AI 回复</dt><dd>{{ selectedActivity.userMessages }} / {{ selectedActivity.assistantMessages }}</dd></div></dl><div v-if="selectedDateFeatures.length" class="date-feature-summary"><h3>完成的核心功能</h3><ul><li v-for="item in selectedDateFeatures" :key="item">{{ item }}</li></ul></div><div class="activity-actions"><button v-if="selectedActivity.dailyReportId" class="button primary" @click="openReport(selectedActivity.dailyReportId)">▤ 今天做了什么</button><button v-if="selectedActivity.weeklyReportId" class="button secondary" @click="openReport(selectedActivity.weeklyReportId)">▤ 本周做了什么</button><button class="button secondary" @click="selectedTimeDate=selectedDate">◷ 工时明细与修正</button><button class="button secondary" @click="router.push(`/tokens?date=${selectedDate}`)">◔ Token 明细</button><button v-if="selectedActivity.testRuns" class="button secondary" @click="router.push('/testing')">✓ 查看测试报告</button><button v-if="selectedActivity.knowledgeCount" class="button secondary" @click="router.push('/knowledge')">⌘ 查看知识</button><button v-if="selectedActivity.contentIdeaCount" class="button secondary" @click="router.push(`/content?date=${selectedDate}`)">✦ 查看内容候选</button></div></template><p v-else-if="loadingActivity">正在读取活动…</p><p v-else class="panel-empty">当天没有 Codex、任务、测试、工时、Git、知识、报告或内容活动。</p></aside></div>
    <div v-if="selectedAlmanac" class="activity-backdrop almanac-backdrop" @click.self="selectedAlmanacDate=''">
      <section class="panel almanac-dialog"><header><div><small>传统黄历 · 仅供日程参考</small><h2>{{ selectedAlmanac.date }} {{ selectedAlmanac.week }}</h2><p>{{ selectedAlmanac.lunarDate }} · {{ selectedAlmanac.ganZhi }}</p></div><button class="icon-button" @click="selectedAlmanacDate=''">×</button></header><div class="almanac-hero"><div><span>{{ selectedAlmanac.luck }}</span><b>{{ selectedAlmanac.duty }}</b><small>{{ selectedAlmanac.heavenlyGod }}值日 · {{ selectedAlmanac.zodiac }}</small></div><dl><div><dt>节气 / 节日</dt><dd>{{ [selectedAlmanac.jieQi,...selectedAlmanac.festivals].filter(item=>item!=='非节气日').join(' · ') || '无' }}</dd></div><div><dt>冲煞</dt><dd>{{ selectedAlmanac.clash }} · {{ selectedAlmanac.sha }}</dd></div><div><dt>星宿</dt><dd>{{ selectedAlmanac.mansion }}（{{ selectedAlmanac.mansionLuck }}）</dd></div></dl></div><div class="almanac-yi-ji"><article class="good"><h3><i>宜</i>适宜事项</h3><div><span v-for="item in selectedAlmanac.yi" :key="item">{{ item }}</span></div></article><article class="avoid"><h3><i>忌</i>谨慎事项</h3><div><span v-for="item in selectedAlmanac.ji" :key="item">{{ item }}</span></div></article></div><div class="almanac-details"><article><h3>吉神宜趋</h3><p>{{ selectedAlmanac.auspiciousGods.join('、') || '无' }}</p></article><article><h3>凶煞宜忌</h3><p>{{ selectedAlmanac.inauspiciousGods.join('、') || '无' }}</p></article><article><h3>吉神方位</h3><p>喜神 {{ selectedAlmanac.joyPosition }} · 福神 {{ selectedAlmanac.fortunePosition }} · 财神 {{ selectedAlmanac.wealthPosition }}</p></article><article><h3>彭祖百忌</h3><p>{{ selectedAlmanac.pengZu.join('；') }}</p></article></div><footer>黄历属于传统民俗信息，不用于替代工作优先级、医疗、法律或财务判断。</footer></section>
    </div>
    <TaskEditor :open="Boolean(selectedTask)" :task="selectedTask" @close="selectedTask = null" @update="store.updateTask" @remove="store.removeTask" />
    <WorkTimeDrawer :open="Boolean(selectedTimeDate)" :start-date="selectedTimeDate" :end-date="selectedTimeDate" :title="`${selectedTimeDate} 工时明细`" @close="selectedTimeDate=''" @changed="loadActivity" />
    </template>
    <TasksView v-else embedded />
  </div>
</template>
