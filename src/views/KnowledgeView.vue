<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { askKnowledge, deleteKnowledge, isTauriRuntime, listKnowledge, listKnowledgeCodexJobs, listKnowledgeVersions, listRepositoryAssets, saveKnowledge, startKnowledgeCodexJob, syncKnowledge, type KnowledgeAnswer, type KnowledgeCodexJob, type KnowledgeItem, type KnowledgeVersion, type RepositoryAsset } from "../services/backend";
import { confirmAction } from "../utils/confirm";

const now = new Date().toISOString();
const demos: KnowledgeItem[] = [
  { id: "demo-k1", kind: "decision", title: "本地桌面工作台应采用什么数据存储方案？", content: "适用场景：单用户桌面应用需要离线保存任务、报告和知识。\n核心结论：使用 SQLite，应用无需额外启动数据库服务。\n实施方法：\n1. 将数据库文件放在应用数据目录。\n2. 通过版本号执行增量迁移。\n注意事项：密钥不要写入 SQLite，应保存到系统凭据库。", project: "星枢 ASTRION", sourceType: "report", sourceId: "demo-report", tags: "可复用方法,SQLite,本地优先", confirmed: true, createdAt: now, updatedAt: now },
  { id: "demo-k2", kind: "experience", title: "Codex Token 应如何按会话和日期准确统计？", content: "适用场景：需要同时展示会话总量和每日 Token 趋势。\n核心结论：会话总量读取最后一次累计值，每日用量计算相邻事件的正向差量。\n实施方法：\n1. 按时间排序 Token 事件。\n2. 仅累加大于零的差值并按日期归组。\n验证标准：各日期用量之和不应大于会话累计总量。", project: "星枢 ASTRION", sourceType: "conversation", sourceId: "demo-2", tags: "可复用方法,Codex,Token", confirmed: true, createdAt: now, updatedAt: now },
  { id: "demo-k3", kind: "risk", title: "自动周报生成时如何避免遗漏当天工作？", content: "适用场景：周报与日报可能在同一天定时生成。\n核心结论：周报直接读取任务、对话和 Git 原始数据，不依赖日报是否已生成。\n注意事项：若周报只汇总日报，22:00 前的当日日报不存在时会漏掉整天内容。", project: "星枢 ASTRION", sourceType: "manual", tags: "可复用方法,报告,调度", confirmed: true, createdAt: now, updatedAt: now },
];
const items = ref<KnowledgeItem[]>(isTauriRuntime() ? [] : demos);
const route = useRoute();
const router = useRouter();
const highlightedId = ref("");
const query = ref("");
const asked = ref("");
const aiAnswer = ref<KnowledgeAnswer | null>(null);
const aiLoading = ref(false);
const activeKind = ref("all");
const activeProject = ref("全部项目");
const editorOpen = ref(false);
const editingId = ref("");
const error = ref("");
const message = ref("");
const syncing = ref(false);
const detailOpen = ref(false);
const detailItem = ref<KnowledgeItem | null>(null);
const versions = ref<KnowledgeVersion[]>([]);
const codexJobs = ref<KnowledgeCodexJob[]>([]);
const repositories = ref<RepositoryAsset[]>([]);
const selectedRepository = ref("");
const codexInstruction = ref("");
const sending = ref(false);
const form = reactive({ kind: "decision" as KnowledgeItem["kind"], title: "", content: "", project: "", tags: "", confirmed: true });
const kindMeta: Record<KnowledgeItem["kind"], { label: string; icon: string }> = {
  decision: { label: "技术决策", icon: "◆" }, experience: { label: "实现经验", icon: "!" }, risk: { label: "避坑指南", icon: "△" }, skill: { label: "操作规范", icon: "#" },
};
const projects = computed(() => ["全部项目", ...new Set(items.value.map(item=>item.project || "未归类项目"))]);
const filtered = computed(() => items.value.filter((item) => {
  const kindMatches = activeKind.value === "all" || item.kind === activeKind.value;
  const projectMatches = activeProject.value === "全部项目" || (item.project || "未归类项目") === activeProject.value;
  const needle = query.value.trim().toLowerCase();
  return kindMatches && projectMatches && (!needle || `${item.title} ${item.content} ${item.project ?? ""} ${item.tags}`.toLowerCase().includes(needle));
}));
const answerSources = computed(() => {
  const needle = asked.value.trim().toLowerCase();
  if (!needle) return [];
  const words = needle.split(/[\s，。？、]+/).filter((word) => word.length > 1);
  return items.value.map((item) => ({ item, score: words.filter((word) => `${item.title}${item.content}${item.tags}`.toLowerCase().includes(word)).length }))
    .filter((entry) => entry.score > 0 && entry.item.confirmed).sort((a, b) => b.score - a.score).slice(0, 3).map((entry) => entry.item);
});

function resetForm(item?: KnowledgeItem) {
  editingId.value = item?.id ?? ""; form.kind = item?.kind ?? "decision"; form.title = item?.title ?? ""; form.content = item?.content ?? ""; form.project = item?.project ?? ""; form.tags = item?.tags ?? ""; form.confirmed = item?.confirmed ?? true; editorOpen.value = true;
}
async function refresh() { if (isTauriRuntime()) items.value = await listKnowledge(); }
async function autoSummarize() {
  if (!isTauriRuntime()) return;
  syncing.value = true; error.value = ""; message.value = "";
  try {
    const summary = await syncKnowledge();
    await refresh();
    message.value = `已检查 ${summary.conversationsScanned} 个 Codex 对话，筛选并提炼 ${summary.itemsGenerated} 条可复用知识：技术决策 ${summary.decisions} 条、实现经验 ${summary.experiences} 条、避坑指南 ${summary.risks} 条、操作规范 ${summary.skills} 条。`;
  } catch (cause) { error.value = String(cause); }
  finally { syncing.value = false; }
}
async function persist() {
  error.value = "";
  const existing = items.value.find((item) => item.id === editingId.value);
  const item: KnowledgeItem = { id: editingId.value, kind: form.kind, title: form.title, content: form.content, project: form.project || undefined, sourceType: existing?.sourceType ?? "manual", sourceId: existing?.sourceId, tags: form.tags, confirmed: form.confirmed, createdAt: existing?.createdAt ?? "", updatedAt: existing?.updatedAt ?? "" };
  if (!isTauriRuntime()) { item.id ||= crypto.randomUUID(); item.createdAt ||= new Date().toISOString(); item.updatedAt = new Date().toISOString(); const index = items.value.findIndex((value) => value.id === item.id); if (index >= 0) items.value[index] = item; else items.value.unshift(item); editorOpen.value = false; return; }
  try { const saved = await saveKnowledge(item); const index = items.value.findIndex((value) => value.id === saved.id); if (index >= 0) items.value[index] = saved; else items.value.unshift(saved); editorOpen.value = false; }
  catch (cause) { error.value = String(cause); }
}
async function remove(item: KnowledgeItem) {
  if (!await confirmAction({ title:"删除知识条目", message:`确定删除“${item.title}”吗？删除后不能恢复。`, confirmText:"删除", tone:"danger" })) return;
  if (isTauriRuntime()) await deleteKnowledge(item.id);
  items.value = items.value.filter((value) => value.id !== item.id);
}
async function openKnowledgeTools(item: KnowledgeItem) {
  detailItem.value = item; detailOpen.value = true; error.value = "";
  if (!isTauriRuntime()) return;
  try {
    [versions.value, codexJobs.value, repositories.value] = await Promise.all([listKnowledgeVersions(item.id), listKnowledgeCodexJobs(item.id), listRepositoryAssets()]);
    selectedRepository.value ||= repositories.value[0]?.path ?? "";
  } catch (cause) { error.value = String(cause); }
}
async function sendToCodex() {
  if (!detailItem.value || !selectedRepository.value) return;
  sending.value = true; error.value = "";
  try {
    const job = await startKnowledgeCodexJob(detailItem.value.id, selectedRepository.value, codexInstruction.value);
    codexJobs.value.unshift(job); message.value = "已发送给 Codex。完成后会进入首页结果待审箱，并弹出 Windows 通知。";
  } catch (cause) { error.value = String(cause); }
  finally { sending.value = false; }
}
async function ask() {
  asked.value = query.value.trim(); aiAnswer.value = null;
  if (!asked.value || !isTauriRuntime()) return;
  aiLoading.value = true; error.value = "";
  try { aiAnswer.value = await askKnowledge(asked.value); }
  catch (cause) { error.value = String(cause); }
  finally { aiLoading.value = false; }
}
function sourceText(item: KnowledgeItem) { return item.sourceType === "conversation" ? "Codex 对话" : item.sourceType === "report" ? "工作报告" : item.sourceType === "api" ? "接口文档" : "人工记录"; }
function statusText(item: KnowledgeItem) { return item.id.startsWith("auto:") ? "自动总结" : item.confirmed ? "已确认" : "AI 草稿"; }
function openSource(item: KnowledgeItem) { if (item.sourceType === "conversation" && item.sourceId) void router.push(`/tokens?conversation=${item.sourceId}`); else if (item.sourceType === "report" && item.sourceId) void router.push(`/reports?report=${item.sourceId}`); else if (item.sourceType === "api" && item.sourceId) void router.push(`/api-docs?endpoint=${item.sourceId}`); }
watch([() => route.query.item, () => items.value.length], () => { const id = String(route.query.item || ""); if (items.value.some(item => item.id === id)) highlightedId.value = id; }, { immediate: true });
onMounted(() => { void refresh(); });
</script>

<template>
  <div class="view">
    <header class="page-header"><div><h1>知识库</h1><p>自动把历史工作提炼成以后可以照着做的方案、步骤和注意事项</p></div><div><button class="button secondary" @click="resetForm()">＋ 手动补充</button><button class="button primary" :disabled="syncing" @click="autoSummarize">{{ syncing ? '整理中…' : '✦ 重新提炼知识' }}</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="knowledge-search"><div><b>◇ 搜索可直接复用的做法</b><p>根据已确认知识回答怎么做，并保留原始 Codex 对话来源</p></div><select v-model="activeProject" class="button secondary"><option v-for="item in projects" :key="item">{{ item }}</option></select><label>⌕<input v-model="query" placeholder="例如：页面开发时字典怎么接入？" @keyup.enter="ask" /><kbd @click="ask">Enter</kbd></label></section>
    <section v-if="asked" class="panel knowledge-answer"><div><b>✦ {{ isTauriRuntime() ? 'DeepSeek 知识回答' : '本地知识回答' }}</b><button @click="asked = ''; aiAnswer = null">×</button></div><p v-if="aiLoading">正在依据已确认知识生成回答…</p><p v-else-if="aiAnswer">{{ aiAnswer.answer }}</p><p v-else-if="answerSources.length">根据已确认知识：{{ answerSources.map(item => item.content).join('；') }}</p><p v-else>没有找到足够相关的已确认知识。可以换一组关键词，或先新增一条知识。</p><template v-if="aiAnswer"><small v-for="(item,index) in aiAnswer.sources" :key="item.id">[{{ index + 1 }}] {{ item.title }}</small></template><template v-else><small v-for="item in answerSources" :key="item.id">[{{ sourceText(item) }}] {{ item.title }}</small></template></section>
    <section class="knowledge-layout">
      <aside class="panel category-panel"><b>知识分类</b><button :class="{ active: activeKind === 'all' }" @click="activeKind = 'all'">◇ 全部知识 <em>{{ items.length }}</em></button><button v-for="(meta,kind) in kindMeta" :key="kind" :class="{ active: activeKind === kind }" @click="activeKind = kind"><span>{{ meta.icon }} {{ meta.label }}</span><em>{{ items.filter(item => item.kind === kind).length }}</em></button></aside>
      <main><article v-for="item in filtered" :key="item.id" class="panel knowledge-item" :class="{ highlighted: highlightedId === item.id }"><button class="knowledge-kind-icon" title="查看并编辑这条知识" @click="resetForm(item)">{{ kindMeta[item.kind].icon }}</button><div><small>{{ kindMeta[item.kind].label }} · {{ item.project || '未归类项目' }} · {{ statusText(item) }}</small><h2>{{ item.title }}</h2><p class="knowledge-content">{{ item.content }}</p><button class="knowledge-source" :disabled="item.sourceType === 'manual' || !item.sourceId" @click="openSource(item)">{{ item.tags.split(',').filter(Boolean).map(tag => `# ${tag.trim()}`).join('　') }}　↗ {{ sourceText(item) }}</button></div><div class="knowledge-actions"><button title="版本与发送给 Codex" @click="openKnowledgeTools(item)">↗</button><button title="编辑" @click="resetForm(item)">✎</button><button title="删除" @click="remove(item)">×</button></div></article><p v-if="!filtered.length" class="empty-state">还没有可复用知识。点击“重新提炼知识”即可从全部 Codex 历史生成。</p></main>
    </section>
    <div v-if="detailOpen" class="editor-backdrop" @click.self="detailOpen = false"><aside class="task-editor knowledge-editor knowledge-tools"><header><div><h2>版本与 Codex 实践</h2><p>{{ detailItem?.title }}</p></div><button class="icon-button" @click="detailOpen = false">×</button></header><section><h3>发送给 Codex</h3><p>选择一个已识别的本地项目，Codex 会参考这条知识执行。不会自动提交或推送。</p><label>目标项目<select v-model="selectedRepository"><option value="">请选择项目</option><option v-for="repo in repositories" :key="repo.path" :value="repo.path">{{ repo.name }} · {{ repo.path }}</option></select></label><label>补充要求（可选）<textarea v-model="codexInstruction" rows="3" placeholder="例如：只检查现有实现并给出改进建议"></textarea></label><button class="button primary" :disabled="sending || !selectedRepository || !detailItem?.confirmed" @click="sendToCodex">{{ sending ? '发送中…' : '发送给 Codex' }}</button><small v-if="detailItem && !detailItem.confirmed">这条知识尚未人工确认，确认后才能发送。</small></section><section><h3>最近执行</h3><article v-for="job in codexJobs" :key="job.id" class="knowledge-job"><b>{{ job.status === 'running' ? '执行中' : job.status === 'completed' ? '已完成' : '失败' }}</b><small>{{ job.repositoryPath }}</small><pre v-if="job.output || job.errorMessage">{{ job.output || job.errorMessage }}</pre></article><p v-if="!codexJobs.length" class="empty-state">还没有发送记录。</p></section><section><h3>历史版本</h3><article v-for="version in versions" :key="version.id" class="knowledge-version"><b>版本 {{ version.versionNumber }} · {{ version.changeSource === 'auto_sync' ? '自动提炼更新' : '人工编辑' }}</b><small>{{ new Date(version.createdAt).toLocaleString('zh-CN') }}</small><details><summary>{{ version.title }}</summary><pre>{{ version.content }}</pre></details></article><p v-if="!versions.length" class="empty-state">当前还没有旧版本；首次修改后会自动保留。</p></section></aside></div>
    <div v-if="editorOpen" class="editor-backdrop" @click.self="editorOpen = false"><aside class="task-editor knowledge-editor"><header><div><h2>{{ editingId ? '编辑知识' : '新增知识' }}</h2><p>内容保存在本机 SQLite 数据库</p></div><button class="icon-button" @click="editorOpen = false">×</button></header><label>类型<select v-model="form.kind"><option v-for="(meta,kind) in kindMeta" :key="kind" :value="kind">{{ meta.label }}</option></select></label><label>标题<input v-model="form.title" placeholder="一句话说明知识主题"></label><label>项目<input v-model="form.project" placeholder="例如：星枢 ASTRION"></label><label>内容<textarea v-model="form.content" rows="9" placeholder="记录结论、适用条件和注意事项"></textarea></label><label>标签（英文逗号分隔）<input v-model="form.tags" placeholder="本地优先,Token"></label><label class="confirm-row"><input v-model="form.confirmed" type="checkbox">已人工确认</label><footer><span></span><button class="button secondary" @click="editorOpen = false">取消</button><button class="button primary" @click="persist">保存知识</button></footer></aside></div>
  </div>
</template>
