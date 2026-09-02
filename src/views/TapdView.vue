<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import NavIcon from "../components/NavIcon.vue";
import {
  continueTapdCodexJob, getTapdStatus, isTauriRuntime, listRepositoryAssets,
  listTapdCodexJobs, listTapdItems, readTapdProcessReport, removeTapdProject,
  reviewTapdCodexJob, runTapdCodexJobTests, saveTapdProject, startTapdCodexJob,
  syncTapdItems, type RepositoryAsset, type TapdCodexJob, type TapdProjectConfig,
  type TapdStatus, type TapdWorkItem,
} from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";
import { confirmAction } from "../utils/confirm";

const route = useRoute();
const status = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"", workspaceName:"", owner:"", itemCount:0, warnings:[], autoFixEnabled:false, autoFixRepositoryPath:"", automationPaused:false, projects:[] });
const items = ref<TapdWorkItem[]>([]);
const jobs = ref<TapdCodexJob[]>([]);
const repositories = ref<RepositoryAsset[]>([]);
const selectedWorkspaceId = ref("all");
const selected = ref<TapdWorkItem | null>(null);
const repositoryPath = ref("");
const statusFilter = ref("待处理");
const search = ref("");
const loading = ref(false);
const message = ref("");
const error = ref("");
const reviewNote = ref("");
const codexNote = ref("");
const processReport = ref("");
const reportOpen = ref(false);
const reportLoading = ref(false);
const projectEditorOpen = ref(false);
const projectDraft = ref({ workspaceId:"", workspaceName:"", owner:"", enabled:true, sortOrder:0 });
let jobTimer = 0;

const projects = computed(() => status.value.projects);
const projectMap = computed(() => new Map(projects.value.map((project) => [project.workspaceId, project])));
const selectedProject = computed(() => projectMap.value.get(selectedWorkspaceId.value));
const scopedItems = computed(() => selectedWorkspaceId.value === "all" ? items.value : items.value.filter((item) => item.workspaceId === selectedWorkspaceId.value));
function isCompletedItem(item: TapdWorkItem) {
  const completionStatus=projectMap.value.get(item.workspaceId)?.completionStatus || "已解决";
  return ["resolved","verified","closed"].includes(item.status) || item.status===completionStatus || item.statusLabel===completionStatus;
}
const counts = computed(() => ({
  all: scopedItems.value.length,
  pending: scopedItems.value.filter((item) => ["new","reopened"].includes(item.status)).length,
  processing: scopedItems.value.filter((item) => ["in_progress","progressing"].includes(item.status)).length,
  closed: scopedItems.value.filter(isCompletedItem).length,
}));
const statusOptions = computed(() => ["待处理", ...new Set(scopedItems.value.map((item) => item.statusLabel).filter((value) => value && value !== "待处理"))]);
const filtered = computed(() => scopedItems.value.filter((item) => {
  if (statusFilter.value !== "all" && item.statusLabel !== statusFilter.value) return false;
  const term = search.value.trim().toLowerCase();
  const projectName = projectMap.value.get(item.workspaceId)?.workspaceName || "";
  return !term || `${item.title} ${item.description} ${item.owner} ${projectName}`.toLowerCase().includes(term);
}));
const selectedJob = computed(() => selected.value ? jobs.value.find((job) => job.itemKey === selected.value?.itemKey) : undefined);
const jobsByItem = computed(() => {
  const result = new Map<string, TapdCodexJob>();
  for (const job of jobs.value) if (!result.has(job.itemKey)) result.set(job.itemKey, job);
  return result;
});
const selectedTitle = computed(() => compactDetailTitle(selected.value?.title || "TAPD 缺陷", "TAPD"));
const running = computed(() => jobs.value.some((job) => ["queued","running"].includes(job.status)));
const testPassed = computed(() => selectedJob.value?.testSummary.startsWith("项目测试通过") || false);
const acceptanceBlocked = computed(() => Boolean(selectedJob.value?.testRequired && !testPassed.value));

function formatTime(value?: string) {
  if (!value) return "尚未同步";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("zh-CN", { month:"2-digit", day:"2-digit", hour:"2-digit", minute:"2-digit" }).format(date);
}
function shortPath(value: string) { return value.replace(/^.*[\\/]/, ""); }
function statusTagClass(item: TapdWorkItem) {
  if (isCompletedItem(item)) return "done";
  if (["in_progress","progressing"].includes(item.status)) return "active";
  if (["new","reopened"].includes(item.status)) return "pending";
  return "neutral";
}
function selectProject(workspaceId: string) { selectedWorkspaceId.value = workspaceId; statusFilter.value = "待处理"; }
function editProject(project?: TapdProjectConfig) {
  const nextSortOrder = projects.value.reduce((maximum, item) => Math.max(maximum, item.sortOrder), 0) + 1;
  projectDraft.value = project ? { workspaceId:project.workspaceId, workspaceName:project.workspaceName, owner:project.owner, enabled:project.enabled, sortOrder:project.sortOrder } : { workspaceId:"", workspaceName:"", owner:"", enabled:true, sortOrder:nextSortOrder };
  projectEditorOpen.value = true;
}
async function persistProject() {
  loading.value=true; error.value=""; message.value="";
  try {
    const saved = await saveTapdProject(projectDraft.value);
    projectEditorOpen.value=false; await load(); selectedWorkspaceId.value=saved.workspaceId;
    message.value=`项目“${saved.workspaceName}”已保存，只会同步该负责人名下的缺陷。`;
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function deleteProject(project: TapdProjectConfig) {
  if (!await confirmAction({ title:"移除 TAPD 项目配置", message:`确认移除“${project.workspaceName}”的工作台配置？本地历史缺陷不会删除。`, confirmText:"移除配置", tone:"danger" })) return;
  loading.value=true; error.value="";
  try { await removeTapdProject(project.workspaceId); selectedWorkspaceId.value="all"; projectEditorOpen.value=false; await load(); message.value=`已移除“${project.workspaceName}”配置。`; }
  catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
function openItem(item: TapdWorkItem) {
  selected.value=item; codexNote.value=""; processReport.value=""; reportOpen.value=false;
  const project=projectMap.value.get(item.workspaceId);
  repositoryPath.value=project?.repositoryPath || repositories.value.find((repo) => repo.name.toLowerCase()==="client")?.path || repositories.value[0]?.path || "";
}
async function loadJobs() { if (isTauriRuntime()) jobs.value=await listTapdCodexJobs(); }
function refreshAfterBackgroundSync() { void load(); }
async function load() {
  if (!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    [status.value,items.value,repositories.value,jobs.value]=await Promise.all([getTapdStatus(),listTapdItems(),listRepositoryAssets(),listTapdCodexJobs()]);
    const requestedProject=String(route.query.project || "");
    if (requestedProject && status.value.projects.some((project) => project.workspaceId===requestedProject)) selectedWorkspaceId.value=requestedProject;
    const requested=String(route.query.item || "");
    if (requested) { const item=items.value.find((entry) => entry.id===requested && (!requestedProject || entry.workspaceId===requestedProject)); if (item) { selectedWorkspaceId.value=item.workspaceId; openItem(item); } }
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function sync() {
  loading.value=true; error.value=""; message.value="";
  try {
    const result=await syncTapdItems();
    const warning=result.warnings.length ? `；${result.warnings.join("；")}` : "";
    message.value=`同步完成：${result.projectsSynced} 个项目、${result.bugs} 个缺陷；新增 ${result.notificationsCreated} 条消息，自动队列新增 ${result.autoJobsQueued} 项${warning}`;
    await load(); window.dispatchEvent(new CustomEvent("tapd-items-synced"));
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function sendToCodex() {
  if (!selected.value || !repositoryPath.value) return;
  loading.value=true; error.value=""; message.value="";
  try { const job=await startTapdCodexJob(selected.value.itemKey,repositoryPath.value,codexNote.value); jobs.value.unshift(job); codexNote.value=""; message.value="已发送给 Codex，完成后会在工作台产生未读提醒。"; window.dispatchEvent(new CustomEvent("workbench-active-operations-changed")); }
  catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function runProjectTests() {
  if (!selectedJob.value) return;
  loading.value=true; error.value="";
  try { const updated=await runTapdCodexJobTests(selectedJob.value.id); jobs.value=jobs.value.map((item) => item.id===updated.id ? updated : item); message.value=updated.testSummary.startsWith("项目测试通过") ? "项目测试通过，可以继续确认结果。" : "项目测试未通过，请查看输出。"; }
  catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function showProcessReport() {
  if (!selectedJob.value) return;
  reportOpen.value=true; reportLoading.value=true; error.value="";
  try { processReport.value=await readTapdProcessReport(selectedJob.value.id); }
  catch(cause) { processReport.value=""; error.value=String(cause); } finally { reportLoading.value=false; }
}
async function copyProcessReport() { if (processReport.value) { await navigator.clipboard.writeText(processReport.value); message.value="处理报告已复制。"; } }
async function reviewResult(decision: "accepted" | "changes_requested") {
  if (!selectedJob.value) return;
  if (decision==="changes_requested" && !reviewNote.value.trim()) { error.value="请先填写需要继续修改的说明。"; return; }
  let allowUntested=false;
  if (decision==="accepted") {
    if (acceptanceBlocked.value) { error.value=selectedJob.value.testSummary.startsWith("项目测试失败") ? "项目测试未通过，请先继续修改。" : "请先运行并通过项目测试。"; return; }
    if (!selectedJob.value.testRequired) {
      allowUntested=await confirmAction({ title:"确认未测试风险", message:"该项目没有配置自动测试命令。是否明确接受“未运行自动测试”的风险并继续？", confirmText:"接受并继续", tone:"warning" });
      if (!allowUntested) return;
    }
    const completionStatus=projectMap.value.get(selected.value?.workspaceId || "")?.completionStatus || "已解决";
    if (!await confirmAction({ title:"完成并归档缺陷", message:`确认后会把该 TAPD 缺陷更新为“${completionStatus}”并完成本地归档，是否继续？`, confirmText:"完成并归档", tone:"warning" })) return;
  }
  loading.value=true; error.value="";
  try {
    const updated=decision==="changes_requested" ? await continueTapdCodexJob(selectedJob.value.id,reviewNote.value) : await reviewTapdCodexJob(selectedJob.value.id,decision,reviewNote.value,allowUntested);
    jobs.value=jobs.value.map((item) => item.id===updated.id ? updated : item);
    if (decision==="accepted") { const completionStatus=projectMap.value.get(updated.workspaceId)?.completionStatus || "已解决"; items.value=items.value.map((item) => item.itemKey===updated.itemKey ? {...item,status:completionStatus,statusLabel:completionStatus} : item); selected.value=null; message.value=`TAPD 已更新为“${completionStatus}”，结果已归档。`; }
    else message.value="补充说明已再次发送给原 Codex 任务。";
    reviewNote.value="";
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
onMounted(async () => {
  await load(); if (status.value.configured && !items.value.length) await sync();
  jobTimer=window.setInterval(() => { if (running.value) void loadJobs(); },3000);
  window.addEventListener("tapd-background-synced",refreshAfterBackgroundSync);
});
onBeforeUnmount(() => { window.clearInterval(jobTimer); window.removeEventListener("tapd-background-synced",refreshAfterBackgroundSync); });
</script>

<template>
  <div class="view tapd-view">
    <header class="page-header"><div><h1>TAPD 工作</h1><p>{{ projects.length }} 个项目 · 只同步与展示配置负责人名下的缺陷</p></div><div class="tapd-header-actions"><a v-if="selectedProject" class="button secondary link-button" :href="`https://www.tapd.cn/${selectedProject.workspaceId}`" target="_blank" rel="noreferrer">打开 TAPD</a><button class="button secondary" @click="editProject()">＋ 配置项目</button><button class="button primary" :disabled="loading || !status.configured || !projects.some(project => project.enabled)" @click="sync">{{ loading ? "同步中…" : "↻ 同步缺陷" }}</button></div></header>
    <div v-if="message || error" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <section v-if="!status.configured" class="panel tapd-config-notice"><div><b>还差一步：配置 TAPD OpenAPI</b><p>一个凭据可供多个项目共用，令牌或密码只保存在 Windows 凭据库。</p></div><RouterLink class="button primary link-button" to="/settings">前往设置</RouterLink></section>
    <section class="panel tapd-project-strip"><button :class="{active:selectedWorkspaceId==='all'}" @click="selectProject('all')"><b>全部项目</b><span>{{ items.length }} 个缺陷</span></button><div v-for="project in projects" :key="project.workspaceId" class="tapd-project-tab" :class="{active:selectedWorkspaceId===project.workspaceId,disabled:!project.enabled}"><button class="tapd-project-select" @click="selectProject(project.workspaceId)"><b>{{ project.workspaceName }}</b><span>{{ project.itemCount }} 个缺陷 · {{ project.owner }}</span><i v-if="project.autoEnabled">自动处理</i></button><button class="tapd-project-edit" type="button" title="编辑项目配置" aria-label="编辑项目配置" @click="editProject(project)"><NavIcon name="settings" /></button></div><button class="tapd-project-add" @click="editProject()">＋ 新增项目</button></section>
    <section class="tapd-summary"><article class="panel"><small>已配置项目</small><b>{{ projects.length }}</b><span>{{ projects.filter(project => project.enabled).length }} 个已启用</span></article><article class="panel"><small>当前缺陷</small><b>{{ counts.all }}</b><span>{{ selectedProject?.workspaceName || "全部项目" }}</span></article><article class="panel"><small>待处理 / 处理中 / 已关闭</small><b>{{ counts.pending }} / {{ counts.processing }} / {{ counts.closed }}</b><span>任务和需求不会同步</span></article><article class="panel"><small>最近同步</small><b>{{ formatTime(selectedProject?.lastSyncedAt || status.lastSyncedAt) }}</b><span>{{ status.configured ? status.source : "未配置连接" }}</span></article></section>
    <section class="panel tapd-work-list"><header><div><b>缺陷列表</b><small>共 {{ filtered.length }} 条</small></div><div class="tapd-filters"><select v-model="statusFilter"><option value="all">全部状态</option><option v-for="value in statusOptions" :key="value" :value="value">{{ value }}</option></select><label>⌕<input v-model="search" placeholder="搜索项目、标题、描述或处理人"></label></div></header><div class="tapd-table-head"><span>项目</span><span>缺陷标题</span><span>状态</span><span>优先级</span><span>处理人</span><span>预计结束</span><span>更新时间</span></div><button v-for="item in filtered" :key="item.itemKey" class="tapd-row" @click="openItem(item)"><span><i class="bug">{{ projectMap.get(item.workspaceId)?.workspaceName || item.workspaceId }}</i></span><span><b>{{ item.title }}</b><small>#{{ item.id }}{{ item.description ? ` · ${item.description}` : "" }}<i v-if="jobsByItem.get(item.itemKey)?.processReportPath" class="tapd-report-mark">处理报告</i></small></span><span><em :class="statusTagClass(item)">{{ item.statusLabel }}</em></span><span>{{ item.priority || "-" }}</span><span>{{ item.owner || "-" }}</span><span>{{ item.dueDate || "-" }}</span><span>{{ item.modifiedAt || item.createdAt || "-" }}</span></button><p v-if="!filtered.length && !loading" class="panel-empty">{{ status.configured ? "当前项目和状态下没有缺陷。" : "配置凭据与项目后即可同步缺陷。" }}</p></section>

    <div v-if="projectEditorOpen" class="activity-backdrop" @click.self="projectEditorOpen=false"><aside class="activity-drawer panel tapd-project-editor"><header><div><small>多项目配置</small><h2>{{ projectMap.has(projectDraft.workspaceId) ? "编辑 TAPD 项目" : "新增 TAPD 项目" }}</h2><p>每个项目单独配置负责人，自动规则请到“自动处理”菜单设置。</p></div><button class="icon-button" @click="projectEditorOpen=false">×</button></header><section class="tapd-form"><label>项目 ID<input v-model="projectDraft.workspaceId" :disabled="projectMap.has(projectDraft.workspaceId)" inputmode="numeric" placeholder="例如 37583308"></label><label>项目名称<input v-model="projectDraft.workspaceName" placeholder="用于工作台展示"></label><label>缺陷负责人<input v-model="projectDraft.owner" placeholder="只同步此人负责的缺陷"></label><label>排序<input v-model.number="projectDraft.sortOrder" type="number" min="0" max="9999" step="1" inputmode="numeric"><small>数字越小越靠前，相同数字按项目名称排序。</small></label><label class="tapd-switch"><input v-model="projectDraft.enabled" type="checkbox"><span>启用同步</span></label><div class="settings-actions"><button v-if="projectMap.has(projectDraft.workspaceId)" class="button secondary danger-button" :disabled="loading" @click="deleteProject(projectMap.get(projectDraft.workspaceId)!)">移除项目</button><button class="button primary" :disabled="loading || !projectDraft.workspaceId.trim() || !projectDraft.workspaceName.trim() || !projectDraft.owner.trim() || !Number.isInteger(projectDraft.sortOrder) || projectDraft.sortOrder < 0 || projectDraft.sortOrder > 9999" @click="persistProject">保存项目</button></div></section><section class="tapd-rule-note"><b>同步范围</b><p>只调用 TAPD 缺陷接口；任务、需求及其他工作项不会读取、展示或进入自动队列。</p></section></aside></div>

    <div v-if="selected" class="activity-backdrop" @click.self="selected=null">
      <aside class="activity-drawer panel tapd-drawer">
        <header>
          <div>
            <small>{{ projectMap.get(selected.workspaceId)?.workspaceName || selected.workspaceId }} · 缺陷 #{{ selected.id }}</small>
            <h2 :title="selected.title">{{ selectedTitle }}</h2>
            <p>{{ selected.statusLabel }} · {{ selected.owner || "未指定处理人" }}</p>
          </div>
          <button class="icon-button" @click="selected=null">×</button>
        </header>
        <div class="tapd-detail-meta">
          <span><small>优先级</small><b>{{ selected.priority || "未设置" }}</b></span>
          <span><small>预计开始</small><b>{{ selected.beginDate || "未设置" }}</b></span>
          <span><small>预计结束</small><b>{{ selected.dueDate || "未设置" }}</b></span>
          <span><small>创建人</small><b>{{ selected.creator || "未记录" }}</b></span>
        </div>
        <section>
          <h3>详细描述</h3>
          <p>{{ selected.description || "TAPD 未填写详细描述，请结合标题和项目代码确认。" }}</p>
          <a :href="selected.sourceUrl" target="_blank" rel="noreferrer">在 TAPD 查看原始缺陷 →</a>
        </section>
        <section class="tapd-codex-panel">
          <h3>发送给 Codex</h3>
          <p>Codex 会同时参考缺陷内容和补充备注，不会自动提交或推送。</p>
          <select v-model="repositoryPath">
            <option value="" disabled>选择本地项目</option>
            <option v-for="repo in repositories" :key="repo.path" :value="repo.path">{{ repo.name }} · {{ repo.path }}</option>
          </select>
          <label class="tapd-codex-note">
            <span>补充备注（选填）</span>
            <textarea v-model="codexNote" rows="4" maxlength="4000" placeholder="补充业务背景、修改要求、参考页面或验收标准…"></textarea>
            <small>{{ codexNote.length }} / 4000</small>
          </label>
          <button class="button primary" :disabled="loading || !repositoryPath || ['queued','running'].includes(selectedJob?.status || '')" @click="sendToCodex">{{ ["queued","running"].includes(selectedJob?.status || "") ? "Codex 执行中…" : "发送给 Codex" }}</button>
          <small v-if="repositoryPath">目标：{{ shortPath(repositoryPath) }}</small>
        </section>
        <section v-if="selectedJob" class="tapd-job-result">
          <h3>Codex 结果 <i :class="selectedJob.status">{{ selectedJob.status === "completed" ? "已完成" : selectedJob.status === "failed" ? "失败" : selectedJob.status === "queued" ? "排队中" : "执行中" }}</i></h3>
          <div class="tapd-report-actions">
            <span>{{ selectedJob.triggerSource === "auto" ? "自动处理" : "人工发送" }} · 过程报告保存在本地</span>
            <button class="button secondary small" :disabled="reportLoading || !selectedJob.processReportPath" @click="showProcessReport">{{ reportLoading ? "读取中…" : "查看处理报告" }}</button>
          </div>
          <div v-if="reportOpen" class="tapd-process-report">
            <header><b>处理过程与结果</b><div><button class="button secondary small" :disabled="!processReport" @click="copyProcessReport">复制 Markdown</button><button class="icon-button" @click="reportOpen=false">×</button></div></header>
            <pre>{{ processReport || "报告正在生成，任务完成后重新打开即可查看。" }}</pre>
          </div>
          <pre v-if="selectedJob.output || selectedJob.errorMessage">{{ selectedJob.output || selectedJob.errorMessage }}</pre>
          <p v-else>任务正在后台执行，完成后将生成未读提醒。</p>
          <div v-if="selectedJob.status === 'completed'" class="tapd-closure">
            <div>
              <b>Git 变更证据</b>
              <small>{{ selectedJob.changedFiles.length }} 个文件{{ selectedJob.baselineWorktree ? " · 启动前已有未提交改动" : "" }}</small>
              <code v-for="file in selectedJob.changedFiles.slice(0,12)" :key="file">{{ file }}</code>
              <small v-if="!selectedJob.changedFiles.length">未检测到工作区或提交变化。</small>
            </div>
            <div>
              <b>项目测试</b>
              <pre v-if="selectedJob.testSummary">{{ selectedJob.testSummary }}</pre>
              <button v-else-if="selectedJob.testRequired" class="button secondary" :disabled="loading" @click="runProjectTests">运行项目测试命令</button>
              <small v-else>该项目未配置自动测试命令，确认时需要再次明确风险。</small>
            </div>
            <div>
              <b>人工确认</b>
              <small>需要继续修改会把说明发回原 Codex 任务；确认完成才会回写 TAPD“{{ projectMap.get(selected.workspaceId)?.completionStatus || "已解决" }}”。不会自动提交或推送代码。</small>
              <small v-if="acceptanceBlocked" class="tapd-acceptance-warning">项目测试尚未通过，暂时不能确认完成。</small>
              <textarea v-model="reviewNote" rows="2" maxlength="4000" placeholder="继续修改时必填；确认完成时可填写验收备注"></textarea>
              <div class="settings-actions">
                <button class="button secondary" :disabled="loading || !reviewNote.trim()" @click="reviewResult('changes_requested')">需要继续修改</button>
                <button class="button primary" :disabled="loading || acceptanceBlocked" @click="reviewResult('accepted')">确认完成并归档</button>
              </div>
            </div>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.tapd-view{padding-bottom:24px}.tapd-header-actions,.tapd-filters,.settings-actions{display:flex;align-items:center;gap:8px}.tapd-header-actions{flex-wrap:wrap;justify-content:flex-end}
.tapd-project-strip{display:flex;align-items:stretch;gap:8px;padding:10px;overflow-x:auto;margin-bottom:12px}
.tapd-project-strip>button,.tapd-project-tab{min-width:180px;min-height:62px;text-align:left;border:1px solid var(--line);background:var(--surface-2);color:var(--text);border-radius:9px;padding:10px 12px;display:grid;gap:5px;white-space:nowrap}
.tapd-project-tab{position:relative;padding:0;overflow:hidden}.tapd-project-tab .tapd-project-select{width:100%;height:100%;border:0;background:transparent;color:inherit;text-align:left;padding:10px 46px 10px 12px;display:grid;gap:4px}
.tapd-project-tab .tapd-project-edit{position:absolute;top:8px;right:8px;width:30px;height:30px;border:1px solid transparent;border-radius:7px;background:transparent;color:var(--muted);display:grid;place-items:center;padding:0}.tapd-project-tab .tapd-project-edit :deep(.nav-icon){width:16px;height:16px;flex-basis:16px}.tapd-project-tab .tapd-project-edit:hover{border-color:var(--line);background:var(--surface);color:var(--primary)}
.tapd-project-strip>button.active,.tapd-project-tab.active{border-color:var(--primary);background:var(--primary-soft)}
.tapd-project-tab.disabled{opacity:.55}.tapd-project-strip span{font-size:10px;color:var(--muted)}.tapd-project-strip i{font-size:9px;color:var(--success);font-style:normal}
.tapd-project-strip .tapd-project-add{min-width:130px;place-content:center;text-align:center;color:var(--primary)}
.tapd-table-head,.tapd-row{grid-template-columns:150px minmax(240px,1fr) 82px 70px 92px 92px 130px}
.tapd-work-list>header>div:first-child{display:grid;gap:2px}.tapd-work-list>header small{color:var(--muted)}
.tapd-project-editor{width:520px;overflow-x:hidden;padding-bottom:0}.tapd-project-editor>header{padding:16px 20px}.tapd-project-editor .tapd-form{display:grid!important;grid-template-columns:minmax(0,1fr)!important;gap:14px;padding:18px 20px;border-bottom:1px solid var(--line)}.tapd-form>label{grid-column:1;min-width:0;display:grid;gap:7px;color:var(--muted)}.tapd-form input:not([type=checkbox]){width:100%;height:40px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:0 11px}.tapd-form .tapd-switch{min-height:42px;display:flex;align-items:center;gap:8px;padding:0 12px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text)}.tapd-form .settings-actions{grid-column:1;justify-content:flex-end;padding-top:2px}
.tapd-rule-note{margin:16px 20px 20px;padding:14px 15px;border:1px solid var(--line);background:var(--surface-2);border-radius:9px}.tapd-rule-note p{margin:6px 0 0;color:var(--muted);line-height:1.6}
.tapd-acceptance-warning{color:var(--danger)!important}
@media(max-width:900px){.tapd-summary{grid-template-columns:repeat(2,minmax(0,1fr))}.tapd-work-list{overflow-x:auto}.tapd-project-editor{width:min(520px,100vw)}}
</style>
