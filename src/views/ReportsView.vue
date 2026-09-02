<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { backfillHistoricalReports, generateReport, getReportSources, isTauriRuntime, listReports, refineReportWithAi, saveReport, setReportLocked, type HistoricalReportSummary, type ReportRecord, type ReportSource } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";

const today = new Date().toISOString().slice(0, 10);
const demoReports: ReportRecord[] = [{
  id: "demo-report", reportType: "weekly", periodStart: "2026-07-27", periodEnd: "2026-08-02",
  title: "2026年第31周工作总结", status: "draft", createdAt: "2026-08-02T16:00:00+08:00", updatedAt: "2026-08-02T16:00:00+08:00",
  contentMarkdown: "# 2026年第31周工作总结\n\n统计周期：2026-07-27 至 2026-08-02\n\n## 工作概览\n\n- 完成任务：8 项\n- 活跃项目：3 个\n- Token 使用：542800\n\n## 已完成事项\n\n- [星枢工作台] 完成产品原型与正式版基础搭建。\n- [星枢工作台] 确认 B 指挥中心布局及双主题。\n\n## 问题与风险\n\n- 桌面端仍需安装 Windows C++ 链接工具后才能完成打包验证。\n\n## 下一步计划\n\n- 完成本地数据采集与自动报告链路。",
}];

const reports = ref<ReportRecord[]>(isTauriRuntime() ? [] : demoReports);
const route = useRoute();
const router = useRouter();
const store = useWorkbenchStore();
const selectedId = ref(reports.value[0]?.id ?? "");
const editing = ref(false);
const draftTitle = ref("");
const draftContent = ref("");
const reportType = ref<ReportRecord["reportType"]>("daily");
const referenceDate = ref(today);
const loading = ref(false);
const error = ref("");
const message = ref("");
const searchQuery = ref("");
const typeFilter = ref<"all" | ReportRecord["reportType"]>("all");
const projectFilter = ref("全部项目");
const sourceOpen = ref(false);
const reportSources = ref<ReportSource[]>([]);
const backfillSummary = ref<HistoricalReportSummary | null>(null);
const selected = computed(() => reports.value.find((report) => report.id === selectedId.value) ?? null);
const reportProjects = computed(() => ["全部项目", ...new Set(reports.value.flatMap(report => report.contentMarkdown.split("\n").filter(line => line.startsWith("### ")).map(line => line.slice(4).trim())).filter(Boolean))]);
const filteredReports = computed(() => reports.value.filter(report => (typeFilter.value === "all" || report.reportType === typeFilter.value) && (projectFilter.value === "全部项目" || report.contentMarkdown.split("\n").some(line => line.trim() === `### ${projectFilter.value}`)) && (!searchQuery.value.trim() || `${report.title} ${report.contentMarkdown}`.toLowerCase().includes(searchQuery.value.trim().toLowerCase()))));
const lines = computed(() => selected.value?.contentMarkdown.split("\n") ?? []);

function reportLabel(type: ReportRecord["reportType"]) {
  return ({ daily: "日报", weekly: "周报", monthly: "月报" })[type];
}
function lineClass(line: string) {
  if (line.startsWith("# ")) return "md-h1";
  if (line.startsWith("## ")) return "md-h2";
  if (line.startsWith("> ")) return "md-quote";
  if (line.startsWith("- ")) return "md-list";
  return line ? "md-text" : "md-space";
}
function cleanLine(line: string) { return line.replace(/^(##? |[-]>? |> )/, ""); }

async function refresh(preferredId?: string) {
  if (!isTauriRuntime()) return;
  reports.value = await listReports();
  selectedId.value = preferredId && reports.value.some((report) => report.id === preferredId)
    ? preferredId : (reports.value[0]?.id ?? "");
}
async function createReport() {
  if (!isTauriRuntime()) { error.value = "浏览器模式只展示示例，请在桌面端生成真实报告。"; return; }
  loading.value = true; error.value = "";
  try { const report = await generateReport(reportType.value, referenceDate.value); await refresh(report.id); await store.hydrate(); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function organizeHistory() {
  if (!isTauriRuntime()) { error.value = "请在桌面端整理真实历史。"; return; }
  loading.value = true; error.value = ""; message.value = "";
  try {
    backfillSummary.value = await backfillHistoricalReports();
    await refresh();
    await store.hydrate();
    const summary = backfillSummary.value;
    message.value = `已扫描 ${summary.normalFilesScanned} 个普通会话和 ${summary.archivedFilesScanned} 个归档会话，共 ${summary.conversationsTotal} 个对话、${summary.messagesTotal.toLocaleString()} 条消息；覆盖 ${summary.firstDate || '-'} 至 ${summary.lastDate || '-'}，新增 ${summary.dailyGenerated + summary.weeklyGenerated} 份、更新 ${summary.dailyUpdated + summary.weeklyUpdated} 份日/周总结${summary.lockedSkipped ? `，保留 ${summary.lockedSkipped} 份已锁定报告` : ''}。`;
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
function beginEdit() {
  if (!selected.value || selected.value.status === "locked") return;
  draftTitle.value = selected.value.title;
  draftContent.value = selected.value.contentMarkdown;
  editing.value = true;
}
async function persistEdit() {
  if (!selected.value) return;
  loading.value = true; error.value = "";
  try {
    const updated = await saveReport({ ...selected.value, title: draftTitle.value.trim(), contentMarkdown: draftContent.value });
    const index = reports.value.findIndex((report) => report.id === updated.id);
    if (index >= 0) reports.value[index] = updated;
    editing.value = false;
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function toggleLock() {
  if (!selected.value || !isTauriRuntime()) return;
  const locked = selected.value.status !== "locked";
  await setReportLocked(selected.value.id, locked);
  selected.value.status = locked ? "locked" : "draft";
  editing.value = false;
}
async function regenerate() {
  if (!selected.value) return;
  reportType.value = selected.value.reportType;
  referenceDate.value = selected.value.periodStart;
  await createReport();
}
async function refineWithAi() {
  if (!selected.value || !isTauriRuntime()) { error.value = "请在桌面端使用 AI 润色。"; return; }
  loading.value = true; error.value = "";
  try { selected.value.contentMarkdown = await refineReportWithAi(selected.value.id); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function openSources() {
  if (!selected.value || !isTauriRuntime()) return;
  loading.value=true; error.value="";
  try { reportSources.value=await getReportSources(selected.value.id); sourceOpen.value=true; }
  catch (cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
function openSource(source: ReportSource) {
  if (source.kind === "Codex 对话") void router.push(`/tokens?conversation=${source.id}`);
  else if (source.kind === "任务") void router.push(`/tasks?task=${source.id}`);
  else if (source.kind === "测试") void router.push(`/testing?run=${source.id}`);
  else if (source.kind === "TAPD 缺陷") void router.push(`/tapd?item=${source.id}`);
}
function exportWord() {
  if (!selected.value) return;
  const escaped = selected.value.contentMarkdown.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const html = `<html><head><meta charset="utf-8"><title>${selected.value.title}</title></head><body><pre style="font:14px/1.8 Microsoft YaHei;white-space:pre-wrap">${escaped}</pre></body></html>`;
  const url = URL.createObjectURL(new Blob(["\ufeff", html], { type: "application/msword;charset=utf-8" }));
  const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${selected.value.title}.doc`; anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

watch(() => route.query.report, (value) => { const id = String(value || ""); if (reports.value.some(report => report.id === id)) selectedId.value = id; }, { immediate: true });
watch(filteredReports, (value) => { if (!value.some(report => report.id === selectedId.value)) selectedId.value=value[0]?.id || ""; });
onMounted(async () => { await refresh(String(route.query.report || "") || undefined); const date = String(route.query.date || ""); const matched = reports.value.find(report => report.reportType === "daily" && report.periodStart === date); if (matched) selectedId.value = matched.id; });
</script>

<template>
  <div class="view">
    <header class="page-header">
      <div><h1>报告中心</h1><p>普通与归档 Codex 对话统一分析；按天回答“今天做了什么”，按周还原完整工作轨迹</p></div>
      <div class="report-create"><button class="button secondary" :disabled="loading" title="扫描普通与归档会话，并重建所有未锁定的日报和周报" @click="organizeHistory">▦ 扫描并重建全部历史</button><select v-model="reportType"><option value="daily">日报</option><option value="weekly">周报</option><option value="monthly">月报</option></select><input v-model="referenceDate" type="date"><button class="button primary" :disabled="loading" @click="createReport">{{ loading ? "处理中…" : "＋ 生成报告" }}</button></div>
    </header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="split-layout">
      <aside class="panel list-panel">
        <div class="report-filter"><label>⌕<input v-model="searchQuery" placeholder="搜索报告内容"></label><select v-model="typeFilter"><option value="all">全部</option><option value="daily">日报</option><option value="weekly">周报</option><option value="monthly">月报</option></select><select v-model="projectFilter"><option v-for="item in reportProjects" :key="item">{{ item }}</option></select></div>
        <button v-for="report in filteredReports" :key="report.id" :class="{ active: report.id === selectedId }" @click="selectedId = report.id; editing = false">
          <span>▤</span><div><b>{{ report.title }}</b><small>{{ reportLabel(report.reportType) }} · {{ report.status === 'locked' ? '已锁定' : '可编辑' }}</small></div><em>›</em>
        </button>
        <p v-if="!filteredReports.length" class="empty-state">没有符合条件的报告。</p>
      </aside>
      <article v-if="selected" class="panel report-paper">
        <div class="report-actions"><span>{{ reportLabel(selected.reportType) }} · {{ selected.status === 'locked' ? '已锁定' : '可编辑' }}</span><div><button class="button secondary small" :disabled="loading" @click="openSources">◎ 查看来源</button><button class="button secondary small" :disabled="selected.status === 'locked'" @click="beginEdit">✎ 编辑</button><button class="button secondary small" :disabled="selected.status === 'locked' || loading" @click="refineWithAi">✦ AI 润色</button><button class="button secondary small" @click="toggleLock">{{ selected.status === 'locked' ? '解锁' : '▣ 锁定' }}</button><button class="button secondary small" :disabled="selected.status === 'locked'" @click="regenerate">↻ 重新生成</button><button class="button primary small" @click="exportWord">⇩ 导出 Word</button></div></div>
        <div v-if="editing" class="report-editor"><input v-model="draftTitle"><textarea v-model="draftContent"></textarea><div><button class="button secondary" @click="editing = false">取消</button><button class="button primary" :disabled="loading" @click="persistEdit">保存修改</button></div></div>
        <div v-else class="paper markdown-paper"><p v-for="(line,index) in lines" :key="index" :class="lineClass(line)">{{ cleanLine(line) }}</p></div>
      </article>
      <article v-else class="panel report-paper empty-report"><b>还没有可查看的报告</b><p>首次使用时先导入 Codex、扫描 Git，再生成报告，内容会更完整。</p></article>
    </section>
    <div v-if="sourceOpen" class="activity-backdrop" @click.self="sourceOpen=false"><aside class="activity-drawer panel report-source-drawer"><header><div><h2>报告证据链</h2><p>{{ selected?.periodStart }}—{{ selected?.periodEnd }} · 数据截至 {{ selected?.updatedAt ? new Date(selected.updatedAt).toLocaleString('zh-CN') : '未知' }}</p></div><button class="icon-button" @click="sourceOpen=false">×</button></header><div class="report-evidence-note"><b>来源事实</b><span>以下记录来自本地 Codex、Git、任务、测试和 TAPD；报告中的功能描述是基于这些事实自动归纳。</span></div><div class="report-source-summary"><span>Codex {{ reportSources.filter(item=>item.kind==='Codex 对话').length }}</span><span>Git {{ reportSources.filter(item=>item.kind==='Git 提交').length }}</span><span>任务 {{ reportSources.filter(item=>item.kind==='任务').length }}</span><span>测试 {{ reportSources.filter(item=>item.kind==='测试').length }}</span><span>TAPD {{ reportSources.filter(item=>item.kind==='TAPD 缺陷').length }}</span></div><div class="report-source-list"><button v-for="item in reportSources" :key="`${item.kind}:${item.id}`" @click="openSource(item)"><i>{{ item.kind }}</i><span><b>{{ item.title }}</b><small>{{ item.project }} · {{ item.date }} · {{ item.detail }}</small></span><em>{{ item.kind === 'Git 提交' ? '来源记录' : '查看 →' }}</em></button><p v-if="!reportSources.length">该报告周期没有可关联的本地来源。</p></div></aside></div>
  </div>
</template>
