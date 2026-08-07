<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import {
  getTapdStatus,
  isTauriRuntime,
  listRepositoryAssets,
  listTapdCodexJobs,
  listTapdItems,
  startTapdCodexJob,
  syncTapdItems,
  type RepositoryAsset,
  type TapdCodexJob,
  type TapdStatus,
  type TapdWorkItem,
} from "../services/backend";

const route = useRoute();
const status = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"37583308", workspaceName:"安全生产管理", owner:"刘子世康", itemCount:0, warnings:[] });
const items = ref<TapdWorkItem[]>([]);
const jobs = ref<TapdCodexJob[]>([]);
const repositories = ref<RepositoryAsset[]>([]);
const selected = ref<TapdWorkItem | null>(null);
const repositoryPath = ref("");
const typeFilter = ref("all");
const statusFilter = ref("all");
const search = ref("");
const loading = ref(false);
const message = ref("");
const error = ref("");
let jobTimer = 0;

const typeLabels:Record<string,string> = { bug:"缺陷", task:"任务", story:"需求" };
const counts = computed(() => ({
  all:items.value.length,
  bug:items.value.filter(item=>item.itemType==="bug").length,
  task:items.value.filter(item=>item.itemType==="task").length,
  story:items.value.filter(item=>item.itemType==="story").length,
}));
const statuses = computed(() => [...new Set(items.value.map(item=>item.statusLabel).filter(Boolean))]);
const filtered = computed(() => items.value.filter(item => {
  if (typeFilter.value !== "all" && item.itemType !== typeFilter.value) return false;
  if (statusFilter.value !== "all" && item.statusLabel !== statusFilter.value) return false;
  const term = search.value.trim().toLowerCase();
  return !term || `${item.title} ${item.description} ${item.owner}`.toLowerCase().includes(term);
}));
const selectedJob = computed(() => selected.value ? jobs.value.find(job=>job.itemId===selected.value?.id) : undefined);
const running = computed(() => jobs.value.some(job=>job.status==="running"));

function formatTime(value?:string) {
  if (!value) return "尚未同步";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"}).format(date);
}
function shortPath(value:string) { return value.replace(/^.*[\\/]/,""); }
function openItem(item:TapdWorkItem) {
  selected.value=item;
  if (!repositoryPath.value) {
    repositoryPath.value=repositories.value.find(repo=>repo.name.toLowerCase()==="client")?.path
      || repositories.value.find(repo=>repo.name.toLowerCase()==="app")?.path
      || repositories.value[0]?.path || "";
  }
}
async function loadJobs() { if (isTauriRuntime()) jobs.value=await listTapdCodexJobs(); }
async function load() {
  if (!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    [status.value,items.value,repositories.value,jobs.value]=await Promise.all([getTapdStatus(),listTapdItems(),listRepositoryAssets(),listTapdCodexJobs()]);
    const requested=String(route.query.item||"");
    if (requested) selected.value=items.value.find(item=>item.id===requested)||null;
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function sync() {
  loading.value=true; error.value=""; message.value="";
  try {
    const result=await syncTapdItems();
    const warning=result.warnings.length ? ` 当前令牌权限不完整：${result.warnings.join("；")}。` : "";
    message.value=`同步完成：${result.bugs} 个缺陷、${result.tasks} 个任务、${result.stories} 个需求。${warning}`;
    await load();
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function sendToCodex() {
  if (!selected.value || !repositoryPath.value) return;
  loading.value=true; error.value=""; message.value="";
  try {
    const job=await startTapdCodexJob(selected.value.id,repositoryPath.value);
    jobs.value.unshift(job);
    message.value="已发送给 Codex，完成后会在工作台产生未读提醒。";
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
onMounted(async()=>{
  await load();
  if (status.value.configured && !items.value.length) await sync();
  jobTimer=window.setInterval(()=>{ if (running.value) void loadJobs(); },3000);
});
onBeforeUnmount(()=>window.clearInterval(jobTimer));
</script>

<template>
  <div class="view tapd-view">
    <header class="page-header"><div><h1>TAPD 工作</h1><p>安全生产管理 · 只读同步“我的工作”，选择工作项后可发送给 Codex</p></div><div><a class="button secondary link-button" href="https://www.tapd.cn/37583308" target="_blank" rel="noreferrer">打开 TAPD</a><button class="button primary" :disabled="loading || !status.configured" @click="sync">{{ loading ? '同步中…' : '↻ 同步项目' }}</button></div></header>
    <div v-if="message || error" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <section v-if="!status.configured" class="panel tapd-config-notice"><div><b>还差一步：配置 TAPD OpenAPI</b><p>浏览器登录只能用于查看页面，工作台持续同步需要 TAPD 提供的 API 用户名和密码。凭据只保存到 Windows 凭据库。</p></div><RouterLink class="button primary link-button" to="/settings">前往设置</RouterLink></section>
    <section v-else-if="status.warnings.length" class="panel tapd-permission-warning"><div><b>当前令牌权限不完整，已授权的数据仍会正常显示</b><p>{{ status.warnings.join('；') }}。请在 TAPD 个人访问令牌中补充上面缺少的读取权限后，再点击“同步项目”。</p></div><a class="button secondary link-button" href="https://open.tapd.cn/document/api-doc/API%E6%96%87%E6%A1%A3/%E6%8E%88%E6%9D%83%E5%87%AD%E8%AF%81/scopes.html" target="_blank" rel="noreferrer">查看权限说明</a></section>
    <section class="tapd-summary">
      <article class="panel"><small>项目</small><b>{{ status.workspaceName }}</b><span>{{ status.workspaceId }}</span></article>
      <article class="panel"><small>我的工作</small><b>{{ counts.all }}</b><span>负责人：{{ status.owner }}</span></article>
      <article class="panel"><small>缺陷 / 任务 / 需求</small><b>{{ counts.bug }} / {{ counts.task }} / {{ counts.story }}</b><span>本地只读缓存</span></article>
      <article class="panel"><small>最近同步</small><b>{{ formatTime(status.lastSyncedAt) }}</b><span>{{ status.configured ? status.source : '未配置连接' }}</span></article>
    </section>
    <section class="panel tapd-work-list">
      <header><div class="tapd-tabs"><button v-for="tab in [{id:'all',label:'全部'},{id:'bug',label:'缺陷'},{id:'task',label:'任务'},{id:'story',label:'需求'}]" :key="tab.id" :class="{active:typeFilter===tab.id}" @click="typeFilter=tab.id">{{ tab.label }} <i>{{ counts[tab.id as keyof typeof counts] }}</i></button></div><div class="tapd-filters"><select v-model="statusFilter"><option value="all">全部状态</option><option v-for="value in statuses" :key="value" :value="value">{{ value }}</option></select><label>⌕<input v-model="search" placeholder="搜索标题、描述或处理人"></label></div></header>
      <div class="tapd-table-head"><span>类型</span><span>标题</span><span>状态</span><span>优先级</span><span>处理人</span><span>预计结束</span><span>更新时间</span></div>
      <button v-for="item in filtered" :key="item.id" class="tapd-row" @click="openItem(item)"><span><i :class="item.itemType">{{ typeLabels[item.itemType] }}</i></span><span><b>{{ item.title }}</b><small>#{{ item.id }}{{ item.description ? ` · ${item.description}` : '' }}</small></span><span><em>{{ item.statusLabel }}</em></span><span>{{ item.priority || '-' }}</span><span>{{ item.owner || '-' }}</span><span>{{ item.dueDate || '-' }}</span><span>{{ item.modifiedAt || item.createdAt || '-' }}</span></button>
      <p v-if="!filtered.length && !loading" class="panel-empty">{{ status.configured ? '当前筛选条件下没有工作项。' : '配置 OpenAPI 后即可同步安全生产管理项目。' }}</p>
    </section>
    <div v-if="selected" class="activity-backdrop" @click.self="selected=null"><aside class="activity-drawer panel tapd-drawer"><header><div><small>{{ typeLabels[selected.itemType] }} · #{{ selected.id }}</small><h2>{{ selected.title }}</h2><p>{{ selected.statusLabel }} · {{ selected.owner || '未指定处理人' }}</p></div><button class="icon-button" @click="selected=null">×</button></header><div class="tapd-detail-meta"><span><small>优先级</small><b>{{ selected.priority || '未设置' }}</b></span><span><small>预计开始</small><b>{{ selected.beginDate || '未设置' }}</b></span><span><small>预计结束</small><b>{{ selected.dueDate || '未设置' }}</b></span><span><small>创建人</small><b>{{ selected.creator || '未记录' }}</b></span></div><section><h3>详细描述</h3><p>{{ selected.description || 'TAPD 未填写详细描述，请结合标题和项目代码确认。' }}</p><a :href="selected.sourceUrl" target="_blank" rel="noreferrer">在 TAPD 查看原始工作项 →</a></section><section class="tapd-codex-panel"><h3>发送给 Codex</h3><p>Codex 会在所选项目中检查并实现，不会自动提交或推送。</p><select v-model="repositoryPath"><option value="" disabled>选择本地项目</option><option v-for="repo in repositories" :key="repo.path" :value="repo.path">{{ repo.name }} · {{ repo.path }}</option></select><button class="button primary" :disabled="loading || !repositoryPath || selectedJob?.status==='running'" @click="sendToCodex">{{ selectedJob?.status==='running' ? 'Codex 执行中…' : '发送给 Codex' }}</button><small v-if="repositoryPath">目标：{{ shortPath(repositoryPath) }}</small></section><section v-if="selectedJob" class="tapd-job-result"><h3>Codex 结果 <i :class="selectedJob.status">{{ selectedJob.status==='completed'?'已完成':selectedJob.status==='failed'?'失败':'执行中' }}</i></h3><pre v-if="selectedJob.output || selectedJob.errorMessage">{{ selectedJob.output || selectedJob.errorMessage }}</pre><p v-else>任务正在后台执行，完成后将生成未读提醒。</p></section></aside></div>
  </div>
</template>
