<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { generateDailyContent, getCodexCliStatus, getContentVideoJob, isTauriRuntime, listContentIdeas, startContentVideoJob, updateContentStatus, type CodexCliStatus, type ContentIdea, type VideoJob } from "../services/backend";

const route = useRoute();
const router = useRouter();
const today = new Date().toLocaleDateString("sv-SE");
const selectedDate = ref(typeof route.query.date === "string" ? route.query.date : today);
const contentType = ref<ContentIdea["contentType"]>(route.query.type === "reasoning" ? "reasoning" : "tech");
const ideas = ref<ContentIdea[]>([]);
const selectedId = ref(typeof route.query.idea === "string" ? route.query.idea : "");
const loading = ref(false);
const message = ref("");
const error = ref("");
const activeSection = ref<"script" | "storyboard" | "visuals" | "editing">("script");
const cliStatus = ref<CodexCliStatus | null>(null);
const activeJob = ref<VideoJob | null>(null);
const startingVideo = ref(false);
let jobTimer = 0;

const selected = computed(() => ideas.value.find(item => item.id === selectedId.value) || ideas.value[0]);
const selectedCount = computed(() => ideas.value.filter(item => item.status === "selected" || item.status === "published").length);
const statusLabel: Record<ContentIdea["status"], string> = { candidate: "候选", selected: "已选择", rejected: "已淘汰", published: "已发布" };
const jobStatusLabel: Record<VideoJob["status"], string> = { queued:"等待启动", running:"Codex 制作中", finalizing:"正在验收交付", complete:"制作完成", "needs-attention":"需要补齐", failed:"执行失败" };
const jobStageLabel: Record<VideoJob["currentStage"], string> = { selection:"等待启动", codex:"读取规范", script:"脚本校验", assets:"画面素材", voice:"配音字幕", composition:"视频合成", quality:"质量检查", render:"成片渲染", finalizing:"交付验收", failed:"执行失败", cover:"封面", publish:"发布文案", delivery:"交付完成" };
const jobRunning = computed(() => Boolean(activeJob.value && ["queued","running","finalizing"].includes(activeJob.value.status)));

function fallbackIdeas(date: string): ContentIdea[] {
  const reasoning = contentType.value === "reasoning";
  const techTitles = ["如果 AI 开始替你管理一天，会发生什么？", "AI 眼镜真的会成为下一部手机吗？", "2035 年的普通家庭，可能已经不需要开关了", "为什么所有科技公司突然都在做机器人？", "2030 年的办公桌，会变成什么样？"];
  const reasoningTitles = ["2、6、12、20，下一项是多少？", "9枚硬币中有1枚较轻，最少称几次？", "涂色立方体切成27块，几块有两面颜色？", "21颗棋子轮流拿，怎样保证自己拿到最后一颗？", "三个开关控制一盏灯，只进房间一次怎么判断？"];
  return Array.from({ length: 5 }, (_, index) => ({ id: `preview-${contentType.value}-${index}`, ideaDate: date, contentType: contentType.value, category: reasoning ? ["数字规律", "称重逻辑", "空间想象", "策略博弈", "实验推理"][index] : ["AI未来", "智能硬件", "未来生活", "科技趋势", "个人科技升级"][index], title: (reasoning ? reasoningTitles : techTitles)[index], hook: reasoning ? "不只靠直觉，你能在十秒内找到唯一解法吗？" : "未来最懂你的人，可能不是人。", script: reasoning ? "题面、选项、十秒思考、唯一答案和逐步解法都会在桌面版中完整显示。" : "桌面预览模式不会写入数据。请从开发版桌面程序打开内容工坊，程序会自动生成当天 5 套完整内容。", storyboard: reasoning ? "| 时间 | 画面 | 字幕 |\n|---|---|---|\n| 0-3秒 | 馆主抛出逻辑挑战 | 你能找到规律吗？ |\n| 3-20秒 | 条件与选项依次出现 | A / B / C |\n| 20-30秒 | 倒计时 | 10秒思考 |\n| 30-45秒 | 分步演示并揭晓 | 唯一答案 |" : "| 时间 | 画面 | 字幕 |\n|---|---|---|\n| 0-3秒 | 未来科技轮廓 | 变化已经开始 |", visualPrompts: reasoning ? "二次元逻辑游戏馆、低饱和灰蓝与暗紫、数字和道具信息清晰、竖屏 9:16。" : "写实电影感、近未来、竖屏 9:16、无品牌 Logo。", editingGuide: reasoning ? "45—50秒；前3秒出题；保留思考时间；逐步演示解法并高亮唯一答案。" : "总时长 55—60 秒，前 3 秒快速建立悬念。", coverTitle: reasoning ? "你能找出答案吗？" : "提前看见未来", status: "candidate", source: "local", createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }));
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    if (!isTauriRuntime()) ideas.value = fallbackIdeas(selectedDate.value);
    else {
      ideas.value = await listContentIdeas(selectedDate.value, contentType.value);
      if (ideas.value.length < 5) ideas.value = await generateDailyContent(selectedDate.value, false, false, contentType.value);
    }
    if (!ideas.value.some(item => item.id === selectedId.value)) selectedId.value = ideas.value[0]?.id || "";
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function regenerate() {
  if (!isTauriRuntime()) return;
  if ((selectedCount.value > 0) && !window.confirm("已选择的内容会保留，其余候选会重新生成。是否继续？")) return;
  loading.value = true;
  message.value = "";
  error.value = "";
  try {
    ideas.value = await generateDailyContent(selectedDate.value, true, contentType.value === "tech", contentType.value);
    const firstNewCandidate = ideas.value.find(item => item.status === "candidate");
    selectedId.value = firstNewCandidate?.id || ideas.value[0]?.id || "";
    const candidateCount = ideas.value.filter(item => item.status === "candidate").length;
    message.value = contentType.value === "reasoning" ? `已生成 ${candidateCount} 个跨不同思维类型、带唯一答案和完整解法的新案例。` : ideas.value.some(item => item.source === "deepseek") ? `已使用 DeepSeek 生成 ${candidateCount} 套新内容。` : `未连接 DeepSeek，已使用本地方案生成 ${candidateCount} 套新内容。`;
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function setStatus(status: ContentIdea["status"]) {
  if (!selected.value || !isTauriRuntime()) return;
  await updateContentStatus(selected.value.id, status);
  selected.value.status = status;
  message.value = status === "selected" ? "已加入你的待制作列表。" : status === "rejected" ? "已淘汰这条候选。" : "状态已更新。";
}

async function loadVideoJob() {
  if (!isTauriRuntime() || !selected.value) { activeJob.value=null; return; }
  try { activeJob.value=await getContentVideoJob(selected.value.id); }
  catch (cause) { console.error("读取 Codex 视频任务失败", cause); }
}

async function startCodexVideo() {
  if (!selected.value || !isTauriRuntime() || startingVideo.value || jobRunning.value) return;
  error.value=""; message.value="";
  try {
    if (!cliStatus.value?.authenticated) cliStatus.value=await getCodexCliStatus();
    if (!cliStatus.value.installed || !cliStatus.value.authenticated) { error.value=cliStatus.value.message; return; }
    const skill = selected.value.contentType === "reasoning" ? "$generate-reasoning-short-video" : "$generate-tech-short-video";
    const action = activeJob.value ? "继续同一个 Codex 视频任务" : "创建新的 Codex 视频任务";
    if (!window.confirm(`${action}并调用 ${skill}。\n\n为正常启动 Chrome、FFmpeg、配音和截图工具，本次视频任务将使用完整本地执行权限；制作目标仍固定为该视频项目目录。完成后会发送工作台消息。是否开始？`)) return;
    startingVideo.value=true;
    activeJob.value=await startContentVideoJob(selected.value.id);
    message.value=`已启动 ${skill}，可以继续使用工作台；完成后会收到消息。`;
  } catch (cause) { error.value=String(cause); }
  finally { startingVideo.value=false; }
}

async function copyPackage() {
  if (!selected.value) return;
  const item = selected.value;
  const text = `# ${item.title}\n\n分类：${item.category}\n封面：${item.coverTitle}\n\n## 3秒钩子\n${item.hook}\n\n## 完整口播\n${item.script}\n\n## 分镜脚本\n${item.storyboard}\n\n## AI画面提示词\n${item.visualPrompts}\n\n## 剪辑指导\n${item.editingGuide}`;
  await navigator.clipboard.writeText(text);
  message.value = "完整制作包已复制。";
}
function choose(item: ContentIdea) {
  selectedId.value = item.id;
  activeSection.value = "script";
  void router.replace({ query: { date: selectedDate.value, type: contentType.value, idea: item.id } });
}

async function switchContentType(value: ContentIdea["contentType"]) {
  if (contentType.value === value) return;
  contentType.value = value;
  selectedId.value = "";
  activeSection.value = "script";
  await router.replace({ query: { date: selectedDate.value, type: value } });
  await load();
}

watch(selectedDate, async () => { await router.replace({ query: { date: selectedDate.value, type: contentType.value } }); selectedId.value = ""; await load(); });
watch(() => route.query.idea, (id) => { if (typeof id === "string" && ideas.value.some(item => item.id === id)) selectedId.value = id; });
watch(() => selected.value?.id, () => void loadVideoJob());
onMounted(async () => {
  await load();
  if (!isTauriRuntime()) return;
  cliStatus.value=await getCodexCliStatus();
  await loadVideoJob();
  jobTimer=window.setInterval(() => { if (jobRunning.value) void loadVideoJob(); }, 2000);
});
onBeforeUnmount(() => window.clearInterval(jobTimer));
</script>

<template>
  <div class="view content-view">
    <header class="page-header"><div><h1>内容工坊</h1><p>{{ contentType === 'reasoning' ? '每天 5 个多维度逻辑思维案例，覆盖推理、脑力游戏、数字、空间、概率与策略' : '每天 5 个“小众科技探索”选题，完整内容可直接进入制作' }}</p></div><div><input v-model="selectedDate" class="button secondary content-date" type="date"><button class="button primary" :disabled="loading" @click="regenerate">{{ loading ? '生成中…' : '✦ 重新生成 5 条' }}</button></div></header>
    <nav class="content-channel-switch panel"><button :class="{ active:contentType === 'tech' }" @click="switchContentType('tech')"><b>未来科技探索</b><small>趋势、设备与未来生活</small></button><button :class="{ active:contentType === 'reasoning' }" @click="switchContentType('reasoning')"><b>逻辑思维案例</b><small>推理、脑力与策略 · 5题可选</small></button></nav>
    <p v-if="message" class="scan-message">{{ message }}</p><p v-if="error" class="scan-message error">{{ error }}</p>
    <section class="content-summary panel"><div><b>{{ ideas.length }}</b><span>今日候选</span></div><div><b>{{ selectedCount }}</b><span>已选择</span></div><div><b>{{ ideas.filter(item => item.status === 'rejected').length }}</b><span>已淘汰</span></div><p><strong>{{ contentType === 'reasoning' ? '逻辑规则' : '内容边界' }}</strong><span>{{ contentType === 'reasoning' ? '每天覆盖不同思维类型；每题先验证唯一答案，再提供完整解法。' : '不伪装开箱或亲身体验；趋势与推测会明确表达。' }}</span></p></section>
    <section class="content-layout">
      <aside class="panel content-list"><header><b>{{ selectedDate }} 候选标题</b><small>点击查看完整制作包</small></header><button v-for="(item,index) in ideas" :key="item.id" :class="[item.status,{ active:selected?.id === item.id }]" @click="choose(item)"><i>{{ index + 1 }}</i><span><small>{{ item.category }} · {{ item.source === 'deepseek' ? 'AI 生成' : '本地生成' }}</small><b>{{ item.title }}</b><em>{{ statusLabel[item.status] }}</em></span></button><p v-if="!ideas.length && !loading" class="panel-empty">当天还没有候选内容。</p></aside>
      <main v-if="selected" class="panel content-detail">
        <header><div><span>{{ selected.category }}</span><h2>{{ selected.title }}</h2><p>封面标题：<b>{{ selected.coverTitle }}</b></p></div><div><button class="button secondary" @click="setStatus(selected.status === 'candidate' ? 'rejected' : 'candidate')">{{ selected.status === 'candidate' ? '淘汰' : '恢复候选' }}</button><button class="button primary" :disabled="selected.status === 'selected'" @click="setStatus('selected')">{{ selected.status === 'selected' ? '✓ 已选择' : '✓ 选择这条' }}</button><button class="button secondary" @click="copyPackage">复制全部</button></div></header>
        <section v-if="selected.status === 'selected'" class="codex-job-panel" :class="activeJob?.status || 'ready'">
          <div class="codex-job-head"><span><i></i><strong>{{ activeJob ? jobStatusLabel[activeJob.status] : cliStatus?.message || '正在检查 Codex CLI…' }}</strong><small v-if="cliStatus?.version">{{ cliStatus.version }} · {{ selected.contentType === 'reasoning' ? '$generate-reasoning-short-video' : '$generate-tech-short-video' }}</small></span><div><button v-if="activeJob" class="text-button" @click="router.push('/videos?tab=pipeline')">查看视频中心 →</button><button class="button primary small" :disabled="startingVideo || jobRunning" @click="startCodexVideo">{{ startingVideo ? '正在启动…' : jobRunning ? 'Codex 制作中…' : activeJob ? '继续 Codex 制作' : '使用 Codex Skill 生成视频' }}</button></div></div>
          <div v-if="activeJob" class="codex-progress"><div><span>{{ activeJob.progressMessage || jobStatusLabel[activeJob.status] }}</span><b>{{ activeJob.progressPercent }}%</b></div><div class="codex-progress-track"><i :style="{width:`${activeJob.progressPercent}%`}"></i></div><small>当前阶段：{{ jobStageLabel[activeJob.currentStage] }} · 进度来自项目文件和实际执行阶段</small></div>
          <dl v-if="activeJob"><div><dt>视频项目</dt><dd>{{ activeJob.projectRoot }}</dd></div><div><dt>Codex 对话</dt><dd>{{ activeJob.codexThreadId || '启动后自动创建' }}</dd></div></dl>
          <details v-if="activeJob?.codexOutput || activeJob?.failureReason"><summary>查看 Codex 输出与执行信息</summary><pre>{{ activeJob.codexOutput || activeJob.failureReason }}</pre></details>
          <p v-else>{{ activeJob ? '任务在后台执行，可以离开当前页面；工作台每 2 秒更新一次状态。' : '点击上方按钮后，工作台会创建独立 Codex 对话并直接调用对应视频 Skill。启动前仍会要求你确认完整本地执行权限。' }}</p>
        </section>
        <blockquote><small>3 秒钩子</small><b>{{ selected.hook }}</b></blockquote>
        <nav class="content-tabs"><button :class="{active:activeSection==='script'}" @click="activeSection='script'">完整口播</button><button :class="{active:activeSection==='storyboard'}" @click="activeSection='storyboard'">分镜脚本</button><button :class="{active:activeSection==='visuals'}" @click="activeSection='visuals'">AI 画面</button><button :class="{active:activeSection==='editing'}" @click="activeSection='editing'">剪辑指导</button></nav>
        <article class="content-copy"><pre v-if="activeSection==='script'">{{ selected.script }}</pre><pre v-else-if="activeSection==='storyboard'">{{ selected.storyboard }}</pre><pre v-else-if="activeSection==='visuals'">{{ selected.visualPrompts }}</pre><pre v-else>{{ selected.editingGuide }}</pre></article>
      </main>
    </section>
  </div>
</template>

<style scoped>
.codex-job-panel{margin:14px 16px 0;padding:14px;border:1px solid var(--line);border-radius:10px;background:color-mix(in srgb,var(--primary) 5%,var(--surface-2))}.codex-job-head{display:flex;align-items:center;justify-content:space-between;gap:12px}.codex-job-head>span{min-width:0;flex:1;display:grid;grid-template-columns:9px minmax(0,1fr);gap:3px 8px}.codex-job-head>div{flex:0 0 auto;display:flex;align-items:center;gap:8px}.codex-job-head>div .button{white-space:nowrap}.codex-job-head i{grid-row:1/3;width:8px;height:8px;margin-top:5px;border-radius:50%;background:var(--primary);box-shadow:0 0 0 4px var(--primary-soft)}.codex-job-head strong,.codex-job-head small{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.codex-job-head small{color:var(--muted)}.codex-job-panel.running .codex-job-head i,.codex-job-panel.queued .codex-job-head i,.codex-job-panel.finalizing .codex-job-head i{animation:codex-pulse 1.4s infinite}.codex-job-panel.complete .codex-job-head i{background:var(--success)}.codex-job-panel.failed .codex-job-head i,.codex-job-panel.needs-attention .codex-job-head i{background:var(--warning)}.codex-progress{display:grid;gap:7px;margin-top:13px}.codex-progress>div:first-child{display:flex;justify-content:space-between;gap:10px;font-size:12px}.codex-progress>div:first-child span{color:var(--text)}.codex-progress>div:first-child b{color:var(--primary)}.codex-progress-track{height:7px;overflow:hidden;border-radius:99px;background:var(--surface)}.codex-progress-track i{display:block;height:100%;border-radius:inherit;background:linear-gradient(90deg,var(--primary),#53c895);transition:width .35s ease}.codex-progress>small{color:var(--muted)}.codex-job-panel dl{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin:12px 0 0}.codex-job-panel dl>div{min-width:0;padding:9px 10px;border-radius:7px;background:var(--surface)}.codex-job-panel dt{color:var(--muted);font-size:9px}.codex-job-panel dd{margin:5px 0 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.codex-job-panel>p{margin:10px 0 0;color:var(--muted);line-height:1.6}.codex-job-panel details{margin-top:12px}.codex-job-panel summary{cursor:pointer;color:var(--primary)}.codex-job-panel pre{max-height:220px;overflow:auto;margin:10px 0 0;padding:12px;border-radius:8px;background:var(--surface);white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.6}@keyframes codex-pulse{50%{opacity:.4;transform:scale(.72)}}
</style>
