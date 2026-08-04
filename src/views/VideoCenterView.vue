<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauriRuntime, listLocalVideos, openLocalVideo, readVideoCover, revealLocalVideo, type VideoItem } from "../services/backend";

const demoVideos: VideoItem[] = [
  { id:"demo-1", title:"使用 Codex 2 小时完成个人工作台_最终版", project:"使用codex2小时完成个人工作台", path:"C:\\Users\\11429\\Documents\\视频创作\\使用codex2小时完成个人工作台\\output\\最终版.mp4", folder:"output", sourceRoot:"视频创作", fileName:"最终版.mp4", extension:"MP4", sizeBytes:19_587_568, modifiedAt:new Date().toISOString(), status:"final" },
  { id:"demo-2", title:"谁在说谎？", project:"everyday-reasoning-01-who-lied", path:"C:\\Users\\11429\\Documents\\视频创作\\videos\\everyday-reasoning-01-who-lied\\renders\\video-v2-final-clean.mp4", folder:"renders", sourceRoot:"视频创作 / videos", fileName:"video-v2-final-clean.mp4", extension:"MP4", sizeBytes:21_993_181, modifiedAt:new Date(Date.now()-86_400_000).toISOString(), status:"final" },
];

const videos = ref<VideoItem[]>([]);
const covers = ref<Record<string,string>>({});
const query = ref("");
const projectFilter = ref("全部项目");
const statusFilter = ref<"all" | VideoItem["status"]>("all");
const loading = ref(false);
const error = ref("");
const selected = ref<VideoItem | null>(null);

const projects = computed(() => ["全部项目", ...new Set(videos.value.map(item => item.project))]);
const filtered = computed(() => videos.value.filter(item => {
  if (projectFilter.value !== "全部项目" && item.project !== projectFilter.value) return false;
  if (statusFilter.value !== "all" && item.status !== statusFilter.value) return false;
  const keyword = query.value.trim().toLowerCase();
  return !keyword || `${item.title} ${item.project} ${item.fileName}`.toLowerCase().includes(keyword);
}));
const totalBytes = computed(() => videos.value.reduce((sum,item)=>sum+item.sizeBytes,0));
const selectedVideoSrc = computed(() => selected.value && isTauriRuntime() ? convertFileSrc(selected.value.path) : "");
const statusText: Record<VideoItem["status"],string> = { final:"最终成片", output:"输出版本", render:"渲染版本" };

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
    await loadCovers(videos.value);
  } catch (cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
function play(item:VideoItem) { selected.value=item; }
async function openSystem(item:VideoItem) {
  if (!isTauriRuntime()) return;
  try { await openLocalVideo(item.path); } catch (cause) { error.value=String(cause); }
}
async function reveal(item:VideoItem) {
  if (!isTauriRuntime()) return;
  try { await revealLocalVideo(item.path); } catch (cause) { error.value=String(cause); }
}

onMounted(refresh);
</script>

<template>
  <div class="view video-center-view">
    <header class="page-header"><div><h1>视频中心</h1><p>集中查看和管理“视频创作”及其 videos 子目录中的本地成片</p></div><div><button class="button secondary" :disabled="loading" @click="refresh">{{ loading ? '扫描中…' : '↻ 重新扫描' }}</button></div></header>
    <p v-if="error" class="scan-message error">{{ error }}</p>
    <section class="video-metrics">
      <button @click="statusFilter='all'"><span>本地视频</span><b>{{ videos.length }}</b><small>排除 clip 和 work 中间文件</small></button>
      <button @click="statusFilter='final'"><span>最终成片</span><b>{{ videos.filter(item=>item.status==='final').length }}</b><small>文件名已标记 final / 最终 / 成片</small></button>
      <div><span>创作项目</span><b>{{ projects.length - 1 }}</b><small>按作品目录自动归类</small></div>
      <div><span>视频空间</span><b>{{ formatSize(totalBytes) }}</b><small>只统计当前列表文件</small></div>
    </section>
    <section class="panel video-library">
      <header class="video-toolbar"><label>⌕<input v-model="query" placeholder="搜索视频标题、项目或文件名"></label><select v-model="projectFilter"><option v-for="project in projects" :key="project">{{ project }}</option></select><div class="mode-switch"><button :class="{active:statusFilter==='all'}" @click="statusFilter='all'">全部</button><button :class="{active:statusFilter==='final'}" @click="statusFilter='final'">成片</button><button :class="{active:statusFilter==='output'}" @click="statusFilter='output'">输出</button><button :class="{active:statusFilter==='render'}" @click="statusFilter='render'">渲染</button></div></header>
      <div class="video-grid">
        <article v-for="item in filtered" :key="item.id" class="video-card" @dblclick="play(item)">
          <button class="video-cover" :style="item.coverPath && covers[item.coverPath] ? {backgroundImage:`linear-gradient(180deg,transparent 42%,rgba(4,7,15,.88)),url('${covers[item.coverPath]}')`} : {}" title="双击或点击播放" @click="play(item)"><span>▶</span><em>{{ statusText[item.status] }}</em><i>{{ item.extension }}</i></button>
          <div><small>{{ item.project }}</small><h2 :title="item.title">{{ item.title }}</h2><p><span>{{ formatSize(item.sizeBytes) }}</span><span>{{ formatTime(item.modifiedAt) }}</span></p><footer><button class="button primary small" @click="play(item)">播放</button><button class="button secondary small" @click="selected=item">详情</button><button class="text-button" @click="reveal(item)">定位文件</button></footer></div>
        </article>
        <div v-if="!filtered.length && !loading" class="empty-state video-empty"><b>没有符合条件的视频</b><p>工作台只展示成片和 renders/output 中的渲染结果，中间 clip 会自动忽略。</p></div>
      </div>
    </section>
    <div v-if="selected" class="activity-backdrop" @click.self="selected=null"><aside class="activity-drawer panel video-detail-drawer"><header><div><h2>{{ selected.title }}</h2><p>工作台内置播放器 · 本地文件不会上传</p></div><button class="icon-button" @click="selected=null">×</button></header><video v-if="selectedVideoSrc" :key="selected.path" class="internal-video-player" :src="selectedVideoSrc" controls autoplay playsinline preload="metadata">当前环境不支持播放该视频。</video><div v-else class="video-detail-cover" :style="selected.coverPath && covers[selected.coverPath] ? {backgroundImage:`url('${covers[selected.coverPath]}')`} : {}"><span>▶</span><small>桌面版可在工作台内直接播放</small></div><dl><div><dt>项目</dt><dd>{{ selected.project }}</dd></div><div><dt>状态</dt><dd>{{ statusText[selected.status] }}</dd></div><div><dt>格式 / 大小</dt><dd>{{ selected.extension }} · {{ formatSize(selected.sizeBytes) }}</dd></div><div><dt>来源目录</dt><dd>{{ selected.sourceRoot }}</dd></div><div class="path-row"><dt>文件路径</dt><dd>{{ selected.path }}</dd></div></dl><div class="activity-actions"><button class="button primary" @click="openSystem(selected)">使用系统播放器打开</button><button class="button secondary" @click="reveal(selected)">在资源管理器中定位</button></div></aside></div>
  </div>
</template>
