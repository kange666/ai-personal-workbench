<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  getDailyActivity,
  getWorkSummary,
  isTauriRuntime,
  listReports,
  listWorkSessions,
  type CodexQuotaSnapshot,
  type CodexQuotaWindow,
  type DailyActivity,
  type ReportRecord,
  type RunningProjectProcess,
  type TapdCodexJob,
  type TestRun,
  type WorkbenchNotification,
  type WorkSession,
  type WorkSummary,
} from "../services/backend";
import { activityHeatLevel, cockpitHours, compactCockpitNumber, localDateIso, recentDateKeys } from "../utils/cockpit";
import { estimateTestRunProgress } from "../utils/testRunProgress";
import CockpitIcon from "./CockpitIcon.vue";

type Period = "today" | "week" | "month" | "custom";
type TimelineLane = "codex" | "git" | "test" | "report";
const periodOptions:Array<{key:Period;label:string}> = [{key:"today",label:"今日"},{key:"week",label:"本周"},{key:"month",label:"本月"},{key:"custom",label:"自定义"}];

const props = defineProps<{
  quota: CodexQuotaSnapshot;
  notifications: WorkbenchNotification[];
  runningProjects: RunningProjectProcess[];
  testRuns: TestRun[];
  tapdJobs: TapdCodexJob[];
}>();
const emit = defineEmits<{ close: []; navigate: [route: string] }>();

const returnButton = ref<HTMLButtonElement | null>(null);
const period = ref<Period>("week");
const now = ref(new Date());
const todayIso = computed(() => localDateIso(now.value));
const customStart = ref(todayIso.value);
const customEnd = ref(todayIso.value);
const loading = ref(false);
const errorMessage = ref("");
const periodActivity = ref<DailyActivity[]>([]);
const heatActivity = ref<DailyActivity[]>([]);
const reports = ref<ReportRecord[]>([]);
const workSessions = ref<WorkSession[]>([]);
const workSummary = ref<WorkSummary>({ startDate:"", endDate:"", totalMinutes:0, estimatedMinutes:0, manualMinutes:0, hasManualCorrections:false, byProject:[], byType:[], daily:[] });
const pulseHoverIndex = ref<number | null>(null);
let clockTimer = 0;
let refreshTimer = 0;

function startOfWeek(date: Date) {
  const result = new Date(date);
  result.setDate(result.getDate() - ((result.getDay() + 6) % 7));
  return result;
}

const selectedRange = computed(() => {
  const end = new Date(now.value);
  let start = new Date(end);
  if (period.value === "week") start = startOfWeek(end);
  else if (period.value === "month") start = new Date(end.getFullYear(), end.getMonth(), 1, 12);
  else if (period.value === "custom") {
    const left = customStart.value || todayIso.value;
    const right = customEnd.value || left;
    return left <= right ? { start:left, end:right } : { start:right, end:left };
  }
  return { start:localDateIso(start), end:localDateIso(end) };
});

function isWithinRange(value: string | undefined, start: string, end: string) {
  if (!value) return false;
  const date = localDateIso(new Date(value));
  return date >= start && date <= end;
}

async function loadCockpitData() {
  if (!isTauriRuntime()) return;
  loading.value = true;
  errorMessage.value = "";
  const heatDates = recentDateKeys(90, now.value);
  const range = selectedRange.value;
  const [activityResult, heatResult, workResult, reportResult, sessionResult] = await Promise.allSettled([
    getDailyActivity(range.start, range.end),
    getDailyActivity(heatDates[0], heatDates[heatDates.length - 1]),
    getWorkSummary(range.start, range.end, false),
    listReports(),
    listWorkSessions(todayIso.value, todayIso.value, false),
  ]);
  if (activityResult.status === "fulfilled") periodActivity.value = activityResult.value;
  if (heatResult.status === "fulfilled") heatActivity.value = heatResult.value;
  if (workResult.status === "fulfilled") workSummary.value = workResult.value;
  if (reportResult.status === "fulfilled") reports.value = reportResult.value;
  if (sessionResult.status === "fulfilled") workSessions.value = sessionResult.value;
  const failed = [activityResult, heatResult, workResult, reportResult, sessionResult].filter(item => item.status === "rejected");
  if (failed.length) errorMessage.value = `${failed.length} 项本地数据暂时读取失败，将继续自动刷新。`;
  loading.value = false;
}

const filteredTests = computed(() => props.testRuns.filter(run => !["queued","running"].includes(run.status) && isWithinRange(run.startedAt, selectedRange.value.start, selectedRange.value.end)));
const reportCount = computed(() => reports.value.filter(report => report.periodEnd >= selectedRange.value.start && report.periodStart <= selectedRange.value.end).length);
const conversationCount = computed(() => periodActivity.value.reduce((sum,item) => sum + item.conversationCount, 0));
const tokenCount = computed(() => periodActivity.value.reduce((sum,item) => sum + item.totalTokens, 0));
const activeProjectCount = computed(() => workSummary.value.byProject.length);
const testPassRate = computed(() => {
  const totals = filteredTests.value.reduce((result,run) => ({ total:result.total + run.totalCount, passed:result.passed + run.passedCount }), { total:0, passed:0 });
  if (totals.total) return Math.round(totals.passed / totals.total * 100);
  if (!filteredTests.value.length) return 0;
  return Math.round(filteredTests.value.filter(run => run.status === "passed").length / filteredTests.value.length * 100);
});
const workTimeLabel = computed(() => period.value === "today" ? "今日投入" : period.value === "week" ? "本周投入" : period.value === "month" ? "本月投入" : "区间投入");
const kpis = computed(() => [
  { label:"活跃项目", value:String(activeProjectCount.value), unit:"", icon:"project" },
  { label:workTimeLabel.value, value:cockpitHours(workSummary.value.totalMinutes), unit:"", icon:"clock" },
  { label:"Codex 对话", value:String(conversationCount.value), unit:"", icon:"chat" },
  { label:"Token", value:compactCockpitNumber(tokenCount.value), unit:"", icon:"token" },
  { label:"测试通过率", value:String(testPassRate.value), unit:"%", icon:"test" },
  { label:"报告数量", value:String(reportCount.value), unit:"", icon:"report" },
]);

const heatDates = computed(() => recentDateKeys(90, now.value));
const heatCells = computed(() => {
  const byDate = new Map(heatActivity.value.map(item => [item.date,item]));
  const values = heatDates.value.map(date => {
    const item = byDate.get(date);
    const value = item ? item.conversationCount + item.gitCommits + item.testRuns + item.testsPassed + item.workMinutes / 30 + Number(Boolean(item.dailyReportId)) : 0;
    return { date, value, item };
  });
  const maximum = Math.max(...values.map(item => item.value), 0);
  return values.map(item => ({ ...item, level:activityHeatLevel(item.value, maximum) }));
});

const pulsePoints = computed(() => {
  const byDate = new Map(heatActivity.value.map(item => [item.date,item]));
  return heatDates.value.slice(-7).map(date => byDate.get(date) || ({ date, conversationCount:0, archivedConversationCount:0, messageCount:0, userMessages:0, assistantMessages:0, inputTokens:0, cachedInputTokens:0, outputTokens:0, reasoningOutputTokens:0, totalTokens:0, gitCommits:0, contentIdeaCount:0, workMinutes:0, estimatedWorkMinutes:0, manualWorkMinutes:0, testRuns:0, testsPassed:0, knowledgeCount:0, taskActivityCount:0, quickCaptureCount:0, completedVideoCount:0 } satisfies DailyActivity));
});

const chart = { width:760, height:220, left:34, right:18, top:24, bottom:32 };
function chartX(index:number, count:number, dimensions = chart) {
  const width = dimensions.width - dimensions.left - dimensions.right;
  return count <= 1 ? dimensions.left + width / 2 : dimensions.left + index / (count - 1) * width;
}
function chartY(value:number, maximum:number, dimensions = chart) {
  const height = dimensions.height - dimensions.top - dimensions.bottom;
  return dimensions.top + height - (maximum ? value / maximum * height : 0);
}
function linePath(values:number[], maximum:number, dimensions = chart) {
  return values.map((value,index) => `${index ? "L" : "M"}${chartX(index,values.length,dimensions).toFixed(1)},${chartY(value,maximum,dimensions).toFixed(1)}`).join(" ");
}

const pulseSeries = computed(() => {
  const values = [
    { key:"work", label:"工时", values:pulsePoints.value.map(item => Number((item.workMinutes / 60).toFixed(1))), dash:"" },
    { key:"conversation", label:"对话", values:pulsePoints.value.map(item => item.conversationCount), dash:"8 5" },
    { key:"commit", label:"提交", values:pulsePoints.value.map(item => item.gitCommits), dash:"2 5" },
    { key:"test", label:"测试", values:pulsePoints.value.map(item => item.testRuns), dash:"10 4 2 4" },
  ];
  const maximum = Math.max(...values.flatMap(item => item.values), 1);
  return values.map((item,index) => ({ ...item, index, path:linePath(item.values,maximum), maximum }));
});

const projectRanking = computed(() => workSummary.value.byProject.slice(0,5));
const projectMaxMinutes = computed(() => Math.max(...projectRanking.value.map(item => item.minutes), 1));
const outcomePoints = computed(() => pulsePoints.value.map(item => ({ date:item.date, hours:Number((item.workMinutes / 60).toFixed(1)), outcomes:item.gitCommits + item.testsPassed + Number(Boolean(item.dailyReportId)) })));
const outcomeDimensions = { width:320, height:145, left:22, right:12, top:12, bottom:24 };
const outcomeMax = computed(() => Math.max(...outcomePoints.value.flatMap(item => [item.hours,item.outcomes]), 1));
const outcomeWorkPath = computed(() => linePath(outcomePoints.value.map(item => item.hours),outcomeMax.value,outcomeDimensions));
const outcomeResultPath = computed(() => linePath(outcomePoints.value.map(item => item.outcomes),outcomeMax.value,outcomeDimensions));

function quotaLabel(item: CodexQuotaWindow) {
  if (item.windowMinutes >= 10_080) return "7d";
  if (item.windowMinutes >= 60) return `${Math.round(item.windowMinutes / 60)}h`;
  return `${item.windowMinutes}m`;
}
const quotaWindows = computed(() => [props.quota.primary,props.quota.secondary].filter((item):item is CodexQuotaWindow => Boolean(item)).slice(0,2));
const quotaFreshnessText = computed(() => ({ fresh:"刚刚更新",recent:"近期快照",stale:"快照较旧" } as Record<string,string>)[props.quota.freshness] || "实时快照");
const latestNotifications = computed(() => props.notifications.slice(0,3));
function messageTime(value:string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : new Intl.DateTimeFormat("zh-CN",{hour:"2-digit",minute:"2-digit"}).format(date);
}

const activeTest = computed(() => props.testRuns.find(run => run.status === "queued" || run.status === "running"));
const activeTapd = computed(() => props.tapdJobs.find(job => job.status === "queued" || job.status === "running"));
const activeProject = computed(() => props.runningProjects[0]);
const activeTestProgress = computed(() => activeTest.value ? estimateTestRunProgress(activeTest.value,props.testRuns,now.value.getTime()) : null);
const runningCount = computed(() => Number(Boolean(activeProject.value)) + Number(Boolean(activeTest.value)) + Number(Boolean(activeTapd.value)));

function minutesFromTime(value:string) {
  const [hour,minute] = value.split(":").map(Number);
  return Number.isFinite(hour) && Number.isFinite(minute) ? hour * 60 + minute : 0;
}
function dateMinutes(value:string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 0 : date.getHours() * 60 + date.getMinutes();
}
function workLane(value:string):TimelineLane {
  if (/测试|验证/i.test(value)) return "test";
  if (/报告|周报|日报|月报/i.test(value)) return "report";
  if (/git|提交|代码/i.test(value)) return "git";
  return "codex";
}
interface TimelineBar { id:string; lane:TimelineLane; left:number; width:number; label:string }
const timelineBars = computed<TimelineBar[]>(() => {
  const bars:TimelineBar[] = workSessions.value.map(session => {
    const start=minutesFromTime(session.startTime);const end=Math.max(start+5,minutesFromTime(session.endTime));
    return { id:`work:${session.id}`,lane:workLane(session.workType),left:start/1440*100,width:Math.max((end-start)/1440*100,.45),label:`${session.project} · ${session.workType}` };
  });
  for (const run of props.testRuns.filter(item => isWithinRange(item.startedAt,todayIso.value,todayIso.value))) {
    const start=dateMinutes(run.startedAt);const end=run.finishedAt ? dateMinutes(run.finishedAt) : Math.max(start+15,dateMinutes(now.value.toISOString()));
    bars.push({id:`test:${run.id}`,lane:"test",left:start/1440*100,width:Math.max((end-start)/1440*100,.6),label:run.menuName});
  }
  for (const report of reports.value.filter(item => isWithinRange(item.updatedAt,todayIso.value,todayIso.value))) {
    const start=dateMinutes(report.updatedAt);
    bars.push({id:`report:${report.id}`,lane:"report",left:start/1440*100,width:1,label:report.title});
  }
  return bars;
});
const currentTimePercent = computed(() => (now.value.getHours() * 60 + now.value.getMinutes()) / 1440 * 100);
const currentTimeText = computed(() => new Intl.DateTimeFormat("zh-CN",{hour:"2-digit",minute:"2-digit"}).format(now.value));
const liveUpdatedText = computed(() => new Intl.DateTimeFormat("zh-CN",{hour:"2-digit",minute:"2-digit",second:"2-digit"}).format(now.value));

function navigate(route:string) { emit("navigate",route); }
function notificationIcon(item:WorkbenchNotification) {
  if (item.kind === "tapd_item" || item.title.includes("失败")) return "warning";
  if (item.kind === "jenkins_publish") return "deploy";
  return "check";
}

watch([period,customStart,customEnd], () => void loadCockpitData());
onMounted(() => {
  void loadCockpitData();
  clockTimer=window.setInterval(() => { now.value=new Date(); },1_000);
  refreshTimer=window.setInterval(() => void loadCockpitData(),60_000);
  void nextTick(() => returnButton.value?.focus());
});
onBeforeUnmount(() => { window.clearInterval(clockTimer);window.clearInterval(refreshTimer); });
</script>

<template>
  <section class="cockpit-screen" role="dialog" aria-modal="true" aria-label="数据驾驶舱屏保模式" @keydown.esc.stop="emit('close')">
      <header class="cockpit-header">
        <button ref="returnButton" class="cockpit-return" @click="emit('close')">← 返回工作台</button>
        <div class="cockpit-title"><h1>数据驾驶舱</h1><span>屏保模式</span><small><i></i>实时更新 {{ liveUpdatedText }}</small></div>
        <div class="cockpit-quota-summary" :title="quota.available ? quotaFreshnessText : '暂无有效额度快照'"><small>Codex 剩余额度</small><span v-for="item in quotaWindows" :key="item.windowMinutes"><em>{{ quotaLabel(item) }}</em><b>{{ Math.round(item.remainingPercent) }}%</b></span><strong v-if="!quotaWindows.length">--</strong><i></i></div>
        <nav class="cockpit-periods" aria-label="统计时间范围">
          <button v-for="item in periodOptions" :key="item.key" :class="{active:period===item.key}" @click="period=item.key">{{ item.label }}</button>
        </nav>
        <div v-if="period==='custom'" class="cockpit-custom-range"><input v-model="customStart" type="date" aria-label="开始日期"><span>—</span><input v-model="customEnd" type="date" aria-label="结束日期"></div>
      </header>

      <div class="cockpit-kpis">
        <article v-for="item in kpis" :key="item.label"><i><CockpitIcon :name="item.icon" /></i><span><small>{{ item.label }}</small><b>{{ item.value }}<em>{{ item.unit }}</em></b></span></article>
      </div>

      <div class="cockpit-main">
        <div class="cockpit-analytics">
          <article class="cockpit-panel pulse-panel">
            <header><h2>工作脉搏</h2><div class="pulse-meta"><small>近 7 天实际活动</small><nav><span v-for="item in pulseSeries" :key="item.key" :class="`series-${item.index}`"><i></i>{{ item.label }}</span></nav></div></header>
            <div class="pulse-chart" @mouseleave="pulseHoverIndex=null">
              <svg :viewBox="`0 0 ${chart.width} ${chart.height}`" preserveAspectRatio="none" aria-label="近七天工作脉搏">
                <g class="cockpit-grid"><line v-for="index in 5" :key="index" :x1="chart.left" :x2="chart.width-chart.right" :y1="chart.top+(index-1)*(chart.height-chart.top-chart.bottom)/4" :y2="chart.top+(index-1)*(chart.height-chart.top-chart.bottom)/4" /></g>
                <path v-for="item in pulseSeries" :key="item.key" class="pulse-line" :class="`series-${item.index}`" :d="item.path" :stroke-dasharray="item.dash" />
                <g v-for="(point,index) in pulsePoints" :key="point.date" class="pulse-hit" @mouseenter="pulseHoverIndex=index">
                  <rect :x="chartX(index,pulsePoints.length)-30" :y="chart.top" width="60" :height="chart.height-chart.top-chart.bottom" />
                  <circle v-if="pulseHoverIndex===index" :cx="chartX(index,pulsePoints.length)" :cy="chartY(point.conversationCount,pulseSeries[0]?.maximum || 1)" r="4" />
                </g>
              </svg>
              <div class="pulse-axis"><span v-for="point in pulsePoints" :key="point.date">{{ point.date.slice(5) }}</span></div>
              <div v-if="pulseHoverIndex!==null" class="pulse-tooltip" :style="{left:`${chartX(pulseHoverIndex,pulsePoints.length)/chart.width*100}%`}"><b>{{ pulsePoints[pulseHoverIndex].date }}</b><span>工时 {{ cockpitHours(pulsePoints[pulseHoverIndex].workMinutes) }}</span><span>对话 {{ pulsePoints[pulseHoverIndex].conversationCount }}</span><span>提交 {{ pulsePoints[pulseHoverIndex].gitCommits }}</span><span>测试 {{ pulsePoints[pulseHoverIndex].testRuns }}</span></div>
            </div>
          </article>

          <div class="cockpit-lower-grid">
            <article class="cockpit-panel ranking-panel"><header><h2>项目投入排行</h2><small>按所选周期工时</small></header><div><p v-for="(item,index) in projectRanking" :key="item.name"><em>{{ index+1 }}</em><span><b>{{ item.name }}</b><i><u :style="{width:`${item.minutes/projectMaxMinutes*100}%`}"></u></i></span><strong>{{ cockpitHours(item.minutes) }}</strong></p><small v-if="!projectRanking.length" class="cockpit-empty">暂无项目投入记录</small></div></article>
            <article class="cockpit-panel heat-panel"><header><h2>近 90 天活跃热力</h2><small>对话、Git、测试、报告与工时</small></header><div class="heat-grid"><i v-for="item in heatCells" :key="item.date" :class="`level-${item.level}`" :title="`${item.date} · 活跃信号 ${Math.round(item.value)}`"></i></div><footer><span>低活跃</span><i v-for="level in 5" :key="level" :class="`level-${level-1}`"></i><span>高活跃</span></footer></article>
            <article class="cockpit-panel outcome-panel"><header><h2>投入与成果</h2><small>近 7 天</small></header><svg :viewBox="`0 0 ${outcomeDimensions.width} ${outcomeDimensions.height}`" preserveAspectRatio="none" aria-label="投入与成果趋势"><g class="cockpit-grid"><line v-for="index in 4" :key="index" :x1="outcomeDimensions.left" :x2="outcomeDimensions.width-outcomeDimensions.right" :y1="outcomeDimensions.top+(index-1)*(outcomeDimensions.height-outcomeDimensions.top-outcomeDimensions.bottom)/3" :y2="outcomeDimensions.top+(index-1)*(outcomeDimensions.height-outcomeDimensions.top-outcomeDimensions.bottom)/3" /></g><path class="outcome-work" :d="outcomeWorkPath"/><path class="outcome-result" :d="outcomeResultPath"/></svg><footer><span><i></i>工时</span><span><i></i>完成成果</span></footer></article>
          </div>

          <article class="cockpit-panel timeline-panel">
            <header><h2>24 小时工作轨迹</h2><span><i></i>真实本地活动区间</span></header>
            <div class="timeline-chart">
              <div class="timeline-labels"><span>Codex</span><span>Git</span><span>测试</span><span>报告</span></div>
              <div class="timeline-tracks"><div v-for="lane in ['codex','git','test','report']" :key="lane" class="timeline-track"><i v-for="item in timelineBars.filter(bar=>bar.lane===lane)" :key="item.id" :style="{left:`${item.left}%`,width:`${item.width}%`}" :title="item.label"></i></div><span class="current-time" :style="{left:`${currentTimePercent}%`}"><b>{{ currentTimeText }}</b><i></i></span></div>
              <div class="timeline-hours"><span v-for="hour in [0,4,8,12,16,20,24]" :key="hour">{{ String(hour).padStart(2,'0') }}:00</span></div>
            </div>
          </article>
        </div>

        <aside class="cockpit-live-rail">
          <article class="cockpit-panel messages-panel"><header><h2>消息通知 <b>{{ latestNotifications.length }}</b></h2><button @click="navigate('/inbox')">查看全部 →</button></header><div><button v-for="item in latestNotifications" :key="item.id" @click="navigate(item.route || '/inbox')"><i :class="{warning:notificationIcon(item)==='warning'}"><CockpitIcon :name="notificationIcon(item)" /></i><span><b>{{ item.title.replace(/^Codex 任务已完成：/,'') }}</b><small>{{ item.body }}</small></span><time>{{ messageTime(item.createdAt) }}</time></button><p v-if="!latestNotifications.length" class="cockpit-empty">暂无新消息</p></div></article>
          <article class="cockpit-panel running-panel"><header><h2>正在运行 <b>{{ runningCount }}</b></h2><small>每 3 秒刷新</small></header><div>
            <button v-if="activeProject" @click="navigate(`/projects?project=${encodeURIComponent(activeProject.projectPath)}`)"><i></i><span><b>项目 · {{ activeProject.projectName }}</b><small>{{ activeProject.status==='starting'?'正在启动':'运行中' }}</small><u><em></em></u></span></button>
            <button v-if="activeTest" @click="navigate(`/testing?run=${encodeURIComponent(activeTest.id)}`)"><i></i><span><b>测试 · {{ activeTest.menuName }}</b><small>{{ activeTestProgress ? `${activeTestProgress.percent}% · ${activeTestProgress.etaText}` : '执行中' }}</small><u class="determinate"><em :style="{width:`${activeTestProgress?.percent || 0}%`}"></em></u></span></button>
            <button v-if="activeTapd" @click="navigate('/tapd-automation')"><i></i><span><b>自动处理 · TAPD #{{ activeTapd.itemId }}</b><small>{{ activeTapd.status==='queued'?'等待处理':'分析中' }}</small><u><em></em></u></span></button>
            <p v-if="!runningCount" class="running-empty">当前没有运行中的任务</p>
          </div></article>
        </aside>
      </div>
  </section>
</template>

<style scoped>
.cockpit-screen{--c-bg:#03070c;--c-panel:#07101a;--c-panel-2:#0a1420;--c-line:#203449;--c-text:#edf5ff;--c-muted:#7890a7;--c-primary:#64adff;--c-primary-soft:rgba(100,173,255,.14);--c-warning:#d8a23c;position:fixed;inset:0;z-index:500;min-width:1180px;min-height:720px;padding:14px 18px 10px;overflow:auto;color:var(--c-text);background:radial-gradient(circle at 50% -20%,rgba(54,116,179,.17),transparent 40%),linear-gradient(180deg,#03070c,#050a10 70%,#03070c);font-family:"Segoe UI","Microsoft YaHei",sans-serif;display:grid;grid-template-rows:48px 92px minmax(360px,1fr) 156px 26px;gap:10px;isolation:isolate}.cockpit-screen:before{content:"";position:fixed;inset:0;pointer-events:none;opacity:.18;background-image:linear-gradient(rgba(100,173,255,.035) 1px,transparent 1px),linear-gradient(90deg,rgba(100,173,255,.035) 1px,transparent 1px);background-size:36px 36px;mask-image:linear-gradient(to bottom,black,transparent 80%)}button{font:inherit}.cockpit-header{display:flex;align-items:center;gap:14px;position:relative;z-index:2}.cockpit-return{height:38px;border:1px solid #4f85b8;border-radius:7px;padding:0 14px;background:linear-gradient(180deg,rgba(20,41,60,.88),rgba(8,17,27,.92));color:#bfe0ff;box-shadow:inset 0 0 16px rgba(100,173,255,.07),0 0 18px rgba(54,132,204,.06);cursor:pointer}.cockpit-return:hover,.cockpit-return:focus{outline:0;border-color:var(--c-primary);color:#fff;box-shadow:0 0 18px rgba(100,173,255,.18)}.cockpit-title{display:flex;align-items:center;gap:11px}.cockpit-title h1{margin:0;font-size:25px;letter-spacing:1px}.cockpit-title>span{padding:5px 9px;border:1px solid var(--c-line);border-radius:99px;color:#a7bdd1;font-size:10px}.cockpit-title small{display:flex;align-items:center;gap:7px;color:var(--c-muted)}.cockpit-title small i,.quota-panel header small i{width:7px;height:7px;border-radius:50%;background:var(--c-primary);box-shadow:0 0 0 4px rgba(100,173,255,.09);animation:cockpit-breathe 2s ease-in-out infinite}.cockpit-periods{margin-left:auto;height:36px;display:flex;border:1px solid var(--c-line);border-radius:7px;background:#07101a;overflow:hidden}.cockpit-periods button{min-width:72px;border:0;border-left:1px solid var(--c-line);background:transparent;color:var(--c-muted)}.cockpit-periods button:first-child{border-left:0}.cockpit-periods button.active{background:linear-gradient(180deg,rgba(100,173,255,.2),rgba(100,173,255,.07));color:#fff;box-shadow:inset 0 -2px var(--c-primary)}.cockpit-custom-range{position:absolute;right:0;top:43px;z-index:10;display:flex;align-items:center;gap:7px;padding:8px;border:1px solid var(--c-line);border-radius:7px;background:var(--c-panel)}.cockpit-custom-range input{height:30px;border:1px solid var(--c-line);border-radius:5px;background:var(--c-panel-2);color:var(--c-text);padding:0 8px;color-scheme:dark}.cockpit-kpis{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:10px;position:relative;z-index:1}.cockpit-kpis article{min-width:0;border:1px solid var(--c-line);border-radius:8px;background:linear-gradient(145deg,rgba(12,25,39,.97),rgba(5,11,18,.96));display:flex;align-items:center;gap:12px;padding:12px 14px;box-shadow:inset 0 1px rgba(255,255,255,.025)}.cockpit-kpis article>i{width:42px;height:42px;border:1px solid #315474;border-radius:50%;display:grid;place-items:center;color:var(--c-primary);font-size:20px;font-style:normal;box-shadow:inset 0 0 20px rgba(100,173,255,.08)}.cockpit-kpis span{min-width:0}.cockpit-kpis small{color:#aab9c7;white-space:nowrap}.cockpit-kpis b{display:block;margin-top:3px;font-size:28px;line-height:1;font-weight:600;letter-spacing:.2px;animation:cockpit-number-in .42s ease-out}.cockpit-kpis em{font-size:15px;font-style:normal;margin-left:2px}.cockpit-main{min-height:0;display:grid;grid-template-columns:minmax(0,1fr) 350px;gap:10px}.cockpit-analytics{min-height:0;display:grid;grid-template-rows:minmax(210px,1fr) 190px;gap:10px}.cockpit-panel{border:1px solid var(--c-line);border-radius:8px;background:linear-gradient(145deg,rgba(8,18,29,.98),rgba(4,10,16,.98));box-shadow:inset 0 1px rgba(255,255,255,.025),0 16px 32px rgba(0,0,0,.13);overflow:hidden}.cockpit-panel>header{min-height:42px;padding:9px 14px;display:flex;align-items:center;justify-content:space-between;gap:10px}.cockpit-panel h2{margin:0;font-size:15px;letter-spacing:.2px}.cockpit-panel header small{color:var(--c-muted);font-size:9px}.pulse-panel{display:grid;grid-template-rows:48px minmax(0,1fr)}.pulse-panel>header>div{display:flex;align-items:baseline;gap:9px}.pulse-panel nav{display:flex;gap:15px;color:var(--c-muted);font-size:9px}.pulse-panel nav span{display:flex;align-items:center;gap:5px}.pulse-panel nav i{width:19px;border-top:2px solid var(--c-primary);opacity:var(--series-opacity,.9)}.pulse-panel nav .series-1 i{border-top-style:dashed}.pulse-panel nav .series-2 i{border-top-style:dotted}.pulse-chart{min-height:0;position:relative;padding:0 12px 5px}.pulse-chart svg{width:100%;height:calc(100% - 21px);overflow:visible}.cockpit-grid line{stroke:#192a3a;stroke-width:1;vector-effect:non-scaling-stroke;stroke-dasharray:3 5}.pulse-line{fill:none;stroke:var(--c-primary);stroke-width:2.2;vector-effect:non-scaling-stroke;filter:drop-shadow(0 0 4px rgba(100,173,255,.28));animation:cockpit-line-flow 12s linear infinite}.pulse-line.series-1{opacity:.72}.pulse-line.series-2{opacity:.5}.pulse-line.series-3{opacity:.34}.pulse-hit rect{fill:transparent}.pulse-hit circle{fill:var(--c-bg);stroke:var(--c-primary);stroke-width:2}.pulse-axis{position:absolute;left:6%;right:3%;bottom:3px;display:flex;justify-content:space-between;color:var(--c-muted);font-size:9px}.pulse-tooltip{position:absolute;top:18px;transform:translateX(-50%);width:126px;padding:8px 10px;border:1px solid #376386;border-radius:6px;background:rgba(4,11,18,.94);display:grid;grid-template-columns:1fr 1fr;gap:4px 8px;pointer-events:none;box-shadow:0 12px 30px rgba(0,0,0,.4)}.pulse-tooltip b{grid-column:1/3}.pulse-tooltip span{font-size:9px;color:var(--c-muted)}.cockpit-lower-grid{display:grid;grid-template-columns:1.05fr 1.05fr .9fr;gap:10px;min-height:0}.ranking-panel,.heat-panel,.outcome-panel{padding:0 12px 10px}.ranking-panel>header,.heat-panel>header,.outcome-panel>header{padding-left:0;padding-right:0}.ranking-panel>div{display:grid;gap:7px}.ranking-panel p{margin:0;display:grid;grid-template-columns:20px minmax(0,1fr) 40px;gap:8px;align-items:center}.ranking-panel p>em{width:19px;height:19px;border:1px solid var(--c-line);border-radius:4px;display:grid;place-items:center;color:#bcd1e4;font-size:9px;font-style:normal}.ranking-panel p>span{min-width:0}.ranking-panel p b{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:10px}.ranking-panel p span>i{display:block;height:5px;margin-top:5px;border-radius:4px;background:#142331;overflow:hidden}.ranking-panel p u{display:block;height:100%;border-radius:inherit;background:linear-gradient(90deg,#316ca4,var(--c-primary));box-shadow:0 0 8px rgba(100,173,255,.15)}.ranking-panel strong{font-size:9px;color:#bdd1e3;text-align:right}.heat-grid{height:92px;display:grid;grid-template-columns:repeat(10,1fr);grid-template-rows:repeat(3,1fr);gap:5px}.heat-grid i,.heat-panel footer i{border:1px solid #182a3a;border-radius:2px;background:#0d1823}.heat-grid i.level-1,.heat-panel footer i.level-1{background:#142b43}.heat-grid i.level-2,.heat-panel footer i.level-2{background:#214c72}.heat-grid i.level-3,.heat-panel footer i.level-3{background:#397db7}.heat-grid i.level-4,.heat-panel footer i.level-4{background:#70b8ff;box-shadow:0 0 7px rgba(100,173,255,.22)}.heat-panel footer{margin-top:10px;display:flex;justify-content:flex-end;align-items:center;gap:4px;color:var(--c-muted);font-size:8px}.heat-panel footer i{width:11px;height:11px}.outcome-panel svg{width:100%;height:100px}.outcome-panel path{fill:none;stroke-width:2;vector-effect:non-scaling-stroke}.outcome-work{stroke:var(--c-primary)}.outcome-result{stroke:#aabed1;stroke-dasharray:5 4}.outcome-panel footer{display:flex;justify-content:center;gap:15px;color:var(--c-muted);font-size:8px}.outcome-panel footer span{display:flex;align-items:center;gap:5px}.outcome-panel footer i{width:14px;border-top:2px solid var(--c-primary)}.outcome-panel footer span+span i{border-color:#aabed1;border-top-style:dashed}.cockpit-live-rail{display:grid;grid-template-rows:150px minmax(135px,1fr) minmax(150px,1fr);gap:10px;min-height:0}.quota-panel>header small{display:flex;align-items:center;gap:7px}.quota-panel>div{display:grid;grid-template-columns:1fr 1fr;height:95px}.quota-panel section{display:grid;grid-template-columns:66px 1fr;align-items:center;padding:2px 12px}.quota-panel section+section{border-left:1px solid var(--c-line)}.quota-ring{--quota:0deg;width:59px;height:59px;border-radius:50%;background:conic-gradient(var(--c-primary) var(--quota),#14212d 0);display:grid;place-items:center;position:relative}.quota-ring:after{content:"";position:absolute;inset:6px;border-radius:50%;background:#07101a}.quota-ring b{position:relative;z-index:1;font-size:17px}.quota-panel section>small{color:var(--c-muted);font-size:9px}.messages-panel,.running-panel{display:grid;grid-template-rows:42px minmax(0,1fr)}.messages-panel header h2 b,.running-panel header h2 b{margin-left:5px;color:var(--c-primary)}.messages-panel header button{border:0;background:transparent;color:var(--c-primary);font-size:9px}.messages-panel>div,.running-panel>div{min-height:0;overflow:auto;padding:0 11px 8px}.messages-panel>div>button{width:100%;min-height:46px;border:0;border-top:1px solid var(--c-line);background:transparent;color:inherit;display:grid;grid-template-columns:23px minmax(0,1fr) auto;gap:8px;align-items:center;padding:5px 2px;text-align:left}.messages-panel>div>button:hover{background:rgba(100,173,255,.045)}.messages-panel button>i{width:21px;height:21px;border:1px solid #315d82;border-radius:50%;display:grid;place-items:center;color:var(--c-primary);font-style:normal;font-size:9px}.messages-panel button>i.warning{border-color:#715c2b;color:var(--c-warning)}.messages-panel button span{min-width:0}.messages-panel button b,.messages-panel button small{display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.messages-panel button b{font-size:10px}.messages-panel button small,.messages-panel time{color:var(--c-muted);font-size:8px}.running-panel>div{display:grid;gap:6px}.running-panel>div>button{width:100%;min-height:42px;border:1px solid #162a3b;border-radius:6px;background:#08131d;color:inherit;display:grid;grid-template-columns:8px minmax(0,1fr);gap:8px;align-items:center;padding:7px 9px;text-align:left}.running-panel>div>button>i{width:7px;height:7px;border-radius:50%;background:var(--c-primary);box-shadow:0 0 0 4px rgba(100,173,255,.08);animation:cockpit-breathe 2s ease-in-out infinite}.running-panel button.empty{opacity:.45}.running-panel button span{min-width:0}.running-panel button b,.running-panel button small{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.running-panel button b{font-size:9px}.running-panel button small{margin-top:2px;color:var(--c-muted);font-size:8px}.running-panel u{display:block;height:3px;margin-top:5px;border-radius:3px;background:#142331;overflow:hidden}.running-panel u em{display:block;width:42%;height:100%;border-radius:inherit;background:linear-gradient(90deg,transparent,var(--c-primary),transparent);animation:cockpit-progress 2.4s ease-in-out infinite}.running-panel u.determinate em{background:var(--c-primary);animation:none}.timeline-panel{padding:0 12px 8px}.timeline-panel>header{height:38px;padding:5px 5px}.timeline-panel header span{display:flex;align-items:center;gap:6px;color:var(--c-muted);font-size:8px}.timeline-panel header span i{width:13px;border-top:2px solid var(--c-primary)}.timeline-chart{height:106px;display:grid;grid-template-columns:76px minmax(0,1fr);grid-template-rows:80px 22px}.timeline-labels{display:grid;grid-template-rows:repeat(4,1fr);padding-top:2px}.timeline-labels span{display:flex;align-items:center;color:#bfd0df;font-size:9px}.timeline-tracks{position:relative;display:grid;grid-template-rows:repeat(4,1fr);background:repeating-linear-gradient(90deg,transparent 0,transparent calc(16.666% - 1px),rgba(67,98,125,.18) calc(16.666% - 1px),rgba(67,98,125,.18) 16.666%)}.timeline-track{position:relative;border-top:1px solid #142535}.timeline-track:last-child{border-bottom:1px solid #142535}.timeline-track>i{position:absolute;top:6px;height:7px;min-width:4px;border-radius:5px;background:linear-gradient(90deg,#315f8a,var(--c-primary));box-shadow:0 0 8px rgba(100,173,255,.2)}.current-time{position:absolute;top:-2px;bottom:0;width:1px;background:var(--c-primary);filter:drop-shadow(0 0 4px var(--c-primary));animation:cockpit-current-pulse 2s ease-in-out infinite}.current-time b{position:absolute;top:-22px;left:50%;transform:translateX(-50%);padding:3px 6px;border:1px solid #407ab0;border-radius:4px;background:#0b2135;color:#d8edff;font-size:8px}.current-time i{position:absolute;left:-3px;bottom:-3px;width:7px;height:7px;border-radius:50%;background:#fff;border:2px solid var(--c-primary)}.timeline-hours{grid-column:2;display:flex;justify-content:space-between;padding-top:4px;color:var(--c-muted);font-size:8px}.cockpit-exit-hint,.cockpit-status{margin:0;text-align:center;color:#69839a;font-size:9px;letter-spacing:.5px}.cockpit-exit-hint kbd{padding:2px 6px;border:1px solid var(--c-line);border-radius:4px;background:#08131d;color:#bad8f3}.cockpit-status.warning{color:var(--c-warning)}.cockpit-empty{color:var(--c-muted);font-size:9px;text-align:center}.quota-panel>.cockpit-empty{padding:20px;margin:0}
.cockpit-screen{grid-template-rows:48px 92px minmax(400px,1fr) 104px 26px}.cockpit-header>.cockpit-periods{margin-left:0}.cockpit-quota-summary{height:36px;margin-left:auto;padding:0 11px;border:1px solid var(--c-line);border-radius:7px;background:rgba(7,16,26,.74);display:flex;align-items:center;gap:10px;white-space:nowrap}.cockpit-quota-summary>small{color:var(--c-muted);font-size:9px}.cockpit-quota-summary>span{display:flex;align-items:baseline;gap:4px}.cockpit-quota-summary em{color:var(--c-muted);font-size:8px;font-style:normal}.cockpit-quota-summary b,.cockpit-quota-summary strong{color:#dceeff;font-size:17px;font-weight:600}.cockpit-quota-summary>i{width:6px;height:6px;border-radius:50%;background:var(--c-primary);box-shadow:0 0 7px rgba(100,173,255,.55);animation:cockpit-breathe 2s ease-in-out infinite}.cockpit-main{grid-template-columns:minmax(0,1fr) minmax(390px,28vw)}.cockpit-live-rail{grid-template-rows:minmax(180px,.9fr) minmax(210px,1.1fr)}.cockpit-live-rail .cockpit-panel{border-color:#29435a;box-shadow:inset 0 1px rgba(255,255,255,.035),0 0 0 1px rgba(100,173,255,.025),0 16px 34px rgba(0,0,0,.18)}.cockpit-live-rail h2{font-size:16px}.messages-panel,.running-panel{grid-template-rows:46px minmax(0,1fr)}.messages-panel>div,.running-panel>div{padding:0 13px 10px}.messages-panel>div>button{min-height:56px;grid-template-columns:27px minmax(0,1fr) auto;gap:9px;padding:7px 3px}.messages-panel button>i{width:24px;height:24px;font-size:10px}.messages-panel button b{font-size:11px}.messages-panel button small,.messages-panel time{font-size:9px}.running-panel>div{gap:8px}.running-panel>div>button{min-height:58px;grid-template-columns:9px minmax(0,1fr);gap:9px;padding:9px 10px}.running-panel button b{font-size:11px}.running-panel button small{font-size:9px}.running-panel u{height:4px;margin-top:7px}.running-empty{align-self:center;margin:0;color:var(--c-muted);font-size:10px;text-align:center}.heat-grid{grid-template-columns:repeat(15,1fr);grid-template-rows:repeat(6,1fr);gap:3px}.timeline-panel{padding:0 12px 4px}.timeline-panel>header{height:30px;min-height:30px;padding:3px 5px}.timeline-chart{height:66px;grid-template-rows:46px 18px}.timeline-labels span{font-size:8px}.timeline-track>i{top:4px;height:5px}.timeline-hours{padding-top:2px}
/* 紧凑窗口只降低最小高度；常用分辨率仍按上面的比例扩展实时信息区。 */
.cockpit-screen{grid-template-rows:48px 92px minmax(380px,1fr) 104px 26px}.cockpit-live-rail{grid-template-rows:minmax(160px,.9fr) minmax(180px,1.1fr)}

/* 参考设备仪表盘的视觉收口：单一冷色光谱、统一描边图标和稳定的 12px 节奏。 */
.cockpit-screen {
  --c-bg: #02070c;
  --c-panel: #06111a;
  --c-panel-2: #091720;
  --c-line: rgba(100, 157, 198, 0.24);
  --c-line-strong: rgba(105, 184, 238, 0.44);
  --c-text: #eaf5ff;
  --c-muted: #8299aa;
  --c-primary: #70bfff;
  --c-live: #54ddbd;
  --c-warning: #e2ae49;
  gap: 12px;
  padding: 16px 20px 10px;
  grid-template-rows: 48px 92px minmax(0, 1fr);
  background:
    radial-gradient(ellipse at 78% -18%, rgba(48, 122, 178, 0.15), transparent 42%),
    radial-gradient(ellipse at 8% 80%, rgba(30, 89, 128, 0.08), transparent 38%),
    linear-gradient(155deg, #02070c 0%, #061019 52%, #02070c 100%);
}
.cockpit-screen::before {
  opacity: 0.22;
  background-size: 48px 48px;
  background-image:
    linear-gradient(rgba(112, 191, 255, 0.026) 1px, transparent 1px),
    linear-gradient(90deg, rgba(112, 191, 255, 0.026) 1px, transparent 1px);
}
.cockpit-header { gap: 16px; }
.cockpit-return,
.cockpit-periods,
.cockpit-quota-summary {
  border-color: var(--c-line);
  border-radius: 8px;
  background: linear-gradient(180deg, rgba(10, 25, 36, 0.92), rgba(4, 13, 21, 0.94));
}
.cockpit-return {
  color: #bcd9ef;
  box-shadow: inset 0 1px rgba(255, 255, 255, 0.035);
}
.cockpit-title h1 {
  font-size: 24px;
  font-weight: 650;
  letter-spacing: 1.6px;
  text-shadow: 0 0 22px rgba(112, 191, 255, 0.16);
}
.cockpit-title > span {
  border-color: var(--c-line);
  color: #94adbe;
  background: rgba(8, 22, 32, 0.7);
  letter-spacing: 0.6px;
}
.cockpit-title small i,
.cockpit-quota-summary > i {
  background: var(--c-live);
  box-shadow: 0 0 10px rgba(84, 221, 189, 0.7);
}
.cockpit-quota-summary {
  height: 38px;
  padding: 0 13px;
  gap: 12px;
  box-shadow: inset 0 1px rgba(255, 255, 255, 0.035), 0 0 20px rgba(41, 119, 173, 0.05);
}
.cockpit-quota-summary > small { font-size: 10px; letter-spacing: 0.3px; }
.cockpit-quota-summary em { font-size: 9px; }
.cockpit-quota-summary b,
.cockpit-quota-summary strong {
  font-size: 18px;
  font-variant-numeric: tabular-nums;
}
.cockpit-periods button {
  color: #8099ab;
  font-size: 11px;
  cursor: pointer;
}
.cockpit-periods button.active {
  color: #eaf6ff;
  background: linear-gradient(180deg, rgba(79, 153, 207, 0.24), rgba(38, 91, 130, 0.08));
  box-shadow: inset 0 -2px #70bfff, inset 0 1px rgba(255, 255, 255, 0.04);
}
.cockpit-kpis { gap: 12px; }
.cockpit-kpis article,
.cockpit-panel {
  border-color: var(--c-line);
  border-radius: 8px;
  background:
    linear-gradient(135deg, rgba(13, 29, 41, 0.96), rgba(4, 12, 19, 0.98) 68%),
    var(--c-panel);
  box-shadow: inset 0 1px rgba(255, 255, 255, 0.032), 0 14px 34px rgba(0, 0, 0, 0.15);
}
.cockpit-kpis article {
  position: relative;
  gap: 11px;
  padding: 13px 14px;
  overflow: hidden;
}
.cockpit-kpis article::after {
  content: "";
  position: absolute;
  right: -18px;
  bottom: -25px;
  width: 74px;
  height: 54px;
  border: 1px solid rgba(112, 191, 255, 0.07);
  transform: skewX(-28deg);
}
.cockpit-kpis article > i {
  width: 38px;
  height: 38px;
  flex: 0 0 38px;
  border-color: rgba(112, 191, 255, 0.34);
  border-radius: 9px;
  color: #86caff;
  font-size: 19px;
  background: linear-gradient(145deg, rgba(75, 157, 218, 0.13), rgba(20, 53, 76, 0.04));
  box-shadow: inset 0 0 18px rgba(112, 191, 255, 0.055), 0 0 16px rgba(58, 137, 194, 0.035);
}
.cockpit-kpis small { color: #91a8b9; font-size: 11px; letter-spacing: 0.2px; }
.cockpit-kpis b {
  margin-top: 6px;
  color: #eef8ff;
  font-size: 27px;
  font-weight: 620;
  font-variant-numeric: tabular-nums;
}
.cockpit-kpis em { color: #bad9ed; font-size: 13px; }
.cockpit-main,
.cockpit-lower-grid { gap: 12px; }
.cockpit-analytics {
  grid-template-rows: minmax(190px, 1fr) clamp(160px, 23vh, 235px) clamp(130px, 14vh, 150px);
  gap: 12px;
}
.cockpit-panel { position: relative; }
.cockpit-panel > header { padding: 10px 14px; }
.cockpit-panel h2 {
  color: #e2f1fb;
  font-size: 15px;
  font-weight: 620;
  letter-spacing: 0.45px;
}
.cockpit-panel header small { color: #7f98aa; font-size: 10px; }
.pulse-panel > header > div::before,
.ranking-panel > header::before,
.heat-panel > header::before,
.outcome-panel > header::before {
  content: "";
  width: 2px;
  height: 13px;
  margin-right: 7px;
  border-radius: 2px;
  background: #70bfff;
  box-shadow: 0 0 9px rgba(112, 191, 255, 0.32);
}
.pulse-panel > header > div,
.ranking-panel > header,
.heat-panel > header,
.outcome-panel > header { align-items: center; }
.cockpit-grid line { stroke: rgba(86, 132, 163, 0.18); }
.pulse-line { filter: drop-shadow(0 0 5px rgba(112, 191, 255, 0.25)); }
.ranking-panel p { grid-template-columns: 22px minmax(0, 1fr) 45px; gap: 9px; }
.ranking-panel p > em {
  width: 21px;
  height: 21px;
  border-color: var(--c-line);
  background: rgba(12, 29, 41, 0.65);
  color: #a9c2d5;
  font-size: 10px;
}
.ranking-panel p b { color: #cfe1ed; font-size: 11px; }
.ranking-panel p span > i,
.running-panel u { background: #0f2432; }
.ranking-panel p u,
.running-panel u em,
.timeline-track > i {
  background: linear-gradient(90deg, #387fb4, #74c4ff);
  box-shadow: 0 0 9px rgba(112, 191, 255, 0.2);
}
.ranking-panel strong { color: #a9c4d6; font-size: 10px; }
.heat-grid i,
.heat-panel footer i { border-color: rgba(78, 124, 155, 0.2); }
.heat-grid i.level-1,
.heat-panel footer i.level-1 { background: #112a3a; }
.heat-grid i.level-2,
.heat-panel footer i.level-2 { background: #1c4863; }
.heat-grid i.level-3,
.heat-panel footer i.level-3 { background: #31799f; }
.heat-grid i.level-4,
.heat-panel footer i.level-4 {
  background: #68c1d3;
  box-shadow: 0 0 7px rgba(104, 193, 211, 0.28);
}
.cockpit-live-rail {
  grid-template-rows: minmax(240px, 1fr) clamp(210px, 32vh, 316px);
  gap: 12px;
}
.cockpit-live-rail .cockpit-panel {
  border-color: var(--c-line-strong);
  background:
    linear-gradient(145deg, rgba(9, 24, 35, 0.985), rgba(3, 11, 17, 0.99)),
    var(--c-panel);
  box-shadow: inset 0 1px rgba(255, 255, 255, 0.045), 0 0 28px rgba(54, 137, 194, 0.065), 0 18px 36px rgba(0, 0, 0, 0.2);
}
.cockpit-live-rail .cockpit-panel::after {
  content: "";
  position: absolute;
  top: 0;
  left: 14px;
  width: 58px;
  height: 1px;
  background: linear-gradient(90deg, #8ed0ff, transparent);
  box-shadow: 0 0 8px rgba(112, 191, 255, 0.32);
}
.cockpit-live-rail h2 { font-size: 16px; }
.messages-panel header h2 b,
.running-panel header h2 b {
  display: inline-grid;
  min-width: 20px;
  height: 20px;
  margin-left: 7px;
  place-items: center;
  border: 1px solid rgba(112, 191, 255, 0.28);
  border-radius: 5px;
  color: #9ed3fb;
  background: rgba(76, 152, 207, 0.08);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.messages-panel header button {
  color: #83bfe9;
  font-size: 10px;
  cursor: pointer;
}
.messages-panel > div,
.running-panel > div { padding: 0 12px 12px; }
.messages-panel > div > button {
  min-height: 58px;
  padding: 7px 8px;
  border: 1px solid transparent;
  border-top-color: rgba(89, 137, 171, 0.18);
  border-radius: 6px;
  grid-template-columns: 29px minmax(0, 1fr) auto;
  gap: 10px;
  cursor: pointer;
}
.messages-panel > div > button:hover {
  border-color: rgba(112, 191, 255, 0.22);
  background: rgba(61, 131, 180, 0.075);
}
.messages-panel button > i {
  width: 27px;
  height: 27px;
  border-color: rgba(112, 191, 255, 0.44);
  color: #89ccff;
  font-size: 15px;
  background: rgba(48, 116, 165, 0.07);
}
.messages-panel button > i.warning {
  border-color: rgba(226, 174, 73, 0.52);
  color: var(--c-warning);
  background: rgba(226, 174, 73, 0.06);
}
.messages-panel button b { color: #d8e8f3; font-size: 12px; font-weight: 590; }
.messages-panel button small,
.messages-panel time { color: #7790a3; font-size: 9px; }
.messages-panel time { font-variant-numeric: tabular-nums; }
.running-panel > div { gap: 9px; }
.running-panel > div > button {
  min-height: 62px;
  padding: 9px 11px;
  border-color: rgba(92, 146, 181, 0.22);
  border-radius: 7px;
  background: linear-gradient(90deg, rgba(12, 31, 44, 0.9), rgba(6, 18, 27, 0.9));
  grid-template-columns: 10px minmax(0, 1fr);
  gap: 10px;
  cursor: pointer;
}
.running-panel > div > button:hover { border-color: rgba(112, 191, 255, 0.38); }
.running-panel > div > button > i {
  width: 7px;
  height: 7px;
  background: var(--c-live);
  box-shadow: 0 0 10px rgba(84, 221, 189, 0.7), 0 0 0 4px rgba(84, 221, 189, 0.07);
}
.running-panel button b { color: #d7eaf6; font-size: 12px; font-weight: 590; }
.running-panel button small { color: #7f99aa; font-size: 9px; }
.running-panel u { height: 4px; margin-top: 8px; }
.running-empty {
  align-self: center;
  color: #657d8e;
  letter-spacing: 0.4px;
}
.timeline-panel {
  min-height: 0;
  padding-bottom: 8px;
  opacity: 0.9;
}
.timeline-panel h2 { font-size: 13px; }
.timeline-panel > header { height: 36px; min-height: 36px; padding: 5px 7px; }
.timeline-chart {
  height: calc(100% - 36px);
  min-height: 78px;
  grid-template-rows: minmax(54px, 1fr) 20px;
}
.timeline-track > i { top: 7px; height: 6px; }
.timeline-tracks { background-size: auto; }

/* 所有卡片标题使用同一结构：标题靠左，说明和操作靠右。 */
.pulse-panel > header > div::before,
.ranking-panel > header::before,
.heat-panel > header::before,
.outcome-panel > header::before { content: none; }
.cockpit-panel > header { justify-content: flex-start; }
.cockpit-panel > header > h2 {
  display: flex;
  align-items: center;
  gap: 9px;
  margin: 0;
  color: #e2f1fb;
  font-size: 15px;
  font-weight: 620;
  line-height: 20px;
  letter-spacing: 0.45px;
  text-align: left;
  white-space: nowrap;
  flex: 0 0 auto;
}
.cockpit-panel > header > h2::before {
  content: "";
  width: 2px;
  height: 14px;
  flex: 0 0 2px;
  border-radius: 2px;
  background: #70bfff;
  box-shadow: 0 0 9px rgba(112, 191, 255, 0.32);
}
.cockpit-panel > header > :not(h2) {
  min-width: 0;
  margin-left: auto;
  text-align: right;
}
.pulse-panel > header > .pulse-meta {
  display: flex;
  align-items: center;
  gap: 16px;
}
@keyframes cockpit-breathe{0%,100%{opacity:.55;transform:scale(.9)}50%{opacity:1;transform:scale(1.08)}}@keyframes cockpit-number-in{from{opacity:0;transform:translateY(5px)}to{opacity:1;transform:none}}@keyframes cockpit-line-flow{to{stroke-dashoffset:-120}}@keyframes cockpit-progress{0%{transform:translateX(-100%)}100%{transform:translateX(260%)}}@keyframes cockpit-current-pulse{0%,100%{opacity:.58}50%{opacity:1}}
@media(max-width:1280px){.cockpit-screen{padding-left:10px;padding-right:10px}.cockpit-kpis article{padding:10px}.cockpit-kpis article>i{display:grid;width:32px;height:32px;flex-basis:32px;font-size:16px}.cockpit-kpis b{font-size:23px}.cockpit-main{grid-template-columns:minmax(0,1fr) 370px}.cockpit-title small{display:none}.cockpit-periods button{min-width:62px}}
@media(prefers-reduced-motion:reduce){.cockpit-screen *{animation:none!important;transition:none!important}}
</style>
