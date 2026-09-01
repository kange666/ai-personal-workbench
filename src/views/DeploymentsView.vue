<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRoute } from "vue-router";
import {
  getJenkinsConnectionStatus,
  isTauriRuntime,
  listJenkinsJobBranches,
  listJenkinsJobs,
  listJenkinsPublishRecords,
  openJenkinsUrl,
  saveJenkinsConnection,
  setJenkinsJobFavorite,
  testJenkinsConnection,
  triggerJenkinsPublish,
  type JenkinsBranchOptions,
  type JenkinsConnectionStatus,
  type JenkinsJob,
  type JenkinsPipelineStage,
  type JenkinsPublishRecord,
} from "../services/backend";

const route = useRoute();
const connection = ref<JenkinsConnectionStatus>({ configured:false, baseUrl:"", username:"", version:"", lastVerifiedAt:"" });
const jobs = ref<JenkinsJob[]>([]);
const records = ref<JenkinsPublishRecord[]>([]);
const selectedJobName = ref("");
const selectedBranch = ref("");
const branchOptions = ref<JenkinsBranchOptions | null>(null);
const projectMenuOpen = ref(false);
const projectQuery = ref("");
const configuring = ref(false);
const configBaseUrl = ref("");
const configUsername = ref("");
const configToken = ref("");
const loadingJobs = ref(false);
const loadingBranches = ref(false);
const publishing = ref(false);
const configBusy = ref(false);
const message = ref("");
const error = ref("");
const nowTick = ref(Date.now());
let timer = 0;
let eventUnlisten: UnlistenFn | undefined;

const selectedJob = computed(() => jobs.value.find(item => item.fullName === selectedJobName.value) || null);
const filteredJobs = computed(() => {
  const keyword = projectQuery.value.trim().toLocaleLowerCase();
  return jobs.value
    .filter(item => !keyword || `${item.name} ${item.fullName}`.toLocaleLowerCase().includes(keyword))
    .sort((left,right) => Number(right.favorite)-Number(left.favorite) || left.fullName.localeCompare(right.fullName,"zh-CN"));
});
const favoriteJobs = computed(() => filteredJobs.value.filter(item => item.favorite));
const normalJobs = computed(() => filteredJobs.value.filter(item => !item.favorite));
const activeRecords = computed(() => records.value.filter(item => item.status === "queued" || item.status === "running"));
const finishedRecords = computed(() => records.value.filter(item => item.status !== "queued" && item.status !== "running"));
const canPublish = computed(() => connection.value.configured && selectedJob.value && selectedBranch.value && !loadingBranches.value && !publishing.value);
const highlightedRun = computed(() => typeof route.query.run === "string" ? route.query.run : "");

function formatTime(value?: string) {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { hour12:false });
}
function durationText(record:JenkinsPublishRecord) {
  const start=Date.parse(record.buildStartedAt || record.startedAt);
  const end=record.finishedAt ? Date.parse(record.finishedAt) : nowTick.value;
  if (!Number.isFinite(start) || !Number.isFinite(end)) return "—";
  const seconds=Math.max(0,Math.floor((end-start)/1000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes=Math.floor(seconds/60);
  return minutes < 60 ? `${minutes} 分钟` : `${Math.floor(minutes/60)} 小时 ${minutes%60} 分`;
}
function statusLabel(record:JenkinsPublishRecord) {
  if (record.syncState === "reconnecting") return "连接中断，正在重试";
  return ({queued:"排队中",running:"发布中",success:"发布成功",failed:"发布失败",aborted:"已中止"} as Record<string,string>)[record.status] || record.status;
}
function stageLabel(stage:JenkinsPipelineStage) {
  return ({SUCCESS:"成功",FAILED:"失败",ABORTED:"中止",UNSTABLE:"不稳定",IN_PROGRESS:"进行中",PAUSED_PENDING_INPUT:"等待审批",NOT_EXECUTED:"未执行"} as Record<string,string>)[stage.status] || stage.status;
}
function completedStages(record:JenkinsPublishRecord) {
  return record.stages.filter(stage => !["IN_PROGRESS","PAUSED_PENDING_INPUT","UNKNOWN"].includes(stage.status)).length;
}
function activeDetail(record:JenkinsPublishRecord) {
  if (record.syncState === "reconnecting") return record.errorMessage || "暂时无法连接 Jenkins";
  if (record.status === "queued") return record.queueReason || "等待 Jenkins 分配执行节点";
  if (record.currentStage) return `当前阶段：${record.currentStage}`;
  return record.status === "running" ? "Jenkins 正在执行发布任务" : record.errorMessage || record.result || "发布已结束";
}

async function loadConnection() {
  if (!isTauriRuntime()) return;
  connection.value=await getJenkinsConnectionStatus();
  configBaseUrl.value=connection.value.baseUrl;
  configUsername.value=connection.value.username;
}
async function loadJobs() {
  if (!connection.value.configured || loadingJobs.value) return;
  loadingJobs.value=true; error.value="";
  try {
    jobs.value=await listJenkinsJobs();
    if (selectedJobName.value && !jobs.value.some(item=>item.fullName===selectedJobName.value)) {
      selectedJobName.value=""; selectedBranch.value=""; branchOptions.value=null;
    }
  } catch(cause) { error.value=String(cause); }
  finally { loadingJobs.value=false; }
}
async function loadRecords() {
  if (!isTauriRuntime()) return;
  try { records.value=await listJenkinsPublishRecords(); }
  catch(cause) { if (!error.value) error.value=String(cause); }
}
async function selectJob(job:JenkinsJob) {
  selectedJobName.value=job.fullName;
  projectMenuOpen.value=false;
  projectQuery.value="";
  selectedBranch.value="";
  branchOptions.value=null;
  loadingBranches.value=true; error.value="";
  try {
    branchOptions.value=await listJenkinsJobBranches(job.fullName);
    if (branchOptions.value.branches.length === 1) selectedBranch.value=branchOptions.value.branches[0];
  } catch(cause) { error.value=String(cause); }
  finally { loadingBranches.value=false; }
}
async function toggleFavorite(job:JenkinsJob) {
  const next=!job.favorite;
  error.value="";
  try {
    await setJenkinsJobFavorite(job.fullName,next);
    job.favorite=next;
    jobs.value=[...jobs.value].sort((left,right)=>Number(right.favorite)-Number(left.favorite)||left.fullName.localeCompare(right.fullName,"zh-CN"));
  } catch(cause) { error.value=String(cause); }
}
async function publish() {
  const job=selectedJob.value;
  if (!job || !selectedBranch.value || publishing.value) return;
  const confirmed=window.confirm(`确认发布？\n\n项目：${job.fullName}\n分支：${selectedBranch.value}\n\n其他参数将使用 Jenkins 中配置的默认值。`);
  if (!confirmed) return;
  publishing.value=true; error.value=""; message.value="";
  try {
    const record=await triggerJenkinsPublish(job.fullName,selectedBranch.value);
    records.value=[record,...records.value.filter(item=>item.id!==record.id)];
    message.value=`${job.name} · ${selectedBranch.value} 已进入 Jenkins 队列。`;
    window.dispatchEvent(new CustomEvent("workbench-active-operations-changed"));
  } catch(cause) { error.value=String(cause); }
  finally { publishing.value=false; }
}
function openConfiguration() {
  configBaseUrl.value=connection.value.baseUrl;
  configUsername.value=connection.value.username;
  configToken.value="";
  configuring.value=true; error.value=""; message.value="";
}
async function testConfiguration() {
  configBusy.value=true; error.value=""; message.value="";
  try {
    const result=await testJenkinsConnection(configBaseUrl.value,configUsername.value,configToken.value);
    message.value=`连接成功：Jenkins ${result.version}`;
  } catch(cause) { error.value=String(cause); }
  finally { configBusy.value=false; }
}
async function saveConfiguration() {
  configBusy.value=true; error.value=""; message.value="";
  try {
    connection.value=await saveJenkinsConnection(configBaseUrl.value,configUsername.value,configToken.value);
    configToken.value="";
    configuring.value=false;
    message.value=`Jenkins ${connection.value.version} 已连接。`;
    await loadJobs();
  } catch(cause) { error.value=String(cause); }
  finally { configBusy.value=false; }
}
async function initialize() {
  try {
    await loadConnection();
    await Promise.all([loadJobs(),loadRecords()]);
  } catch(cause) { error.value=String(cause); }
}

onMounted(() => {
  void initialize();
  timer=window.setInterval(() => {
    nowTick.value=Date.now();
    if (activeRecords.value.length) void loadRecords();
  },2000);
  if (isTauriRuntime()) void listen("jenkins-publish-updated",()=>void loadRecords()).then(unlisten=>{eventUnlisten=unlisten;});
});
onBeforeUnmount(() => { window.clearInterval(timer); eventUnlisten?.(); });
</script>

<template>
  <div class="view deployments-view">
    <header class="page-header">
      <div><h1>发布中心</h1><p>选择 Jenkins 项目和已配置分支，触发发布并查看实时状态</p></div>
      <div><button class="button secondary" :disabled="loadingJobs || !connection.configured" @click="loadJobs">{{ loadingJobs ? "刷新中…" : "↻ 刷新项目" }}</button><button class="button secondary" @click="openConfiguration">配置 Jenkins</button></div>
    </header>

    <div v-if="message" class="status-banner success">{{ message }}</div>
    <div v-if="error" class="status-banner error">{{ error }}</div>

    <section v-if="!connection.configured" class="panel deployment-empty-connection">
      <span>J</span><div><h2>尚未连接 Jenkins</h2><p>先保存 Jenkins 地址、用户名和 API Token，工作台不会修改 Jenkins 中的任何配置。</p></div><button class="button primary" @click="openConfiguration">配置 Jenkins</button>
    </section>

    <template v-else>
      <section class="panel deployment-launcher">
        <header><div><small>JENKINS PUBLISH</small><h2>选择项目与分支</h2></div><span class="connection-ready"><i></i>Jenkins {{ connection.version || "已连接" }}</span></header>
        <div class="deployment-fields">
          <label class="project-select-field"><span>项目</span><div class="project-select-control"><button type="button" class="project-select-trigger" :class="{open:projectMenuOpen}" @click="projectMenuOpen=!projectMenuOpen"><b>{{ selectedJob?.fullName || "选择 Jenkins 项目" }}</b><em>{{ projectMenuOpen ? "▴" : "▾" }}</em></button><button v-if="selectedJob" class="selected-favorite" :class="{active:selectedJob.favorite}" :title="selectedJob.favorite?'取消收藏':'收藏项目'" @click="toggleFavorite(selectedJob)">{{ selectedJob.favorite ? "★" : "☆" }}</button></div>
            <div v-if="projectMenuOpen" class="project-picker panel">
              <input v-model="projectQuery" autofocus placeholder="搜索 Jenkins 项目">
              <div class="project-picker-list">
                <section v-if="favoriteJobs.length"><h3>已收藏</h3><button v-for="job in favoriteJobs" :key="job.fullName" @click="selectJob(job)"><span><b>{{ job.name }}</b><small>{{ job.fullName }}</small></span><i class="active" title="取消收藏" @click.stop="toggleFavorite(job)">★</i></button></section>
                <section v-if="normalJobs.length"><h3>{{ favoriteJobs.length ? "全部项目" : "项目" }}</h3><button v-for="job in normalJobs" :key="job.fullName" @click="selectJob(job)"><span><b>{{ job.name }}</b><small>{{ job.fullName }}</small></span><i title="收藏项目" @click.stop="toggleFavorite(job)">☆</i></button></section>
                <p v-if="!filteredJobs.length">没有匹配的 Jenkins 项目。</p>
              </div>
            </div>
          </label>
          <label><span>分支</span><select v-model="selectedBranch" :disabled="!selectedJob || loadingBranches"><option value="">{{ loadingBranches ? "正在读取分支…" : selectedJob ? "选择已配置分支" : "请先选择项目" }}</option><option v-for="branch in branchOptions?.branches || []" :key="branch" :value="branch">{{ branch }}</option></select><small v-if="branchOptions">参数：{{ branchOptions.parameterName }} · 其他参数使用 Jenkins 默认值</small></label>
          <button class="button primary publish-button" :disabled="!canPublish" @click="publish">{{ publishing ? "正在提交…" : "发布" }}</button>
        </div>
      </section>

      <section v-if="activeRecords.length" class="deployment-section">
        <header><div><h2>正在发布</h2><p>工作台关闭不会中止 Jenkins 构建</p></div><b>{{ activeRecords.length }}</b></header>
        <div class="deployment-grid active-grid">
          <article v-for="record in activeRecords" :key="record.id" class="panel deployment-card active" :class="[record.status,{highlighted:highlightedRun===record.id}]">
            <header><div><small>{{ record.jobFullName }}</small><h3>{{ record.branch }}</h3></div><span :class="record.syncState">{{ statusLabel(record) }}</span></header>
            <div class="deployment-progress"><i></i></div>
            <p>{{ activeDetail(record) }}</p>
            <div v-if="record.stages.length" class="stage-list"><span v-for="stage in record.stages" :key="stage.id || stage.name" :class="stage.status.toLowerCase()"><i></i><b>{{ stage.name }}</b><small>{{ stageLabel(stage) }}</small></span></div>
            <footer><span>{{ record.stages.length ? `${completedStages(record)} / ${record.stages.length} 阶段` : `已运行 ${durationText(record)}` }}</span><button v-if="record.buildUrl || record.jobUrl" class="text-button" @click="openJenkinsUrl(record.buildUrl || record.jobUrl)">打开 Jenkins ↗</button></footer>
          </article>
        </div>
      </section>

      <section class="deployment-section history-section">
        <header><div><h2>发布记录</h2><p>最近 {{ finishedRecords.length }} 次已结束发布</p></div></header>
        <div class="panel deployment-history">
          <article v-for="record in finishedRecords" :key="record.id" :class="[record.status,{highlighted:highlightedRun===record.id}]">
            <i class="result-dot"></i><div class="history-main"><span><b>{{ record.jobName }}</b><small>{{ record.jobFullName }}</small></span><code>{{ record.branch }}</code></div><div class="history-result"><b>{{ statusLabel(record) }}</b><small>{{ record.currentStage || record.result || record.errorMessage }}</small></div><div class="history-time"><b>{{ durationText(record) }}</b><small>{{ formatTime(record.finishedAt || record.updatedAt) }}</small></div><button class="button secondary small" :disabled="!record.buildUrl && !record.jobUrl" @click="openJenkinsUrl(record.buildUrl || record.jobUrl)">打开 Jenkins</button>
          </article>
          <div v-if="!finishedRecords.length" class="empty-state"><b>还没有发布记录</b><p>选择 Jenkins 项目和分支后点击发布，状态会显示在这里。</p></div>
        </div>
      </section>
    </template>

    <div v-if="configuring" class="activity-backdrop jenkins-config-backdrop" @click.self="configuring=false">
      <form class="panel jenkins-config-dialog" @submit.prevent="saveConfiguration">
        <header><div><h2>连接 Jenkins</h2><p>只保存连接信息，不会修改 Jenkins Job 或发布规则。</p></div><button type="button" class="icon-button" @click="configuring=false">×</button></header>
        <div><label>Jenkins 地址<input v-model="configBaseUrl" placeholder="https://jenkins.example.com"></label><label>用户名<input v-model="configUsername" autocomplete="username" placeholder="Jenkins 用户名"></label><label>API Token<input v-model="configToken" type="password" autocomplete="new-password" :placeholder="connection.configured ? '留空则继续使用已保存 Token' : '粘贴 Jenkins API Token'"><small>Token 保存到 Windows 凭据库，保存后不再回显。</small></label></div>
        <footer><button type="button" class="button secondary" :disabled="configBusy || !configBaseUrl.trim() || !configUsername.trim()" @click="testConfiguration">{{ configBusy ? "连接中…" : "测试连接" }}</button><span></span><button type="button" class="button secondary" @click="configuring=false">取消</button><button class="button primary" :disabled="configBusy || !configBaseUrl.trim() || !configUsername.trim() || (!connection.configured && !configToken)">{{ configBusy ? "保存中…" : "保存" }}</button></footer>
      </form>
    </div>
  </div>
</template>

<style scoped>
.deployments-view{max-width:1500px}.status-banner{margin-bottom:12px;padding:11px 14px;border-radius:9px}.status-banner.success{background:color-mix(in srgb,var(--success) 12%,var(--surface));color:var(--success)}.status-banner.error{background:color-mix(in srgb,var(--danger) 12%,var(--surface));color:var(--danger)}
.deployment-empty-connection{min-height:180px;padding:28px;display:grid;grid-template-columns:64px minmax(0,1fr) auto;gap:18px;align-items:center}.deployment-empty-connection>span{width:58px;height:58px;border-radius:14px;background:var(--primary);color:#fff;display:grid;place-items:center;font-size:27px;font-weight:900}.deployment-empty-connection h2{margin:0}.deployment-empty-connection p{margin:8px 0 0;color:var(--muted)}
.deployment-launcher{padding:20px;overflow:visible;position:relative;z-index:10}.deployment-launcher>header,.deployment-section>header{display:flex;align-items:center;justify-content:space-between;gap:16px}.deployment-launcher h2,.deployment-section h2{margin:4px 0 0}.deployment-launcher header small{color:var(--primary);font:10px ui-monospace,monospace;letter-spacing:1.4px}.connection-ready{padding:6px 9px;border-radius:99px;background:color-mix(in srgb,var(--success) 12%,transparent);color:var(--success);font-size:11px}.connection-ready i{display:inline-block;width:7px;height:7px;margin-right:6px;border-radius:50%;background:var(--success)}
.deployment-fields{display:grid;grid-template-columns:minmax(300px,1.25fr) minmax(260px,1fr) 130px;gap:14px;align-items:end;margin-top:20px}.deployment-fields label{position:relative;display:grid;gap:7px;color:var(--muted)}.deployment-fields select,.project-select-trigger{width:100%;height:43px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--text);padding:0 12px}.deployment-fields label>small{min-height:14px;font-size:9px}.publish-button{height:43px}.project-select-control{display:grid;grid-template-columns:minmax(0,1fr) 43px;gap:7px}.project-select-trigger{display:flex;align-items:center;justify-content:space-between;text-align:left}.project-select-trigger b{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.project-select-trigger em{font-style:normal}.project-select-trigger.open{border-color:var(--primary)}.selected-favorite{height:43px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--muted);font-size:20px}.selected-favorite.active{color:#f2b94b}
.project-picker{position:absolute;left:0;top:76px;width:100%;max-height:430px;padding:10px;z-index:30;box-shadow:0 18px 45px rgba(0,0,0,.28)}.project-picker>input{width:100%;height:38px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text);padding:0 10px}.project-picker-list{max-height:360px;margin-top:8px;overflow:auto}.project-picker-list section+section{margin-top:8px}.project-picker-list h3{margin:0;padding:7px 8px;color:var(--muted);font-size:10px}.project-picker-list button{width:100%;min-height:48px;padding:7px 8px;border:0;border-radius:7px;background:transparent;color:var(--text);display:grid;grid-template-columns:minmax(0,1fr) 34px;align-items:center;text-align:left}.project-picker-list button:hover{background:var(--primary-soft)}.project-picker-list button span{min-width:0;display:flex;flex-direction:column;gap:4px}.project-picker-list button b,.project-picker-list button small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.project-picker-list button small{color:var(--muted)}.project-picker-list button i{height:32px;display:grid;place-items:center;font-style:normal;font-size:18px;color:var(--muted)}.project-picker-list button i.active{color:#f2b94b}.project-picker-list>p{padding:22px;text-align:center;color:var(--muted)}
.deployment-section{margin-top:20px}.deployment-section>header{margin-bottom:12px}.deployment-section>header p{margin:5px 0 0;color:var(--muted)}.deployment-section>header>b{padding:6px 10px;border-radius:99px;background:var(--primary-soft);color:var(--primary)}.deployment-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px}.deployment-card{padding:17px;border-color:color-mix(in srgb,var(--primary) 32%,var(--line))}.deployment-card.highlighted,.deployment-history article.highlighted{box-shadow:0 0 0 2px var(--primary)}.deployment-card>header,.deployment-card>footer{display:flex;align-items:center;justify-content:space-between;gap:12px}.deployment-card h3{margin:5px 0 0}.deployment-card header small{color:var(--muted)}.deployment-card header>span{padding:5px 8px;border-radius:99px;background:var(--primary-soft);color:var(--primary);font-size:10px}.deployment-card header>span.reconnecting{background:color-mix(in srgb,var(--warning) 12%,transparent);color:var(--warning)}.deployment-card>p{margin:10px 0;color:var(--muted)}.deployment-progress{height:6px;margin-top:14px;border-radius:99px;background:var(--surface-2);overflow:hidden}.deployment-progress i{display:block;width:38%;height:100%;border-radius:inherit;background:linear-gradient(90deg,transparent,var(--primary),transparent);animation:deployment-loading 1.4s linear infinite}.stage-list{display:flex;flex-wrap:wrap;gap:6px;margin:12px 0}.stage-list span{padding:6px 8px;border-radius:7px;background:var(--surface-2);display:flex;align-items:center;gap:5px}.stage-list span>i{width:6px;height:6px;border-radius:50%;background:var(--muted)}.stage-list span.success>i{background:var(--success)}.stage-list span.failed>i,.stage-list span.aborted>i{background:var(--danger)}.stage-list span.in_progress>i,.stage-list span.paused_pending_input>i{background:var(--primary)}.stage-list small{color:var(--muted);font-size:9px}.deployment-card>footer{padding-top:11px;border-top:1px solid var(--line);color:var(--muted);font-size:10px}.text-button{border:0;background:transparent;color:var(--primary);cursor:pointer}@keyframes deployment-loading{from{transform:translateX(-100%)}to{transform:translateX(280%)}}
.deployment-history{overflow:hidden}.deployment-history>article{min-height:68px;padding:10px 14px;border-bottom:1px solid var(--line);display:grid;grid-template-columns:10px minmax(260px,1fr) minmax(160px,.6fr) 145px auto;gap:12px;align-items:center}.deployment-history>article:last-child{border-bottom:0}.result-dot{width:8px;height:8px;border-radius:50%;background:var(--muted)}.deployment-history article.success .result-dot{background:var(--success)}.deployment-history article.failed .result-dot{background:var(--danger)}.deployment-history article.aborted .result-dot{background:var(--warning)}.history-main{min-width:0;display:flex;align-items:center;gap:12px}.history-main>span,.history-result,.history-time{min-width:0;display:flex;flex-direction:column;gap:4px}.history-main b,.history-main small,.history-result small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.history-main small,.history-result small,.history-time small{color:var(--muted);font-size:9px}.history-main code{padding:4px 7px;border-radius:5px;background:var(--surface-2);color:var(--primary);white-space:nowrap}.history-result b{color:var(--text)}.deployment-history article.success .history-result b{color:var(--success)}.deployment-history article.failed .history-result b{color:var(--danger)}
.jenkins-config-backdrop{z-index:220}.jenkins-config-dialog{width:560px;overflow:hidden}.jenkins-config-dialog>header{height:72px;padding:0 18px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between}.jenkins-config-dialog h2{margin:0}.jenkins-config-dialog header p{margin:5px 0 0;color:var(--muted)}.jenkins-config-dialog>div{padding:18px;display:grid;gap:14px}.jenkins-config-dialog label{display:grid;gap:7px;color:var(--muted)}.jenkins-config-dialog input{height:41px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text);padding:0 11px}.jenkins-config-dialog label small{font-size:9px}.jenkins-config-dialog>footer{height:64px;padding:0 18px;border-top:1px solid var(--line);display:grid;grid-template-columns:auto 1fr auto auto;gap:8px;align-items:center}
@media(max-width:1050px){.deployment-fields{grid-template-columns:1fr}.deployment-grid{grid-template-columns:1fr}.deployment-history>article{grid-template-columns:10px minmax(0,1fr) auto}.history-result,.history-time{display:none}}
</style>
