<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { isTauriRuntime, listTestMenus, listTestRuns, readTestReport, startTestRun, type StartTestOptions, type TestMenu, type TestRun } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";

const demoMenus: TestMenu[] = [
  { id: "client:safe-responsibility", project: "client", name: "安全责任", route: "/safetyManagement/safeResponsibility", sourcePath: "src/views/safe/safetyManagement/safeResponsibility/index.vue", caseId: "safe-responsibility", capabilities: { mock: true, realApi: true, sourceStyle: true, browserStyle: true }, tested: true, latestStatus: "passed", latestTime: "2026-07-21T17:36:51+08:00" },
  { id: "app:pages/mainPackage/tabbar/index", project: "APP", name: "首页", route: "/pages/mainPackage/tabbar/index", sourcePath: "pages/mainPackage/tabbar/index.vue", capabilities: { mock: false, realApi: false, sourceStyle: true, browserStyle: false }, tested: false },
];
const store = useWorkbenchStore();
const route = useRoute();
const menus = ref<TestMenu[]>(isTauriRuntime() ? [] : demoMenus);
const runs = ref<TestRun[]>([]);
const projectFilter = ref<"all" | "client" | "APP">("all");
const statusFilter = ref<"all" | "tested" | "untested" | "passed" | "failed">("all");
const typeFilter = ref<"all" | "functional" | "style">("all");
const query = ref("");
const loading = ref(false);
const error = ref("");
const message = ref("");
const configuring = ref<TestMenu | null>(null);
const reportTitle = ref("");
const reportContent = ref("");
const useEnvironmentToken = ref(true);
const account = ref("");
const token = ref("");
const mode = ref<TestRun["mode"]>("mock");

const filteredMenus = computed(() => menus.value.filter((menu) => {
  if (projectFilter.value !== "all" && menu.project !== projectFilter.value) return false;
  if (statusFilter.value === "tested" && !menu.tested) return false;
  if (statusFilter.value === "untested" && menu.tested) return false;
  if (statusFilter.value === "passed" && menu.latestStatus !== "passed") return false;
  if (statusFilter.value === "failed" && menu.latestStatus !== "failed") return false;
  if (typeFilter.value === "functional" && !menu.capabilities.mock) return false;
  if (typeFilter.value === "style" && !menu.capabilities.sourceStyle) return false;
  return !query.value.trim() || `${menu.name} ${menu.route} ${menu.sourcePath}`.toLowerCase().includes(query.value.trim().toLowerCase());
}));
const stats = computed(() => ({
  total: menus.value.length,
  tested: menus.value.filter((menu) => menu.tested).length,
  passed: menus.value.filter((menu) => menu.latestStatus === "passed").length,
  failed: menus.value.filter((menu) => menu.latestStatus === "failed").length,
}));
const projectCounts = computed(() => ({ client: menus.value.filter(menu=>menu.project==="client").length, app: menus.value.filter(menu=>menu.project==="APP").length }));

function modeLabel(value: TestRun["mode"]) {
  return ({ mock: "功能测试（模拟接口）", real: "功能测试（真实接口）", "source-style": "页面源码与样式检查", "browser-style": "浏览器页面样式测试" })[value];
}
function statusLabel(menu: TestMenu) {
  if (!menu.tested) return "未测试";
  return menu.latestStatus === "passed" ? "通过" : "未通过";
}
function formatTime(value?: string) {
  return value ? new Intl.DateTimeFormat("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" }).format(new Date(value)) : "—";
}
function localDate(value: string) { return new Date(value).toLocaleDateString("sv-SE"); }
function availableModes(menu: TestMenu): TestRun["mode"][] {
  const values: TestRun["mode"][] = [];
  if (menu.capabilities.mock) values.push("mock");
  if (menu.capabilities.realApi) values.push("real");
  if (menu.capabilities.sourceStyle) values.push("source-style");
  if (menu.capabilities.browserStyle) values.push("browser-style");
  return values;
}
function openConfig(menu: TestMenu) {
  configuring.value = menu;
  mode.value = menu.project === "APP" ? "source-style" : "mock";
  account.value = ""; token.value = ""; useEnvironmentToken.value = true; error.value = ""; message.value = "";
}
async function refresh() {
  if (!isTauriRuntime()) return;
  menus.value = await listTestMenus();
  runs.value = await listTestRuns();
}
async function runTest() {
  if (!configuring.value || loading.value) return;
  loading.value = true; error.value = ""; message.value = "";
  const options: StartTestOptions = { menuId: configuring.value.id, mode: mode.value, account: account.value.trim() || undefined, token: token.value || undefined, useEnvironmentToken: useEnvironmentToken.value };
  token.value = "";
  try {
    if (!isTauriRuntime()) throw new Error("浏览器预览只能查看界面，请在桌面开发版中运行测试。");
    const run = await startTestRun(options);
    reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`;
    reportContent.value = run.reportMarkdown;
    message.value = run.status === "passed" ? "测试已完成并通过，报告已保存。" : "测试已完成但未通过，请查看报告中的失败项。";
    configuring.value = null;
    await refresh();
    await store.hydrate();
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function openReport(menu: TestMenu) {
  error.value = "";
  const run = runs.value.find((item) => item.menuId === menu.id);
  try {
    reportTitle.value = `${menu.name} · 最近测试报告`;
    if (run) reportContent.value = run.reportMarkdown;
    else if (menu.latestReportPath && isTauriRuntime()) reportContent.value = await readTestReport(menu.latestReportPath);
    else reportContent.value = "当前菜单还没有可查看的测试报告。";
  } catch (cause) { error.value = String(cause); }
}

function openRun(run: TestRun) {
  reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`;
  reportContent.value = run.reportMarkdown || run.errorMessage || "该次测试没有可显示的报告内容。";
}

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
    <header class="page-header"><div><h1>测试中心</h1><p>复用 client 现有菜单用例与 APP 已注册页面，统一发起测试并保存报告</p></div><div><button class="button secondary" :disabled="loading" @click="refresh">↻ 刷新菜单与报告</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="metric-grid testing-metrics"><article class="clickable-card" @click="statusFilter='all'"><span>菜单 / 页面</span><b>{{ stats.total }}</b><p>点击查看全部</p></article><article class="clickable-card" @click="statusFilter='tested'"><span>已有报告</span><b>{{ stats.tested }}</b><p>点击筛选已测试</p></article><article class="clickable-card" @click="statusFilter='passed'"><span>最近通过</span><b>{{ stats.passed }}</b><p class="success-text">● 点击筛选</p></article><article class="clickable-card" @click="statusFilter='failed'"><span>最近未通过</span><b>{{ stats.failed }}</b><p :class="stats.failed ? 'warning-text' : ''">● 点击筛选</p></article></section>
    <section class="panel testing-panel">
      <div class="testing-toolbar"><label>⌕<input v-model="query" placeholder="搜索菜单名称、路由或源码路径"></label><select v-model="projectFilter"><option value="all">全部项目</option><option value="client">client</option><option value="APP">APP</option></select><select v-model="statusFilter"><option value="all">全部状态</option><option value="tested">已测试</option><option value="untested">未测试</option><option value="passed">最近通过</option><option value="failed">最近未通过</option></select><select v-model="typeFilter"><option value="all">全部能力</option><option value="functional">已有功能用例</option><option value="style">页面 / 样式检查</option></select></div>
      <div class="testing-note"><b>执行边界</b><span>当前读取 client {{ projectCounts.client }} 个已有菜单用例、APP {{ projectCounts.app }} 个 pages.json 注册页面。项目目录不存在时列表为空；真实接口测试可能创建、修改和清理 E2E 前缀测试数据，默认不启用。</span></div>
      <div class="test-table-wrap"><table class="test-table"><thead><tr><th>项目</th><th>功能菜单 / 页面</th><th>已有测试能力</th><th>最近结果</th><th>测试报告</th><th>操作</th></tr></thead><tbody><tr v-for="menu in filteredMenus" :key="menu.id"><td><span class="project-badge" :class="menu.project.toLowerCase()">{{ menu.project }}</span></td><td><b>{{ menu.name }}</b><small>{{ menu.route }}</small><em>{{ menu.sourcePath }}</em></td><td><div class="capability-tags"><span v-if="menu.capabilities.mock">功能用例</span><span v-if="menu.capabilities.realApi">真实接口</span><span v-if="menu.capabilities.sourceStyle">源码 / 样式</span><span v-if="menu.capabilities.browserStyle">浏览器样式</span><span v-if="!menu.caseId" class="muted">暂无交互用例</span></div></td><td><span class="test-status" :class="menu.latestStatus || 'untested'">{{ statusLabel(menu) }}</span><small>{{ formatTime(menu.latestTime) }}</small></td><td><button class="text-button" :disabled="!menu.tested" @click="openReport(menu)">{{ menu.tested ? '查看报告' : '暂无报告' }}</button></td><td><button class="button primary small" :disabled="loading" @click="openConfig(menu)">▶ 测试</button></td></tr></tbody></table><div v-if="!filteredMenus.length" class="empty-state"><b>没有符合条件的菜单</b><p>请调整项目、状态、测试能力或关键词筛选。</p></div></div>
    </section>

    <div v-if="configuring" class="activity-backdrop test-dialog-backdrop" @click.self="!loading && (configuring = null)"><section class="panel test-config-dialog"><header><div><h2>测试 {{ configuring.name }}</h2><p>{{ configuring.project }} · {{ configuring.route }}</p></div><button class="icon-button" :disabled="loading" @click="configuring = null">×</button></header><div class="test-config-body"><label>测试类型<select v-model="mode"><option v-for="value in availableModes(configuring)" :key="value" :value="value">{{ modeLabel(value) }}</option></select></label><div v-if="mode === 'real'" class="real-test-warning"><b>真实接口写入提醒</b><p>现有 client 真实用例会创建、修改或删除 E2E 前缀测试数据，并在结束时尝试清理。只有确认测试环境允许写入时才执行。</p></div><label v-if="mode === 'real'" class="check-row"><input v-model="useEnvironmentToken" type="checkbox">读取 Windows 用户环境变量 HLZT_TOKEN</label><label v-if="mode === 'real' && !useEnvironmentToken">临时 Token<input v-model="token" type="password" autocomplete="off" placeholder="只传给本次测试子进程，不保存"></label><label v-if="mode === 'real'">测试账号（可选）<input v-model="account" autocomplete="off" placeholder="仅传给支持 E2E_TEST_ACCOUNT 的已有用例"><small>当前多数 client 用例使用 Token 登录；账号字段不会写入数据库。</small></label><div class="reuse-source"><b>复用来源</b><code v-if="configuring.caseId">client/e2e/menu-cases/{{ configuring.caseId }}.json</code><code v-else>APP/pages.json + {{ configuring.sourcePath }}</code></div></div><footer><span>{{ loading ? '正在运行项目现有测试，完成后自动打开报告…' : '测试结果和报告保存在个人工作台本地数据库；凭据不保存。' }}</span><button class="button secondary" :disabled="loading" @click="configuring = null">取消</button><button class="button primary" :disabled="loading" @click="runTest">{{ loading ? '测试中…' : '开始测试' }}</button></footer></section></div>

    <div v-if="reportContent" class="activity-backdrop report-dialog-backdrop" @click.self="reportContent = ''"><section class="panel test-report-dialog"><header><div><h2>{{ reportTitle }}</h2><p>项目现有测试报告 / 工作台静态检查报告</p></div><button class="icon-button" @click="reportContent = ''">×</button></header><pre>{{ reportContent }}</pre></section></div>
  </div>
</template>
