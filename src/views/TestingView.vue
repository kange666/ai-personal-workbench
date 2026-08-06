<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { isTauriRuntime, listFeatureParity, listTestMenus, listTestRuns, readTestReport, saveFeatureParityReview, startTestRun, syncFeatureParity, type FeatureParity, type RegressionEvidence, type StartTestOptions, type TestMenu, type TestRun } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";

const demoMenus: TestMenu[] = [
  { id: "client:safe-responsibility", project: "client", name: "安全责任", route: "/safetyManagement/safeResponsibility", sourcePath: "src/views/safe/safetyManagement/safeResponsibility/index.vue", caseId: "safe-responsibility", capabilities: { mock: true, realApi: true, sourceStyle: true, browserStyle: true }, tested: true, latestStatus: "passed", latestTime: "2026-07-21T17:36:51+08:00" },
  { id: "app:pages/mainPackage/tabbar/index", project: "APP", name: "首页", route: "/pages/mainPackage/tabbar/index", sourcePath: "pages/mainPackage/tabbar/index.vue", capabilities: { mock: false, realApi: false, sourceStyle: true, browserStyle: false }, tested: false },
];
const demoRuns: TestRun[] = [{ id:"demo-run", menuId:"client:safe-responsibility", project:"client", menuName:"安全责任", mode:"mock", status:"passed", startedAt:"2026-08-04T14:20:00+08:00", finishedAt:"2026-08-04T14:21:18+08:00", reportMarkdown:"# 测试结论\n\n✅ 安全责任菜单核心功能测试通过。\n\n## 覆盖范围\n\n- ✅ 页面加载与查询条件\n- ✅ 新增、编辑和详情弹窗\n- ✅ 表格分页和行操作\n\n## 执行信息\n\n- 使用项目已有菜单用例\n- 使用模拟接口，不写入真实数据\n\n## 命令输出\n\n```text\n3 tests passed\nDuration: 78s\n```", outputExcerpt:"3 tests passed", errorMessage:"" }];
const store = useWorkbenchStore();
const route = useRoute();
const menus = ref<TestMenu[]>(isTauriRuntime() ? [] : demoMenus);
const runs = ref<TestRun[]>(isTauriRuntime() ? [] : demoRuns);
const parities = ref<FeatureParity[]>([]);
const activeSection = ref<"menus" | "parity">("menus");
const selectedParity = ref<FeatureParity | null>(null);
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
const activeReportRun = ref<TestRun | null>(null);
const activeReportStatus = ref<"passed" | "failed" | "unknown">("unknown");
const useEnvironmentToken = ref(true);
const account = ref("");
const token = ref("");
const mode = ref<TestRun["mode"]>("mock");
const verificationKinds: RegressionEvidence["verificationType"][] = ["static", "api", "browser"];

function regression(feature: FeatureParity, platform: "PC" | "APP", kind: RegressionEvidence["verificationType"]) {
  return feature.regression.find((item) => item.platform === platform && item.verificationType === kind);
}
function evidenceLabel(status?: RegressionEvidence["status"]) {
  return status === "passed" ? "已通过" : status === "failed" ? "未通过" : "未执行";
}
function parityLabel(status: FeatureParity["parityStatus"]) {
  return ({ pending: "待核对", "static-aligned": "静态一致", confirmed: "人工确认一致", different: "存在差异" })[status];
}

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
  await syncFeatureParity();
  [menus.value, runs.value, parities.value] = await Promise.all([listTestMenus(), listTestRuns(), listFeatureParity()]);
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
  if (!configuring.value || loading.value) return;
  loading.value = true; error.value = ""; message.value = "";
  const options: StartTestOptions = { menuId: configuring.value.id, mode: mode.value, account: account.value.trim() || undefined, token: token.value || undefined, useEnvironmentToken: useEnvironmentToken.value };
  token.value = "";
  try {
    if (!isTauriRuntime()) throw new Error("浏览器预览只能查看界面，请在桌面开发版中运行测试。");
    const run = await startTestRun(options);
    activeReportRun.value = run;
    activeReportStatus.value = run.status;
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
    activeReportRun.value = run || null;
    activeReportStatus.value = run?.status || menu.latestStatus || "unknown";
    if (run) reportContent.value = run.reportMarkdown;
    else if (menu.latestReportPath && isTauriRuntime()) reportContent.value = await readTestReport(menu.latestReportPath);
    else reportContent.value = "当前菜单还没有可查看的测试报告。";
  } catch (cause) { error.value = String(cause); }
}

function openRun(run: TestRun) {
  activeReportRun.value = run;
  activeReportStatus.value = run.status;
  reportTitle.value = `${run.menuName} · ${modeLabel(run.mode)}`;
  reportContent.value = run.reportMarkdown || run.errorMessage || "该次测试没有可显示的报告内容。";
}
function closeReport() { reportContent.value=""; activeReportRun.value=null; activeReportStatus.value="unknown"; }

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
    <header class="page-header"><div><h1>测试中心</h1><p>复用项目已有用例，并将 PC 与 APP 的静态、接口、浏览器证据分层记录</p></div><div><button class="button secondary" :disabled="loading" @click="refresh">↻ 刷新菜单、矩阵与报告</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <div class="testing-tabs"><button :class="{active:activeSection==='menus'}" @click="activeSection='menus'">菜单自动化测试</button><button :class="{active:activeSection==='parity'}" @click="activeSection='parity'">PC / APP 对照矩阵</button></div>
    <section v-if="activeSection==='menus'" class="metric-grid testing-metrics"><article class="clickable-card" @click="statusFilter='all'"><span>菜单 / 页面</span><b>{{ stats.total }}</b><p>点击查看全部</p></article><article class="clickable-card" @click="statusFilter='tested'"><span>已有报告</span><b>{{ stats.tested }}</b><p>点击筛选已测试</p></article><article class="clickable-card" @click="statusFilter='passed'"><span>最近通过</span><b>{{ stats.passed }}</b><p class="success-text">● 点击筛选</p></article><article class="clickable-card" @click="statusFilter='failed'"><span>最近未通过</span><b>{{ stats.failed }}</b><p :class="stats.failed ? 'warning-text' : ''">● 点击筛选</p></article></section>
    <section v-if="activeSection==='menus'" class="panel testing-panel">
      <div class="testing-toolbar"><label>⌕<input v-model="query" placeholder="搜索菜单名称、路由或源码路径"></label><select v-model="projectFilter"><option value="all">全部项目</option><option value="client">client</option><option value="APP">APP</option></select><select v-model="statusFilter"><option value="all">全部状态</option><option value="tested">已测试</option><option value="untested">未测试</option><option value="passed">最近通过</option><option value="failed">最近未通过</option></select><select v-model="typeFilter"><option value="all">全部能力</option><option value="functional">已有功能用例</option><option value="style">页面 / 样式检查</option></select></div>
      <div class="testing-note"><b>执行边界</b><span>当前读取 client {{ projectCounts.client }} 个已有菜单用例、APP {{ projectCounts.app }} 个 pages.json 注册页面。项目目录不存在时列表为空；真实接口测试可能创建、修改和清理 E2E 前缀测试数据，默认不启用。</span></div>
      <div class="test-table-wrap"><table class="test-table"><thead><tr><th>项目</th><th>功能菜单 / 页面</th><th>已有测试能力</th><th>最近结果</th><th>测试报告</th><th>操作</th></tr></thead><tbody><tr v-for="menu in filteredMenus" :key="menu.id"><td><span class="project-badge" :class="menu.project.toLowerCase()">{{ menu.project }}</span></td><td><b>{{ menu.name }}</b><small>{{ menu.route }}</small><em>{{ menu.sourcePath }}</em></td><td><div class="capability-tags"><span v-if="menu.capabilities.mock">功能用例</span><span v-if="menu.capabilities.realApi">真实接口</span><span v-if="menu.capabilities.sourceStyle">源码 / 样式</span><span v-if="menu.capabilities.browserStyle">浏览器样式</span><span v-if="!menu.caseId" class="muted">暂无交互用例</span></div></td><td><span class="test-status" :class="menu.latestStatus || 'untested'">{{ statusLabel(menu) }}</span><small>{{ formatTime(menu.latestTime) }}</small></td><td><button class="text-button" :disabled="!menu.tested" @click="openReport(menu)">{{ menu.tested ? '查看报告' : '暂无报告' }}</button></td><td><button class="button primary small" :disabled="loading" @click="openConfig(menu)">▶ 测试</button></td></tr></tbody></table><div v-if="!filteredMenus.length" class="empty-state"><b>没有符合条件的菜单</b><p>请调整项目、状态、测试能力或关键词筛选。</p></div></div>
    </section>

    <section v-else class="panel parity-panel">
      <div class="testing-note"><b>验证口径</b><span>“静态”只证明页面、API 文件和约定接口存在；“接口”和“浏览器”必须实际执行后才会变为通过。未执行不会显示为失败，也不会冒充已验证。</span></div>
      <div class="parity-grid">
        <article v-for="feature in parities" :key="feature.id" class="parity-card" @click="selectedParity=feature">
          <header><div><small>{{ feature.domain }}</small><h3>{{ feature.featureName }}</h3></div><span :class="feature.parityStatus">{{ parityLabel(feature.parityStatus) }}</span></header>
          <div class="platform-matrix">
            <div><b>PC</b><span v-for="kind in verificationKinds" :key="kind" :class="regression(feature,'PC',kind)?.status"><i></i>{{ kind==='static'?'静态':kind==='api'?'接口':'浏览器' }} · {{ evidenceLabel(regression(feature,'PC',kind)?.status) }}</span></div>
            <div><b>APP</b><span v-for="kind in verificationKinds" :key="kind" :class="regression(feature,'APP',kind)?.status"><i></i>{{ kind==='static'?'静态':kind==='api'?'接口':'浏览器' }} · {{ evidenceLabel(regression(feature,'APP',kind)?.status) }}</span></div>
          </div>
          <footer><span>{{ feature.contracts.length }} 条接口契约</span><button class="text-button">查看证据 →</button></footer>
        </article>
      </div>
    </section>

    <div v-if="selectedParity" class="activity-backdrop" @click.self="selectedParity=null"><section class="panel parity-detail">
      <header><div><small>{{ selectedParity.domain }} · PARITY REVIEW</small><h2>{{ selectedParity.featureName }}</h2><p>人工确认只记录你的判断，不会修改 client 或 APP 项目。</p></div><button class="icon-button" @click="selectedParity=null">×</button></header>
      <div class="parity-paths"><article><b>PC 页面</b><code>{{ selectedParity.pcPage }}</code></article><article><b>APP 页面</b><code>{{ selectedParity.appPage }}</code></article></div>
      <h3>分层验证证据</h3><div class="evidence-list"><article v-for="item in selectedParity.regression" :key="`${item.platform}-${item.verificationType}`"><span class="project-badge" :class="item.platform.toLowerCase()">{{ item.platform }}</span><b>{{ item.verificationType==='static'?'静态检查':item.verificationType==='api'?'接口测试':'浏览器测试' }}</b><span class="evidence-status" :class="item.status">{{ evidenceLabel(item.status) }}</span><p>{{ item.resultSummary }}</p><code>{{ item.sourcePath || '暂无执行来源' }}</code></article></div>
      <h3>接口契约</h3><div class="contract-list"><div v-for="contract in selectedParity.contracts" :key="contract.id"><span>{{ contract.platform }}</span><b>{{ contract.method }}</b><code>{{ contract.url }}</code><em>{{ contract.verificationLevel === 'static' ? '静态读取' : contract.verificationLevel }}</em></div></div>
      <footer class="parity-review"><label>核对结论<select v-model="selectedParity.parityStatus"><option value="static-aligned">静态一致，待实际测试</option><option value="confirmed">人工确认一致</option><option value="different">存在差异</option><option value="pending">待核对</option></select></label><label class="check-row"><input v-model="selectedParity.intentionalDifference" type="checkbox">差异属于有意设计</label><button class="button primary" :disabled="loading" @click="saveParity(selectedParity)">保存人工核对</button></footer>
    </section></div>

    <div v-if="configuring" class="activity-backdrop test-dialog-backdrop" @click.self="!loading && (configuring = null)"><section class="panel test-config-dialog"><header><div><h2>测试 {{ configuring.name }}</h2><p>{{ configuring.project }} · {{ configuring.route }}</p></div><button class="icon-button" :disabled="loading" @click="configuring = null">×</button></header><div class="test-config-body"><label>测试类型<select v-model="mode"><option v-for="value in availableModes(configuring)" :key="value" :value="value">{{ modeLabel(value) }}</option></select></label><div v-if="mode === 'real'" class="real-test-warning"><b>真实接口写入提醒</b><p>现有 client 真实用例会创建、修改或删除 E2E 前缀测试数据，并在结束时尝试清理。只有确认测试环境允许写入时才执行。</p></div><label v-if="mode === 'real'" class="check-row"><input v-model="useEnvironmentToken" type="checkbox">读取 Windows 用户环境变量 HLZT_TOKEN</label><label v-if="mode === 'real' && !useEnvironmentToken">临时 Token<input v-model="token" type="password" autocomplete="off" placeholder="只传给本次测试子进程，不保存"></label><label v-if="mode === 'real'">测试账号（可选）<input v-model="account" autocomplete="off" placeholder="仅传给支持 E2E_TEST_ACCOUNT 的已有用例"><small>当前多数 client 用例使用 Token 登录；账号字段不会写入数据库。</small></label><div class="reuse-source"><b>复用来源</b><code v-if="configuring.caseId">client/e2e/menu-cases/{{ configuring.caseId }}.json</code><code v-else>APP/pages.json + {{ configuring.sourcePath }}</code></div></div><footer><span>{{ loading ? '正在运行项目现有测试，完成后自动打开报告…' : '测试结果和报告保存在个人工作台本地数据库；凭据不保存。' }}</span><button class="button secondary" :disabled="loading" @click="configuring = null">取消</button><button class="button primary" :disabled="loading" @click="runTest">{{ loading ? '测试中…' : '开始测试' }}</button></footer></section></div>

    <div v-if="reportContent" class="activity-backdrop report-dialog-backdrop" @click.self="closeReport"><section class="panel test-report-dialog designed-report"><header><div><small>TEST REPORT</small><h2>{{ reportTitle }}</h2><p>项目现有测试报告 / 工作台静态检查报告</p></div><div class="report-header-actions"><span class="report-result-pill" :class="activeReportStatus">{{ activeReportStatus === 'passed' ? '✓ 测试通过' : activeReportStatus === 'failed' ? '× 测试未通过' : '— 结果未知' }}</span><button class="icon-button" @click="closeReport">×</button></div></header><div class="report-overview"><article><small>执行结果</small><b :class="activeReportStatus">{{ activeReportStatus === 'passed' ? '通过' : activeReportStatus === 'failed' ? '需处理' : '待确认' }}</b><span>{{ activeReportStatus === 'passed' ? '当前用例达到预期' : activeReportStatus === 'failed' ? '请查看失败项和输出' : '报告未包含明确状态' }}</span></article><article><small>测试方式</small><b>{{ activeReportRun ? modeLabel(activeReportRun.mode) : '已有报告' }}</b><span>{{ activeReportRun?.project || '本地项目' }} · {{ activeReportRun ? formatTime(activeReportRun.startedAt) : '历史记录' }}</span></article><article><small>报告结构</small><b>{{ reportStats.sections }} 个分区</b><span>{{ reportStats.passed }} 个通过标记 · {{ reportStats.failed }} 个失败标记</span></article></div><div class="structured-report-body"><article v-for="(section,index) in reportSections" :key="`${section.title}-${index}`" class="report-section-card"><header><i>{{ String(index+1).padStart(2,'0') }}</i><h3>{{ section.title }}</h3></header><div class="report-section-content"><p v-for="(text,textIndex) in section.paragraphs" :key="textIndex">{{ text }}</p><ul v-if="section.bullets.length"><li v-for="item in section.bullets" :key="item"><span :class="{good:/✅|通过|passed/i.test(item),bad:/❌|失败|未通过|failed/i.test(item)}"></span>{{ item }}</li></ul><pre v-if="section.code.length"><code>{{ section.code.join('\n') }}</code></pre></div></article></div></section></div>
  </div>
</template>

<style scoped>
.testing-tabs{display:flex;gap:6px;margin:0 0 16px;padding:5px;width:max-content;border:1px solid var(--border-color);border-radius:12px;background:var(--panel-bg)}
.testing-tabs button{padding:9px 16px;border:0;border-radius:8px;background:transparent;color:var(--text-secondary);cursor:pointer}.testing-tabs button.active{background:var(--primary);color:#fff}
.parity-panel{padding:18px}.parity-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;margin-top:16px}.parity-card{padding:18px;border:1px solid var(--border-color);border-radius:14px;background:var(--panel-bg);cursor:pointer;transition:.18s}.parity-card:hover{transform:translateY(-2px);border-color:var(--primary)}
.parity-card>header,.parity-card>footer,.parity-detail>header{display:flex;align-items:flex-start;justify-content:space-between;gap:16px}.parity-card h3{margin:4px 0 0}.parity-card header>span{padding:5px 9px;border-radius:999px;font-size:12px;background:var(--muted-bg)}.parity-card header>span.static-aligned,.parity-card header>span.confirmed{color:#53c895;background:rgba(83,200,149,.12)}.parity-card header>span.different{color:#f39a62;background:rgba(243,154,98,.12)}
.platform-matrix{display:grid;grid-template-columns:1fr 1fr;gap:14px;margin:18px 0}.platform-matrix>div{display:grid;gap:8px}.platform-matrix span{font-size:12px;color:var(--text-secondary)}.platform-matrix i{display:inline-block;width:7px;height:7px;margin-right:6px;border-radius:50%;background:#7c8293}.platform-matrix .passed i,.evidence-status.passed{color:#53c895}.platform-matrix .passed i{background:#53c895}.platform-matrix .failed i,.evidence-status.failed{color:#ef6d75}.platform-matrix .failed i{background:#ef6d75}.parity-card>footer{padding-top:12px;border-top:1px solid var(--border-color);font-size:12px;color:var(--text-secondary)}
.parity-detail{width:min(1040px,calc(100vw - 80px));max-height:calc(100vh - 70px);padding:24px;overflow:auto}.parity-detail h3{margin:22px 0 10px}.parity-paths{display:grid;grid-template-columns:1fr 1fr;gap:12px;margin-top:20px}.parity-paths article{padding:13px;border:1px solid var(--border-color);border-radius:10px}.parity-paths code,.evidence-list code{display:block;margin-top:7px;white-space:normal;word-break:break-all;color:var(--text-secondary)}
.evidence-list{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}.evidence-list article{display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:8px;padding:12px;border:1px solid var(--border-color);border-radius:10px}.evidence-list p,.evidence-list code{grid-column:1/-1;margin:0}.contract-list{display:grid;gap:6px}.contract-list>div{display:grid;grid-template-columns:48px 64px 1fr 70px;gap:8px;align-items:center;padding:9px 12px;border-radius:8px;background:var(--muted-bg)}.contract-list em{font-size:11px;color:var(--text-secondary)}
.parity-review{display:flex;align-items:end;gap:16px;margin-top:22px;padding-top:18px;border-top:1px solid var(--border-color)}.parity-review label:first-child{display:grid;gap:6px}.parity-review .check-row{margin-right:auto}@media(max-width:1000px){.parity-grid,.evidence-list,.parity-paths{grid-template-columns:1fr}}
</style>
