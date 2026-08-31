<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { exportTestReportMarkdown, exportTestReportPdf, getExistingTestReportPdf, isTauriRuntime, openTestReportPdf, readTestArtifact, type TestRun } from "../services/backend";

const props = defineProps<{ run: TestRun | null; title: string; fallbackMarkdown?: string }>();
const emit = defineEmits<{ close: [] }>();
const router = useRouter();
const exporting = ref(false);
const exportingMarkdown = ref(false);
const checkingPdf = ref(false);
const openingPdf = ref(false);
const exportMessage = ref("");
const exportError = ref("");
const exportedPdfPath = ref("");
const screenshotSources = ref<Record<string, string>>({});
const previewScreenshot = ref<{ src: string; name: string } | null>(null);

const problemScenarios = computed(() => props.run?.scenarioResults.filter((item) => ["failed", "blocked"].includes(item.status)) ?? []);
const otherScenarios = computed(() => props.run?.scenarioResults.filter((item) => !["failed", "blocked"].includes(item.status)) ?? []);
const screenshotCount = computed(() => problemScenarios.value.flatMap((item) => item.artifacts).filter((item) => item.kind === "screenshot").length);
const hasStructuredReport = computed(() => Boolean(props.run?.scenarioResults.length));
const legacyContent = computed(() => props.fallbackMarkdown || props.run?.reportMarkdown || props.run?.errorMessage || "该报告没有可显示内容。");

type LegacyBlock =
  | { type: "paragraph" | "bullet" | "ordered" | "code"; text: string }
  | { type: "table"; rows: string[][] };
type LegacySection = { title: string; blocks: LegacyBlock[] };

const legacySections = computed<LegacySection[]>(() => {
  const sections: LegacySection[] = [];
  let current: LegacySection = { title: "报告摘要", blocks: [] };
  let tableRows: string[][] = [];
  let codeLines: string[] = [];
  let inCode = false;
  const flushTable = () => {
    if (tableRows.length) current.blocks.push({ type: "table", rows: tableRows });
    tableRows = [];
  };
  const flushCode = () => {
    if (codeLines.length) current.blocks.push({ type: "code", text: codeLines.join("\n") });
    codeLines = [];
  };
  const flushSection = () => {
    flushTable(); flushCode();
    if (current.blocks.length) sections.push(current);
  };
  for (const raw of legacyContent.value.split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (line.trim().startsWith("```")) {
      flushTable();
      if (inCode) flushCode();
      inCode = !inCode;
      continue;
    }
    if (inCode) { codeLines.push(line); continue; }
    const heading = line.match(/^#{1,4}\s+(.+)/);
    if (heading) {
      flushSection();
      current = { title: heading[1].replace(/^\d+\.\s*/, ""), blocks: [] };
      continue;
    }
    if (/^\|.*\|$/.test(line.trim())) {
      const cells = line.trim().slice(1, -1).split("|").map(item => item.trim());
      if (!cells.every(item => /^:?-+:?$/.test(item))) tableRows.push(cells);
      continue;
    }
    flushTable();
    const bullet = line.match(/^[-*]\s+(.+)/);
    if (bullet) { current.blocks.push({ type: "bullet", text: bullet[1] }); continue; }
    const ordered = line.match(/^\d+\.\s+(.+)/);
    if (ordered) { current.blocks.push({ type: "ordered", text: ordered[1] }); continue; }
    if (line.trim()) current.blocks.push({ type: "paragraph", text: line.replace(/^>\s?/, "") });
  }
  flushSection();
  return sections.length ? sections : [{ title: "报告内容", blocks: [{ type: "paragraph", text: "当前报告没有可解析的内容。" }] }];
});

function legacyTableRows(block: LegacyBlock) {
  return block.type === "table" ? block.rows : [];
}

function statusLabel(status?: TestRun["status"]) {
  return ({ queued: "等待执行", running: "执行中", passed: "测试通过", failed: "测试未通过", blocked: "环境阻塞", error: "执行异常", cancelled: "已取消" } as const)[status || "blocked"];
}

function scenarioStatusLabel(status: string) {
  return status === "passed" ? "通过" : status === "skipped" ? "跳过" : status === "blocked" ? "环境阻塞" : "失败";
}

function modeLabel(mode?: TestRun["mode"]) {
  return ({ mock: "功能测试（模拟接口）", real: "功能测试（真实接口）", "source-style": "页面源码与样式检查", "browser-style": "浏览器页面样式测试" } as const)[mode || "source-style"];
}

function durationLabel(value = 0) {
  if (value < 1000) return `${value} ms`;
  if (value < 60_000) return `${(value / 1000).toFixed(1)} 秒`;
  return `${Math.floor(value / 60_000)} 分 ${Math.round((value % 60_000) / 1000)} 秒`;
}

async function loadScreenshots() {
  screenshotSources.value = {};
  if (!props.run) return;
  const screenshots = problemScenarios.value.flatMap((item) => item.artifacts).filter((item) => item.kind === "screenshot");
  await Promise.all(screenshots.map(async (item) => {
    try { screenshotSources.value[item.path] = isTauriRuntime() ? await readTestArtifact(props.run!.id, item.path) : item.path; }
    catch { screenshotSources.value[item.path] = ""; }
  }));
}

async function exportPdf() {
  if (!props.run || !hasStructuredReport.value || exporting.value) return;
  exporting.value = true; exportMessage.value = ""; exportError.value = ""; exportedPdfPath.value = "";
  try {
    const path = await exportTestReportPdf(props.run.id);
    exportedPdfPath.value = path;
    exportMessage.value = `PDF 已保存：${path}`;
  } catch (cause) { exportError.value = String(cause); }
  finally { exporting.value = false; }
}

async function exportMarkdown() {
  if (!props.run || exportingMarkdown.value) return;
  exportingMarkdown.value = true; exportMessage.value = ""; exportError.value = "";
  try {
    const path = await exportTestReportMarkdown(props.run.id);
    exportMessage.value = `MD 已保存：${path}`;
  } catch (cause) { exportError.value = String(cause); }
  finally { exportingMarkdown.value = false; }
}

async function loadExistingPdf() {
  exportedPdfPath.value = "";
  if (!props.run || !hasStructuredReport.value || !isTauriRuntime()) return;
  const runId = props.run.id;
  checkingPdf.value = true;
  try {
    const path = await getExistingTestReportPdf(runId);
    if (props.run?.id === runId) exportedPdfPath.value = path || "";
  } catch (cause) { console.error("检查已导出的测试 PDF 失败", cause); }
  finally { if (props.run?.id === runId) checkingPdf.value = false; }
}

function handlePdfAction() {
  if (exportedPdfPath.value) void openPdf();
  else void exportPdf();
}

async function openPdf() {
  if (!exportedPdfPath.value || openingPdf.value) return;
  openingPdf.value = true; exportError.value = "";
  try { await openTestReportPdf(exportedPdfPath.value); }
  catch (cause) {
    exportError.value = `PDF 已导出，但打开失败：${String(cause)}`;
    await loadExistingPdf();
  }
  finally { openingPdf.value = false; }
}

function openTasks() {
  emit("close");
  router.push({ path: "/tasks", query: props.run ? { source: "test", run: props.run.id } : {} });
}

watch(() => props.run?.id, () => {
  previewScreenshot.value = null; exportedPdfPath.value = ""; exportMessage.value = ""; exportError.value = "";
  void loadScreenshots();
  void loadExistingPdf();
}, { immediate: true });
</script>

<template>
  <div class="activity-backdrop report-dialog-backdrop" @click.self="emit('close')">
    <section class="panel test-report-dialog test-report-v2">
      <header>
        <div class="report-title-block">
          <small>TEST REPORT</small>
          <h2>{{ title }}</h2>
          <p>{{ run ? `${run.project} · ${modeLabel(run.mode)} · ${new Date(run.startedAt).toLocaleString('zh-CN')}` : "历史 Markdown 报告" }}</p>
        </div>
        <div class="report-header-actions">
          <span v-if="run" class="report-result-pill" :class="run.status">{{ statusLabel(run.status) }}</span>
          <button class="button secondary small" data-testid="export-markdown" :disabled="!run || exportingMarkdown || checkingPdf || exporting || openingPdf" title="导出 Markdown 格式的测试报告" @click="exportMarkdown">{{ exportingMarkdown ? "导出中…" : "导出 MD" }}</button>
          <button class="button secondary small" data-testid="pdf-action" :disabled="!run || !hasStructuredReport || exportingMarkdown || checkingPdf || exporting || openingPdf" :title="exportedPdfPath ? '打开已经导出的 PDF' : hasStructuredReport ? '导出含场景与截图的 PDF' : '旧版文字报告暂不支持 PDF 导出'" @click="handlePdfAction">{{ checkingPdf ? "检查 PDF…" : openingPdf ? "打开中…" : exportedPdfPath ? "打开 PDF" : exporting ? "导出中…" : "导出 PDF" }}</button>
          <button class="icon-button" title="关闭报告" @click="emit('close')">×</button>
        </div>
      </header>

      <div v-if="exportMessage || exportError" class="report-export-message" :class="{ error: exportError }"><span>{{ exportError || exportMessage }}</span></div>

      <template v-if="run && hasStructuredReport">
        <section class="report-overview report-overview-v2">
          <article><small>执行结果</small><b :class="run.status">{{ statusLabel(run.status) }}</b><span>{{ run.errorMessage || (run.status === "passed" ? "所选场景达到预期" : "优先查看下方问题场景") }}</span></article>
          <article><small>场景统计</small><b>{{ run.totalCount }} 个</b><span>{{ run.passedCount }} 通过 · {{ run.failedCount }} 失败 · {{ run.skippedCount }} 跳过</span></article>
          <article><small>执行耗时</small><b>{{ durationLabel(run.durationMs) }}</b><span>{{ run.environmentSummary }}</span></article>
          <article><small>失败截图</small><b>{{ screenshotCount }} 张</b><span>{{ screenshotCount ? "已关联到对应问题场景" : "本次报告没有页面截图" }}</span></article>
        </section>

        <div class="report-body-v2">
          <section v-if="problemScenarios.length" class="problem-section">
            <header><div><small>NEEDS ATTENTION</small><h3>问题场景详情</h3><p>失败场景优先展示，包含目的、步骤、验证内容、错误信息和页面截图。</p></div><button class="button secondary small" @click="openTasks">打开整改任务</button></header>
            <article v-for="(scenario, index) in problemScenarios" :key="scenario.id" class="problem-card">
              <header><i>{{ String(index + 1).padStart(2, "0") }}</i><div><h4>{{ scenario.title }}</h4><p>{{ scenario.purpose }}</p></div><b :class="scenario.status">{{ scenarioStatusLabel(scenario.status) }}</b></header>
              <pre v-if="scenario.errorMessage" class="scenario-error">{{ scenario.errorMessage }}</pre>
              <div class="scenario-detail-grid">
                <section><h5>测试步骤</h5><ol><li v-for="item in scenario.steps" :key="item">{{ item }}</li></ol></section>
                <section><h5>验证内容</h5><ul><li v-for="item in scenario.checks" :key="item">{{ item }}</li></ul></section>
              </div>
              <div v-if="scenario.artifacts.some(item => item.kind === 'screenshot')" class="scenario-screenshots">
                <h5>失败页面截图</h5>
                <figure v-for="item in scenario.artifacts.filter(value => value.kind === 'screenshot')" :key="item.path">
                  <button v-if="screenshotSources[item.path]" class="screenshot-preview-button" title="点击查看原尺寸大图" @click="previewScreenshot={src:screenshotSources[item.path],name:item.name}"><img :src="screenshotSources[item.path]" :alt="item.name"><span>点击查看大图</span></button>
                  <div v-else class="screenshot-unavailable"><b>截图文件当前不可用</b><span>历史截图可能已被项目测试清理；报告仍保留原始路径供核对。</span></div>
                  <figcaption>{{ item.name }}<small>{{ item.path }}</small></figcaption>
                </figure>
              </div>
            </article>
          </section>

          <section v-else class="report-success-state"><i>✓</i><div><h3>所选场景全部通过</h3><p>当前没有需要优先处理的问题场景。</p></div></section>

          <section class="completed-section">
            <header><div><small>ALL SCENARIOS</small><h3>全部场景</h3></div><span>{{ otherScenarios.length }} 项</span></header>
            <details v-for="scenario in otherScenarios" :key="scenario.id" class="scenario-row">
              <summary><span :class="scenario.status"></span><b>{{ scenario.title }}</b><em>{{ scenarioStatusLabel(scenario.status) }} · {{ durationLabel(scenario.durationMs) }}</em></summary>
              <p>{{ scenario.purpose }}</p>
              <div><ol><li v-for="item in scenario.steps" :key="item">{{ item }}</li></ol><ul><li v-for="item in scenario.checks" :key="item">{{ item }}</li></ul></div>
            </details>
          </section>

          <details v-if="run.outputExcerpt" class="raw-output"><summary>查看原始命令输出</summary><pre>{{ run.outputExcerpt }}</pre></details>
        </div>
      </template>
      <div v-else class="legacy-report-readable">
        <div class="legacy-report-notice"><b>历史报告兼容视图</b><span>已把旧版 Markdown 报告按章节、清单和表格重新排版；原始结论与内容不做改写。</span></div>
        <section v-for="(section, sectionIndex) in legacySections" :key="`${section.title}-${sectionIndex}`" class="legacy-section">
          <h3>{{ section.title }}</h3>
          <template v-for="(block, blockIndex) in section.blocks" :key="blockIndex">
            <p v-if="block.type==='paragraph'">{{ block.text }}</p>
            <div v-else-if="block.type==='bullet'" class="legacy-list-item"><i>•</i><span>{{ block.text }}</span></div>
            <div v-else-if="block.type==='ordered'" class="legacy-list-item ordered"><i>{{ blockIndex + 1 }}</i><span>{{ block.text }}</span></div>
            <pre v-else-if="block.type==='code'">{{ block.text }}</pre>
            <div v-else class="legacy-table-wrap"><table><thead><tr><th v-for="(cell, cellIndex) in legacyTableRows(block)[0]" :key="cellIndex">{{ cell }}</th></tr></thead><tbody><tr v-for="(row, rowIndex) in legacyTableRows(block).slice(1)" :key="rowIndex"><td v-for="(cell, cellIndex) in row" :key="cellIndex">{{ cell }}</td></tr></tbody></table></div>
          </template>
        </section>
      </div>
    </section>
    <div v-if="previewScreenshot" class="screenshot-lightbox" @click.self="previewScreenshot=null">
      <header><b>{{ previewScreenshot.name }}</b><button class="icon-button" title="关闭大图" @click="previewScreenshot=null">×</button></header>
      <img :src="previewScreenshot.src" :alt="previewScreenshot.name">
    </div>
  </div>
</template>

<style scoped>
.test-report-v2{width:min(1160px,calc(100vw - 70px));height:min(880px,calc(100vh - 64px));display:flex;flex-direction:column;overflow:hidden;background:var(--surface)}
.test-report-v2>header{min-height:84px;display:flex;align-items:center;gap:18px;padding:0 20px;border-bottom:1px solid var(--line)}
.report-title-block{min-width:0;flex:1}.report-title-block small,.problem-section>header small,.completed-section>header small{color:var(--primary);font-size:8px;letter-spacing:2px}.report-title-block h2{max-width:100%;margin:5px 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.report-title-block p{margin:0;color:var(--muted)}
.report-header-actions{margin-left:auto;display:flex;align-items:center;justify-content:flex-end;gap:9px;white-space:nowrap}.report-result-pill{padding:8px 11px;border-radius:8px;background:var(--surface-2);font-weight:800}.report-result-pill.passed{color:var(--success);background:color-mix(in srgb,var(--success) 13%,transparent)}.report-result-pill.failed,.report-result-pill.error{color:var(--danger);background:color-mix(in srgb,var(--danger) 13%,transparent)}.report-result-pill.blocked,.report-result-pill.cancelled{color:var(--warning);background:color-mix(in srgb,var(--warning) 13%,transparent)}
.report-export-message{display:flex;align-items:center;gap:12px;padding:9px 20px;border-bottom:1px solid var(--line);background:color-mix(in srgb,var(--success) 8%,transparent);color:var(--success)}.report-export-message span{min-width:0;flex:1;word-break:break-all}.report-export-message.error{background:color-mix(in srgb,var(--danger) 8%,transparent);color:var(--danger)}.report-export-message .button{flex:0 0 auto}
.report-overview-v2{grid-template-columns:repeat(4,minmax(0,1fr));padding:14px 18px}.report-overview-v2 article{min-height:96px}.report-overview-v2 b.blocked,.report-overview-v2 b.cancelled{color:var(--warning)}.report-overview-v2 b.error{color:var(--danger)}.report-overview-v2 span{max-height:34px;overflow:hidden;line-height:1.5}
.report-body-v2{flex:1;overflow:auto;padding:0 18px 22px}.problem-section{margin-top:4px}.problem-section>header,.completed-section>header{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:14px 0 10px}.problem-section h3,.completed-section h3{margin:4px 0}.problem-section>header p{margin:0;color:var(--muted)}
.problem-card{margin-bottom:12px;border:1px solid color-mix(in srgb,var(--danger) 42%,var(--line));border-radius:11px;overflow:hidden;background:color-mix(in srgb,var(--danger) 3%,var(--surface-2))}.problem-card>header{display:flex;align-items:flex-start;gap:11px;padding:13px 14px;border-bottom:1px solid var(--line)}.problem-card>header i{display:grid;place-items:center;width:28px;height:28px;border-radius:7px;background:color-mix(in srgb,var(--danger) 12%,transparent);color:var(--danger);font-style:normal}.problem-card>header div{min-width:0;flex:1}.problem-card h4{margin:0 0 5px}.problem-card header p{margin:0;color:var(--muted)}.problem-card>header b{padding:5px 8px;border-radius:99px;color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.problem-card>header b.blocked{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}
.scenario-error{margin:12px 14px 0;padding:11px;border-radius:8px;background:#161923;color:#ffb8be;white-space:pre-wrap;word-break:break-word;font:11px/1.6 Consolas,monospace}.scenario-detail-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:13px 14px}.scenario-detail-grid section{padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface)}.scenario-detail-grid h5,.scenario-screenshots h5{margin:0 0 8px}.scenario-detail-grid ol,.scenario-detail-grid ul{margin:0;padding-left:19px;line-height:1.8;color:var(--muted)}
.scenario-screenshots{padding:0 14px 14px}.scenario-screenshots figure{margin:9px 0 0;padding:10px;border:1px solid var(--line);border-radius:9px;background:var(--surface)}.screenshot-preview-button{position:relative;display:block;width:100%;padding:0;border:0;border-radius:7px;overflow:hidden;background:var(--surface-2);cursor:zoom-in}.screenshot-preview-button img{display:block;max-width:100%;max-height:560px;margin:auto}.screenshot-preview-button span{position:absolute;right:10px;bottom:10px;padding:6px 9px;border-radius:7px;background:rgba(17,20,29,.82);color:#fff;font-size:11px}.scenario-screenshots figcaption{display:flex;flex-direction:column;gap:4px;margin-top:8px}.scenario-screenshots figcaption small{color:var(--muted);word-break:break-all}.screenshot-unavailable{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:5px;min-height:130px;padding:18px;color:var(--muted);text-align:center;background:var(--surface-2)}.screenshot-unavailable b{color:var(--warning)}
.report-success-state{display:flex;align-items:center;gap:12px;margin:15px 0;padding:15px;border:1px solid color-mix(in srgb,var(--success) 35%,var(--line));border-radius:10px;background:color-mix(in srgb,var(--success) 7%,transparent)}.report-success-state i{display:grid;place-items:center;width:32px;height:32px;border-radius:50%;background:var(--success);color:white;font-style:normal}.report-success-state h3,.report-success-state p{margin:0}.report-success-state p{margin-top:4px;color:var(--muted)}
.completed-section>header span{color:var(--muted)}.scenario-row{border-top:1px solid var(--line)}.scenario-row summary{display:grid;grid-template-columns:9px minmax(0,1fr) auto;align-items:center;gap:9px;padding:12px 4px;cursor:pointer}.scenario-row summary>span{width:8px;height:8px;border-radius:50%;background:var(--muted)}.scenario-row summary>span.passed{background:var(--success)}.scenario-row summary>span.skipped{background:var(--warning)}.scenario-row summary em{color:var(--muted);font-style:normal}.scenario-row>p{margin:0 24px 9px;color:var(--muted)}.scenario-row>div{display:grid;grid-template-columns:1fr 1fr;gap:14px;margin:0 24px 12px;padding:10px;border-radius:8px;background:var(--surface-2)}.scenario-row ol,.scenario-row ul{margin:0;padding-left:18px;line-height:1.7}
.raw-output{margin-top:15px;border:1px solid var(--line);border-radius:9px;overflow:hidden}.raw-output summary{padding:11px 13px;cursor:pointer;color:var(--primary)}.raw-output pre{margin:0;padding:14px;background:#0d1118;color:#c9d3e7;white-space:pre-wrap;word-break:break-word;font:11px/1.65 Consolas,monospace}
.legacy-report-readable{flex:1;overflow:auto;padding:18px 20px 24px;background:var(--surface-2)}.legacy-report-notice{display:flex;align-items:center;gap:12px;margin-bottom:13px;padding:12px 14px;border:1px solid color-mix(in srgb,var(--primary) 28%,var(--line));border-radius:9px;background:color-mix(in srgb,var(--primary) 6%,var(--surface))}.legacy-report-notice b{white-space:nowrap;color:var(--primary)}.legacy-report-notice span{color:var(--muted)}.legacy-section{margin-bottom:12px;padding:15px 16px;border:1px solid var(--line);border-radius:10px;background:var(--surface)}.legacy-section h3{margin:0 0 11px;padding-bottom:9px;border-bottom:1px solid var(--line)}.legacy-section p{margin:7px 0;line-height:1.7}.legacy-list-item{display:grid;grid-template-columns:18px minmax(0,1fr);gap:7px;margin:6px 0;line-height:1.65}.legacy-list-item i{color:var(--primary);font-style:normal;font-weight:800}.legacy-list-item.ordered i{display:grid;place-items:center;width:18px;height:18px;margin-top:2px;border-radius:50%;background:color-mix(in srgb,var(--primary) 12%,transparent);font-size:9px}.legacy-section pre{padding:11px;border-radius:8px;background:#111722;color:#d8e1f1;white-space:pre-wrap;word-break:break-word;font:11px/1.65 Consolas,monospace}.legacy-table-wrap{max-width:100%;margin:9px 0;overflow:auto;border:1px solid var(--line);border-radius:8px}.legacy-table-wrap table{width:100%;border-collapse:collapse}.legacy-table-wrap th,.legacy-table-wrap td{padding:9px 10px;border-bottom:1px solid var(--line);text-align:left;white-space:nowrap}.legacy-table-wrap th{background:var(--surface-2);color:var(--muted)}.legacy-table-wrap tbody tr:last-child td{border-bottom:0}
.screenshot-lightbox{position:fixed;inset:22px;z-index:20;display:flex;flex-direction:column;padding:12px;border:1px solid var(--line);border-radius:12px;background:rgba(10,13,20,.96);box-shadow:0 24px 80px rgba(0,0,0,.45)}.screenshot-lightbox header{display:flex;align-items:center;gap:12px;padding:0 4px 10px;color:#fff}.screenshot-lightbox header b{min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.screenshot-lightbox img{min-width:0;min-height:0;max-width:100%;max-height:calc(100vh - 92px);margin:auto;object-fit:contain}
@media(max-width:900px){.test-report-v2{width:calc(100vw - 24px)}.report-overview-v2{grid-template-columns:1fr 1fr}.scenario-detail-grid,.scenario-row>div{grid-template-columns:1fr}.report-result-pill{display:none}}
</style>
