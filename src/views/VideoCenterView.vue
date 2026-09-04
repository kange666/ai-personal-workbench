<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  getVideoProjectDetails,
  isTauriRuntime,
  listLocalVideos,
  listVideoJobs,
  listVideoPublishRecords,
  openLocalVideo,
  readVideoCover,
  revealLocalFile,
  revealLocalVideo,
  saveVideoJobType,
  saveVideoPublishRecord,
  syncVideoPipeline,
  type VideoDeliverable,
  type VideoItem,
  type VideoJob,
  type VideoProjectDetails,
  type VideoPublishRecord,
} from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";

const demoVideos: VideoItem[] = [
  { id:"demo-1", title:"使用 Codex 2 小时完成个人工作台_最终版", project:"使用codex2小时完成个人工作台", path:"C:\\Users\\11429\\Documents\\视频创作\\使用codex2小时完成个人工作台\\output\\最终版.mp4", folder:"output", sourceRoot:"视频创作", fileName:"最终版.mp4", extension:"MP4", sizeBytes:19_587_568, modifiedAt:new Date().toISOString(), status:"final", collection:"tech" },
  { id:"demo-2", title:"谁在说谎？", project:"everyday-reasoning-01-who-lied", path:"C:\\Users\\11429\\Documents\\视频创作\\videos\\everyday-reasoning-01-who-lied\\renders\\video-v2-final-clean.mp4", folder:"renders", sourceRoot:"视频创作 / videos", fileName:"video-v2-final-clean.mp4", extension:"MP4", sizeBytes:21_993_181, modifiedAt:new Date(Date.now()-86_400_000).toISOString(), status:"final", collection:"reasoning" },
];
const route = useRoute();

const videos = ref<VideoItem[]>([]);
const covers = ref<Record<string,string>>({});
const query = ref("");
const projectFilter = ref("全部项目");
const collectionFilter = ref<"all" | VideoItem["collection"]>("all");
const statusFilter = ref<"all" | VideoItem["status"]>("all");
const loading = ref(false);
const error = ref("");
const selected = ref<VideoItem | null>(null);
const details = ref<VideoProjectDetails | null>(null);
const detailsLoading = ref(false);
const detailMessage = ref("");
const activeSection = ref<"library" | "pipeline" | "publish">(route.query.tab === "pipeline" ? "pipeline" : route.query.tab === "publish" ? "publish" : "library");
const jobs = ref<VideoJob[]>([]);
const publishRecords = ref<VideoPublishRecord[]>([]);
const savingPublishId = ref("");
let jobTimer = 0;

const selectedTitle = computed(() =>
  compactDetailTitle(selected.value?.title || "视频详情", selected.value?.project || "视频"),
);

const projects = computed(() => ["全部项目", ...new Set(videos.value.map(item => item.project))]);
const filtered = computed(() => videos.value.filter(item => {
  if (collectionFilter.value !== "all" && item.collection !== collectionFilter.value) return false;
  if (projectFilter.value !== "全部项目" && item.project !== projectFilter.value) return false;
  if (statusFilter.value !== "all" && item.status !== statusFilter.value) return false;
  const keyword = query.value.trim().toLowerCase();
  return !keyword || `${item.title} ${item.project} ${item.fileName}`.toLowerCase().includes(keyword);
}));
const totalBytes = computed(() => videos.value.reduce((sum,item)=>sum+item.sizeBytes,0));
const selectedVideoSrc = computed(() => selected.value && isTauriRuntime() ? convertFileSrc(selected.value.path) : "");
const statusText: Record<VideoItem["status"],string> = { final:"最终成片", output:"输出版本", render:"渲染版本" };
const deliverableIcons: Record<VideoDeliverable["kind"], string> = { video:"▶", cover:"▧", script:"▤", publish:"✎" };
const collections: Array<{ id:VideoItem["collection"]; title:string; subtitle:string; mark:string }> = [
  { id:"human-weakness", title:"人性的弱点", subtitle:"海底人性课 · 沟通与人性洞察", mark:"人" },
  { id:"tech", title:"AI未来观察局", subtitle:"AI、科技趋势与产品实验", mark:"AI" },
  { id:"reasoning", title:"谜题推演社", subtitle:"每日推理、逻辑与脑力挑战", mark:"谜" },
];
const collectionText: Record<VideoItem["collection"], string> = { "human-weakness":"人性的弱点", tech:"AI未来观察局", reasoning:"谜题推演社" };
const collectionCounts = computed(() => Object.fromEntries(collections.map(item => [item.id, videos.value.filter(video => video.collection === item.id).length])) as Record<VideoItem["collection"],number>);
const jobTypeText: Record<VideoJob["videoType"], string> = collectionText;
const stageText: Record<VideoJob["currentStage"], string> = { selection:"等待启动", codex:"读取规范", script:"脚本校验", assets:"画面素材", voice:"配音字幕", composition:"视频合成", quality:"质量检查", render:"成片渲染", finalizing:"交付验收", failed:"执行失败", cover:"封面", publish:"发布文案", delivery:"交付完成" };
const jobStatusText: Record<VideoJob["status"], string> = { queued:"等待启动", running:"制作中", finalizing:"验收中", complete:"完整交付", "needs-attention":"需要补齐", failed:"执行失败" };
const pipelineStats = computed(() => ({
  complete: jobs.value.filter(item=>item.status==="complete").length,
  attention: jobs.value.filter(item=>item.status!=="complete").length,
  types: new Set(jobs.value.filter(item=>item.status==="complete").map(item=>item.videoType)).size,
}));

function formatSize(bytes:number) {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
}
function formatTime(value:string) { return new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"}).format(new Date(value)); }

async function loadCovers(items: VideoItem[]) {
  if (!isTauriRuntime()) return;
  const unique = [...new Set(items.map(item=>item.coverPath).filter((value): value is string => Boolean(value)))].slice(0,30);
  await Promise.all(unique.map(async path => { try { covers.value[path] = await readVideoCover(path); } catch { /* 封面缺失时使用内置占位图 */ } }));
}
async function refresh() {
  loading.value=true; error.value="";
  try {
    videos.value = isTauriRuntime() ? await listLocalVideos() : demoVideos;
    if (isTauriRuntime()) {
      await syncVideoPipeline();
      jobs.value = await listVideoJobs();
      publishRecords.value = await listVideoPublishRecords();
    }
    await loadCovers(videos.value);
  } catch (cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function savePublish(record: VideoPublishRecord, markPublished = false) {
  if (!isTauriRuntime()) return;
  savingPublishId.value=record.id; error.value="";
  try {
    if (markPublished) record.status="published";
    await saveVideoPublishRecord({ videoJobId:record.videoJobId, platform:record.platform, status:record.status, publishUrl:record.publishUrl, publishedAt:record.publishedAt, views:record.views, likes:record.likes, comments:record.comments, favorites:record.favorites, notes:record.notes });
    publishRecords.value=await listVideoPublishRecords();
  } catch (cause) { error.value=String(cause); }
  finally { savingPublishId.value=""; }
}
async function changeJobType(job: VideoJob) {
  if (!isTauriRuntime()) return;
  try {
    await saveVideoJobType(job.id, job.videoType);
    job.manuallyConfirmedType = true;
  } catch (cause) { error.value=String(cause); }
}
function demoDetails(item: VideoItem): VideoProjectDetails {
  return {
    projectRoot: item.folder,
    deliverables: [
      { kind:"video", label:"最终视频 MP4", path:item.path, fileName:item.fileName, available:true },
      { kind:"cover", label:"竖屏封面 PNG", path:item.coverPath, fileName:item.coverPath?.split(/[\\/]/).pop(), available:Boolean(item.coverPath) },
      { kind:"script", label:"完整脚本", fileName:"SCRIPT.md", content:"桌面版会读取视频项目中的完整脚本，并在这里显示。", available:true },
      { kind:"publish", label:"标题、配文及置顶评论", fileName:"发布信息.md", content:"## 发布标题\n使用 Codex 2 小时完成个人工作台\n\n## 发布配文\n桌面版会显示项目中的真实发布内容。\n\n## 置顶评论\n你最想让 Codex 帮你做什么？", available:true },
    ],
  };
}
async function openDetails(item:VideoItem) {
  selected.value=item;
  details.value=null;
  detailMessage.value="";
  detailsLoading.value=true;
  try {
    details.value = isTauriRuntime() ? await getVideoProjectDetails(item.path) : demoDetails(item);
  } catch (cause) {
    detailMessage.value=`读取交付文件失败：${String(cause)}`;
  } finally {
    detailsLoading.value=false;
  }
}
function play(item:VideoItem) { void openDetails(item); }
async function openSystem(item:VideoItem) {
  if (!isTauriRuntime()) return;
  try { await openLocalVideo(item.path); } catch (cause) { error.value=String(cause); }
}
async function reveal(item:VideoItem) {
  if (!isTauriRuntime()) return;
  try { await revealLocalVideo(item.path); } catch (cause) { error.value=String(cause); }
}
async function copyText(value:string | undefined, label:string) {
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    detailMessage.value=`已复制${label}`;
  } catch {
    detailMessage.value="复制失败，请选中文本后手工复制。";
  }
}
async function revealDeliverable(item:VideoDeliverable) {
  if (!item.path || !isTauriRuntime()) return;
  try {
    await revealLocalFile(item.path);
  } catch (cause) {
    detailMessage.value=String(cause);
  }
}

onMounted(async () => {
  await refresh();
  jobTimer=window.setInterval(async () => {
    if (isTauriRuntime() && jobs.value.some(item=>["queued","running","finalizing"].includes(item.status))) {
      try { jobs.value=await listVideoJobs(); } catch { /* 保留上一次可见进度 */ }
    }
  }, 2000);
});
onBeforeUnmount(() => window.clearInterval(jobTimer));
</script>

<template>
  <div class="view video-center-view">
    <header class="page-header"><div><h1>视频中心</h1><p>管理本地成片，并按脚本、视频、封面、发布文案检查生产交付</p></div><div><button class="button secondary" :disabled="loading" @click="refresh">{{ loading ? '扫描中…' : '↻ 重新扫描' }}</button></div></header>
    <p v-if="error" class="scan-message error">{{ error }}</p>
    <div class="video-center-tabs"><button :class="{active:activeSection==='library'}" @click="activeSection='library'">本地视频库</button><button :class="{active:activeSection==='pipeline'}" @click="activeSection='pipeline'">生产流水线</button><button :class="{active:activeSection==='publish'}" @click="activeSection='publish'">发布与复盘</button></div>
    <section v-if="activeSection==='library'" class="video-collections">
      <button v-for="item in collections" :key="item.id" :class="[item.id,{active:collectionFilter===item.id}]" @click="collectionFilter=collectionFilter===item.id?'all':item.id">
        <i>{{ item.mark }}</i><span><b>{{ item.title }}</b><small>{{ item.subtitle }}</small></span><em>{{ collectionCounts[item.id] }} 个作品</em>
      </button>
    </section>
    <section v-if="activeSection==='library'" class="video-metrics">
      <button @click="statusFilter='all'"><span>本地视频</span><b>{{ videos.length }}</b><small>排除 clip 和 work 中间文件</small></button>
      <button @click="statusFilter='final'"><span>最终成片</span><b>{{ videos.filter(item=>item.status==='final').length }}</b><small>文件名已标记 final / 最终 / 成片</small></button>
      <div><span>创作项目</span><b>{{ projects.length - 1 }}</b><small>按作品目录自动归类</small></div>
      <div><span>视频空间</span><b>{{ formatSize(totalBytes) }}</b><small>只统计当前列表文件</small></div>
    </section>
    <section v-if="activeSection==='library'" class="panel video-library">
      <header class="video-toolbar"><label>⌕<input v-model="query" placeholder="搜索视频标题、项目或文件名"></label><select v-model="projectFilter"><option v-for="project in projects" :key="project">{{ project }}</option></select><div class="mode-switch"><button :class="{active:statusFilter==='all'}" @click="statusFilter='all'">全部</button><button :class="{active:statusFilter==='final'}" @click="statusFilter='final'">成片</button><button :class="{active:statusFilter==='output'}" @click="statusFilter='output'">输出</button><button :class="{active:statusFilter==='render'}" @click="statusFilter='render'">渲染</button></div></header>
      <div class="video-grid">
        <article v-for="item in filtered" :key="item.id" class="video-card" @dblclick="play(item)">
          <button class="video-cover" :style="item.coverPath && covers[item.coverPath] ? {backgroundImage:`linear-gradient(180deg,transparent 42%,rgba(4,7,15,.88)),url('${covers[item.coverPath]}')`} : {}" title="双击或点击播放" @click="play(item)"><span>▶</span><em>{{ statusText[item.status] }}</em><i>{{ item.extension }}</i></button>
          <div><small><i class="collection-tag">{{ collectionText[item.collection] }}</i>{{ item.project }}</small><h2 :title="item.title">{{ item.title }}</h2><p><span>{{ formatSize(item.sizeBytes) }}</span><span>{{ formatTime(item.modifiedAt) }}</span></p><footer><button class="button primary small" @click="play(item)">播放</button><button class="button secondary small" @click="openDetails(item)">详情</button><button class="text-button" @click="reveal(item)">定位文件</button></footer></div>
        </article>
        <div v-if="!filtered.length && !loading" class="empty-state video-empty"><b>没有符合条件的视频</b><p>工作台只展示成片和 renders/output 中的渲染结果，中间 clip 会自动忽略。</p></div>
      </div>
    </section>
    <template v-if="activeSection==='pipeline'">
      <section class="video-metrics pipeline-metrics"><div><span>生产项目</span><b>{{ jobs.length }}</b><small>每个目录只保留一条任务</small></div><div><span>完整交付</span><b>{{ pipelineStats.complete }}</b><small>四项交付全部可读</small></div><div><span>需要补齐</span><b>{{ pipelineStats.attention }}</b><small>明确显示缺少哪一项</small></div><div><span>已覆盖合集</span><b>{{ pipelineStats.types }}/3</b><small>人性、科技、推理</small></div></section>
      <section class="panel pipeline-panel">
        <div class="pipeline-explain"><b>统一验收口径</b><span>脚本 → 成片 → 封面 → 发布文案。工作台只检查并管理现有本地交付，不会未经确认自动发布视频。</span></div>
        <div class="job-grid"><article v-for="job in jobs" :key="job.id" class="video-job-card" :class="job.status">
          <header><div><small>{{ jobTypeText[job.videoType] }}<template v-if="job.skillName"> · ${{ job.skillName }}</template></small><h2>{{ job.title }}</h2></div><span>{{ jobStatusText[job.status] }}</span></header>
          <div class="job-progress-label"><span>{{ job.progressMessage || stageText[job.currentStage] }}</span><b>{{ job.progressPercent }}%</b></div><div class="job-stage"><i :style="{width:`${job.progressPercent}%`}"></i></div>
          <div class="job-deliverables"><span v-for="item in job.deliverables" :key="item.kind" :class="item.status"><b>{{ deliverableIcons[item.kind] }}</b>{{ item.kind==='script'?'脚本':item.kind==='video'?'成片':item.kind==='cover'?'封面':'发布' }}<em>{{ item.status==='ready'?'已就绪':'缺失' }}</em><small>{{ item.qualitySummary }}</small></span></div>
          <p v-if="job.failureReason" class="job-failure">{{ job.status==='failed'?'失败原因：':'下一步：' }}{{ job.failureReason }}</p><p v-else-if="['queued','running','finalizing'].includes(job.status)" class="job-running">Codex 正在后台执行，完成后会自动扫描四项交付并发送工作台消息。</p><p v-else class="job-success">四项交付完整，可在视频库中播放、查看和复制。</p>
          <details v-if="job.codexOutput" class="job-codex-output"><summary>查看 Codex 输出</summary><pre>{{ job.codexOutput }}</pre></details>
          <footer><label>合集<select v-model="job.videoType" @change="changeJobType(job)"><option value="human-weakness">人性的弱点</option><option value="tech">AI未来观察局</option><option value="reasoning">谜题推演社</option></select></label><span>{{ job.manuallyConfirmedType?'已人工确认':'自动识别' }}</span><b>当前阶段：{{ stageText[job.currentStage] }}</b></footer>
        </article><div v-if="!jobs.length&&!loading" class="empty-state"><b>尚未建立视频生产任务</b><p>点击重新扫描后，会从本地视频项目自动建立。</p></div></div>
      </section>
    </template>
    <section v-if="activeSection==='publish'" class="panel publish-review-panel">
      <header><div><h2>发布与数据复盘</h2><p>完整交付后自动进入待发布。工作台只记录状态和数据，不会在未授权的情况下操作抖音账号。</p></div><b>{{ publishRecords.filter(item=>item.status==='ready').length }} 条待发布</b></header>
      <div class="publish-records"><article v-for="record in publishRecords" :key="record.id" :class="record.status"><header><div><small>{{ jobTypeText[record.videoType] }} · {{ record.platform }}</small><h3>{{ record.title }}</h3></div><span>{{ record.status==='published'?'已发布':'待发布' }}</span></header><div class="publish-fields"><label>作品链接<input v-model="record.publishUrl" placeholder="发布后粘贴抖音作品链接"></label><label>播放<input v-model.number="record.views" min="0" type="number"></label><label>点赞<input v-model.number="record.likes" min="0" type="number"></label><label>评论<input v-model.number="record.comments" min="0" type="number"></label><label>收藏<input v-model.number="record.favorites" min="0" type="number"></label></div><label>复盘备注<textarea v-model="record.notes" rows="2" placeholder="记录开头留存、评论反馈和下一条改进点"></textarea></label><footer><small v-if="record.publishedAt">发布于 {{ formatTime(record.publishedAt) }}</small><span></span><button class="button secondary small" :disabled="savingPublishId===record.id" @click="savePublish(record)">保存数据</button><button v-if="record.status==='ready'" class="button primary small" :disabled="savingPublishId===record.id" @click="savePublish(record,true)">标记已发布</button></footer></article><div v-if="!publishRecords.length" class="empty-state"><b>还没有待发布作品</b><p>视频的脚本、成片、封面和发布文案全部就绪后，会自动出现在这里。</p></div></div>
    </section>
    <div v-if="selected" class="activity-backdrop" @click.self="selected=null">
      <aside class="activity-drawer panel video-detail-drawer">
        <header>
          <div><h2 :title="selected.title">{{ selectedTitle }}</h2><p>本地交付内容 · 可直接查看、复制和定位</p></div>
          <button class="icon-button" @click="selected=null">×</button>
        </header>
        <video v-if="selectedVideoSrc" :key="selected.path" class="internal-video-player" :src="selectedVideoSrc" controls autoplay playsinline preload="metadata">当前环境不支持播放该视频。</video>
        <div v-else class="video-detail-cover" :style="selected.coverPath && covers[selected.coverPath] ? {backgroundImage:`url('${covers[selected.coverPath]}')`} : {}"><span>▶</span><small>桌面版可在工作台内直接播放</small></div>

        <section class="video-deliverables">
          <div class="deliverable-heading">
            <div><h3>交付文件</h3><p>自动读取当前视频所属项目，不上传本地内容</p></div>
            <span v-if="detailsLoading">读取中…</span>
          </div>
          <p v-if="detailMessage" class="detail-message">{{ detailMessage }}</p>
          <article v-for="item in details?.deliverables || []" :key="item.kind" class="deliverable-card" :class="{missing:!item.available}">
            <div class="deliverable-summary">
              <i>{{ deliverableIcons[item.kind] }}</i>
              <span><b>{{ item.label }}</b><small>{{ item.fileName || '未找到对应文件' }}</small></span>
              <em>{{ item.available ? '已找到' : '缺失' }}</em>
            </div>
            <div v-if="item.available" class="deliverable-actions">
              <button v-if="item.content" class="button primary small" @click="copyText(item.content, item.label)">复制全部内容</button>
              <button v-if="item.path" class="button secondary small" @click="copyText(item.path, '文件路径')">复制路径</button>
              <button v-if="item.path" class="text-button" @click="revealDeliverable(item)">定位文件</button>
            </div>
            <details v-if="item.content" open class="deliverable-content">
              <summary>查看{{ item.label }}</summary>
              <pre>{{ item.content }}</pre>
            </details>
          </article>
          <p v-if="!detailsLoading && !details" class="deliverable-empty">未能读取当前项目的交付文件。</p>
        </section>

        <dl>
          <div><dt>项目</dt><dd>{{ selected.project }}</dd></div>
          <div><dt>状态</dt><dd>{{ statusText[selected.status] }}</dd></div>
          <div><dt>格式 / 大小</dt><dd>{{ selected.extension }} · {{ formatSize(selected.sizeBytes) }}</dd></div>
          <div><dt>来源目录</dt><dd>{{ selected.sourceRoot }}</dd></div>
          <div class="path-row"><dt>项目目录</dt><dd>{{ details?.projectRoot || selected.folder }}</dd></div>
        </dl>
        <div class="activity-actions"><button class="button primary" @click="openSystem(selected)">使用系统播放器打开</button><button class="button secondary" @click="reveal(selected)">在资源管理器中定位</button></div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.video-collections{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:12px;margin-bottom:14px}.video-collections>button{min-width:0;min-height:86px;padding:13px 14px;border:1px solid var(--line);border-radius:12px;background:var(--surface);color:inherit;display:grid;grid-template-columns:42px minmax(0,1fr) auto;align-items:center;gap:11px;text-align:left}.video-collections>button:hover,.video-collections>button.active{border-color:var(--primary);background:var(--primary-soft)}.video-collections i{width:42px;height:42px;border-radius:11px;background:linear-gradient(145deg,var(--primary),#526cff);color:#fff;display:grid;place-items:center;font-style:normal;font-weight:900}.video-collections .human-weakness i{background:linear-gradient(145deg,#e1a957,#c77b4c)}.video-collections .reasoning i{background:linear-gradient(145deg,#5f6f9d,#28324c)}.video-collections span{min-width:0;display:flex;flex-direction:column;gap:5px}.video-collections span b,.video-collections span small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.video-collections span small{color:var(--muted);font-size:10px}.video-collections em{color:var(--primary);font-style:normal;font-size:10px;white-space:nowrap}.collection-tag{margin-right:7px;padding:3px 6px;border-radius:5px;background:var(--primary-soft);color:var(--primary);font-style:normal;font-size:9px}
.video-center-tabs{display:flex;gap:6px;margin:0 0 16px;padding:5px;width:max-content;border:1px solid var(--line);border-radius:12px;background:var(--surface)}.video-center-tabs button{padding:9px 16px;border:0;border-radius:8px;background:transparent;color:var(--muted);cursor:pointer}.video-center-tabs button.active{background:var(--primary-fill);color:#fff}.pipeline-panel{padding:18px}.pipeline-explain{display:flex;gap:14px;padding:14px 16px;border-radius:12px;background:rgba(117,100,245,.08)}.pipeline-explain span{color:var(--muted)}.job-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;margin-top:16px}.video-job-card{padding:18px;border:1px solid var(--line);border-radius:14px}.video-job-card>header,.video-job-card>footer{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}.video-job-card h2{margin:4px 0 0;font-size:17px}.video-job-card>header small{display:block;max-width:520px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.video-job-card>header>span{padding:5px 9px;border-radius:99px;background:rgba(243,154,98,.12);color:#f39a62;font-size:12px;white-space:nowrap}.video-job-card.complete>header>span{background:rgba(83,200,149,.12);color:#53c895}.video-job-card.running>header>span,.video-job-card.queued>header>span,.video-job-card.finalizing>header>span{background:var(--primary-soft);color:var(--primary)}.video-job-card.failed>header>span{background:color-mix(in srgb,var(--danger) 12%,transparent);color:var(--danger)}.job-progress-label{display:flex;justify-content:space-between;gap:12px;margin-top:16px;font-size:12px}.job-progress-label span{color:var(--muted)}.job-progress-label b{color:var(--primary)}.job-stage{height:7px;margin:7px 0 16px;border-radius:99px;background:var(--surface-2);overflow:hidden}.job-stage i{display:block;height:100%;border-radius:inherit;background:linear-gradient(90deg,var(--primary),#53c895);transition:width .35s ease}.job-deliverables{display:grid;grid-template-columns:repeat(4,1fr);gap:7px}.job-deliverables>span{display:grid;gap:3px;padding:9px 7px;border-radius:9px;background:var(--surface-2);font-size:12px}.job-deliverables b{font-size:16px}.job-deliverables em,.job-deliverables small{font-size:10px;color:var(--muted)}.job-deliverables .ready em{color:#53c895}.job-deliverables .missing em{color:#f39a62}.job-success,.job-failure,.job-running{margin:14px 0;color:var(--muted);font-size:12px}.job-success{color:#53c895}.job-running{color:var(--primary)}.job-codex-output{margin:12px 0}.job-codex-output summary{cursor:pointer;color:var(--primary)}.job-codex-output pre{max-height:180px;overflow:auto;padding:10px;border-radius:8px;background:var(--surface-2);white-space:pre-wrap;overflow-wrap:anywhere}.video-job-card>footer{align-items:end;padding-top:12px;border-top:1px solid var(--line);font-size:11px;color:var(--muted)}.video-job-card>footer label{display:grid;gap:4px}.video-job-card>footer select{padding:6px 8px}.video-job-card>footer b{font-size:11px;color:var(--text)}@media(max-width:1050px){.job-grid{grid-template-columns:1fr}}
.publish-review-panel{padding:18px}.publish-review-panel>header{display:flex;justify-content:space-between;align-items:start;gap:18px}.publish-review-panel h2,.publish-review-panel h3{margin:0}.publish-review-panel header p{margin:6px 0 0;color:var(--muted)}.publish-review-panel>header>b{color:var(--primary);white-space:nowrap}.publish-records{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;margin-top:16px}.publish-records>article{display:grid;gap:12px;padding:16px;border:1px solid var(--line);border-radius:13px;background:var(--surface)}.publish-records>article>header,.publish-records>article>footer{display:flex;align-items:center;justify-content:space-between;gap:12px}.publish-records>article>header span{padding:5px 9px;border-radius:99px;background:var(--primary-soft);color:var(--primary)}.publish-records>article.published>header span{background:color-mix(in srgb,var(--success) 12%,transparent);color:var(--success)}.publish-fields{display:grid;grid-template-columns:2fr repeat(4,1fr);gap:8px}.publish-records label{display:grid;gap:5px;color:var(--muted);font-size:10px}.publish-records input,.publish-records textarea{min-width:0;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text);padding:9px;resize:vertical}.publish-records footer span{flex:1}@media(max-width:1120px){.publish-records{grid-template-columns:1fr}.publish-fields{grid-template-columns:1fr 1fr}.publish-fields label:first-child{grid-column:1/3}}
</style>
