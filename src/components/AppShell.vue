<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch, watchEffect } from "vue";
import { RouterLink, RouterView, useRouter } from "vue-router";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import { isRegistered as isShortcutRegistered, register as registerShortcut, unregister as unregisterShortcut } from "@tauri-apps/plugin-global-shortcut";
import { useWorkbenchStore } from "../stores/workbench";
import { databaseHealth, getCodexCliStatus, getCodexQuota, getEmailNotificationStatus, getSystemPerformanceSnapshot, getTapdStatus, getVipStatus, getWorkSummary, isTauriRuntime, listJenkinsPublishRecords, listNotifications, listRepositoryAssets, listRunningRepositoryProjects, listTapdCodexJobs, listTestCaseGenerations, listTestRuns, listWorkSessions, markAllNotificationsRead, markNotificationRead, openRepositoryRuntimeUrl, optimizeSystemMemory, retryFailedEmails, setCodexEmailEnabled, stopRepositoryProject, syncCodexNotifications, syncTapdItems, type CodexQuotaSnapshot, type EmailNotificationStatus, type JenkinsPublishRecord, type RepositoryAsset, type RunningProjectProcess, type SystemPerformanceSnapshot, type TapdCodexJob, type TestCaseGenerationJob, type TestRun, type VipStatus, type WorkbenchNotification, type WorkSession, type WorkSummary } from "../services/backend";
import { getAlmanac } from "../utils/almanac";
import appLogo from "../assets/app-logo.png";
import appLogoWarm from "../assets/app-logo-warm.png";
import { APP_BRAND } from "../utils/brand";
import BrandWordmark from "./BrandWordmark.vue";
import HeaderIcon from "./HeaderIcon.vue";
import NavIcon from "./NavIcon.vue";
import SettingsLink from "./SettingsLink.vue";
import WindowControlIcon from "./WindowControlIcon.vue";
import ThemeSwitch from "./ThemeSwitch.vue";
import TaskEditor from "./TaskEditor.vue";
import WorkspaceSearch from "./WorkspaceSearch.vue";
import NotificationDrawer from "./NotificationDrawer.vue";
import QuickCapture from "./QuickCapture.vue";
import TranslationDialog from "./TranslationDialog.vue";
import CockpitScreensaver from "./CockpitScreensaver.vue";
import SystemPerformanceCard from "./SystemPerformanceCard.vue";
import SystemPerformanceDialog from "./SystemPerformanceDialog.vue";
import RestRailCard from "./rest/RestRailCard.vue";
import StartupSplash from "./StartupSplash.vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import { loadHiddenNavigationPaths, loadNavigationOrder, navigationOrderChangedEvent, orderedNavigationItems } from "../utils/navigation";
import { estimateTestRunProgress } from "../utils/testRunProgress";
import { cockpitIdleSettingsChangedEvent, cockpitIdleState, loadCockpitIdleMinutes } from "../utils/cockpit";
import { buildJenkinsActiveOperations } from "../utils/jenkinsActiveOperations";
import { confirmAction } from "../utils/confirm";
import { loadWorkbenchNotificationsEnabled, workbenchNotificationPreferenceChangedEvent } from "../utils/notificationPreference";
import { appendPerformanceHistory, emptySystemPerformanceSnapshot, formatPerformanceBytes, type SystemPerformanceHistoryPoint } from "../utils/systemPerformance";
import { backgroundRequest } from "../services/backendRequest";
import { usePageLoading } from "../composables/usePageLoading";
import { useBackgroundPolling } from "../composables/useBackgroundPolling";
import { latestTestsByProjectAndMode, notificationsChanged } from "../utils/workStatus";
import { listBackgroundTaskStatus, type BackgroundTaskStatus } from "../services/backend";
import CodexQuotaPopover from "./CodexQuotaPopover.vue";

const store = useWorkbenchStore();
const router = useRouter();
const { pageLoading, pageLoadingSlow, pageLoadingTitle, revealSlowPage } = usePageLoading(router);
const polling = useBackgroundPolling();
const editorOpen = ref(false);
const searchOpen = ref(false);
const quickCaptureOpen = ref(false);
const translationOpen = ref(false);
const performanceOpen = ref(false);
const cockpitOpen = ref(false);
const cockpitIdleWarningSeconds = ref(0);
const refreshing = ref(false);
const quotaOpen = ref(false);
const quotaLoading = ref(false);
const quota = ref<CodexQuotaSnapshot>({ available:false, freshness:"", selectionReason:"" });
const healthOpen = ref(false);
const healthLoading = ref(false);
const healthItems = ref<Array<{ label:string; state:"ok"|"warning"|"idle"|"error"; detail:string }>>([]);
const healthUpdatedAt = ref("");
const notificationOpen = ref(false);
const notificationLoading = ref(false);
const notifications = ref<WorkbenchNotification[]>([]);
const notificationToast = ref<WorkbenchNotification | null>(null);
const selectedNotification = ref<WorkbenchNotification | null>(null);
const workbenchNotificationsEnabled = ref(loadWorkbenchNotificationsEnabled());
const emailStatus = ref<EmailNotificationStatus>({ configured:false, enabled:false, state:"unconfigured", maskedEmail:"", lastError:"", retryingCount:0, failedCount:0 });
const emailLoading = ref(false);
const vipStatus = ref<VipStatus>({ active:false });
const windowMaximized = ref(false);
const activeTestRuns = ref<TestRun[]>([]);
const activeTapdJobs = ref<TapdCodexJob[]>([]);
const allTestRuns = ref<TestRun[]>([]);
const allTestCaseGenerations = ref<TestCaseGenerationJob[]>([]);
const activeTestCaseGenerations = ref<TestCaseGenerationJob[]>([]);
const allTapdJobs = ref<TapdCodexJob[]>([]);
const activeJenkinsPublishes = ref<JenkinsPublishRecord[]>([]);
const runningProjects = ref<RunningProjectProcess[]>([]);
const repositoryAssets = ref<RepositoryAsset[]>([]);
const todayWorkSessions = ref<WorkSession[]>([]);
const todayWorkSummary = ref<WorkSummary>({ startDate:"", endDate:"", totalMinutes:0, estimatedMinutes:0, manualMinutes:0, hasManualCorrections:false, byProject:[], byType:[], daily:[] });
const railNow = ref(Date.now());
const almanacExpanded = ref(false);
const systemPerformance = ref<SystemPerformanceSnapshot>(emptySystemPerformanceSnapshot());
const performanceHistory = ref<SystemPerformanceHistoryPoint[]>([]);
const memoryOptimizationState = ref<"idle" | "loading" | "done" | "error">("idle");
const memoryOptimizationMessage = ref("");
const recentActivitiesOpen = ref(window.localStorage.getItem("workbench-right-rail-recent-open") === "true");
const projectActionPath = ref("");
const projectActionMessage = ref("");
const startupLoading = ref(true);
const startupSlow = ref(false);
const allIssuesOpen = ref(false);
const refreshError = ref("");
const backgroundTasks = ref<BackgroundTaskStatus[]>([]);
let topbarDragStart: { x:number; y:number } | null = null;
const navigationOrder = ref(loadNavigationOrder());
const hiddenNavigationPaths = ref(loadHiddenNavigationPaths());
const visibleNavItems = computed(() => {
  const hidden = new Set(hiddenNavigationPaths.value);
  return orderedNavigationItems(navigationOrder.value).filter(item => !hidden.has(item.path) && (!item.vip || vipStatus.value.active));
});
// 复用侧栏时钟，避免应用跨天运行后日期和黄历仍停留在启动当天。
const currentDate = computed(() => new Date(railNow.value));
const dateText = computed(() => new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "long", day: "numeric", weekday: "short" }).format(currentDate.value));
const todayIso = computed(() => currentDate.value.toLocaleDateString("sv-SE"));
const todayDay = computed(() => currentDate.value.getDate());
const todayAlmanac = computed(() => getAlmanac(todayIso.value));
const compactLunarDate = computed(() => todayAlmanac.value.lunarDate.replace(/^农历.*年/, "农历"));
watch(todayIso, () => { void loadSidebarWorktime(); });
const latestWorkSession = computed(() => [...todayWorkSessions.value].sort((left,right) => right.endTime.localeCompare(left.endTime))[0]);
const primaryQuota = computed(() => quota.value.primary || quota.value.secondary);
const remainingText = computed(() => primaryQuota.value ? `${Math.round(primaryQuota.value.remainingPercent)}%` : "--");
const quotaFreshnessText = computed(() => ({ fresh:"刚刚更新", recent:"近期快照", stale:"快照较旧" } as Record<string,string>)[quota.value.freshness] || "暂无有效快照");
const healthIssueCount = computed(() => healthItems.value.filter(item => item.state === "warning" || item.state === "error").length);
const healthSummary = computed(() => healthLoading.value ? "检查中" : healthIssueCount.value ? `${healthIssueCount.value} 项需关注` : "本地运行");
const unreadCount = computed(() => notifications.value.filter(item => !item.isRead).length);
const pendingReviewCount = computed(() => notifications.value.filter(item => !["tapd_item","jenkins_publish"].includes(item.kind) && item.reviewStatus === "pending").length);
const activeOperations = computed(() => [
  ...activeTestCaseGenerations.value.map(job => ({
    id:`test-case:${job.id}`,
    kind:"test" as const,
    title:`生成 ${job.menuName} 专属用例`,
    detail:job.progressMessage,
    status:job.status === "queued" ? "等待生成" : "后台生成中",
    href:{ path:"/testing", query:{ project:job.projectPath } },
    progressPercent:job.progressPercent,
    etaText:"可继续操作",
  })),
  ...activeTestRuns.value.map(run => {
    const progress=estimateTestRunProgress(run,allTestRuns.value,railNow.value);
    return {
      id:`test:${run.id}`,
      kind:"test" as const,
      title:run.menuName,
      detail:`${run.totalCount || run.selectedScenarios.length} 个场景 · ${run.project || "测试项目"}`,
      status:run.status === "queued" ? "等待执行" : "正在测试",
      href:"/testing",
      progressPercent:progress.percent,
      etaText:progress.etaText,
    };
  }),
  ...activeTapdJobs.value.map(job => ({
    id:`tapd:${job.id}`,
    kind:"tapd" as const,
    title:`TAPD 缺陷 ${job.itemKey}`,
    detail:`${job.triggerSource === "auto" ? "自动处理" : "手动处理"} · ${job.workspaceId}`,
    status:job.status === "queued" ? "等待处理" : "正在处理",
    href:"/tapd",
    progressPercent:undefined,
    etaText:"",
  })),
  ...buildJenkinsActiveOperations(activeJenkinsPublishes.value,railNow.value),
]);
const railIssues = computed(() => {
  const issues:Array<{ id:string; title:string; detail:string; tone:"danger"|"warning"; to:string | { path:string; query:Record<string,string> } }> = [];
  for (const project of repositoryAssets.value) {
    const live = runningProjects.value.find(item => item.projectPath === project.path);
    const runtimeStatus = live?.status ?? project.runtimeStatus;
    const runtimeError = live?.errorMessage ?? project.runtimeError;
    if (runtimeStatus === "failed" || runtimeError) issues.push({ id:`runtime:${project.path}`, title:`${project.name} ${live ? '编译异常' : '启动异常'}`, detail:runtimeError || "项目最近一次运行失败", tone:"danger", to:{ path:"/projects", query:{ project:project.path } } });
    else if (project.healthLevel === "失败") issues.push({ id:`health:${project.path}`, title:`${project.name} 健康检查失败`, detail:project.healthSummary || "项目目录或 Git 状态无法读取", tone:"danger", to:{ path:"/projects", query:{ project:project.path } } });
    else if (project.behindCount > 0) issues.push({ id:`behind:${project.path}`, title:`${project.name} 落后远程 ${project.behindCount} 个提交`, detail:`当前分支 ${project.defaultBranch || "未识别"}`, tone:"warning", to:{ path:"/projects", query:{ project:project.path, tab:"git" } } });
  }
  for (const run of latestTestsByProjectAndMode(allTestRuns.value)) if (["failed","blocked","error"].includes(run.status)) issues.push({ id:`test:${run.id}`, title:`${run.project} · ${run.menuName} 测试未通过`, detail:run.errorMessage || `${run.failedCount} 个场景失败`, tone:"danger", to:{ path:"/testing", query:{ project:run.projectPath, run:run.id } } });
  for (const job of allTestCaseGenerations.value.filter(item => item.status === "failed").slice(0,3)) issues.push({ id:`test-case:${job.id}`, title:`${job.menuName} 专属用例生成失败`, detail:job.errorMessage || "请检查 Codex CLI 状态", tone:"danger", to:{ path:"/testing", query:{ project:job.projectPath } } });
  for (const job of [...allTapdJobs.value].filter(item => item.status === "failed").sort((left,right) => right.updatedAt.localeCompare(left.updatedAt)).slice(0,3)) issues.push({ id:`tapd:${job.id}`, title:`TAPD 缺陷 ${job.itemKey} 处理失败`, detail:job.errorMessage || "可进入自动处理查看日志", tone:"danger", to:"/tapd-automation" });
  if (emailStatus.value.state === "error") issues.push({ id:"email", title:"邮件通知异常", detail:emailStatus.value.lastError || "请检查邮箱配置", tone:"warning", to:"/settings" });
  for (const task of backgroundTasks.value) if (task.errorMessage) issues.push({ id:`background:${task.id}`, title:`${task.name}异常`, detail:task.errorMessage, tone:"warning", to:"/settings" });
  return issues.sort((left,right) => Number(right.tone === "danger")-Number(left.tone === "danger"));
});
const recentActivities = computed(() => {
  const activities:Array<{ id:string; title:string; detail:string; at:string; to:string | { path:string; query:Record<string,string> }; tone:"success"|"warning"|"primary" }> = [];
  for (const item of notifications.value.slice(0,8)) activities.push({ id:`notice:${item.id}`, title:item.title.replace(/^Codex 任务已完成：/,""), detail:item.kind === "tapd_item" ? "TAPD 消息" : item.kind === "jenkins_publish" ? "Jenkins 发布" : "Codex 完成", at:item.createdAt, to:item.route || "/inbox", tone:item.kind === "jenkins_publish" && item.title.includes("失败") ? "warning" : "success" });
  for (const run of [...allTestRuns.value].filter(item => !["queued","running"].includes(item.status)).sort((left,right) => (right.finishedAt || right.startedAt).localeCompare(left.finishedAt || left.startedAt)).slice(0,6)) activities.push({ id:`test:${run.id}`, title:run.menuName, detail:`测试${run.status === "passed" ? "通过" : "结束"}`, at:run.finishedAt || run.startedAt, to:`/testing?run=${run.id}`, tone:run.status === "passed" ? "success" : "warning" });
  for (const job of [...allTapdJobs.value].filter(item => ["completed","failed"].includes(item.status)).sort((left,right) => (right.completedAt || right.updatedAt).localeCompare(left.completedAt || left.updatedAt)).slice(0,6)) activities.push({ id:`tapd:${job.id}`, title:`TAPD 缺陷 ${job.itemKey}`, detail:job.status === "completed" ? "处理完成" : "处理失败", at:job.completedAt || job.updatedAt, to:"/tapd-automation", tone:job.status === "completed" ? "success" : "warning" });
  for (const project of runningProjects.value) activities.push({ id:`project:${project.projectPath}`, title:project.projectName, detail:"项目已启动", at:project.startedAt, to:{ path:"/projects", query:{ project:project.projectPath } }, tone:"primary" });
  if (latestWorkSession.value) activities.push({ id:`work:${latestWorkSession.value.id}`, title:`${latestWorkSession.value.project} · ${latestWorkSession.value.workType}`, detail:"工时区间已更新", at:latestWorkSession.value.updatedAt, to:"/work-records", tone:"primary" });
  return activities.filter(item => item.at).sort((left,right) => Date.parse(right.at)-Date.parse(left.at)).slice(0,5);
});
const emailButtonText = computed(() => emailStatus.value.state === "error" ? "异常" : emailStatus.value.enabled ? "开" : emailStatus.value.state === "unverified" ? "待验" : emailStatus.value.state === "unconfigured" ? "未配" : "关");
const emailTooltip = computed(() => [
  "Codex完成邮件通知",
  emailStatus.value.enabled ? "开关已开启，项目中的普通对话完成后发送邮件；自动任务不发送" : "开关已关闭",
  `收件人：${emailStatus.value.maskedEmail || "尚未配置"}`,
  emailStatus.value.lastError || "",
].filter(Boolean).join("\n"));

watchEffect(() => document.documentElement.dataset.theme = store.theme);
let notificationToastTimer = 0;
let railStatusTimer = 0;
let cockpitIdleTimer = 0;
let memoryOptimizationResetTimer = 0;
let performanceLoading = false;
let cockpitLastActivityAt = Date.now();
let cockpitIdleMinutes = loadCockpitIdleMinutes();
let railStatusTicks = 0;
let startupStartedAt = Date.now();
let startupSlowTimer = 0;
let startupFinishTimer = 0;
let startupSafetyTimer = 0;
let emailUnlisten: UnlistenFn | undefined;
let vipUnlisten: UnlistenFn | undefined;
let codexDataUnlisten: UnlistenFn | undefined;
let windowResizeUnlisten: UnlistenFn | undefined;
let quickShortcutRegistered = false;

async function initializeStartup() {
  startupStartedAt=Date.now();
  startupSlowTimer=window.setTimeout(() => { startupSlow.value=true; },3_500);
  const notificationsPromise=loadNotifications(true);
  void notificationsPromise.then(syncTapdNotifications);
  const initialData=Promise.allSettled([
    store.hydrate(),
    loadSystemHealth(),
    notificationsPromise,
    loadEmailStatus(),
    loadVipStatus(),
    loadActiveOperations(),
    loadProjectOptions(),
    loadSidebarWorktime(),
    loadSystemPerformance(),
    loadBackgroundDiagnostics(),
  ]);
  const safetyLimit=new Promise<void>(resolve => {
    startupSafetyTimer=window.setTimeout(resolve,12_000);
  });
  await Promise.race([initialData.then(() => undefined),safetyLimit]);
  const minimumVisibleTime=Math.max(0,1_100-(Date.now()-startupStartedAt));
  startupFinishTimer=window.setTimeout(() => {
    startupLoading.value=false;
    startupSlow.value=false;
    revealSlowPage();
    window.clearTimeout(startupSlowTimer);
    window.clearTimeout(startupSafetyTimer);
  },minimumVisibleTime);
}

watch(recentActivitiesOpen, value => window.localStorage.setItem("workbench-right-rail-recent-open", String(value)));
function formatRailMinutes(value:number) {
  const hours=Math.floor(value/60);
  const minutes=value%60;
  return `${hours ? `${hours}小时` : ""}${minutes || !hours ? `${minutes}分钟` : ""}`;
}
function formatRailActivityTime(value:string) {
  const timestamp=Date.parse(value);
  if (!Number.isFinite(timestamp)) return "时间未知";
  const minutes=Math.max(0,Math.round((railNow.value-timestamp)/60_000));
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes}分钟前`;
  if (minutes < 1_440) return `${Math.floor(minutes/60)}小时前`;
  return new Intl.DateTimeFormat("zh-CN",{month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(new Date(timestamp));
}
function openSearch() { searchOpen.value = true; }
function resetCockpitIdle() {
  cockpitLastActivityAt=Date.now();
  cockpitIdleWarningSeconds.value=0;
}
function openCockpit() {
  cockpitIdleWarningSeconds.value=0;
  healthOpen.value=false;
  quotaOpen.value=false;
  notificationOpen.value=false;
  notificationToast.value=null;
  cockpitOpen.value=true;
}
function evaluateCockpitIdle() {
  if (cockpitOpen.value || document.visibilityState === "hidden") return;
  const state=cockpitIdleState(cockpitLastActivityAt, Date.now(), cockpitIdleMinutes);
  cockpitIdleWarningSeconds.value=state.warningSeconds;
  if (state.open) openCockpit();
}
function applyCockpitIdleSettings() {
  cockpitIdleMinutes=loadCockpitIdleMinutes();
  resetCockpitIdle();
}
function handleCockpitActivity() {
  if (!cockpitOpen.value) resetCockpitIdle();
}
function closeCockpit() {
  cockpitOpen.value=false;
  resetCockpitIdle();
}
async function navigateFromCockpit(route:string) {
  closeCockpit();
  await router.push(route);
}
function handleCockpitVisibilityChange() {
  if (document.visibilityState === "visible") {
    railNow.value=Date.now();
    evaluateCockpitIdle();
    void loadSystemPerformance();
    void loadSidebarWorktime();
    void loadActiveOperations();
    void loadNotifications(false, false);
  }
  else cockpitIdleWarningSeconds.value=0;
}
async function loadSystemPerformance() {
  if (!isTauriRuntime() || document.visibilityState === "hidden" || performanceLoading) return;
  performanceLoading=true;
  try {
    const snapshot=await backgroundRequest(() => getSystemPerformanceSnapshot());
    systemPerformance.value=snapshot;
    performanceHistory.value=appendPerformanceHistory(performanceHistory.value,snapshot);
  } catch {
    systemPerformance.value={...systemPerformance.value,available:false,message:"暂时无法读取本机性能信息，将继续自动重试。"};
  } finally {
    performanceLoading=false;
  }
}
async function optimizeMemory() {
  if (memoryOptimizationState.value === "loading" || !isTauriRuntime()) return;
  window.clearTimeout(memoryOptimizationResetTimer);
  memoryOptimizationState.value="loading";
  memoryOptimizationMessage.value="";
  try {
    const result=await optimizeSystemMemory();
    memoryOptimizationState.value=result.success ? "done" : "error";
    memoryOptimizationMessage.value=result.success
      ? result.releasedBytes > 0 ? `已释放${formatPerformanceBytes(result.releasedBytes,0).replace(" ","")}` : "已完成"
      : result.message;
    await loadSystemPerformance();
  } catch {
    memoryOptimizationState.value="error";
    memoryOptimizationMessage.value="内存优化暂时失败，请稍后重试。";
  }
  memoryOptimizationResetTimer=window.setTimeout(() => {
    memoryOptimizationState.value="idle";
    memoryOptimizationMessage.value="";
  },4_000);
}
function handleKeydown(event: KeyboardEvent) {
  if (cockpitOpen.value) {
    if (event.key === "Escape") { event.preventDefault();closeCockpit(); }
    return;
  }
  resetCockpitIdle();
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); openSearch(); }
  if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.code === "Space") { event.preventDefault(); quickCaptureOpen.value=true; }
  if (event.key === "Escape") { healthOpen.value=false; quotaOpen.value=false; notificationOpen.value=false; quickCaptureOpen.value=false; selectedNotification.value=null; }
}
function loadQuota() { return polling.run("loadQuota", loadQuotaData); }
async function loadQuotaData() {
  if (!isTauriRuntime()) return;
  quotaLoading.value=true;
  try { quota.value=await backgroundRequest(() => getCodexQuota()); }
  catch { quota.value={ ...quota.value, resetCreditsAvailable:null, resetCreditsStatus:"error" }; }
  finally { quotaLoading.value=false; }
}
function openQuickCaptureFromPage() { quickCaptureOpen.value=true; }
function refreshNotificationsFromTapd() { void loadNotifications(false); }
async function loadSystemHealth() {
  if (!isTauriRuntime() || document.visibilityState === "hidden" || healthLoading.value) return;
  healthLoading.value=true;
  const [databaseResult,codexResult,tapdResult,repositoriesResult,emailResult,quotaResult] = await Promise.allSettled([
    backgroundRequest(() => databaseHealth()), backgroundRequest(() => getCodexCliStatus()), backgroundRequest(() => getTapdStatus()), backgroundRequest(() => listRepositoryAssets()), backgroundRequest(() => getEmailNotificationStatus()), loadQuota().then(() => quota.value),
  ]);
  const items:Array<{ label:string; state:"ok"|"warning"|"idle"|"error"; detail:string }> = [];
  if (databaseResult.status === "fulfilled") items.push({ label:"本地数据库", state:"ok", detail:`结构版本 ${databaseResult.value.schemaVersion}` });
  else items.push({ label:"本地数据库", state:"error", detail:"无法读取" });
  if (codexResult.status === "fulfilled") {
    const value=codexResult.value;
    items.push({ label:"Codex CLI", state:value.installed && value.authenticated ? "ok" : value.installed ? "warning" : "error", detail:value.installed ? (value.authenticated ? `已登录 · ${value.version || "版本未知"}` : "已安装但未登录") : "未安装" });
  } else items.push({ label:"Codex CLI", state:"error", detail:"检查失败" });
  if (quotaResult.status === "fulfilled") {
    quota.value=quotaResult.value;
    items.push({ label:"额度快照", state:quota.value.available ? (quota.value.freshness === "stale" ? "warning" : "ok") : "warning", detail:quota.value.available ? `${Math.round((quota.value.primary || quota.value.secondary)?.remainingPercent || 0)}% · ${quotaFreshnessText.value}` : "暂无未过期快照" });
  } else items.push({ label:"额度快照", state:"warning", detail:"读取失败" });
  if (tapdResult.status === "fulfilled") {
    const value=tapdResult.value;
    items.push({ label:"TAPD", state:value.configured ? (value.lastSyncedAt ? "ok" : "warning") : "idle", detail:value.configured ? `${value.projects.filter(project => project.enabled).length} 个项目 · ${value.itemCount} 个缺陷` : "未配置" });
  } else items.push({ label:"TAPD", state:"warning", detail:"检查失败" });
  if (repositoriesResult.status === "fulfilled") {
    const repositories=repositoriesResult.value;
    items.push({ label:"Git 项目", state:repositories.length ? "ok" : "warning", detail:repositories.length ? `${repositories.length} 个仓库已纳管` : "尚未扫描仓库" });
  } else items.push({ label:"Git 项目", state:"warning", detail:"检查失败" });
  if (emailResult.status === "fulfilled") {
    emailStatus.value=emailResult.value;
    const value=emailResult.value;
    items.push({ label:"邮件提醒", state:value.state === "error" ? "error" : value.state === "ready" ? "ok" : "idle", detail:value.state === "ready" ? "已启用" : value.state === "error" ? (value.lastError || "发送异常") : "未启用" });
  } else items.push({ label:"邮件提醒", state:"warning", detail:"检查失败" });
  healthItems.value=items;
  healthUpdatedAt.value=new Date().toISOString();
  healthLoading.value=false;
}
function notificationNumericId(value:string) { return Math.abs([...value].reduce((hash,character)=>(hash * 31 + character.charCodeAt(0)) | 0,0)); }
async function sendSystemCompletionNotification(item:WorkbenchNotification) {
  const storageKey=`workbench-os-notification:${item.id}`;
  if (!workbenchNotificationsEnabled.value || !isTauriRuntime() || window.localStorage.getItem(storageKey)) return;
  let granted=await isPermissionGranted();
  if (!granted) granted=(await requestPermission()) === "granted";
  if (!granted) return;
  sendNotification({ id:notificationNumericId(item.id), title:item.title, body:`${item.body}\n打开工作台可查看详细信息。`, autoCancel:true, extra:{ notificationId:item.id } });
  window.localStorage.setItem(storageKey,new Date().toISOString());
}
async function initializeQuickCaptureShortcut() {
  if (!isTauriRuntime()) return;
  try {
    const shortcut="CommandOrControl+Shift+Space";
    if (!await isShortcutRegistered(shortcut)) {
      await registerShortcut(shortcut,async event=>{
        if (event.state!=="Pressed") return;
        await getCurrentWindow().show();
        await getCurrentWindow().setFocus();
        quickCaptureOpen.value=true;
      });
      quickShortcutRegistered=true;
    }
  } catch(cause) { console.error("注册快速记录快捷键失败",cause); }
}
async function loadNotifications(sync = true, announce = true) {
  if (!isTauriRuntime() || document.visibilityState === "hidden" || notificationLoading.value) return;
  notificationLoading.value=true;
  try {
    if (sync) await backgroundRequest(() => syncCodexNotifications());
    if (!workbenchNotificationsEnabled.value) {
      const changed = notifications.value.length > 0;
      notifications.value=[];
      notificationToast.value=null;
      if (changed) window.dispatchEvent(new CustomEvent("workbench-notifications-updated", { detail:[] }));
      return;
    }
    const knownIds = new Set(notifications.value.map(item => item.id));
    const latest = await backgroundRequest(() => listNotifications());
    const newUnread = latest.filter(item => !item.isRead && !knownIds.has(item.id));
    const newestUnread = newUnread[0];
    const changed = notificationsChanged(notifications.value, latest);
    notifications.value=latest;
    if (changed) window.dispatchEvent(new CustomEvent("workbench-notifications-updated", { detail:latest.slice(0,5) }));
    if (announce) for (const item of newUnread) void sendSystemCompletionNotification(item);
    if (announce && newestUnread) {
      notificationToast.value=newestUnread;
      window.clearTimeout(notificationToastTimer);
      notificationToastTimer=window.setTimeout(() => notificationToast.value=null, 8000);
    }
  }
  catch (error) { console.error("读取工作台消息失败", error); }
  finally { notificationLoading.value=false; }
}
function syncTapdNotifications() { return polling.run("syncTapdNotifications", syncTapdNotificationsData); }
async function syncTapdNotificationsData() {
  if (!isTauriRuntime()) return;
  try {
    const status=await backgroundRequest(() => getTapdStatus());
    if (!status.configured || !status.projects.some(project => project.enabled)) return;
    const result=await backgroundRequest(() => syncTapdItems());
    await loadNotifications(false);
    window.dispatchEvent(new CustomEvent("tapd-background-synced",{detail:result}));
  }
  catch (error) { console.error("同步 TAPD 消息失败", error); }
}
function loadEmailStatus() { return polling.run("loadEmailStatus", loadEmailStatusData); }
async function loadEmailStatusData() {
  if (!isTauriRuntime()) return;
  try { emailStatus.value=await backgroundRequest(() => getEmailNotificationStatus()); }
  catch (cause) { console.error("读取 Codex 邮件通知状态失败", cause); }
}
function loadVipStatus() { return polling.run("loadVipStatus", loadVipStatusData); }
async function loadVipStatusData() {
  if (!isTauriRuntime()) return;
  try { vipStatus.value=await backgroundRequest(() => getVipStatus()); }
  catch (cause) { console.error("读取 VIP 状态失败", cause); }
}
async function syncWindowState() {
  if (isTauriRuntime()) windowMaximized.value=await getCurrentWindow().isMaximized();
}
async function minimizeWindow() { if (isTauriRuntime()) await getCurrentWindow().minimize(); }
async function toggleMaximizeWindow() {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().toggleMaximize();
  await syncWindowState();
}
async function closeWindow() { if (isTauriRuntime()) await getCurrentWindow().close(); }
function isTopbarInteractiveTarget(target:HTMLElement) {
  return Boolean(target.closest("button,a,input,select,textarea,[role='button'],.system-health-popover,.quota-popover,.notification-popover"));
}
function applyWorkbenchNotificationPreference(event:Event) {
  workbenchNotificationsEnabled.value = Boolean((event as CustomEvent<boolean>).detail);
  notificationOpen.value=false;
  notificationToast.value=null;
  window.clearTimeout(notificationToastTimer);
  if (workbenchNotificationsEnabled.value) void loadNotifications(false,false);
  else {
    notifications.value=[];
    window.dispatchEvent(new CustomEvent("workbench-notifications-updated", { detail:[] }));
  }
}
function loadActiveOperations() { return polling.run("loadActiveOperations", loadActiveOperationsData); }
async function loadActiveOperationsData() {
  if (!isTauriRuntime()) return;
  railNow.value=Date.now();
  const [testResult,generationResult,tapdResult,jenkinsResult,projectsResult] = await Promise.allSettled([backgroundRequest(() => listTestRuns(undefined,undefined,true)),backgroundRequest(() => listTestCaseGenerations()),backgroundRequest(() => listTapdCodexJobs()),backgroundRequest(() => listJenkinsPublishRecords()),backgroundRequest(() => listRunningRepositoryProjects())]);
  if (testResult.status === "fulfilled") { allTestRuns.value=testResult.value; activeTestRuns.value=testResult.value.filter(item => item.status === "queued" || item.status === "running"); }
  else console.error("读取正在执行的测试失败",testResult.reason);
  if (generationResult.status === "fulfilled") { allTestCaseGenerations.value=generationResult.value; activeTestCaseGenerations.value=generationResult.value.filter(item => item.status === "queued" || item.status === "running"); }
  else console.error("读取正在生成的专属用例失败",generationResult.reason);
  if (tapdResult.status === "fulfilled") { allTapdJobs.value=tapdResult.value; activeTapdJobs.value=tapdResult.value.filter(item => item.status === "queued" || item.status === "running"); }
  else console.error("读取正在处理的 TAPD 缺陷失败",tapdResult.reason);
  if (jenkinsResult.status === "fulfilled") activeJenkinsPublishes.value=jenkinsResult.value.filter(item => item.status === "queued" || item.status === "running");
  else console.error("读取正在执行的 Jenkins 发布失败",jenkinsResult.reason);
  // 后端只返回工作台管理的存活进程；编译失败不代表进程已退出，仍需展示和允许停止。
  if (projectsResult.status === "fulfilled") runningProjects.value=projectsResult.value;
  else console.error("读取正在运行的项目失败",projectsResult.reason);
}
let sidebarRefreshRequested = false;
function loadSidebarWorktime(refresh = false) {
  sidebarRefreshRequested ||= refresh;
  return polling.run("sidebar-worktime", async () => {
    do {
      const rebuild = sidebarRefreshRequested; sidebarRefreshRequested = false;
      await loadSidebarWorktimeData(rebuild);
    } while (sidebarRefreshRequested);
  });
}
async function loadSidebarWorktimeData(refresh = false) {
  if (!isTauriRuntime()) return;
  const date=todayIso.value;
  const [summaryResult]=await Promise.allSettled([backgroundRequest(() => getWorkSummary(date,date,refresh))]);
  const [sessionsResult]=await Promise.allSettled([backgroundRequest(() => listWorkSessions(date,date,false))]);
  if (date !== todayIso.value) return;
  if (summaryResult.status === "fulfilled") todayWorkSummary.value=summaryResult.value;
  else console.error("读取今日工时汇总失败",summaryResult.reason);
  if (sessionsResult.status === "fulfilled") todayWorkSessions.value=sessionsResult.value;
  else console.error("读取当前工作区间失败",sessionsResult.reason);
}
function loadProjectOptions() { return polling.run("loadProjectOptions", loadProjectOptionsData); }
function loadBackgroundDiagnostics() {
  return polling.run("background-diagnostics", async () => {
    if (!isTauriRuntime()) return;
    try { backgroundTasks.value = await backgroundRequest(listBackgroundTaskStatus); }
    catch { /* 诊断读取失败不覆盖上次状态，也不扩大为全页错误。 */ }
  });
}
async function loadProjectOptionsData() {
  if (!isTauriRuntime()) return;
  try {
    repositoryAssets.value=await backgroundRequest(() => listRepositoryAssets());
  } catch (cause) { console.error("读取可启动项目失败",cause); }
}
async function stopRailProject(project:RunningProjectProcess) {
  if (projectActionPath.value) return;
  projectActionPath.value=project.projectPath;
  projectActionMessage.value="";
  try {
    const result=await stopRepositoryProject(project.projectPath);
    projectActionMessage.value=result.message || `${project.projectName} 已停止。`;
    await Promise.all([loadActiveOperations(),loadProjectOptions()]);
  } catch (cause) { projectActionMessage.value=`停止失败：${String(cause)}`; }
  finally { projectActionPath.value=""; }
}
async function openRailProjectUrl(url:string) {
  projectActionMessage.value="";
  try { await openRepositoryRuntimeUrl(url); }
  catch (cause) { projectActionMessage.value=`打开地址失败：${String(cause)}`; }
}
function prepareWindowDragging(event:MouseEvent) {
  if (!isTauriRuntime() || event.button !== 0) return;
  const target=event.target as HTMLElement;
  if (isTopbarInteractiveTarget(target)) return;
  topbarDragStart={x:event.screenX,y:event.screenY};
}
async function continueWindowDragging(event:MouseEvent) {
  if (!topbarDragStart || (event.buttons & 1) === 0) return;
  const moved=Math.hypot(event.screenX-topbarDragStart.x,event.screenY-topbarDragStart.y);
  if (moved < 4) return;
  topbarDragStart=null;
  await getCurrentWindow().startDragging();
}
function cancelWindowDragging() { topbarDragStart=null; }
async function toggleMaximizeFromTopbar(event:MouseEvent) {
  const target=event.target as HTMLElement;
  if (!isTauriRuntime() || event.button !== 0 || isTopbarInteractiveTarget(target)) return;
  topbarDragStart=null;
  event.preventDefault();
  await toggleMaximizeWindow();
}
function refreshNavigationSettings() {
  navigationOrder.value=loadNavigationOrder();
  hiddenNavigationPaths.value=loadHiddenNavigationPaths();
}
async function toggleEmailNotification() {
  if (emailLoading.value) return;
  if (["unconfigured","unverified"].includes(emailStatus.value.state)) {
    window.alert(emailStatus.value.state === "unconfigured" ? "请先在设置中保存 QQ 邮箱和 SMTP 授权码。" : "请先在设置中发送测试邮件，验证 QQ 邮箱和 SMTP 授权码。");
    await router.push("/settings");
    return;
  }
  if (emailStatus.value.state === "error") {
    const detail=emailStatus.value.lastError || "邮件发送失败，请检查网络或 SMTP 授权码。";
    if (await confirmAction({ title:"重试失败邮件", message:`${detail}\n\n是否立即重试失败邮件？`, confirmText:"立即重试", tone:"warning" })) {
      emailLoading.value=true;
      try { emailStatus.value=await retryFailedEmails(); }
      catch (cause) { window.alert(String(cause)); }
      finally { emailLoading.value=false; }
    }
    return;
  }
  emailLoading.value=true;
  try { emailStatus.value=await setCodexEmailEnabled(!emailStatus.value.enabled); }
  catch (cause) { window.alert(String(cause)); await router.push("/settings"); }
  finally { emailLoading.value=false; }
}
function notificationTime(value:string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(date);
}
async function openNotification(item:WorkbenchNotification) {
  if (!item.isRead) {
    await markNotificationRead(item.id);
    item.isRead=true;
    item.readAt=new Date().toISOString();
  }
  notificationToast.value=null;
  window.clearTimeout(notificationToastTimer);
  notificationOpen.value=false;
  selectedNotification.value=item;
}
function openNotificationFromPage(event:Event) {
  const item = (event as CustomEvent<WorkbenchNotification>).detail;
  if (item) void openNotification(item);
}
async function readAllNotifications() {
  if (!unreadCount.value) return;
  await markAllNotificationsRead();
  const readAt = new Date().toISOString();
  notifications.value.forEach(item => { item.isRead=true; item.readAt=readAt; });
}
function handleNotificationReviewed(id:string,decision:"accepted"|"follow_up",note:string) {
  const item=notifications.value.find(notification=>notification.id===id);
  if (item) { item.reviewStatus=decision; item.reviewNote=note; item.reviewedAt=new Date().toISOString(); item.isRead=true; }
  if (selectedNotification.value?.id===id) selectedNotification.value=item || null;
  window.dispatchEvent(new CustomEvent("workbench-notifications-updated", { detail:notifications.value.slice(0,5) }));
}
function toggleNotifications() {
  notificationOpen.value=!notificationOpen.value;
  if (notificationOpen.value) void loadNotifications(false);
}
function closeQuotaOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) quotaOpen.value=false;
}
function closeHealthOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) healthOpen.value=false;
}
function closeNotificationOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) notificationOpen.value=false;
}
async function refreshLocalData() {
  if (refreshing.value) return;
  refreshing.value=true;
  refreshError.value="";
  try {
    await Promise.all([store.hydrate(),loadSystemHealth(),loadVipStatus(),loadSidebarWorktime(true)]);
    await loadNotifications(true);
    window.dispatchEvent(new CustomEvent("workbench-data-refresh-requested"));
  } catch (cause) { refreshError.value=`刷新失败：${String(cause)}`; }
  finally { refreshing.value=false; }
}
onMounted(() => {
  resetCockpitIdle();
  void initializeStartup();
  void initializeQuickCaptureShortcut();
  if (isTauriRuntime()) void listen<EmailNotificationStatus>("codex-email-status-changed", event => { emailStatus.value=event.payload; }).then(unlisten => { emailUnlisten=unlisten; });
  if (isTauriRuntime()) void listen<VipStatus>("vip-status-changed", event => { vipStatus.value=event.payload; }).then(unlisten => { vipUnlisten=unlisten; });
  if (isTauriRuntime()) void listen("codex-data-updated", () => { window.dispatchEvent(new CustomEvent("workbench-codex-data-updated")); void loadSidebarWorktime(true); }).then(unlisten => { codexDataUnlisten=unlisten; });
  if (isTauriRuntime()) {
    void syncWindowState();
    void getCurrentWindow().onResized(() => void syncWindowState()).then(unlisten => { windowResizeUnlisten=unlisten; });
  }
  polling.every("task-status-tick", 60 * 60 * 1000, store.refreshTaskStatuses);
  polling.every("quota-tick", 5 * 60 * 1000, loadQuota);
  polling.every("health-tick", 10 * 60 * 1000, loadSystemHealth);
  polling.every("notification-tick", 15 * 1000, () => loadNotifications(true));
  polling.every("tapd-tick", 5 * 60 * 1000, syncTapdNotifications);
  polling.every("email-tick", 30 * 1000, loadEmailStatus);
  polling.every("operations-tick", 5 * 1000, loadActiveOperations);
  polling.every("diagnostics-tick", 30 * 1000, loadBackgroundDiagnostics);
  railStatusTimer = window.setInterval(() => { railNow.value=Date.now(); railStatusTicks+=1; if (railStatusTicks % 5 === 0) void Promise.all([loadSidebarWorktime(),loadProjectOptions()]); }, 60 * 1000);
  cockpitIdleTimer = window.setInterval(evaluateCockpitIdle,1_000);
  polling.every("performance-tick", 2_000, loadSystemPerformance);
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("pointerdown",handleCockpitActivity,true);
  window.addEventListener("wheel",handleCockpitActivity,{capture:true,passive:true});
  window.addEventListener("touchstart",handleCockpitActivity,{capture:true,passive:true});
  document.addEventListener("visibilitychange",handleCockpitVisibilityChange);
  window.addEventListener(cockpitIdleSettingsChangedEvent,applyCockpitIdleSettings);
  window.addEventListener("open-workbench-search", openSearch);
  window.addEventListener("open-workbench-notification", openNotificationFromPage);
  window.addEventListener("open-quick-capture", openQuickCaptureFromPage);
  window.addEventListener("tapd-items-synced", refreshNotificationsFromTapd);
  window.addEventListener(workbenchNotificationPreferenceChangedEvent, applyWorkbenchNotificationPreference);
  window.addEventListener(navigationOrderChangedEvent, refreshNavigationSettings);
  window.addEventListener("workbench-active-operations-changed", loadActiveOperations);
  window.addEventListener("mousemove", continueWindowDragging);
  window.addEventListener("mouseup", cancelWindowDragging);
});
onBeforeUnmount(() => {
  window.clearInterval(railStatusTimer);
  window.clearInterval(cockpitIdleTimer);
  window.clearTimeout(notificationToastTimer);
  window.clearTimeout(startupSlowTimer);
  window.clearTimeout(startupFinishTimer);
  window.clearTimeout(startupSafetyTimer);
  window.clearTimeout(memoryOptimizationResetTimer);
  emailUnlisten?.();
  vipUnlisten?.();
  codexDataUnlisten?.();
  windowResizeUnlisten?.();
  if (quickShortcutRegistered) void unregisterShortcut("CommandOrControl+Shift+Space");
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("pointerdown",handleCockpitActivity,true);
  window.removeEventListener("wheel",handleCockpitActivity,true);
  window.removeEventListener("touchstart",handleCockpitActivity,true);
  document.removeEventListener("visibilitychange",handleCockpitVisibilityChange);
  window.removeEventListener(cockpitIdleSettingsChangedEvent,applyCockpitIdleSettings);
  window.removeEventListener("open-workbench-search", openSearch);
  window.removeEventListener("open-workbench-notification", openNotificationFromPage);
  window.removeEventListener("open-quick-capture", openQuickCaptureFromPage);
  window.removeEventListener("tapd-items-synced", refreshNotificationsFromTapd);
  window.removeEventListener(workbenchNotificationPreferenceChangedEvent, applyWorkbenchNotificationPreference);
  window.removeEventListener(navigationOrderChangedEvent, refreshNavigationSettings);
  window.removeEventListener("workbench-active-operations-changed", loadActiveOperations);
  window.removeEventListener("mousemove", continueWindowDragging);
  window.removeEventListener("mouseup", cancelWindowDragging);
});
</script>

<template>
  <div class="app-shell">
    <Transition name="startup-screen">
      <StartupSplash v-if="startupLoading" :slow="startupSlow" />
    </Transition>
    <aside class="icon-sidebar">
      <div class="app-brand-slot">
        <RouterLink class="app-brand" to="/" :title="APP_BRAND.name" :aria-label="`${APP_BRAND.name} · 返回首页`">
          <span class="app-mark" aria-hidden="true"><img :src="store.theme === 'warm' ? appLogoWarm : appLogo" :class="{ 'app-logo--warm': store.theme === 'warm' }" alt=""></span>
          <BrandWordmark />
        </RouterLink>
        <button class="cockpit-entry" type="button" title="进入数据驾驶舱" aria-label="进入数据驾驶舱" @click="openCockpit">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 4H5a1 1 0 0 0-1 1v3M16 4h3a1 1 0 0 1 1 1v3M8 20H5a1 1 0 0 1-1-1v-3M16 20h3a1 1 0 0 0 1-1v-3"/><circle cx="12" cy="12" r="2.25"/><path d="M12 7.5v2M12 14.5v2M7.5 12h2M14.5 12h2"/></svg>
        </button>
      </div>
      <nav><RouterLink v-for="item in visibleNavItems" :key="item.path" :to="item.path" :title="item.label" :class="{ 'rest-narrow-nav': item.path === '/rest' }"><NavIcon :name="item.icon" /><em>{{ item.label }}</em><b v-if="item.vip" class="vip-badge">VIP</b></RouterLink></nav>
      <div class="side-footer"><SettingsLink /></div>
    </aside>
    <header class="app-topbar" @mousedown="prepareWindowDragging" @dblclick="toggleMaximizeFromTopbar">
      <button class="top-search" title="搜索全部本地数据" @click="openSearch">⌕<span>搜索任务、对话、报告、知识与接口</span><kbd>Ctrl K</kbd></button>
      <span class="topbar-drag-zone" aria-hidden="true"></span>
      <span class="top-date">{{ dateText }}</span>
      <div class="top-runtime" @focusout="closeHealthOnBlur">
        <button class="top-runtime-status" :class="{ warning:healthIssueCount>0, loading:healthLoading }" title="查看本地数据与连接状态" @click="healthOpen=!healthOpen"><i></i><span>{{ healthSummary }}</span></button>
        <section v-if="healthOpen" class="system-health-popover panel">
          <header><div><b>连接与数据健康</b><small>仅检查本机状态</small></div><button class="text-button" :disabled="healthLoading" @click="loadSystemHealth">{{ healthLoading ? '检查中…' : '重新检查' }}</button></header>
          <div class="system-health-list"><article v-for="item in healthItems" :key="item.label" :class="item.state"><i></i><span><b>{{ item.label }}</b><small>{{ item.detail }}</small></span></article></div>
          <footer><span>{{ healthUpdatedAt ? `${notificationTime(healthUpdatedAt)} 更新` : '尚未完成检查' }}</span><RouterLink to="/settings" @click="healthOpen=false">打开设置</RouterLink></footer>
        </section>
      </div>
      <div class="top-quota" @focusout="closeQuotaOnBlur">
        <button class="top-quota-trigger" :class="{loading:quotaLoading, stale:quota.freshness==='stale'}" :aria-expanded="quotaOpen" title="查看 Codex 剩余用量" @click="quotaOpen=!quotaOpen">
          <i class="header-icon-box"><HeaderIcon name="quota" /></i><span><small>剩余用量</small><b>{{ remainingText }}</b></span><em class="disclosure-icon" aria-hidden="true"></em>
        </button>
        <CodexQuotaPopover v-if="quotaOpen" :quota="quota" :loading="quotaLoading" @close="quotaOpen=false" @refresh="loadQuota" />
      </div>
      <div class="top-notifications" @focusout="closeNotificationOnBlur">
        <button class="icon-button header-action-button notification-trigger" :class="{ active:notificationOpen }" :title="workbenchNotificationsEnabled ? '查看工作台消息' : '工作台消息通知已在设置中关闭'" :disabled="!workbenchNotificationsEnabled" @click="toggleNotifications"><HeaderIcon name="notification" /><b v-if="unreadCount">{{ unreadCount > 99 ? '99+' : unreadCount }}</b></button>
        <section v-if="notificationOpen" class="notification-popover panel">
          <header><div><b>消息中心</b><small>{{ unreadCount ? `${unreadCount} 条未读` : '全部已读' }} · {{ pendingReviewCount }} 条 Codex 结果待确认</small></div><button class="text-button" :disabled="!unreadCount" @click="readAllNotifications">全部已读</button></header>
          <div class="notification-list">
            <button v-for="item in notifications" :key="item.id" class="notification-item" :class="{ unread:!item.isRead }" @click="openNotification(item)"><i></i><span><b>{{ item.title }}</b><p>{{ item.body }}</p><small>{{ notificationTime(item.createdAt) }} · {{ item.isRead ? '已读' : '未读' }}</small></span></button>
            <p v-if="!notifications.length && !notificationLoading" class="notification-empty">暂无消息。Codex 完成结果和 TAPD 缺陷变化会显示在这里。</p>
            <p v-if="notificationLoading" class="notification-empty">正在读取消息…</p>
          </div>
          <footer class="notification-popover-footer"><RouterLink to="/inbox" @click="notificationOpen=false">打开待处理收件箱</RouterLink><span>统一处理 Codex、TAPD、测试和项目风险</span></footer>
        </section>
      </div>
      <button class="icon-button header-action-button" title="中英翻译" aria-label="打开中英翻译" @click="translationOpen=true"><HeaderIcon name="translate" /></button>
      <button class="top-capture-button" title="快速记录（Ctrl + Shift + Space）" @click="quickCaptureOpen=true">＋ 记录</button>
      <button class="top-email-toggle" :class="[emailStatus.state,{loading:emailLoading}]" :title="emailTooltip" :disabled="emailLoading" @click="toggleEmailNotification">✉ <span>邮件</span><b>{{ emailButtonText }}</b></button>
      <button class="icon-button header-action-button refresh-action" :class="{ loading:refreshing }" title="刷新本地任务和额度数据" :disabled="refreshing" @click="refreshLocalData"><HeaderIcon name="refresh" /></button><ThemeSwitch />
      <div class="window-controls" aria-label="窗口控制">
        <button type="button" title="最小化" aria-label="最小化" @click="minimizeWindow"><WindowControlIcon name="minimize" /></button>
        <button type="button" :title="windowMaximized ? '还原' : '最大化'" :aria-label="windowMaximized ? '还原' : '最大化'" @click="toggleMaximizeWindow"><WindowControlIcon :name="windowMaximized ? 'restore' : 'maximize'" /></button>
        <button type="button" class="window-close" title="关闭到系统托盘" aria-label="关闭到系统托盘" @click="closeWindow"><WindowControlIcon name="close" /></button>
      </div>
    </header>
    <main class="page-area">
      <p v-if="refreshError || store.loadError" class="scan-message error" role="alert">{{ refreshError || store.loadError }}</p>
      <RouterView @new-task="editorOpen = true" />
      <Transition name="page-loader">
        <div v-if="pageLoading && !startupLoading" class="workbench-page-loader" role="status" aria-live="polite">
          <div class="page-loader-visual" aria-hidden="true"><i></i><i></i><i></i><span>✦</span></div>
          <div class="page-loader-copy"><small>LOCAL WORKSPACE SYNC</small><b>正在载入{{ pageLoadingTitle }}</b><p>{{ pageLoadingSlow ? '本地数据量较大，仍在继续读取。' : '正在读取本地数据与运行状态…' }}</p></div>
          <button v-if="pageLoadingSlow" class="button secondary small" @click="revealSlowPage">先查看页面</button>
        </div>
      </Transition>
    </main>
    <aside class="ai-rail work-status-rail">
      <section class="rail-almanac">
        <div class="rail-almanac-card">
          <div class="rail-almanac-main">
            <b>{{ String(todayDay).padStart(2, '0') }}</b>
            <span><strong>{{ currentDate.getMonth() + 1 }}月 · {{ todayAlmanac.week.replace('星期', '周') }}</strong><small>{{ compactLunarDate }}</small></span>
            <i v-if="todayAlmanac.jieQi !== '非节气日'" class="rail-almanac-season">{{ todayAlmanac.jieQi }}</i>
            <button type="button" class="icon-button rail-almanac-link" :title="almanacExpanded ? '收起宜忌' : '展开宜忌'" :aria-label="almanacExpanded ? '收起宜忌' : '展开宜忌'" :aria-expanded="almanacExpanded" aria-controls="rail-almanac-yiji" @click="almanacExpanded=!almanacExpanded"><svg viewBox="0 0 16 16" aria-hidden="true"><path :d="almanacExpanded ? 'm4 10 4-4 4 4' : 'm4 6 4 4 4-4'" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg></button>
          </div>
          <div v-show="almanacExpanded" id="rail-almanac-yiji" class="rail-almanac-yiji"><span><i>宜</i><em>{{ todayAlmanac.yi.slice(0,3).join(' · ') || '诸事不宜' }}</em></span><span><i>忌</i><em>{{ todayAlmanac.ji.slice(0,3).join(' · ') || '诸事不忌' }}</em></span></div>
        </div>
      </section>
      <section class="rail-performance">
        <SystemPerformanceCard :snapshot="systemPerformance" :history="performanceHistory" :optimization-state="memoryOptimizationState" :optimization-message="memoryOptimizationMessage" @open="performanceOpen=true" @optimize="optimizeMemory" />
      </section>
      <section v-if="runningProjects.length" class="rail-projects">
        <header class="rail-section-header rail-section-header-success"><span><i></i><b>项目运行</b></span><em>{{ runningProjects.length }}</em></header>
        <article v-for="project in runningProjects" :key="project.projectPath" class="rail-project-card">
          <div><b :title="project.projectName">{{ project.projectName }}</b><small :title="project.status === 'failed' ? `进程仍在运行：${project.errorMessage}` : undefined" :style="project.status === 'failed' ? { color:'var(--danger)' } : undefined"><span>{{ project.status === 'starting' ? '正在启动' : project.status === 'failed' ? '编译异常' : '运行中' }}</span><template v-if="project.localUrl"> · <button type="button" class="rail-project-url" :title="`使用默认浏览器打开 ${project.localUrl}`" :aria-label="`使用默认浏览器打开 ${project.localUrl}`" @click="openRailProjectUrl(project.localUrl)">{{ project.localUrl }} <i aria-hidden="true">↗</i></button></template></small></div>
          <button class="rail-project-stop" :disabled="Boolean(projectActionPath)" @click="stopRailProject(project)">{{ projectActionPath === project.projectPath ? '停止中…' : '停止' }}</button>
        </article>
        <p v-if="projectActionMessage" class="rail-project-message">{{ projectActionMessage }}</p>
      </section>
      <section v-if="activeOperations.length" class="rail-running">
        <header class="rail-section-header rail-section-header-success"><span><i></i><b>正在执行</b></span><em>{{ activeOperations.length }}</em></header>
        <RouterLink v-for="item in activeOperations" :key="item.id" class="rail-running-item" :to="item.href">
          <span class="rail-running-item-head"><i :class="item.kind">{{ item.kind === 'test' ? '测试' : item.kind === 'jenkins' ? '发布' : 'TAPD' }}</i><b :title="item.title">{{ item.title }}</b><em>{{ item.status }}</em></span>
          <span class="rail-running-progress" :class="{ determinate:item.progressPercent !== undefined }" :role="item.progressPercent !== undefined ? 'progressbar' : undefined" :aria-valuenow="item.progressPercent" aria-valuemin="0" aria-valuemax="100"><i :style="item.progressPercent !== undefined ? { width:`${item.progressPercent}%` } : undefined"></i></span>
          <span v-if="item.progressPercent !== undefined || item.etaText" class="rail-running-estimate"><b v-if="item.progressPercent !== undefined">{{ item.progressPercent }}%</b><em>{{ item.etaText }}</em></span>
        </RouterLink>
      </section>
      <section class="rail-rest"><RestRailCard /></section>
      <section class="rail-work-session">
        <header class="rail-section-header rail-section-header-success"><span><i></i><b>当前工作</b></span><em>{{ latestWorkSession ? 1 : 0 }}</em></header>
        <div class="rail-work-card">
          <div v-if="latestWorkSession" class="rail-work-session-main"><b>{{ latestWorkSession.project }}</b><span>{{ latestWorkSession.startTime }}—{{ latestWorkSession.endTime }}</span><small>{{ latestWorkSession.source === 'manual' ? '手工记录' : '估算中' }}</small></div>
          <div v-else class="rail-work-session-main empty"><b>{{ runningProjects[0]?.projectName || '尚未识别到工作区间' }}</b><span>有本地活动后自动估算</span></div>
          <div class="rail-work-session-metrics"><span><small>今日工时</small><b>{{ formatRailMinutes(todayWorkSummary.totalMinutes) }}</b></span><span><small>当前区间</small><b>{{ latestWorkSession ? formatRailMinutes(latestWorkSession.durationMinutes) : '—' }}</b></span><RouterLink class="rail-inline-link" to="/work-records">修正 ›</RouterLink></div>
        </div>
      </section>
      <section v-if="railIssues.length" class="rail-issues">
        <header class="rail-section-header"><span><i></i><b>异常状态</b></span><em>{{ railIssues.length }}</em></header>
        <div class="rail-issue-list"><RouterLink v-for="issue in railIssues.slice(0,allIssuesOpen ? undefined : 2)" :key="issue.id" class="rail-issue" :class="issue.tone" :to="issue.to" :title="issue.detail"><i></i><span><b>{{ issue.title }}</b></span><em>›</em></RouterLink><button v-if="railIssues.length > 2" class="button secondary small" :aria-expanded="allIssuesOpen" @click="allIssuesOpen=!allIssuesOpen">{{ allIssuesOpen ? '收起异常' : `查看全部 ${railIssues.length} 项异常` }}</button></div>
      </section>
      <section class="rail-recent">
        <button class="rail-recent-toggle" :aria-expanded="recentActivitiesOpen" @click="recentActivitiesOpen=!recentActivitiesOpen"><span><i></i><b>最近活动</b></span><em>{{ recentActivities.length }} 条 · {{ recentActivitiesOpen ? '收起' : '全部' }} ›</em></button>
        <div class="rail-recent-list"><RouterLink v-for="activity in recentActivities.slice(0,recentActivitiesOpen ? 5 : 2)" :key="activity.id" :to="activity.to"><i :class="activity.tone"></i><span><b>{{ activity.title }}</b><small>{{ formatRailActivityTime(activity.at) }}</small></span></RouterLink><p v-if="!recentActivities.length">暂时没有新的运行、测试或完成记录。</p></div>
      </section>
    </aside>
    <TaskEditor :open="editorOpen" @close="editorOpen = false" />
    <WorkspaceSearch :open="searchOpen" @close="searchOpen = false" />
    <NotificationDrawer :notification="selectedNotification" @close="selectedNotification=null" @reviewed="handleNotificationReviewed" />
    <QuickCapture :open="quickCaptureOpen" @close="quickCaptureOpen=false" />
    <TranslationDialog :open="translationOpen" @close="translationOpen=false" />
    <SystemPerformanceDialog :open="performanceOpen" :snapshot="systemPerformance" :history="performanceHistory" @close="performanceOpen=false" />
    <ConfirmDialog />
    <Transition name="cockpit-overlay">
      <CockpitScreensaver v-if="cockpitOpen" :quota="quota" :notifications="notifications" :running-projects="runningProjects" :test-runs="allTestRuns" :tapd-jobs="allTapdJobs" :performance="systemPerformance" :performance-history="performanceHistory" @close="closeCockpit" @navigate="navigateFromCockpit" />
    </Transition>
    <Transition name="cockpit-warning">
      <aside v-if="cockpitIdleWarningSeconds&&!cockpitOpen" class="cockpit-idle-warning" role="status" aria-live="polite"><i></i><span><b>{{ cockpitIdleWarningSeconds }} 秒后进入数据驾驶舱</b><small>点击、输入或滚动可继续使用当前页面</small></span><button @click="resetCockpitIdle">继续使用</button></aside>
    </Transition>
    <button v-if="notificationToast" class="notification-toast panel" @click="openNotification(notificationToast)"><i></i><span><small>{{ notificationToast.kind==='tapd_item' ? 'TAPD 缺陷消息' : notificationToast.kind==='jenkins_publish' ? 'Jenkins 发布完成' : 'Codex 任务完成' }}</small><b>{{ notificationToast.title.replace(/^Codex 任务已完成：/, '') }}</b><p>{{ notificationToast.body }}</p></span><em>查看</em></button>
  </div>
</template>

<style scoped>
.app-brand .app-mark img.app-logo--warm{border-radius:9px;filter:none}
.app-brand-slot{height:40px;margin:0 8px;display:flex;align-items:center;gap:4px;min-width:0}.app-brand-slot .app-brand{margin:0;min-width:0;flex:1}.cockpit-entry{width:28px;height:28px;flex:0 0 28px;padding:0;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--muted);opacity:.34;display:grid;place-items:center;cursor:pointer;transition:opacity .16s ease,color .16s ease,border-color .16s ease,background .16s ease}.cockpit-entry svg{width:16px;height:16px;fill:none;stroke:currentColor;stroke-width:1.45;stroke-linecap:round;stroke-linejoin:round}.app-brand-slot:hover .cockpit-entry{opacity:.58}.cockpit-entry:hover,.cockpit-entry:focus-visible{opacity:1;color:var(--primary);border-color:color-mix(in srgb,var(--primary) 32%,var(--line));background:color-mix(in srgb,var(--primary) 7%,transparent);outline:0}.cockpit-entry:active{transform:translateY(1px)}
.cockpit-idle-warning{position:fixed;right:24px;bottom:24px;z-index:490;width:360px;min-height:76px;padding:13px 14px;border:1px solid color-mix(in srgb,var(--primary) 46%,var(--line));border-radius:10px;background:color-mix(in srgb,var(--surface) 94%,transparent);box-shadow:0 18px 45px rgba(0,0,0,.34);backdrop-filter:blur(12px);display:grid;grid-template-columns:10px minmax(0,1fr) auto;align-items:center;gap:10px}.cockpit-idle-warning>i{width:8px;height:8px;border-radius:50%;background:var(--primary);box-shadow:0 0 0 4px var(--primary-soft);animation:cockpit-warning-pulse 1.2s ease-in-out infinite}.cockpit-idle-warning>span{min-width:0;display:flex;flex-direction:column;gap:5px}.cockpit-idle-warning small{color:var(--muted);font-size:9px}.cockpit-idle-warning button{height:30px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:0 10px}.cockpit-idle-warning button:hover{border-color:var(--primary);color:var(--primary)}
:global(.cockpit-overlay-enter-active){transition:opacity .6s ease,transform .6s cubic-bezier(.2,.7,.2,1)}:global(.cockpit-overlay-leave-active){transition:opacity .2s ease,transform .2s ease}:global(.cockpit-overlay-enter-from),:global(.cockpit-overlay-leave-to){opacity:0;transform:scale(1.008)}.cockpit-warning-enter-active,.cockpit-warning-leave-active{transition:opacity .2s ease,transform .2s ease}.cockpit-warning-enter-from,.cockpit-warning-leave-to{opacity:0;transform:translateY(8px)}
@keyframes cockpit-warning-pulse{0%,100%{opacity:.5}50%{opacity:1}}@media(prefers-reduced-motion:reduce){.cockpit-idle-warning>i{animation:none}:global(.cockpit-overlay-enter-active),:global(.cockpit-overlay-leave-active),.cockpit-warning-enter-active,.cockpit-warning-leave-active{transition:none}}
</style>
