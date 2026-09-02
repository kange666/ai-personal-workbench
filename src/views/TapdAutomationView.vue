<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import {
  executeTapdCodexJob, getTapdStatus, isTauriRuntime, listRepositoryAssets,
  listTapdCodexJobs, listTapdItems, previewTapdProjectAutomation,
  saveTapdProjectAutomation, setTapdAutomationPaused, syncTapdItems,
  type RepositoryAsset, type TapdCodexJob, type TapdProjectAutomationInput,
  type TapdAutomationPreview, type TapdProjectConfig, type TapdStatus, type TapdWorkItem,
} from "../services/backend";
import { confirmAction } from "../utils/confirm";

const status = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"", workspaceName:"", owner:"", itemCount:0, warnings:[], autoFixEnabled:false, autoFixRepositoryPath:"", automationPaused:false, projects:[] });
const items = ref<TapdWorkItem[]>([]);
const jobs = ref<TapdCodexJob[]>([]);
const repositories = ref<RepositoryAsset[]>([]);
const drafts = reactive<Record<string, TapdProjectAutomationInput>>({});
const previews = reactive<Record<string, TapdAutomationPreview | undefined>>({});
const workspaceFilter = ref("all");
const queueFilter = ref("active");
const loading = ref(false);
const message = ref("");
const error = ref("");
const rulesOpen = ref(false);
let timer = 0;

const projects = computed(() => status.value.projects.filter((project) => project.enabled));
const itemMap = computed(() => new Map(items.value.map((item) => [item.itemKey, item])));
const projectMap = computed(() => new Map(status.value.projects.map((project) => [project.workspaceId, project])));
const autoJobs = computed(() => jobs.value.filter((job) => job.triggerSource === "auto"));
const queueCounts = computed(() => ({
  queued:autoJobs.value.filter((job) => job.status === "queued").length,
  running:autoJobs.value.filter((job) => job.status === "running").length,
  review:autoJobs.value.filter((job) => job.status === "completed" && job.reviewStatus === "pending").length,
  failed:autoJobs.value.filter((job) => job.status === "failed").length,
  archived:autoJobs.value.filter((job) => job.reviewStatus === "accepted").length,
}));
const visibleJobs = computed(() => autoJobs.value.filter((job) => {
  if (workspaceFilter.value !== "all" && job.workspaceId !== workspaceFilter.value) return false;
  if (queueFilter.value === "active") return ["queued","running"].includes(job.status);
  if (queueFilter.value === "review") return job.status === "completed" && job.reviewStatus === "pending";
  if (queueFilter.value === "failed") return job.status === "failed";
  if (queueFilter.value === "archived") return job.reviewStatus === "accepted";
  return true;
}));
const hasRunning = computed(() => autoJobs.value.some((job) => ["queued","running"].includes(job.status)));

function applyDraft(project: TapdProjectConfig, force=false) {
  if (!force && rulesOpen.value && drafts[project.workspaceId]) return;
  drafts[project.workspaceId] = {
    workspaceId:project.workspaceId,
    repositoryPath:project.repositoryPath,
    autoEnabled:project.autoEnabled,
    autoExecute:project.autoExecute,
    triggerStatuses:[...project.triggerStatuses],
    completionStatus:project.completionStatus,
  };
}
function formatTime(value?: string) {
  if (!value) return "-";
  const date=new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"}).format(date);
}
function jobLabel(job: TapdCodexJob) {
  if (job.status === "queued" && job.executionMode === "manual") return "待手工开始";
  if (job.status === "queued") return "排队中";
  if (job.status === "running") return "执行中";
  if (job.status === "failed") return "失败";
  if (job.reviewStatus === "accepted") return "已归档";
  return "待人工确认";
}
function queueDuration(job: TapdCodexJob) {
  const start=new Date(job.startedAt || job.createdAt).getTime();
  const end=new Date(job.completedAt || job.updatedAt).getTime();
  if (!Number.isFinite(start) || !Number.isFinite(end)) return "-";
  const minutes=Math.max(0,Math.round((end-start)/60000));
  return minutes<60 ? `${minutes} 分钟` : `${Math.floor(minutes/60)} 小时 ${minutes%60} 分钟`;
}
async function load() {
  if (!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    [status.value,items.value,jobs.value,repositories.value]=await Promise.all([getTapdStatus(),listTapdItems(),listTapdCodexJobs(),listRepositoryAssets()]);
    for (const project of status.value.projects) applyDraft(project);
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function saveRule(project: TapdProjectConfig) {
  const draft=drafts[project.workspaceId];
  if (!draft) return;
  loading.value=true; error.value=""; message.value="";
  try {
    const preview=await previewTapdProjectAutomation(draft);
    previews[project.workspaceId]=preview;
    if (draft.autoEnabled && preview.pendingCount>0 && !await confirmAction({ title:"启用自动处理规则", message:`当前规则会让 ${preview.pendingCount} 条现有缺陷在下次同步时进入队列。确认保存吗？`, confirmText:"保存规则", tone:"warning" })) return;
    const saved=await saveTapdProjectAutomation(draft);
    applyDraft(saved,true);
    status.value.projects=status.value.projects.map((entry) => entry.workspaceId===saved.workspaceId ? saved : entry);
    message.value=`“${saved.workspaceName}”自动处理规则已保存。`;
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function previewRule(project: TapdProjectConfig) {
  const draft=drafts[project.workspaceId];
  if (!draft) return;
  loading.value=true; error.value="";
  try { previews[project.workspaceId]=await previewTapdProjectAutomation(draft); }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function togglePause() {
  const paused=!status.value.automationPaused;
  loading.value=true; error.value=""; message.value="";
  try {
    await setTapdAutomationPaused(paused);
    status.value.automationPaused=paused;
    message.value=paused ? "自动处理已暂停：继续同步缺陷，但不会创建或执行新的自动任务。" : "自动处理已恢复，下次同步会重新评估暂停期间的缺陷变更。";
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function execute(job: TapdCodexJob) {
  loading.value=true; error.value=""; message.value="";
  try {
    const updated=await executeTapdCodexJob(job.id);
    jobs.value=jobs.value.map((entry) => entry.id===updated.id ? updated : entry);
    message.value="任务已进入串行执行队列。";
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
async function syncAndQueue() {
  loading.value=true; error.value=""; message.value="";
  try {
    const result=await syncTapdItems();
    message.value=status.value.automationPaused ? `已同步 ${result.projectsSynced} 个项目、${result.bugs} 个缺陷；自动处理处于暂停状态，未创建新任务。` : `已同步 ${result.projectsSynced} 个项目、${result.bugs} 个缺陷，新进入自动队列 ${result.autoJobsQueued} 项。`;
    await load();
    window.dispatchEvent(new CustomEvent("tapd-items-synced"));
  } catch(cause) { error.value=String(cause); } finally { loading.value=false; }
}
onMounted(async () => {
  await load();
  timer=window.setInterval(() => { if (hasRunning.value) void load(); },3000);
});
onBeforeUnmount(() => window.clearInterval(timer));
</script>

<template>
  <div class="view tapd-automation-view">
    <header class="page-header">
      <div><h1>自动处理</h1><p>查看自动队列、执行过程与人工确认状态</p></div>
      <div class="automation-actions">
        <RouterLink class="button secondary link-button" to="/tapd">查看缺陷</RouterLink>
        <button class="button secondary" @click="rulesOpen=true">配置规则</button>
        <button class="button secondary" :class="{ 'pause-active':status.automationPaused }" :disabled="loading" @click="togglePause">{{ status.automationPaused ? "恢复自动处理" : "暂停自动处理" }}</button>
        <button class="button primary" :disabled="loading || !status.configured" @click="syncAndQueue">{{ loading ? "处理中…" : "↻ 同步并检查队列" }}</button>
      </div>
    </header>
    <div v-if="message || error" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <div v-if="status.automationPaused" class="automation-paused"><b>自动处理已暂停</b><span>缺陷仍会同步，但不会创建或执行新的自动任务；恢复后会重新评估暂停期间的变更。</span></div>
    <section class="automation-summary">
      <article class="panel"><small>排队</small><b>{{ queueCounts.queued }}</b><span>含等待手工开始</span></article>
      <article class="panel"><small>执行中</small><b>{{ queueCounts.running }}</b><span>同一时间只执行一项</span></article>
      <article class="panel"><small>待确认</small><b>{{ queueCounts.review }}</b><span>验收通过后才回写 TAPD</span></article>
      <article class="panel"><small>失败</small><b>{{ queueCounts.failed }}</b><span>查看原因后可重新执行</span></article>
    </section>

    <section class="panel queue-section">
      <header><div><b>自动处理队列</b><p>展示触发原因、优先级、截止日期、执行方式和耗时，便于判断下一项工作。</p></div><div class="queue-filters"><select v-model="workspaceFilter"><option value="all">全部项目</option><option v-for="project in projects" :key="project.workspaceId" :value="project.workspaceId">{{ project.workspaceName }}</option></select><select v-model="queueFilter"><option value="active">排队与执行中</option><option value="review">待人工确认</option><option value="failed">执行失败</option><option value="archived">已归档</option><option value="all">全部记录</option></select></div></header>
      <div class="queue-head"><span>项目 / 缺陷</span><span>优先级 / 截止</span><span>触发原因</span><span>执行方式</span><span>状态 / 耗时</span><span>操作</span></div>
      <div v-for="job in visibleJobs" :key="job.id" class="queue-row">
        <span><b>{{ projectMap.get(job.workspaceId)?.workspaceName || "未知项目" }}</b><small>{{ itemMap.get(job.itemKey)?.title || `缺陷 #${job.itemId}` }}</small><small>#{{ job.itemId }} · {{ job.repositoryPath.replace(/^.*[\\/]/, "") }}</small></span>
        <span><b>{{ itemMap.get(job.itemKey)?.priority || "未设置" }}</b><small :class="{ overdue:Boolean(itemMap.get(job.itemKey)?.dueDate && itemMap.get(job.itemKey)!.dueDate < new Date().toISOString().slice(0,10)) }">{{ itemMap.get(job.itemKey)?.dueDate || "未设置截止日期" }}</small></span>
        <span><b>{{ job.triggerReason || "自动规则" }}</b><small>{{ formatTime(job.sourceModifiedAt) }}</small></span>
        <span><b>{{ job.executionMode === "automatic" ? "自动执行" : "手工开始" }}</b><small v-if="job.executionBlockReason" :title="job.executionBlockReason">{{ job.executionBlockReason }}</small></span>
        <span><em :class="job.status">{{ jobLabel(job) }}</em><small>{{ queueDuration(job) }}</small><small v-if="job.errorMessage" class="queue-error">{{ job.errorMessage }}</small></span>
        <span><button v-if="['queued','failed'].includes(job.status)" class="button secondary small" :disabled="loading || status.automationPaused" @click="execute(job)">{{ job.status === "failed" ? "重新执行" : "开始执行" }}</button><RouterLink v-else-if="job.status==='completed'" class="button secondary small link-button" :to="{path:'/tapd',query:{project:job.workspaceId,item:job.itemId}}">查看与确认</RouterLink><i v-else>执行中</i></span>
      </div>
      <p v-if="!visibleJobs.length" class="panel-empty">当前筛选条件下没有自动处理记录。</p>
    </section>

    <div v-if="rulesOpen" class="activity-backdrop" @click.self="rulesOpen=false">
      <aside class="activity-drawer panel automation-rule-drawer">
        <header><div><small>自动处理</small><h2>配置规则</h2><p>保存前会预览当前命中数量；项目存在未提交修改时自动降级为手工开始。</p></div><button class="icon-button" @click="rulesOpen=false">×</button></header>
        <section class="rule-drawer-body">
          <div class="rule-drawer-title"><b>项目规则</b><RouterLink class="text-link" to="/tapd">管理项目 →</RouterLink></div>
          <div v-if="!projects.length" class="panel-empty">请先在 TAPD 工作中添加并启用项目。</div>
          <article v-for="project in projects" :key="project.workspaceId" class="project-rule">
            <header><div><b>{{ project.workspaceName }}</b><span>{{ project.workspaceId }} · 负责人 {{ project.owner }}</span></div><em :class="drafts[project.workspaceId]?.autoEnabled ? 'enabled' : ''">{{ drafts[project.workspaceId]?.autoEnabled ? "已启用" : "未启用" }}</em></header>
            <div v-if="drafts[project.workspaceId]" class="rule-grid">
              <label>对应本地项目<select v-model="drafts[project.workspaceId].repositoryPath"><option value="">请选择</option><option v-for="repo in repositories" :key="repo.path" :value="repo.path">{{ repo.name }} · {{ repo.path }}</option></select></label>
              <label>执行方式<select v-model="drafts[project.workspaceId].autoExecute"><option :value="true">工作区干净时自动执行</option><option :value="false">只进入队列，手工开始</option></select></label>
              <fieldset><legend>触发状态</legend><label><input v-model="drafts[project.workspaceId].triggerStatuses" type="checkbox" value="new"> 待处理</label><label><input v-model="drafts[project.workspaceId].triggerStatuses" type="checkbox" value="reopened"> 重新打开</label></fieldset>
              <label>人工确认后流转到<input v-model="drafts[project.workspaceId].completionStatus" maxlength="80" placeholder="例如：已解决 / 待验证"></label>
              <label class="rule-toggle"><input v-model="drafts[project.workspaceId].autoEnabled" type="checkbox"><span>启用该项目的自动处理</span></label>
            </div>
            <div v-if="previews[project.workspaceId]" class="rule-preview"><b>规则预览</b><span>本地 {{ previews[project.workspaceId]!.totalItems }} 条缺陷，当前状态命中 {{ previews[project.workspaceId]!.matchedCount }} 条；下次同步预计新增 {{ previews[project.workspaceId]!.pendingCount }} 个队列任务。</span><ul v-if="previews[project.workspaceId]!.items.length"><li v-for="item in previews[project.workspaceId]!.items" :key="item.itemKey"><b>{{ item.title }}</b><small>#{{ item.itemId }} · {{ item.priority || "未设置优先级" }} · {{ item.dueDate || "无截止日期" }} · {{ item.triggerReason }}</small></li></ul></div>
            <footer><small v-if="project.lastError" class="rule-error">{{ project.lastError }}</small><span v-else>最近同步：{{ formatTime(project.lastSyncedAt) }}</span><div><button class="button secondary" :disabled="loading" @click="previewRule(project)">预览命中</button><button class="button primary" :disabled="loading || !drafts[project.workspaceId]?.completionStatus.trim()" @click="saveRule(project)">保存规则</button></div></footer>
          </article>
        </section>
        <section class="safety-rules"><b>固定安全规则</b><ol><li><strong>只处理缺陷</strong><span>不读取任务、需求和其他信息。</span></li><li><strong>只处理负责人范围</strong><span>每个项目按配置的负责人筛选。</span></li><li><strong>串行执行</strong><span>避免多个 Codex 同时修改同一工作区。</span></li><li><strong>脏工作区降级</strong><span>检测到已有修改时只入队，不自动执行。</span></li><li><strong>测试门槛</strong><span>配置测试命令的项目必须测试通过才可确认。</span></li><li><strong>不操作 Git 发布</strong><span>不会自动提交、推送、重置、清理或删除。</span></li></ol></section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.tapd-automation-view{padding-bottom:24px}.automation-actions,.queue-filters{display:flex;align-items:center;gap:8px}.automation-actions{flex-wrap:wrap;justify-content:flex-end}.automation-actions .pause-active{color:var(--warning);border-color:var(--warning)}.automation-paused{display:flex;align-items:center;gap:12px;margin-bottom:12px;padding:11px 14px;border:1px solid color-mix(in srgb,var(--warning) 42%,var(--line));border-radius:9px;background:color-mix(in srgb,var(--warning) 10%,var(--surface));color:var(--warning)}.automation-paused span{color:var(--muted)}.automation-summary{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-bottom:12px}.automation-summary article{min-height:96px;padding:14px 16px;display:flex;flex-direction:column;justify-content:center;gap:7px}.automation-summary b{font-size:26px;line-height:1}.automation-summary span,.automation-summary small{color:var(--muted)}
.queue-section{overflow:hidden}.queue-section>header{min-height:62px;padding:12px 16px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:14px}.queue-section>header p{margin:5px 0 0;color:var(--muted);line-height:1.5}
.automation-rule-drawer{width:min(680px,100vw);padding-bottom:0}.rule-drawer-body{padding:16px;display:grid;gap:12px}.rule-drawer-title,.project-rule>header,.project-rule>footer{display:flex;align-items:center;justify-content:space-between;gap:14px}.project-rule{min-width:0;padding:16px;border:1px solid var(--line);border-radius:10px;background:var(--surface)}.project-rule header>div{display:grid;gap:4px}.project-rule header span,.project-rule footer span{color:var(--muted);font-size:10px}.project-rule header em{font-style:normal;font-size:10px;padding:5px 8px;border-radius:999px;background:var(--surface-2);color:var(--muted)}.project-rule header em.enabled{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.project-rule footer{min-height:36px}.project-rule footer>div{display:flex;gap:8px;margin-left:auto}
.rule-grid{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:12px;margin:15px 0}.rule-grid>label,.rule-grid fieldset{min-width:0}.rule-grid>label{display:grid;gap:7px;color:var(--muted)}.rule-grid select,.rule-grid input:not([type=checkbox]),.queue-filters select{width:100%;min-width:0;height:38px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:0 10px}.rule-grid fieldset{min-height:58px;border:1px solid var(--line);border-radius:9px;display:flex;align-items:center;gap:16px;padding:8px 12px}.rule-grid fieldset label{display:flex;align-items:center;gap:6px}.rule-grid legend{padding:0 6px;color:var(--muted)}.rule-grid .rule-toggle{min-height:58px;display:flex;align-items:center;gap:8px;padding:0 12px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--text)}.rule-error{color:var(--danger)}.rule-preview{margin:4px 0 14px;padding:12px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);display:grid;gap:7px}.rule-preview>span{color:var(--muted);line-height:1.55}.rule-preview ul{margin:2px 0 0;padding:0;list-style:none;display:grid;gap:6px}.rule-preview li{display:grid;gap:3px;padding-top:7px;border-top:1px solid var(--line)}.rule-preview small{color:var(--muted)}
.queue-filters{flex:0 0 auto}.queue-filters select{width:auto;max-width:180px}.queue-head,.queue-row{display:grid;grid-template-columns:minmax(210px,1.55fr) 105px minmax(110px,.8fr) minmax(120px,.85fr) 120px 92px;gap:12px;align-items:center;padding-left:16px;padding-right:16px}.queue-head{height:40px;color:var(--muted);font-size:10px;border-bottom:1px solid var(--line);background:var(--surface-2)}.queue-row{min-height:78px;border-bottom:1px solid var(--line)}.queue-row>span{min-width:0;display:grid;gap:4px}.queue-row small{display:block;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.queue-row small.overdue,.queue-row .queue-error{color:var(--danger)}.queue-row em{display:inline-flex;width:max-content;font-style:normal;border-radius:999px;padding:4px 8px;background:var(--surface-2)}.queue-row em.running{color:var(--primary)}.queue-row em.failed{color:var(--danger)}.queue-row em.completed{color:var(--success)}.queue-section>.panel-empty{margin:0;padding:42px 16px}
.safety-rules{padding:18px 20px 22px;border-top:1px solid var(--line);background:var(--surface-2)}.safety-rules ol{counter-reset:rule;list-style:none;padding:0;margin:14px 0 0;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px}.safety-rules li{counter-increment:rule;display:grid;grid-template-columns:22px 1fr;column-gap:9px}.safety-rules li::before{content:counter(rule);width:22px;height:22px;display:grid;place-items:center;border-radius:7px;background:var(--primary-soft);color:var(--primary);font-size:10px}.safety-rules strong,.safety-rules span{grid-column:2}.safety-rules span{color:var(--muted);font-size:10px;line-height:1.55;margin-top:3px}
@media(max-width:1180px){.automation-summary{grid-template-columns:repeat(2,1fr)}}
@media(max-width:640px){.rule-grid,.safety-rules ol{grid-template-columns:1fr}.queue-section{overflow-x:auto}}
</style>
