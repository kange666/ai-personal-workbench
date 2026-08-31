<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { cancelTestRun, ensureWeeklyAudit, getTestCaseGeneration, getTestRun, isTauriRuntime, listFeatureParity, listTestMenus, listTestProjects, listTestRuns, listTestScenarios, listTestSuites, listToolchains, listWeeklyAudits, preflightTest, readTestReport, recommendTestsFromGit, runWeeklyAudit, saveFeatureParityReview, scanToolchains, startTestCaseGeneration, startTestRun, syncFeatureParity, type FeatureParity, type RegressionEvidence, type StartTestOptions, type TestCaseGenerationJob, type TestMenu, type TestPreflight, type TestProject, type TestRecommendation, type TestRun, type TestScenario, type TestSuite, type TestMode, type ToolchainInventory, type WeeklyAudit } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";
import TestReportDialog from "../components/TestReportDialog.vue";

const demoMenus: TestMenu[] = [
  { id: "project:safe-responsibility", project: "client", projectPath: "F:/TB-project/client", projectKind: "vue", name: "安全责任", route: "/safetyManagement/safeResponsibility", sourcePath: "src/views/safe/safetyManagement/safeResponsibility/index.vue", caseId: "safe-responsibility", hasCaseFile: true, canCreateCaseFile: false, capabilities: { mock: true, realApi: true, sourceStyle: true, browserStyle: true }, tested: true, latestStatus: "failed", latestTime: "2026-08-24T14:21:18+08:00" },
  { id: "project:inspection-plan", project: "client", projectPath: "F:/TB-project/client", projectKind: "vue", name: "检查计划", route: "/safetyManagement/inspectionPlan", sourcePath: "src/views/safe/safetyManagement/inspectionPlan/index.vue", hasCaseFile: false, canCreateCaseFile: true, capabilities: { mock: false, realApi: false, sourceStyle: true, browserStyle: false }, tested: false },
];
const demoRuns: TestRun[] = [{ id:"demo-run", menuId:"project:safe-responsibility", project:"client", projectPath:"F:/TB-project/client", menuName:"安全责任", mode:"browser-style", status:"failed", startedAt:"2026-08-24T14:20:00+08:00", finishedAt:"2026-08-24T14:21:18+08:00", reportMarkdown:"# 测试结论", outputExcerpt:"1 passed · 1 failed", errorMessage:"点击搜索后列表没有刷新", selectedScenarios:["页面基础区域正常显示","搜索结果正确刷新"], scenarioResults:[{id:"scenario-1",title:"搜索结果正确刷新",status:"failed",durationMs:980,purpose:"确认输入关键词并点击搜索后，页面会发起请求并刷新列表。",steps:["进入安全责任页面","输入责任单位关键词","点击搜索按钮"],checks:["筛选参数正确传递","列表只显示匹配结果"],errorMessage:"等待列表刷新超时：点击搜索后没有观察到接口请求。",artifacts:[{name:"搜索失败页面",path:"/src/assets/app-logo.png",contentType:"image/png",kind:"screenshot"}]},{id:"scenario-2",title:"页面基础区域正常显示",status:"passed",durationMs:220,purpose:"确认页面核心区域可以正常显示。",steps:["进入页面"],checks:["标题和列表可见"],errorMessage:"",artifacts:[]}], artifacts:[{name:"搜索失败页面",path:"/src/assets/app-logo.png",contentType:"image/png",kind:"screenshot"}], totalCount:2,passedCount:1,failedCount:1,skippedCount:0,durationMs:1200,exitCode:1,environmentSummary:"浏览器预览示例 · Node + Playwright",cleanupStatus:"not-applicable" }];
const selectedProjectStorageKey = "ai-workbench.testing.selected-project.v1";
function storedProjectPath() {
  try { return window.localStorage.getItem(selectedProjectStorageKey) || ""; }
  catch { return ""; }
}
function normalizedProjectPath(value: string) {
  return value.replace(/^\\\\\?\\/, "").replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
}
const store = useWorkbenchStore();
const route = useRoute();
const menus = ref<TestMenu[]>(isTauriRuntime() ? [] : demoMenus);
const runs = ref<TestRun[]>(isTauriRuntime() ? [] : demoRuns);
const projects = ref<TestProject[]>(isTauriRuntime() ? [] : [{ path:"F:/TB-project/client", name:"client", projectKind:"vue", caseCount:1, pageCount:2, capabilities:{mock:true,realApi:true,sourceStyle:true,browserStyle:true}, warnings:[] }]);
const selectedProjectPath = ref(storedProjectPath() || projects.value[0]?.path || "");
const recommendations = ref<TestRecommendation[]>([]);
const parities = ref<FeatureParity[]>([]);
const activeSection = ref<"menus" | "history" | "parity" | "audit">("menus");
const selectedParity = ref<FeatureParity | null>(null);
const toolchains = ref<ToolchainInventory>({installations:[],conflicts:[]});
const audits = ref<WeeklyAudit[]>([]);
const projectFilter = ref<"all" | "client" | "APP">("all");
const statusFilter = ref<"all" | "tested" | "untested" | "passed" | "failed" | "blocked">("all");
const typeFilter = ref<"all" | "functional" | "style">("all");
const query = ref("");
const parityQuery = ref("");
const parityStatusFilter = ref<"all" | "matched" | FeatureParity["parityStatus"]>("all");
const parityDomainFilter = ref("全部领域");
const paritySourceMessage = ref("");
const loading = ref(false);
const error = ref("");
const message = ref("");
const configuring = ref<TestMenu | null>(null);
const reportTitle = ref("");
const reportContent = ref("");
const activeReportRun = ref<TestRun | null>(null);
const useEnvironmentToken = ref(true);
const confirmedRealWrite = ref(false);
const account = ref("");
const token = ref("");
const mode = ref<TestMode>("mock");
const createCaseFile = ref(false);
const scenarios = ref<TestScenario[]>([]);
const selectedScenarios = ref<string[]>([]);
const testSuites = ref<TestSuite[]>([]);
const selectedTestSuiteId = ref<TestSuite["id"]>("common-real");
const caseGeneration = ref<TestCaseGenerationJob | null>(null);
const preflight = ref<TestPreflight | null>(null);
const scenarioLoading = ref(false);
const selectedRecommendations = ref<string[]>([]);
const batchRunning = ref(false);
const testRunning = ref(false);
const cancellingTest = ref(false);
const activeRunningRun = ref<TestRun | null>(null);
const verificationKinds: RegressionEvidence["verificationType"][] = ["static", "api", "browser"];
const parityPlatforms: Array<"PC" | "APP"> = ["PC", "APP"];

function regression(feature: FeatureParity, platform: "PC" | "APP", kind: RegressionEvidence["verificationType"]) {
  return feature.regression.find((item) => item.platform === platform && item.verificationType === kind);
}
function evidenceLabel(status?: RegressionEvidence["status"]) {
  return status === "passed" ? "已通过" : status === "failed" ? "未通过" : "未执行";
}
function parityLabel(status: FeatureParity["parityStatus"]) {
  return ({ pending: "待核对", "static-aligned": "已匹配", confirmed: "人工确认一致", different: "存在差异", "pc-only": "仅 PC", "app-only": "仅 APP" })[status];
}

const filteredMenus = computed(() => menus.value.filter((menu) => {
  if (statusFilter.value === "tested" && !menu.tested) return false;
  if (statusFilter.value === "untested" && menu.tested) return false;
  if (statusFilter.value === "passed" && menu.latestStatus !== "passed") return false;
  if (statusFilter.value === "failed" && menu.latestStatus !== "failed") return false;
  if (statusFilter.value === "blocked" && !["blocked", "error", "cancelled"].includes(menu.latestStatus || "")) return false;
  if (typeFilter.value === "functional" && !menu.capabilities.mock) return false;
  if (typeFilter.value === "style" && !menu.capabilities.sourceStyle) return false;
  return !query.value.trim() || `${menu.name} ${menu.route} ${menu.sourcePath}`.toLowerCase().includes(query.value.trim().toLowerCase());
}));
const stats = computed(() => ({
  total: menus.value.length,
  tested: menus.value.filter((menu) => menu.tested).length,
  passed: menus.value.filter((menu) => menu.latestStatus === "passed").length,
  failed: menus.value.filter((menu) => menu.latestStatus === "failed").length,
  blocked: menus.value.filter((menu) => ["blocked", "error", "cancelled"].includes(menu.latestStatus || "")).length,
}));
const selectedProject = computed(() => projects.value.find((item) => item.path === selectedProjectPath.value));
const selectedTestSuite = computed(() => testSuites.value.find((item) => item.id === selectedTestSuiteId.value));
const dedicatedTestSuite = computed(() => testSuites.value.find((item) => item.kind === "dedicated"));
const generatingDedicatedCase = computed(() => ["queued", "running"].includes(caseGeneration.value?.status || ""));
const parityDomains = computed(() => ["全部领域", ...new Set(parities.value.map(item=>item.domain).filter(Boolean))]);
const parityStats = computed(() => ({
  total: parities.value.length,
  matched: parities.value.filter(item=>!["pc-only","app-only"].includes(item.parityStatus)).length,
  pcOnly: parities.value.filter(item=>item.parityStatus==="pc-only").length,
  appOnly: parities.value.filter(item=>item.parityStatus==="app-only").length,
}));
const filteredParities = computed(() => parities.value.filter((item) => {
  if (parityStatusFilter.value === "matched" && ["pc-only","app-only"].includes(item.parityStatus)) return false;
  if (!['all','matched'].includes(parityStatusFilter.value) && item.parityStatus !== parityStatusFilter.value) return false;
  if (parityDomainFilter.value !== "全部领域" && item.domain !== parityDomainFilter.value) return false;
  const keyword=parityQuery.value.trim().toLowerCase();
  return !keyword || `${item.featureName} ${item.domain} ${item.pcPage} ${item.appPage} ${item.evidence.join(" ")}`.toLowerCase().includes(keyword);
}));
const reportSections = computed(() => {
  const sections: Array<{title:string; paragraphs:string[]; bullets:string[]; code:string[]}> = [];
  let current = { title:"执行摘要", paragraphs:[] as string[], bullets:[] as string[], code:[] as string[] };
  let inCode = false;
  const flush = () => { if (current.paragraphs.length || current.bullets.length || current.code.length) sections.push(current); };
  for (const raw of reportContent.value.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (line.trim().startsWith("```")) { inCode = !inCode; continue; }
    if (inCode) { current.code.push(line); continue; }
    const heading = line.match(/^#{1,4}\s+(.+)/);
    if (heading) { flush(); current = {title:heading[1],paragraphs:[],bullets:[],code:[]}; continue; }
    const bullet = line.match(/^[-*]\s+(.+)/);
    if (bullet) { current.bullets.push(bullet[1]); continue; }
    if (line.trim() && !/^\|?\s*:?-+/.test(line)) current.paragraphs.push(line.replace(/^>\s?/,"").replace(/\*\*/g,""));
  }
  flush();
  return sections.length ? sections : [{title:"报告内容",paragraphs:["当前报告没有可解析的结构化内容。"],bullets:[],code:[]}];
});
const reportStats = computed(() => {
  const text=reportContent.value;
  const passed=(text.match(/(?:✅|通过|passed)/gi)||[]).length;
  const failed=(text.match(/(?:❌|失败|未通过|failed)/gi)||[]).length;
  return { passed, failed, sections:reportSections.value.length };
});

function modeLabel(value: TestRun["mode"]) {
  return ({ mock: "功能测试（模拟接口）", real: "功能测试（真实接口）", "source-style": "页面源码与样式检查", "browser-style": "浏览器页面样式测试" })[value];
}
function statusLabel(menu: TestMenu) {
  if (!menu.tested) return "未测试";
  return ({ passed:"通过", failed:"未通过", blocked:"环境阻塞", error:"执行异常", cancelled:"已取消", queued:"等待中", running:"执行中" } as const)[menu.latestStatus || "blocked"];
}
function formatTime(value?: string) {
  return value ? new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(new Date(value)) : "—";
}
function localDate(value: string) { return new Date(value).toLocaleDateString("sv-SE"); }
function wait(milliseconds: number) { return new Promise(resolve => window.setTimeout(resolve, milliseconds)); }
async function waitForTestCompletion(initial: TestRun): Promise<TestRun> {
  let current = initial;
  const deadline = Date.now() + 60 * 60 * 1000;
  while (["queued", "running"].includes(current.status)) {
    if (Date.now() > deadline) throw new Error("测试运行超过 60 分钟，工作台已停止等待；测试进程可能仍在后台运行。");
    await wait(650);
    current = await getTestRun(current.id);
    activeRunningRun.value = current;
  }
  return current;
}
function availableModes(menu: TestMenu): TestMode[] {
  const values: TestMode[] = [];
  const capabilities = menu.hasCaseFile || !createCaseFile.value ? menu.capabilities : selectedProject.value?.capabilities || menu.capabilities;
  if (capabilities.mock) values.push("mock");
  if (capabilities.realApi || selectedProject.value?.capabilities.realApi) values.push("real");
  if (capabilities.sourceStyle) values.push("source-style");
  if (capabilities.browserStyle) values.push("browser-style");
  return values;
}
async function loadScenarios() {
  if (!configuring.value || !selectedProjectPath.value) return;
  scenarioLoading.value = true; preflight.value = null;
  try {
    if (mode.value === "real") {
      testSuites.value = isTauriRuntime()
        ? await listTestSuites(selectedProjectPath.value, configuring.value.id)
        : [{id:"common-real",name:"公共通用用例",description:"所有真实接口页面都可以使用的只读检查。",kind:"common",readOnly:true}];
      if (!testSuites.value.some(item => item.id === selectedTestSuiteId.value)) selectedTestSuiteId.value = "common-real";
    } else {
      testSuites.value = [];
    }
    scenarios.value = isTauriRuntime() ? await listTestScenarios(selectedProjectPath.value, configuring.value.id, mode.value, mode.value === "real" ? selectedTestSuiteId.value : undefined) : [{id:"scenario-1",title:"页面基础区域正常显示",description:"确认页面核心区域可以正常显示。",mode:mode.value,defaultSelected:true}];
    selectedScenarios.value = scenarios.value.filter(item => item.defaultSelected).map(item => item.title);
  } catch (cause) { scenarios.value = []; selectedScenarios.value = []; error.value = String(cause); }
  finally { scenarioLoading.value = false; }
}
async function openConfig(menu: TestMenu) {
  configuring.value = menu;
  createCaseFile.value = false;
  selectedTestSuiteId.value = "common-real";
  testSuites.value = [];
  caseGeneration.value = null;
  const values = availableModes(menu);
  mode.value = menu.hasCaseFile && values.includes("mock") ? "mock" : "source-style";
  account.value = ""; token.value = ""; useEnvironmentToken.value = true; confirmedRealWrite.value = false; error.value = ""; message.value = "";
  await loadScenarios();
}

async function generateDedicatedCase() {
  if (!configuring.value || generatingDedicatedCase.value || loading.value || !isTauriRuntime()) return;
  error.value = ""; message.value = "";
  const originalMenuId = configuring.value.id;
  const originalRoute = configuring.value.route;
  try {
    caseGeneration.value = await startTestCaseGeneration(selectedProjectPath.value, originalMenuId);
    while (caseGeneration.value && ["queued", "running"].includes(caseGeneration.value.status)) {
      await wait(650);
      caseGeneration.value = await getTestCaseGeneration(caseGeneration.value.id);
    }
    if (!caseGeneration.value || caseGeneration.value.status !== "completed") {
      throw new Error(caseGeneration.value?.errorMessage || "专属用例生成失败，请查看 Codex CLI 状态。");
    }
    await refreshSelectedProject();
    const refreshed = menus.value.find(item => item.id === originalMenuId || item.route === originalRoute);
    if (refreshed) configuring.value = refreshed;
    selectedTestSuiteId.value = "dedicated-real";
    await loadScenarios();
    message.value = `${caseGeneration.value.menuName} 专属用例已生成并通过校验，已自动切换到专属场景。`;
  } catch (cause) {
    error.value = String(cause);
  }
}
async function openRecommendation(item:TestRecommendation) {
  const menu=menus.value.find(value=>value.id===item.menuId);
  if (!menu) return;
  await openConfig(menu);
  mode.value=item.recommendedMode;
  await loadScenarios();
}
async function refresh() {
  if (!isTauriRuntime()) return;
  const availableProjects = await listTestProjects();
  projects.value = availableProjects;
  const restoredProject = availableProjects.find(item => normalizedProjectPath(item.path) === normalizedProjectPath(selectedProjectPath.value));
  selectedProjectPath.value = restoredProject?.path || availableProjects[0]?.path || "";
  const paritySummary = await syncFeatureParity();
  paritySourceMessage.value = paritySummary.sourceMessage;
  await ensureWeeklyAudit();
  [menus.value, runs.value, parities.value, toolchains.value, audits.value, recommendations.value] = await Promise.all([
    selectedProjectPath.value ? listTestMenus(selectedProjectPath.value) : Promise.resolve([]),
    selectedProjectPath.value ? listTestRuns(undefined, selectedProjectPath.value) : Promise.resolve([]),
    listFeatureParity(), listToolchains(), listWeeklyAudits(),
    selectedProjectPath.value ? recommendTestsFromGit(selectedProjectPath.value) : Promise.resolve([]),
  ]);
  if (!toolchains.value.installations.length) toolchains.value = await scanToolchains();
}

async function refreshSelectedProject() {
  if (!isTauriRuntime() || !selectedProjectPath.value) return;
  loading.value = true; error.value = "";
  try {
    [menus.value, runs.value, recommendations.value] = await Promise.all([listTestMenus(selectedProjectPath.value), listTestRuns(undefined, selectedProjectPath.value), recommendTestsFromGit(selectedProjectPath.value)]);
    selectedRecommendations.value = [];
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function runAudit() {
  if (!isTauriRuntime() || loading.value) return;
  loading.value=true; error.value="";
  try { const result=await runWeeklyAudit(); audits.value=await listWeeklyAudits(); toolchains.value=await listToolchains(); message.value=`${result.weekStart} 周检完成：${result.summary}`; }
  catch(cause){error.value=String(cause);} finally{loading.value=false;}
}
async function rescanToolchains() {
  if (!isTauriRuntime() || loading.value) return;
  loading.value=true; error.value="";
  try{toolchains.value=await scanToolchains();message.value=`已读取 ${toolchains.value.installations.length} 个工具入口，发现 ${toolchains.value.conflicts.length} 个待人工确认项。`;}
  catch(cause){error.value=String(cause);}finally{loading.value=false;}
}
async function saveParity(feature: FeatureParity) {
  if (!isTauriRuntime()) return;
  loading.value = true; error.value = "";
  try {
    await saveFeatureParityReview({ id: feature.id, parityStatus: feature.parityStatus, intentionalDifference: feature.intentionalDifference, manuallyConfirmed: true });
    feature.manuallyConfirmed = true;
    message.value = `${feature.featureName} 的人工核对结果已保存。`;
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function runTest() {
  if (!configuring.value || loading.value || testRunning.value) return;
  testRunning.value = true; cancellingTest.value = false; error.value = ""; message.value = "";
  const options: StartTestOptions = { projectPath: selectedProjectPath.value, menuId: configuring.value.id, mode: mode.value, selectedScenarios: [...selectedScenarios.value], testSuiteId: mode.value === "real" ? selectedTestSuiteId.value : undefined, createCaseFile: mode.value === "real" ? false : createCaseFile.value, confirmedRealWrite: selectedTestSuite.value?.readOnly ? false : confirmedRealWrite.value, account: account.value.trim() || undefined, token: token.value || undefined, useEnvironmentToken: useEnvironmentToken.value };
  token.value = "";
  try {
    if (!isTauriRuntime()) throw new Error("浏览器预览只能查看界面，请在桌面开发版中运行测试。");
    preflight.value = await preflightTest(options);
    const queued = await startTestRun(options);
    activeRunningRun.value = queued;
    configuring.value = null;
    message.value = "测试已进入后台执行，可在右侧“正在执行”查看进度。";
    window.dispatchEvent(new CustomEvent("workbench-active-operations-changed"));
    const run = await waitForTestCompletion(queued);
    activeReportRun.value = run;
    reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`;
    reportContent.value = run.reportMarkdown;
    message.value = run.status === "passed" ? "测试已完成并通过，报告已保存。" : run.status === "blocked" ? "测试未启动：执行前环境检查未通过。" : "测试已完成但未通过，请查看问题场景。";
    activeRunningRun.value = null;
    await refreshSelectedProject();
    await store.hydrate();
  } catch (cause) { error.value = String(cause); }
  finally { testRunning.value = false; cancellingTest.value = false; window.dispatchEvent(new CustomEvent("workbench-active-operations-changed")); }
}

async function cancelCurrentTest() {
  const run = activeRunningRun.value;
  if (!run || cancellingTest.value) return;
  cancellingTest.value = true; error.value = "";
  try {
    await cancelTestRun(run.id);
    message.value = "已发送取消指令，正在等待测试进程安全结束并保存记录。";
  } catch (cause) {
    error.value = String(cause);
    cancellingTest.value = false;
  }
}
async function openReport(menu: TestMenu) {
  error.value = "";
  const run = runs.value.find((item) => item.menuId === menu.id && (item.scenarioResults.length || item.reportMarkdown.trim() || item.errorMessage.trim()));
  try {
    reportTitle.value = `${menu.name} · 最近测试报告`;
    activeReportRun.value = run || null;
    if (run?.reportMarkdown.trim()) reportContent.value = run.reportMarkdown;
    else if (run?.sourceReportPath?.toLowerCase().endsWith(".md") && isTauriRuntime()) reportContent.value = await readTestReport(run.sourceReportPath);
    else if (run) reportContent.value = run.errorMessage || "该次测试没有生成文字报告，请查看结构化执行结果。";
    else if (menu.latestReportPath?.toLowerCase().endsWith(".md") && isTauriRuntime()) reportContent.value = await readTestReport(menu.latestReportPath);
    else reportContent.value = "当前菜单还没有可查看的测试报告。";
  } catch (cause) { error.value = String(cause); }
}

function openRun(run: TestRun) {
  activeReportRun.value = run;
  reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`;
  reportContent.value = run.reportMarkdown || run.errorMessage || "该次测试没有可显示的报告内容。";
}
function closeReport() { reportContent.value=""; activeReportRun.value=null; }

function toggleAllScenarios(checked: boolean) {
  selectedScenarios.value = checked ? scenarios.value.map(item => item.title) : [];
}
function onToggleAllScenarios(event: Event) { toggleAllScenarios((event.target as HTMLInputElement).checked); }

async function runRecommendedBatch() {
  if (batchRunning.value || !selectedRecommendations.value.length) return;
  batchRunning.value = true; error.value = ""; message.value = "";
  let completed = 0; let failed = 0;
  try {
    for (const menuId of selectedRecommendations.value) {
      const recommendation = recommendations.value.find(item => item.menuId === menuId);
      const menu = menus.value.find(item => item.id === menuId);
      if (!recommendation || !menu || !menu.hasCaseFile) { failed += 1; continue; }
      const suiteId = recommendation.recommendedMode === "real" ? "common-real" : undefined;
      const available = await listTestScenarios(selectedProjectPath.value, menuId, recommendation.recommendedMode, suiteId);
      const queued = await startTestRun({ projectPath:selectedProjectPath.value, menuId, mode:recommendation.recommendedMode, selectedScenarios:available.map(item=>item.title), testSuiteId:suiteId, useEnvironmentToken:true });
      const run = await waitForTestCompletion(queued);
      completed += 1; if (run.status !== "passed") failed += 1;
      activeReportRun.value = run; reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`; reportContent.value = run.reportMarkdown;
    }
    message.value = `推荐测试完成：执行 ${completed} 项，${failed} 项需要处理或未能执行。`;
    await refreshSelectedProject();
  } catch (cause) { error.value = String(cause); }
  finally { batchRunning.value = false; }
}

watch(mode, () => { if (configuring.value) void loadScenarios(); });
watch(createCaseFile, (enabled) => {
  if (!configuring.value || !enabled) return;
  const values = availableModes(configuring.value);
  if (!values.includes(mode.value)) mode.value = values[0] || "source-style";
});
watch(selectedProjectPath, (value) => {
  if (!value) return;
  try { window.localStorage.setItem(selectedProjectStorageKey, value); }
  catch { /* WebView 禁用本地存储时不阻断测试中心。 */ }
});

onMounted(async () => {
  loading.value = true;
  try {
    await refresh();
    const runId = String(route.query.run || "");
    const date = String(route.query.date || "");
    const matched = runs.value.find((item) => item.id === runId || (!runId && date && localDate(item.startedAt) === date));
    if (matched) openRun(matched);
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
});
</script>

<template>
  <div class="view testing-view">
    <header class="page-header testing-page-header"><div><div class="testing-title-row"><h1>测试中心</h1><select v-model="selectedProjectPath" :disabled="loading" @change="refreshSelectedProject"><option v-if="!projects.length" value="">项目资产中暂无项目</option><option v-for="item in projects" :key="item.path" :value="item.path">{{ item.name }} · {{ item.projectKind }}</option></select></div><p>选择项目资产中的本地项目，分层记录静态、接口与浏览器测试证据</p></div><div><button class="button secondary" :disabled="loading" @click="refresh">↻ 刷新项目、矩阵与报告</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <div class="testing-tabs"><button :class="{active:activeSection==='menus'}" @click="activeSection='menus'">功能 / 页面测试</button><button :class="{active:activeSection==='history'}" @click="activeSection='history'">测试历史</button><button :class="{active:activeSection==='parity'}" @click="activeSection='parity'">PC / APP 对照矩阵</button><button :class="{active:activeSection==='audit'}" @click="activeSection='audit'">系统周检与工具链</button></div>
    <section v-if="activeSection==='menus'" class="metric-grid testing-metrics testing-metrics-v2"><article class="clickable-card" @click="statusFilter='all'"><span>功能 / 页面</span><b>{{ stats.total }}</b><p>点击查看全部</p></article><article class="clickable-card" @click="statusFilter='tested'"><span>已有报告</span><b>{{ stats.tested }}</b><p>点击筛选已测试</p></article><article class="clickable-card" @click="statusFilter='passed'"><span>最近通过</span><b>{{ stats.passed }}</b><p class="success-text">● 点击筛选</p></article><article class="clickable-card" @click="statusFilter='failed'"><span>最近未通过</span><b>{{ stats.failed }}</b><p :class="stats.failed ? 'warning-text' : ''">● 点击筛选</p></article><article class="clickable-card" @click="statusFilter='blocked'"><span>环境 / 执行问题</span><b>{{ stats.blocked }}</b><p :class="stats.blocked ? 'warning-text' : ''">● 点击筛选</p></article></section>
    <section v-if="activeSection==='menus' && recommendations.length" class="panel test-recommendations"><header><div><b>根据 Git 变更推荐测试</b><p>只推荐与当前项目页面源码直接关联的测试；可勾选多项顺序执行。</p></div><div class="recommendation-actions"><span>{{ recommendations.length }} 项</span><button class="button primary small" :disabled="batchRunning || !selectedRecommendations.length" @click="runRecommendedBatch">{{ batchRunning ? '执行中…' : `执行已选 ${selectedRecommendations.length} 项` }}</button></div></header><div><article v-for="item in recommendations.slice(0,8)" :key="item.menuId"><input v-model="selectedRecommendations" type="checkbox" :value="item.menuId"><span class="project-badge">{{ item.project }}</span><div><b>{{ item.menuName }}</b><small>{{ item.reason }} · {{ item.changedFiles.slice(0,2).join('、') }}</small></div><button class="button secondary small" @click="openRecommendation(item)">配置</button></article></div></section>
    <section v-if="activeSection==='menus'" class="panel testing-panel">
      <div class="testing-toolbar testing-toolbar-v2"><label>⌕<input v-model="query" placeholder="搜索功能名称、路由或源码路径"></label><select v-model="statusFilter"><option value="all">全部状态</option><option value="tested">已测试</option><option value="untested">未测试</option><option value="passed">最近通过</option><option value="failed">最近未通过</option><option value="blocked">环境 / 执行问题</option></select><select v-model="typeFilter"><option value="all">全部能力</option><option value="functional">已有功能用例</option><option value="style">页面 / 样式检查</option></select></div>
      <div class="testing-note"><b>当前项目</b><span>{{ selectedProject?.name || '尚未选择' }} · {{ selectedProjectPath || '请先在项目资产中扫描本地项目' }}。缺少菜单用例的页面可以在测试配置中勾选“添加测试文件”；不会覆盖同名文件。</span></div>
      <div class="test-table-wrap"><table class="test-table"><thead><tr><th>配置</th><th>功能 / 页面</th><th>可用测试能力</th><th>最近结果</th><th>测试报告</th><th>操作</th></tr></thead><tbody><tr v-for="menu in filteredMenus" :key="menu.id"><td><span class="project-badge" :class="{app:menu.projectKind==='uni-app'}">{{ menu.hasCaseFile ? '已有用例' : '仅页面' }}</span></td><td><b>{{ menu.name }}</b><small>{{ menu.route }}</small><em>{{ menu.sourcePath }}</em></td><td><div class="capability-tags"><span v-if="menu.capabilities.mock">模拟接口</span><span v-if="menu.capabilities.realApi">真实接口</span><span v-if="menu.capabilities.sourceStyle">源码 / 样式</span><span v-if="menu.capabilities.browserStyle">浏览器样式</span><span v-if="!menu.hasCaseFile" class="muted">可选择添加测试配置</span></div></td><td><span class="test-status" :class="menu.latestStatus || 'untested'">{{ statusLabel(menu) }}</span><small>{{ formatTime(menu.latestTime) }}</small></td><td><button class="text-button" :disabled="!menu.tested" @click="openReport(menu)">{{ menu.tested ? '查看报告' : '暂无报告' }}</button></td><td><button class="button primary small" :disabled="loading" @click="openConfig(menu)">▶ 测试</button></td></tr></tbody></table><div v-if="!filteredMenus.length" class="empty-state"><b>没有符合条件的功能或页面</b><p>请调整状态、测试能力或关键词筛选；也可以先在项目资产中扫描项目。</p></div></div>
    </section>

    <section v-else-if="activeSection==='history'" class="panel test-history-panel">
      <header><div><small>REGRESSION HISTORY</small><h2>测试历史与回归</h2><p>同一项目的每次执行独立保留；环境阻塞与业务失败分开显示。</p></div><span>{{ runs.length }} 条记录</span></header>
      <div class="history-table-wrap"><table><thead><tr><th>时间</th><th>功能 / 页面</th><th>测试类型</th><th>结果</th><th>场景</th><th>耗时</th><th></th></tr></thead><tbody><tr v-for="run in runs" :key="run.id"><td>{{ new Date(run.startedAt).toLocaleString('zh-CN') }}</td><td><b>{{ run.menuName }}</b><small>{{ run.project }}</small></td><td>{{ modeLabel(run.mode) }}</td><td><span class="test-status" :class="run.status">{{ run.status==='passed'?'通过':run.status==='failed'?'未通过':run.status==='blocked'?'环境阻塞':run.status==='error'?'执行异常':run.status==='cancelled'?'已取消':'执行中' }}</span></td><td>{{ run.passedCount }} / {{ run.totalCount }} 通过</td><td>{{ run.durationMs < 1000 ? `${run.durationMs} ms` : `${(run.durationMs/1000).toFixed(1)} 秒` }}</td><td><button class="text-button" @click="openRun(run)">查看报告</button></td></tr></tbody></table><div v-if="!runs.length" class="empty-state"><b>当前项目还没有测试记录</b><p>运行一次页面测试后，历史结果会显示在这里。</p></div></div>
    </section>

    <section v-else-if="activeSection==='parity'" class="panel parity-panel">
      <div class="testing-note"><b>全量来源</b><span>{{ paritySourceMessage || '正在读取 client / APP 真实菜单与本地页面。' }}。匹配结论只表示功能名称、父级和路径能够对应；“接口”和“浏览器”仍须实际执行后才会显示通过。</span></div>
      <div class="parity-overview">
        <button :class="{active:parityStatusFilter==='all'}" @click="parityStatusFilter='all'"><span>全部功能</span><b>{{ parityStats.total }}</b></button>
        <button :class="{active:parityStatusFilter==='matched'}" @click="parityStatusFilter='matched'"><span>已匹配</span><b>{{ parityStats.matched }}</b></button>
        <button :class="{active:parityStatusFilter==='pc-only'}" @click="parityStatusFilter='pc-only'"><span>仅 PC</span><b>{{ parityStats.pcOnly }}</b></button>
        <button :class="{active:parityStatusFilter==='app-only'}" @click="parityStatusFilter='app-only'"><span>仅 APP</span><b>{{ parityStats.appOnly }}</b></button>
      </div>
      <div class="parity-toolbar"><label>⌕<input v-model="parityQuery" placeholder="搜索功能名称、领域、PC/APP 路径"></label><select v-model="parityDomainFilter"><option v-for="domain in parityDomains" :key="domain">{{ domain }}</option></select><select v-model="parityStatusFilter"><option value="all">全部匹配状态</option><option value="matched">全部已匹配</option><option value="static-aligned">已自动匹配</option><option value="confirmed">人工确认一致</option><option value="different">存在差异</option><option value="pc-only">仅 PC</option><option value="app-only">仅 APP</option><option value="pending">待核对</option></select><span>显示 {{ filteredParities.length }} / {{ parities.length }}</span></div>
      <div class="parity-table-wrap"><table class="parity-table"><thead><tr><th>领域 / 功能</th><th>PC 功能</th><th>APP 功能</th><th>匹配状态</th><th>验证进度</th><th></th></tr></thead><tbody><tr v-for="feature in filteredParities" :key="feature.id" @click="selectedParity=feature"><td><small>{{ feature.domain }}</small><b>{{ feature.featureName }}</b></td><td><span :class="{missing:feature.parityStatus==='app-only'}">{{ feature.parityStatus==='app-only'?'— 未找到对应功能':feature.pcPage||'菜单 / 操作配置' }}</span></td><td><span :class="{missing:feature.parityStatus==='pc-only'}">{{ feature.parityStatus==='pc-only'?'— 未找到对应功能':feature.appPage||'菜单 / 操作配置' }}</span></td><td><i class="parity-status" :class="feature.parityStatus">{{ parityLabel(feature.parityStatus) }}</i></td><td><div class="compact-verification"><span v-for="platform in parityPlatforms" :key="platform"><b>{{ platform }}</b><em v-for="kind in verificationKinds" :key="kind" :class="regression(feature,platform,kind)?.status" :title="`${kind}：${evidenceLabel(regression(feature,platform,kind)?.status)}`"></em></span></div></td><td><button class="text-button">详情 →</button></td></tr></tbody></table><div v-if="!filteredParities.length" class="empty-state"><b>没有符合条件的对照项</b><p>请调整关键词、领域或匹配状态。</p></div></div>
    </section>

    <section v-else class="audit-layout">
      <article class="panel weekly-audit-panel"><header><div><small>WEEKLY ACCEPTANCE</small><h2>每周整体检查</h2><p>每周一 09:00 后自动执行；程序未运行时，下次启动自动补跑。</p></div><button class="button primary" :disabled="loading" @click="runAudit">{{ loading?'检查中…':'立即重新检查' }}</button></header><template v-if="audits[0]"><div class="audit-summary"><b :class="audits[0].status">{{ audits[0].status==='passed'?'全部通过':audits[0].status==='attention'?'有待确认项':'存在失败项' }}</b><span>{{ audits[0].weekStart }} 开始的一周</span><em v-if="audits[0].catchUpRun">漏跑补偿</em></div><div class="audit-checks"><article v-for="item in audits[0].checks" :key="item.checkType" :class="item.status"><i>{{ item.status==='passed'?'✓':item.status==='attention'?'!':'×' }}</i><div><b>{{ item.target }}</b><p>{{ item.summary }}</p></div></article></div><p class="audit-boundary">{{ audits[0].summary }}</p></template><div v-else class="empty-state"><b>本周周检尚未执行</b><p>到计划时间会自动运行，也可以现在手工执行。</p></div></article>
      <article class="panel toolchain-panel"><header><div><small>TOOLCHAIN</small><h2>工具链冲突</h2><p>只读检查 PATH 顺序和版本，不自动卸载或改环境变量。</p></div><button class="button secondary" :disabled="loading" @click="rescanToolchains">重新扫描</button></header><div class="tool-overview"><b>{{ toolchains.installations.length }}</b><span>检测到的入口</span><b :class="{warning:toolchains.conflicts.length}">{{ toolchains.conflicts.length }}</b><span>待人工确认</span></div><div v-if="toolchains.conflicts.length" class="conflict-list"><article v-for="item in toolchains.conflicts" :key="item.id"><header><b>{{ item.toolName }}</b><span>{{ item.conflictType==='multiple-paths'?'重复入口':'版本不一致' }}</span></header><p>{{ item.summary }}</p><small>{{ item.recommendedAction }}</small></article></div><div v-else class="empty-state compact"><b>未发现冲突</b><p>当前 PATH 中的工具入口没有明显重复或版本差异。</p></div><details class="installation-details"><summary>查看全部工具入口</summary><div v-for="item in toolchains.installations" :key="item.id"><b>{{ item.toolName }}</b><code>{{ item.version }}</code><span>#{{ item.pathPriority+1 }} · {{ item.source }}</span><small>{{ item.executablePath }}</small></div></details></article>
    </section>

    <div v-if="selectedParity" class="activity-backdrop" @click.self="selectedParity=null"><section class="panel parity-detail">
      <header><div><small>{{ selectedParity.domain }} · PARITY REVIEW</small><h2>{{ selectedParity.featureName }}</h2><p>人工确认只记录你的判断，不会修改 client 或 APP 项目。</p></div><button class="icon-button" @click="selectedParity=null">×</button></header>
      <div class="parity-paths"><article><b>PC 页面 / 配置</b><code>{{ selectedParity.pcPage || (selectedParity.parityStatus==='app-only'?'未找到对应功能':'菜单或操作配置，暂无独立页面') }}</code></article><article><b>APP 页面 / 配置</b><code>{{ selectedParity.appPage || (selectedParity.parityStatus==='pc-only'?'未找到对应功能':'菜单或操作配置，暂无独立页面') }}</code></article></div>
      <h3>匹配依据</h3><div class="parity-source-evidence"><code v-for="item in selectedParity.evidence" :key="item">{{ item }}</code></div>
      <h3>分层验证证据</h3><div class="evidence-list"><article v-for="item in selectedParity.regression" :key="`${item.platform}-${item.verificationType}`"><span class="project-badge" :class="item.platform.toLowerCase()">{{ item.platform }}</span><b>{{ item.verificationType==='static'?'静态检查':item.verificationType==='api'?'接口测试':'浏览器测试' }}</b><span class="evidence-status" :class="item.status">{{ evidenceLabel(item.status) }}</span><p>{{ item.resultSummary }}</p><code>{{ item.sourcePath || '暂无执行来源' }}</code></article></div>
      <h3>接口契约</h3><div class="contract-list"><div v-for="contract in selectedParity.contracts" :key="contract.id"><span>{{ contract.platform }}</span><b>{{ contract.method }}</b><code>{{ contract.url }}</code><em>{{ contract.verificationLevel === 'static' ? '静态读取' : contract.verificationLevel }}</em></div></div>
      <footer class="parity-review"><label>核对结论<select v-model="selectedParity.parityStatus"><option value="static-aligned">已匹配，待实际测试</option><option value="confirmed">人工确认一致</option><option value="different">存在差异</option><option value="pc-only">仅 PC 存在</option><option value="app-only">仅 APP 存在</option><option value="pending">待核对</option></select></label><label class="check-row"><input v-model="selectedParity.intentionalDifference" type="checkbox">差异属于有意设计</label><button class="button primary" :disabled="loading" @click="saveParity(selectedParity)">保存人工核对</button></footer>
    </section></div>

    <div v-if="configuring" class="activity-backdrop test-dialog-backdrop" @click.self="!loading && !generatingDedicatedCase && (configuring = null)">
      <section class="panel test-config-dialog test-config-v2">
        <header><div><h2>测试 {{ configuring.name }}</h2><p>{{ configuring.project }} · {{ configuring.route }}</p></div><button class="icon-button" :disabled="loading || generatingDedicatedCase" title="关闭弹框，测试继续在后台执行" @click="configuring = null">×</button></header>
        <div class="test-config-body" :class="{locked:testRunning || generatingDedicatedCase}">
          <label v-if="!configuring.hasCaseFile && mode !== 'real'" class="case-create-option"><span><input v-model="createCaseFile" type="checkbox"><b>添加测试配置</b></span><small>根据当前页面源码生成 JSON 配置；这不是 Playwright 专属脚本。真实接口专属脚本请使用下方 Codex CLI 按钮生成。</small></label>
          <label>测试类型<select v-model="mode"><option v-for="value in availableModes(configuring)" :key="value" :value="value">{{ modeLabel(value) }}</option></select><small v-if="!configuring.hasCaseFile && !createCaseFile && mode !== 'real'">当前没有功能用例，可执行页面源码检查，或添加测试配置后使用项目已有运行器。</small></label>
          <section v-if="mode === 'real'" class="test-suite-selector">
            <header><div><b>真实接口测试用例</b><small>公共用例始终可用；专属用例只在 Codex 生成并校验成功后显示。</small></div></header>
            <label v-for="suite in testSuites" :key="suite.id" :class="{active:selectedTestSuiteId===suite.id}"><input v-model="selectedTestSuiteId" type="radio" :value="suite.id" @change="loadScenarios"><span><b>{{ suite.name }}<em>{{ suite.kind === 'common' ? '通用只读' : '业务专属' }}</em></b><small>{{ suite.description }}</small></span></label>
            <div v-if="!dedicatedTestSuite && !generatingDedicatedCase" class="dedicated-case-empty"><div><b>还没有 {{ configuring.name }} 专属用例</b><small>点击后由 Codex CLI 阅读当前页面源码和接口，只生成该业务的 Playwright 脚本；通过校验后才会加入列表。</small></div><button class="button secondary" type="button" @click="generateDedicatedCase">用 Codex CLI 生成专属用例</button></div>
          </section>
          <section v-if="caseGeneration && generatingDedicatedCase" class="case-generation-progress"><header><b><i></i>正在生成 {{ caseGeneration.menuName }} 专属用例</b><span>{{ caseGeneration.progressPercent }}%</span></header><div class="generation-progress-track"><i :style="{width:`${caseGeneration.progressPercent}%`}"></i></div><p>{{ caseGeneration.progressMessage }}</p><small>正在后台调用已登录的 Codex CLI；生成后还会执行 Playwright 场景收集校验。</small></section>
          <section v-else-if="caseGeneration?.status === 'completed'" class="case-generation-progress completed"><header><b>✓ 专属用例已生成并通过校验</b><span>100%</span></header><p>{{ caseGeneration.generatedSpecPath }}</p></section>
          <section class="scenario-selector"><header><div><b>测试场景</b><small>已按当前功能的测试配置和业务能力筛选，默认全选。</small></div><label><input type="checkbox" :checked="selectedScenarios.length === scenarios.length && scenarios.length > 0" @change="onToggleAllScenarios">全选</label></header><div v-if="scenarioLoading" class="scenario-empty">正在读取项目测试场景…</div><label v-for="item in scenarios" v-else :key="item.id"><input v-model="selectedScenarios" type="checkbox" :value="item.title"><span><b>{{ item.title }}</b><small>{{ item.description }}</small></span></label><div v-if="!scenarioLoading && !scenarios.length" class="scenario-empty">当前测试类型没有可识别的场景，请检查项目运行器。</div></section>
          <div v-if="mode === 'real'" class="real-test-warning"><b>{{ selectedTestSuite?.readOnly ? '公共用例为只读测试' : '专属真实接口写入提醒' }}</b><p>{{ selectedTestSuite?.readOnly ? '只检查登录态、真实接口响应、查询、重置和页面稳定性，不提交新增、修改或删除请求。' : '专属用例可能创建、修改或删除 E2E 前缀测试数据，并在结束时尝试清理。只有确认测试环境允许写入时才执行。' }}</p></div>
          <label v-if="mode === 'real' && !selectedTestSuite?.readOnly" class="check-row real-confirm"><input v-model="confirmedRealWrite" type="checkbox">我确认当前是允许写入和清理测试数据的环境</label>
          <label v-if="mode === 'real'" class="check-row"><input v-model="useEnvironmentToken" type="checkbox">读取 Windows 用户环境变量 HLZT_TOKEN</label>
          <label v-if="mode === 'real' && !useEnvironmentToken">临时 Token<input v-model="token" type="password" autocomplete="off" placeholder="只传给本次测试子进程，不保存"></label>
          <label v-if="mode === 'real'">测试账号（可选）<input v-model="account" autocomplete="off" placeholder="仅传给支持 E2E_TEST_ACCOUNT 的已有用例"><small>账号和 Token 都不会写入数据库或报告。</small></label>
          <div v-if="preflight" class="preflight-list" :class="{ready:preflight.ready}"><b>{{ preflight.ready ? '执行前检查通过' : '执行前检查未通过' }}</b><ul><li v-for="item in preflight.checks" :key="item.name" :class="{passed:item.passed}"><i>{{ item.passed?'✓':'×' }}</i><span>{{ item.name }}<small>{{ item.detail }}</small></span></li></ul></div>
          <section v-if="testRunning" class="test-run-progress"><header><b>{{ cancellingTest ? '正在取消测试' : '测试正在后台执行' }}</b><span>运行编号 {{ activeRunningRun?.id.slice(0,8) }}</span></header><div><i class="done">✓</i><span><b>执行前检查</b><small>项目、运行器、场景和凭据检查已完成</small></span></div><div><i class="active">2</i><span><b>{{ cancellingTest ? '停止测试进程' : '执行所选场景' }}</b><small>{{ cancellingTest ? '正在终止浏览器及其子进程' : `正在运行 ${activeRunningRun?.totalCount || selectedScenarios.length} 个场景` }}</small></span></div><div><i>3</i><span><b>整理测试报告</b><small>执行结束后自动汇总问题、截图和修复入口</small></span></div></section>
          <div class="reuse-source"><b>测试来源</b><code v-if="mode === 'real' && selectedTestSuite?.specPath">{{ selectedTestSuite.specPath }}</code><code v-else-if="configuring.caseFilePath">{{ configuring.caseFilePath }}</code><code v-else>{{ configuring.sourcePath }}（尚未添加测试配置）</code></div>
        </div>
        <footer><span>{{ generatingDedicatedCase ? 'Codex 生成和校验完成后会自动选择专属用例。' : testRunning ? '运行期间可离开页面，但请不要关闭桌面程序。' : '测试结果保存在工作台本地数据库；真实凭据不会保存。' }}</span><button v-if="testRunning" class="button secondary" :disabled="cancellingTest" @click="cancelCurrentTest">{{ cancellingTest ? '正在取消…' : '停止本次测试' }}</button><button v-else class="button secondary" :disabled="loading || generatingDedicatedCase" @click="configuring = null">取消</button><button v-if="!testRunning" class="button primary" :disabled="loading || generatingDedicatedCase || scenarioLoading || !selectedScenarios.length || (mode==='real' && !selectedTestSuite?.readOnly && !confirmedRealWrite)" @click="runTest">开始测试 {{ selectedScenarios.length }} 个场景</button></footer>
      </section>
    </div>

    <TestReportDialog v-if="activeReportRun || reportContent" :run="activeReportRun" :title="reportTitle" :fallback-markdown="reportContent" @close="closeReport" />
  </div>
</template>

<style scoped>
.testing-title-row{display:flex;align-items:center;gap:12px}.testing-title-row h1{margin:0}.testing-title-row select{max-width:430px;height:36px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--text);padding:0 11px}.testing-metrics-v2{grid-template-columns:repeat(5,minmax(0,1fr))}.testing-toolbar-v2{grid-template-columns:minmax(320px,1fr) 170px 170px}.test-status.blocked,.test-status.cancelled{background:color-mix(in srgb,var(--warning) 13%,transparent);color:var(--warning)}.test-status.error{background:color-mix(in srgb,var(--danger) 13%,transparent);color:var(--danger)}
.testing-tabs{display:flex;gap:6px;margin:0 0 16px;padding:5px;width:max-content;border:1px solid var(--line);border-radius:12px;background:var(--surface)}
.testing-tabs button{padding:9px 16px;border:0;border-radius:8px;background:transparent;color:var(--muted);cursor:pointer}.testing-tabs button.active{background:var(--primary);color:#fff}
.test-recommendations{margin-bottom:14px;padding:16px}.test-recommendations>header{display:flex;justify-content:space-between;gap:12px}.test-recommendations header p{margin:5px 0 0;color:var(--muted)}.recommendation-actions{display:flex;align-items:center;gap:10px}.recommendation-actions>span{color:var(--primary);font-weight:800}.test-recommendations>div{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:8px;margin-top:12px}.test-recommendations article{min-width:0;padding:10px;border:1px solid var(--line);border-radius:9px;display:grid;grid-template-columns:auto auto minmax(0,1fr) auto;align-items:center;gap:9px;background:var(--surface-2)}.test-recommendations article>div{min-width:0;display:flex;flex-direction:column;gap:5px}.test-recommendations article small{color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.test-config-v2{width:min(760px,calc(100vw - 50px));max-height:calc(100vh - 60px)}.case-create-option{padding:12px;border:1px solid color-mix(in srgb,var(--primary) 35%,var(--line));border-radius:9px;background:color-mix(in srgb,var(--primary) 6%,transparent)}.case-create-option>span{display:flex;align-items:center;gap:8px;color:var(--text)}.scenario-selector{border:1px solid var(--line);border-radius:10px;overflow:hidden}.scenario-selector>header{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:11px 12px;background:var(--surface-2)}.scenario-selector>header>div{display:flex;flex-direction:column;gap:4px}.scenario-selector>header small{color:var(--muted)}.scenario-selector>header label{display:flex;align-items:center;gap:6px;white-space:nowrap}.scenario-selector>label{display:grid;grid-template-columns:auto minmax(0,1fr);gap:9px;padding:10px 12px;border-top:1px solid var(--line);cursor:pointer}.scenario-selector>label>span{display:flex;flex-direction:column;gap:4px}.scenario-selector>label small{color:var(--muted);line-height:1.45}.scenario-empty{padding:15px;text-align:center;color:var(--muted)}.real-confirm{font-weight:700;color:var(--danger)!important}.preflight-list{padding:12px;border:1px solid color-mix(in srgb,var(--danger) 35%,var(--line));border-radius:9px;background:color-mix(in srgb,var(--danger) 5%,transparent)}.preflight-list.ready{border-color:color-mix(in srgb,var(--success) 35%,var(--line));background:color-mix(in srgb,var(--success) 5%,transparent)}.preflight-list ul{display:grid;grid-template-columns:1fr 1fr;gap:7px;margin:10px 0 0;padding:0;list-style:none}.preflight-list li{display:flex;gap:7px;color:var(--danger)}.preflight-list li.passed{color:var(--success)}.preflight-list li span{display:flex;flex-direction:column}.preflight-list li small{color:var(--muted);line-height:1.4}
.test-suite-selector{display:grid;border:1px solid var(--line);border-radius:10px;overflow:hidden}.test-suite-selector>header{padding:11px 12px;background:var(--surface-2)}.test-suite-selector>header>div{display:flex;flex-direction:column;gap:4px}.test-suite-selector small{color:var(--muted);line-height:1.45}.test-suite-selector>label{display:grid;grid-template-columns:auto minmax(0,1fr);align-items:start;gap:9px;padding:11px 12px;border-top:1px solid var(--line);cursor:pointer}.test-suite-selector>label.active{background:color-mix(in srgb,var(--primary) 8%,transparent)}.test-suite-selector>label>span{display:flex;flex-direction:column;gap:4px}.test-suite-selector>label b{display:flex;align-items:center;gap:8px}.test-suite-selector em{padding:2px 6px;border-radius:99px;background:color-mix(in srgb,var(--primary) 12%,transparent);color:var(--primary);font-size:10px;font-style:normal}.dedicated-case-empty{display:flex;align-items:center;justify-content:space-between;gap:14px;padding:12px;border-top:1px solid var(--line);background:color-mix(in srgb,var(--warning) 5%,transparent)}.dedicated-case-empty>div{display:flex;min-width:0;flex-direction:column;gap:4px}.dedicated-case-empty button{flex:0 0 auto}.case-generation-progress{display:grid;gap:9px;padding:13px;border:1px solid color-mix(in srgb,var(--primary) 38%,var(--line));border-radius:10px;background:color-mix(in srgb,var(--primary) 6%,transparent)}.case-generation-progress>header{display:flex;align-items:center;justify-content:space-between;gap:12px}.case-generation-progress>header b{display:flex;align-items:center;gap:8px}.case-generation-progress>header b>i{width:14px;height:14px;border:2px solid color-mix(in srgb,var(--primary) 25%,transparent);border-top-color:var(--primary);border-radius:50%;animation:case-spin .8s linear infinite}.case-generation-progress p,.case-generation-progress small{margin:0;color:var(--muted);overflow-wrap:anywhere}.case-generation-progress.completed{border-color:color-mix(in srgb,var(--success) 40%,var(--line));background:color-mix(in srgb,var(--success) 7%,transparent)}.generation-progress-track{height:7px;border-radius:99px;background:var(--surface-2);overflow:hidden}.generation-progress-track>i{display:block;height:100%;border-radius:inherit;background:linear-gradient(90deg,var(--primary),#8f84ff);transition:width .35s ease}@keyframes case-spin{to{transform:rotate(360deg)}}
.test-config-body.locked>label,.test-config-body.locked>.test-suite-selector,.test-config-body.locked>.scenario-selector,.test-config-body.locked>.real-test-warning{pointer-events:none;opacity:.58}.test-config-body.locked>.case-generation-progress{pointer-events:auto;opacity:1}.test-run-progress{display:grid;gap:10px;padding:13px;border:1px solid color-mix(in srgb,var(--primary) 38%,var(--line));border-radius:10px;background:color-mix(in srgb,var(--primary) 6%,transparent)}.test-run-progress>header{display:flex;justify-content:space-between;align-items:center}.test-run-progress>header span{font:11px ui-monospace,monospace;color:var(--muted)}.test-run-progress>div{display:grid;grid-template-columns:26px minmax(0,1fr);align-items:center;gap:9px}.test-run-progress i{display:grid;place-items:center;width:24px;height:24px;border-radius:50%;background:var(--surface-2);color:var(--muted);font-style:normal;font-size:11px}.test-run-progress i.done{background:color-mix(in srgb,var(--success) 15%,transparent);color:var(--success)}.test-run-progress i.active{background:var(--primary);color:#fff;animation:test-pulse 1.5s ease-in-out infinite}.test-run-progress span{display:flex;flex-direction:column;gap:3px}.test-run-progress small{color:var(--muted)}@keyframes test-pulse{50%{box-shadow:0 0 0 6px color-mix(in srgb,var(--primary) 12%,transparent)}}
.test-history-panel{padding:18px}.test-history-panel>header{display:flex;align-items:flex-start;justify-content:space-between;margin-bottom:14px}.test-history-panel>header small{color:var(--primary);letter-spacing:2px}.test-history-panel h2{margin:5px 0}.test-history-panel header p{margin:0;color:var(--muted)}.test-history-panel>header>span{color:var(--muted)}.history-table-wrap{max-height:650px;overflow:auto;border:1px solid var(--line);border-radius:10px}.history-table-wrap table{width:100%;border-collapse:collapse}.history-table-wrap th{position:sticky;top:0;padding:11px;background:var(--surface-2);text-align:left;color:var(--muted)}.history-table-wrap td{padding:11px;border-top:1px solid var(--line)}.history-table-wrap td:nth-child(2){display:flex;flex-direction:column;gap:4px}.history-table-wrap td:nth-child(2) small{color:var(--muted)}
.parity-panel{padding:18px}.parity-overview{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:10px;margin:14px 0}.parity-overview button{display:flex;align-items:center;justify-content:space-between;padding:13px 15px;border:1px solid var(--line);border-radius:11px;background:var(--surface);color:var(--muted);cursor:pointer}.parity-overview button:hover,.parity-overview button.active{border-color:var(--primary);background:color-mix(in srgb,var(--primary) 8%,var(--surface))}.parity-overview b{font-size:22px;color:var(--text)}
.parity-toolbar{display:grid;grid-template-columns:minmax(280px,1fr) 180px 170px auto;gap:10px;align-items:center;margin-bottom:12px}.parity-toolbar label{display:flex;align-items:center;gap:8px;padding:0 12px;border:1px solid var(--line);border-radius:9px;background:var(--surface)}.parity-toolbar input{width:100%;padding:10px 0;border:0;background:transparent;color:var(--text);outline:0}.parity-toolbar select{padding:10px;border:1px solid var(--line);border-radius:9px;background:var(--surface);color:var(--text)}.parity-toolbar>span{color:var(--muted);font-size:12px;text-align:right}
.parity-table-wrap{max-height:610px;overflow:auto;border:1px solid var(--line);border-radius:11px}.parity-table{width:100%;border-collapse:collapse}.parity-table th{position:sticky;top:0;z-index:1;padding:11px 12px;background:var(--surface-2);color:var(--muted);text-align:left;font-size:12px}.parity-table td{padding:12px;border-top:1px solid var(--line);vertical-align:middle}.parity-table tbody tr{cursor:pointer}.parity-table tbody tr:hover{background:color-mix(in srgb,var(--primary) 5%,transparent)}.parity-table td:first-child{min-width:180px}.parity-table td:first-child small,.parity-table td:first-child b{display:block}.parity-table td:first-child small{margin-bottom:5px;color:var(--muted)}.parity-table td:nth-child(2),.parity-table td:nth-child(3){max-width:270px}.parity-table td:nth-child(2) span,.parity-table td:nth-child(3) span{display:block;overflow:hidden;color:var(--muted);font:11px/1.5 ui-monospace,SFMono-Regular,Consolas,monospace;text-overflow:ellipsis;white-space:nowrap}.parity-table span.missing{color:var(--warning)}
.parity-status{display:inline-flex;padding:5px 9px;border-radius:999px;background:var(--muted-bg);font-size:11px;font-style:normal;white-space:nowrap}.parity-status.static-aligned,.parity-status.confirmed{color:var(--success);background:rgba(83,200,149,.12)}.parity-status.different,.parity-status.pc-only{color:var(--warning);background:rgba(243,154,98,.12)}.parity-status.app-only{color:#8f84ff;background:rgba(143,132,255,.12)}
.compact-verification{display:grid;gap:5px}.compact-verification span{display:flex;align-items:center;gap:5px}.compact-verification b{width:30px;font-size:10px}.compact-verification em{width:8px;height:8px;border-radius:50%;background:#7c8293}.compact-verification em.passed{background:var(--success)}.compact-verification em.failed{background:var(--danger)}
.evidence-status.passed{color:#53c895}.evidence-status.failed{color:#ef6d75}
.parity-detail{width:min(1040px,calc(100vw - 80px));max-height:calc(100vh - 70px);padding:24px;overflow:auto}.parity-detail h3{margin:22px 0 10px}.parity-paths{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-top:20px}.parity-paths article{padding:13px;border:1px solid var(--line);border-radius:10px}.parity-paths code,.evidence-list code{display:block;margin-top:7px;white-space:normal;word-break:break-all;color:var(--muted)}
.parity-source-evidence{display:grid;grid-template-columns:1fr 1fr;gap:8px}.parity-source-evidence code{padding:10px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--muted);word-break:break-all}
.evidence-list{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.evidence-list article{display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:8px;padding:12px;border:1px solid var(--line);border-radius:10px}.evidence-list p,.evidence-list code{grid-column:1/-1;margin:0}.contract-list{display:grid;gap:6px}.contract-list>div{display:grid;grid-template-columns:48px 64px 1fr 70px;gap:8px;align-items:center;padding:9px 12px;border-radius:8px;background:var(--surface-2)}.contract-list em{font-size:11px;color:var(--muted)}
.parity-review{display:flex;align-items:end;gap:16px;margin-top:22px;padding-top:18px;border-top:1px solid var(--line)}.parity-review label:first-child{display:grid;gap:6px}.parity-review .check-row{margin-right:auto}@media(max-width:1000px){.parity-overview{grid-template-columns:repeat(2,1fr)}.parity-toolbar{grid-template-columns:1fr 1fr}.evidence-list,.parity-paths,.parity-source-evidence{grid-template-columns:1fr}}
.audit-layout{display:grid;grid-template-columns:minmax(0,1.35fr) minmax(340px,.65fr);gap:16px}.weekly-audit-panel,.toolchain-panel{min-width:0;padding:20px}.weekly-audit-panel>header,.toolchain-panel>header{display:flex;justify-content:space-between;align-items:flex-start;gap:14px}.weekly-audit-panel h2,.toolchain-panel h2{margin:4px 0}.weekly-audit-panel header p,.toolchain-panel header p{margin:4px 0;color:var(--muted)}.audit-summary{display:flex;align-items:center;gap:10px;margin:18px 0;padding:14px;border-radius:12px;background:var(--surface-2)}.audit-summary b{font-size:18px}.audit-summary b.attention{color:var(--warning)}.audit-summary b.failed{color:var(--danger)}.audit-summary b.passed{color:var(--success)}.audit-summary span{margin-right:auto;color:var(--muted)}.audit-summary em{padding:4px 8px;border-radius:99px;background:rgba(255,180,84,.12);color:var(--warning);font-size:11px}.audit-checks{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:9px}.audit-checks article{display:flex;gap:10px;padding:12px;border:1px solid var(--line);border-radius:10px}.audit-checks i{display:grid;place-items:center;width:24px;height:24px;border-radius:50%;background:var(--surface-2)}.audit-checks .passed i{color:var(--success)}.audit-checks .attention i{color:var(--warning)}.audit-checks .failed i{color:var(--danger)}.audit-checks p{margin:5px 0 0;color:var(--muted);font-size:12px}.audit-boundary{margin:15px 0 0;color:var(--muted);font-size:12px}.tool-overview{display:grid;grid-template-columns:auto 1fr auto 1fr;align-items:baseline;gap:6px 10px;margin:18px 0}.tool-overview b{font-size:26px}.tool-overview b.warning{color:var(--warning)}.tool-overview span{color:var(--muted);font-size:12px}.conflict-list{display:grid;min-width:0;gap:9px}.conflict-list article{min-width:0;max-width:100%;padding:12px;border:1px solid rgba(255,180,84,.25);border-radius:10px;background:rgba(255,180,84,.05);overflow:hidden}.conflict-list header{display:flex;justify-content:space-between}.conflict-list header span{color:var(--warning);font-size:11px}.conflict-list p{max-width:100%;margin:7px 0;overflow-wrap:anywhere;word-break:break-word}.conflict-list small{display:block;max-width:100%;color:var(--muted);overflow-wrap:anywhere;word-break:break-word}.installation-details{margin-top:16px;border-top:1px solid var(--line);padding-top:12px}.installation-details summary{cursor:pointer;color:var(--primary)}.installation-details>div{display:grid;grid-template-columns:70px minmax(0,1fr) auto;gap:5px 8px;padding:9px 0;border-bottom:1px solid var(--line);font-size:11px}.installation-details code,.installation-details span,.installation-details small{min-width:0;overflow-wrap:anywhere}.installation-details small{grid-column:1/-1;word-break:break-all;color:var(--muted)}.empty-state.compact{padding:20px}@media(max-width:1150px){.audit-layout{grid-template-columns:1fr}}
</style>
