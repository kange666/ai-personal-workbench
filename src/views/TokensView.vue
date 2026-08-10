<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import TokenTrendChart from "../components/TokenTrendChart.vue";
import { getModelTokenMetrics, getProjectTokenMetrics, getTokenSummary, getTokenTrend, isTauriRuntime, listConversationMetrics, scanCodexSessions, scanGitRepositories, setConversationProject, type CodexScanSummary, type ConversationMetric, type GitScanSummary, type ModelTokenMetric, type ProjectTokenMetric, type TokenSummary, type TokenTrendPoint } from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";

type Detail = { kind: "day"; value: TokenTrendPoint } | { kind: "conversation"; value: ConversationMetric } | { kind: "project"; value: ProjectTokenMetric } | { kind: "model"; value: ModelTokenMetric };
type PeriodMode = "day" | "week" | "month";
const route = useRoute();
const loading = ref(false);
const error = ref("");
const lastScan = ref<CodexScanSummary | null>(null);
const lastGitScan = ref<GitScanSummary | null>(null);
const range = ref(14);
const periodMode = ref<PeriodMode>("day");
const detail = ref<Detail | null>(null);
const costOpen = ref(false);
const projectEdit = ref("");
const readPrice = (key: string, fallback: number) => Number(localStorage.getItem(key) || fallback);
const inputPrice = ref(readPrice("ai-workbench.price.input", 1));
const cachedPrice = ref(readPrice("ai-workbench.price.cached", 0.25));
const outputPrice = ref(readPrice("ai-workbench.price.output", 4));
const summary = ref<TokenSummary>({ conversationCount: 0, messageCount: 0, activeDays: 0, inputTokens: 0, cachedInputTokens: 0, outputTokens: 0, reasoningOutputTokens: 0, totalTokens: 0 });
const conversations = ref<ConversationMetric[]>([]);
const dailyTrend = ref<TokenTrendPoint[]>([]);
const projects = ref<ProjectTokenMetric[]>([]);
const models = ref<ModelTokenMetric[]>([]);
const runtimeLabel = computed(() => isTauriRuntime() ? "本地数据库" : "浏览器演示数据");
const cacheRate = computed(() => summary.value.inputTokens ? summary.value.cachedInputTokens / summary.value.inputTokens * 100 : 0);
const uncachedInput = computed(() => Math.max(summary.value.inputTokens - summary.value.cachedInputTokens, 0));
const averageConversation = computed(() => summary.value.conversationCount ? summary.value.totalTokens / summary.value.conversationCount : 0);
const outputRate = computed(() => summary.value.totalTokens ? summary.value.outputTokens / summary.value.totalTokens * 100 : 0);
const contextConversation = computed(() => conversations.value.find((item) => item.contextWindow > 0));
const contextRate = computed(() => contextConversation.value?.contextWindow ? Math.round(contextConversation.value.contextUsedTokens / contextConversation.value.contextWindow * 100) : 0);
const archivedConversationCount = computed(() => conversations.value.filter(item => item.archived).length);
const ordinaryConversationCount = computed(() => Math.max(conversations.value.length - archivedConversationCount.value, 0));
const estimatedCost = computed(() => ((Math.max(summary.value.inputTokens-summary.value.cachedInputTokens,0)*inputPrice.value + summary.value.cachedInputTokens*cachedPrice.value + (summary.value.outputTokens+summary.value.reasoningOutputTokens)*outputPrice.value) / 1_000_000));

function bucketKey(dateText: string, mode: PeriodMode) {
  if (mode === "day") return dateText;
  if (mode === "month") return dateText.slice(0,7);
  const date=new Date(`${dateText}T00:00:00`); date.setDate(date.getDate()-((date.getDay()+6)%7)); return date.toLocaleDateString("sv-SE");
}
const periodTrend = computed<TokenTrendPoint[]>(() => {
  if (periodMode.value === "day") return dailyTrend.value;
  const grouped=new Map<string,TokenTrendPoint>();
  for (const item of dailyTrend.value) { const key=bucketKey(item.date,periodMode.value); const current=grouped.get(key) || {date:key,inputTokens:0,cachedInputTokens:0,outputTokens:0,reasoningOutputTokens:0,totalTokens:0}; current.inputTokens+=item.inputTokens; current.cachedInputTokens+=item.cachedInputTokens; current.outputTokens+=item.outputTokens; current.reasoningOutputTokens+=item.reasoningOutputTokens; current.totalTokens+=item.totalTokens; grouped.set(key,current); }
  return [...grouped.values()].sort((a,b)=>a.date.localeCompare(b.date));
});
const displayedTrend = computed(() => range.value ? periodTrend.value.slice(-range.value) : periodTrend.value);
const periodLabel = computed(() => ({day:"日",week:"周",month:"月"})[periodMode.value]);

function compact(value: number) { if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`; if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`; if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`; return String(value); }
function percent(value: number) { return `${value.toFixed(1)}%`; }
function projectName(path: string) { return path.split(/[\\/]/).filter(Boolean).at(-1) || path; }
function openDetail(next: Detail) { detail.value = next; if (next.kind === "conversation") projectEdit.value=projectName(next.value.project); }
function detailTitle() { if (!detail.value) return ""; if (detail.value.kind === "day") return `${detail.value.value.date} Token 明细`; if (detail.value.kind === "conversation") return compactDetailTitle(detail.value.value.title || "未命名会话", projectName(detail.value.value.project)); if (detail.value.kind === "project") return compactDetailTitle(projectName(detail.value.value.project)); return compactDetailTitle(detail.value.value.model); }

async function refresh() {
  if (!isTauriRuntime()) return;
  [summary.value, conversations.value, dailyTrend.value, projects.value, models.value] = await Promise.all([getTokenSummary(), listConversationMetrics(1000), getTokenTrend(0), getProjectTokenMetrics(), getModelTokenMetrics()]);
  const conversationId = String(route.query.conversation || "");
  const date = String(route.query.date || "");
  const conversation = conversations.value.find(item => item.id === conversationId);
  const day = dailyTrend.value.find(item => item.date === date);
  if (conversation) detail.value = { kind: "conversation", value: conversation };
  else if (day) detail.value = { kind: "day", value: day };
}
async function scan() { if (!isTauriRuntime()) { error.value = "浏览器模式不能读取本机 Codex，请在桌面端运行。"; return; } loading.value = true; error.value = ""; lastGitScan.value = null; try { lastScan.value = await scanCodexSessions(); await refresh(); } catch (cause) { error.value = String(cause); } finally { loading.value = false; } }
async function scanGit() { if (!isTauriRuntime()) { error.value = "浏览器模式不能读取本机 Git，请在桌面端运行。"; return; } loading.value = true; error.value = ""; lastScan.value = null; try { lastGitScan.value = await scanGitRepositories(); } catch (cause) { error.value = String(cause); } finally { loading.value = false; } }
async function saveConversationProject(reset=false) { if (detail.value?.kind !== "conversation") return; const id=detail.value.value.id; loading.value=true; try { await setConversationProject(id,reset ? undefined : projectEdit.value); await refresh(); const updated=conversations.value.find(item=>item.id===id); if (updated) { detail.value={kind:"conversation",value:updated}; projectEdit.value=projectName(updated.project); } } catch(cause){error.value=String(cause);} finally{loading.value=false;} }
watch([inputPrice,cachedPrice,outputPrice], () => { localStorage.setItem("ai-workbench.price.input",String(inputPrice.value)); localStorage.setItem("ai-workbench.price.cached",String(cachedPrice.value)); localStorage.setItem("ai-workbench.price.output",String(outputPrice.value)); });
onMounted(refresh);
</script>

<template>
  <div class="view token-view">
    <header class="page-header"><div><h1>Token 分析</h1><p>输入、缓存、输出、推理、模型、项目、普通/归档与成本估算 · {{ runtimeLabel }}</p></div><div><select v-model="periodMode" class="button secondary" title="按日、周或月汇总"><option value="day">按日</option><option value="week">按周</option><option value="month">按月</option></select><select v-model.number="range" class="button secondary" title="切换统计周期数量"><option :value="7">近 7 个周期</option><option :value="14">近 14 个周期</option><option :value="30">近 30 个周期</option><option :value="0">全部历史</option></select><button class="button secondary" :disabled="loading" @click="scanGit">⌘ 扫描 Git</button><button class="button primary" :disabled="loading" @click="scan">{{ loading ? "扫描中…" : "↻ 扫描 Codex" }}</button></div></header>
    <div v-if="lastScan || lastGitScan || error" class="scan-message" :class="{ error: Boolean(error) }"><span v-if="error">{{ error }}</span><span v-else-if="lastGitScan">发现 {{ lastGitScan.repositoriesFound }} 个 Git 仓库，新导入 {{ lastGitScan.commitsImported }} 次提交，记录 {{ lastGitScan.snapshotsCreated }} 个工作区快照。</span><span v-else-if="lastScan">已检查 {{ lastScan.normalFilesScanned }} 个普通会话和 {{ lastScan.archivedFilesScanned }} 个归档会话；更新 {{ lastScan.conversationsImported }} 个会话，跳过 {{ lastScan.filesUnchanged }} 个未变化文件，导入 {{ lastScan.messagesImported }} 条消息。</span></div>
    <section class="metric-grid token-metrics"><article class="clickable-card" @click="detail = null"><span>总 Token</span><b>{{ compact(summary.totalTokens) }}</b><p>{{ summary.conversationCount }} 次对话 · {{ summary.activeDays }} 个活跃日</p></article><article class="clickable-card" @click="detail = null"><span>输入 Token</span><b>{{ compact(summary.inputTokens) }}</b><p>非缓存 {{ compact(uncachedInput) }}</p></article><article class="clickable-card" @click="detail = null"><span>缓存输入</span><b>{{ compact(summary.cachedInputTokens) }}</b><p>命中占比 {{ percent(cacheRate) }}</p></article><article class="clickable-card" @click="detail = null"><span>输出 / 推理</span><b>{{ compact(summary.outputTokens) }}</b><p>推理 {{ compact(summary.reasoningOutputTokens) }} · 输出占比 {{ percent(outputRate) }}</p></article></section>
    <section class="token-breakdown"><button title="普通与归档对话都计入统计" @click="conversations[0] && openDetail({ kind:'conversation', value:conversations[0] })"><b>{{ ordinaryConversationCount }}/{{ archivedConversationCount }}</b><span>普通 / 归档</span></button><button title="查看最高消耗会话" @click="conversations[0] && openDetail({ kind:'conversation', value:conversations[0] })"><b>{{ compact(averageConversation) }}</b><span>平均每次对话</span></button><button title="查看最高消耗模型" @click="models[0] && openDetail({ kind:'model', value:models[0] })"><b>{{ models.length }}</b><span>使用模型</span></button><button title="查看最高消耗项目" @click="projects[0] && openDetail({ kind:'project', value:projects[0] })"><b>{{ projects.length }}</b><span>活跃项目</span></button><button title="按可调整参考单价估算" @click="costOpen=true"><b>${{ estimatedCost.toFixed(2) }}</b><span>估算成本</span></button></section>
    <section class="token-layout"><article class="panel large-token-chart"><div class="panel-head"><div><h2>Token 使用趋势 · 按{{ periodLabel }}</h2><p>按相邻 token_count 的正向差量统计；纵轴为 Token，悬停看构成，点击看明细</p></div><span>{{ range ? `最近 ${displayedTrend.length} 个${periodLabel}周期` : `全部 ${displayedTrend.length} 个${periodLabel}周期` }}</span></div><TokenTrendChart :points="displayedTrend" @select="openDetail({ kind:'day', value:$event })" /></article><article class="panel context-panel clickable-card" @click="contextConversation && openDetail({ kind:'conversation', value:contextConversation })"><h2>上下文占用</h2><div class="context-ring" :style="{ background: `conic-gradient(var(--primary) 0 ${contextRate}%,var(--surface-2) ${contextRate}%)` }"><b>{{ contextRate }}<small>%</small></b></div><p><span>最近有窗口数据的会话</span><b>{{ contextConversation?.title?.slice(0, 16) || '暂无数据' }}</b></p><p><span>已用 / 窗口</span><b>{{ compact(contextConversation?.contextUsedTokens || 0) }} / {{ compact(contextConversation?.contextWindow || 0) }}</b></p><small>点击查看该会话完整 Token 构成</small></article></section>
    <section class="token-detail-grid three-columns"><article class="panel conversation-ranking"><h2>高消耗会话</h2><button v-for="(item,index) in conversations.slice(0,10)" :key="item.id" @click="openDetail({ kind:'conversation', value:item })"><b>{{ index + 1 }}</b><span><strong>{{ item.title || '未命名会话' }}</strong><small>{{ projectName(item.project) }} · {{ item.model || '未知模型' }}<template v-if="item.archived"> · 归档</template></small></span><em>{{ compact(item.totalTokens) }}</em></button><p v-if="!conversations.length">扫描 Codex 后显示真实会话。</p></article><article class="panel conversation-ranking"><h2>项目 Token 排行</h2><button v-for="(item,index) in projects.slice(0,10)" :key="item.project" @click="openDetail({ kind:'project', value:item })"><b>{{ index + 1 }}</b><span><strong>{{ projectName(item.project) }}</strong><small>{{ item.conversationCount }} 次对话 · 缓存 {{ percent(item.inputTokens ? item.cachedInputTokens / item.inputTokens * 100 : 0) }}</small></span><em>{{ compact(item.totalTokens) }}</em></button><p v-if="!projects.length">尚无项目统计。</p></article><article class="panel conversation-ranking"><h2>模型 Token 排行</h2><button v-for="(item,index) in models" :key="item.model" @click="openDetail({ kind:'model', value:item })"><b>{{ index + 1 }}</b><span><strong>{{ item.model }}</strong><small>{{ item.conversationCount }} 次对话 · 输出 {{ compact(item.outputTokens) }}</small></span><em>{{ compact(item.totalTokens) }}</em></button><p v-if="!models.length">尚无模型统计。</p></article></section>
    <div v-if="detail" class="activity-backdrop" @click.self="detail = null"><aside class="activity-drawer panel token-detail-drawer"><header><div><h2>{{ detailTitle() }}</h2><p>{{ detail.kind === 'day' ? `按${periodLabel}汇总的差量` : detail.kind === 'conversation' ? '会话累计值' : detail.kind === 'project' ? '项目累计值' : '模型累计值' }}</p></div><button class="icon-button" @click="detail = null">×</button></header><template v-if="detail.kind === 'day'"><div class="activity-metrics"><div><b>{{ compact(detail.value.totalTokens) }}</b><span>总 Token</span></div><div><b>{{ compact(detail.value.inputTokens) }}</b><span>输入</span></div><div><b>{{ compact(detail.value.cachedInputTokens) }}</b><span>缓存</span></div><div><b>{{ compact(detail.value.outputTokens) }}</b><span>输出</span></div></div><dl><div><dt>推理 Token</dt><dd>{{ detail.value.reasoningOutputTokens.toLocaleString() }}</dd></div></dl></template><template v-else><div class="activity-metrics"><div><b>{{ compact(detail.value.totalTokens) }}</b><span>总 Token</span></div><div><b>{{ compact(detail.value.inputTokens) }}</b><span>输入</span></div><div><b>{{ compact(detail.value.cachedInputTokens) }}</b><span>缓存</span></div><div><b>{{ compact(detail.value.outputTokens) }}</b><span>输出</span></div></div><dl><div><dt>推理 Token</dt><dd>{{ detail.value.reasoningOutputTokens.toLocaleString() }}</dd></div><div><dt>缓存命中</dt><dd>{{ percent(detail.value.inputTokens ? detail.value.cachedInputTokens / detail.value.inputTokens * 100 : 0) }}</dd></div><div v-if="detail.kind === 'conversation'"><dt>模型</dt><dd>{{ detail.value.model || '未知' }}</dd></div><div v-if="detail.kind === 'conversation'"><dt>上下文</dt><dd>{{ compact(detail.value.contextUsedTokens) }} / {{ compact(detail.value.contextWindow) }}</dd></div><div v-if="detail.kind !== 'conversation'"><dt>对话数</dt><dd>{{ detail.value.conversationCount }}</dd></div></dl></template></aside></div>
    <div v-if="costOpen" class="activity-backdrop" @click.self="costOpen=false"><aside class="activity-drawer panel cost-drawer"><header><div><h2>Token 成本估算</h2><p>仅按你设置的参考单价换算，不代表实际账单</p></div><button class="icon-button" @click="costOpen=false">×</button></header><div class="cost-total"><small>全部历史估算</small><b>${{ estimatedCost.toFixed(2) }}</b><span>美元</span></div><div class="cost-price-form"><label>非缓存输入（美元 / 1M）<input v-model.number="inputPrice" type="number" min="0" step="0.01"></label><label>缓存输入（美元 / 1M）<input v-model.number="cachedPrice" type="number" min="0" step="0.01"></label><label>输出与推理（美元 / 1M）<input v-model.number="outputPrice" type="number" min="0" step="0.01"></label><p>计算：非缓存输入 × 单价 + 缓存输入 × 单价 +（输出 + 推理）× 单价。Token 仅反映 AI 使用量，不用于评价工作价值或个人效率。</p></div></aside></div>
    <section v-if="detail?.kind === 'conversation'" class="conversation-project-editor panel"><b>项目归类</b><small>默认根据工作目录识别；识别错误时可手工修正，不会修改本地目录。</small><input v-model="projectEdit" placeholder="例如：client"><div><button class="button secondary small" :disabled="loading" @click="saveConversationProject(true)">恢复自动识别</button><button class="button primary small" :disabled="loading || !projectEdit.trim()" @click="saveConversationProject(false)">保存归类</button></div></section>
  </div>
</template>

<style scoped>
.conversation-project-editor{position:fixed;right:500px;bottom:24px;width:360px;z-index:145;padding:14px;display:grid;gap:9px;box-shadow:var(--shadow)}
.conversation-project-editor small{color:var(--muted);line-height:1.5}.conversation-project-editor input{height:38px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--text);padding:0 9px}.conversation-project-editor div{display:flex;justify-content:flex-end;gap:7px}
</style>
