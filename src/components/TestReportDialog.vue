<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { exportTestReportPdf, isTauriRuntime, readTestArtifact, type TestRun } from "../services/backend";

const props = defineProps<{ run: TestRun | null; title: string; fallbackMarkdown?: string }>();
const emit = defineEmits<{ close: [] }>();
const router = useRouter();
const exporting = ref(false);
const exportMessage = ref("");
const exportError = ref("");
const screenshotSources = ref<Record<string, string>>({});

const problemScenarios = computed(() => props.run?.scenarioResults.filter((item) => ["failed", "blocked"].includes(item.status)) ?? []);
const otherScenarios = computed(() => props.run?.scenarioResults.filter((item) => !["failed", "blocked"].includes(item.status)) ?? []);
const screenshotCount = computed(() => problemScenarios.value.flatMap((item) => item.artifacts).filter((item) => item.kind === "screenshot").length);

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
  if (!props.run || exporting.value) return;
  exporting.value = true; exportMessage.value = ""; exportError.value = "";
  try {
    const path = await exportTestReportPdf(props.run.id);
    exportMessage.value = `PDF 已保存：${path}`;
  } catch (cause) { exportError.value = String(cause); }
  finally { exporting.value = false; }
}

function openTasks() {
  emit("close");
  router.push({ path: "/tasks", query: props.run ? { source: "test", run: props.run.id } : {} });
}

watch(() => props.run?.id, loadScreenshots, { immediate: true });
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
          <button class="button secondary small" :disabled="!run || exporting" @click="exportPdf">{{ exporting ? "导出中…" : "导出 PDF" }}</button>
          <button class="icon-button" title="关闭报告" @click="emit('close')">×</button>
        </div>
      </header>

      <div v-if="exportMessage || exportError" class="report-export-message" :class="{ error: exportError }">{{ exportError || exportMessage }}</div>

      <template v-if="run">
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
                  <img v-if="screenshotSources[item.path]" :src="screenshotSources[item.path]" :alt="item.name">
                  <div v-else class="screenshot-unavailable">截图无法读取或已被项目清理</div>
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
      <pre v-else class="legacy-report">{{ fallbackMarkdown || "该报告没有可显示内容。" }}</pre>
    </section>
  </div>
</template>

<style scoped>
.test-report-v2{width:min(1160px,calc(100vw - 70px));height:min(880px,calc(100vh - 64px));display:flex;flex-direction:column;overflow:hidden;background:var(--surface)}
.test-report-v2>header{min-height:84px;display:flex;align-items:center;gap:18px;padding:0 20px;border-bottom:1px solid var(--line)}
.report-title-block{min-width:0;flex:1}.report-title-block small,.problem-section>header small,.completed-section>header small{color:var(--primary);font-size:8px;letter-spacing:2px}.report-title-block h2{max-width:100%;margin:5px 0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.report-title-block p{margin:0;color:var(--muted)}
.report-header-actions{margin-left:auto;display:flex;align-items:center;justify-content:flex-end;gap:9px;white-space:nowrap}.report-result-pill{padding:8px 11px;border-radius:8px;background:var(--surface-2);font-weight:800}.report-result-pill.passed{color:var(--success);background:color-mix(in srgb,var(--success) 13%,transparent)}.report-result-pill.failed,.report-result-pill.error{color:var(--danger);background:color-mix(in srgb,var(--danger) 13%,transparent)}.report-result-pill.blocked,.report-result-pill.cancelled{color:var(--warning);background:color-mix(in srgb,var(--warning) 13%,transparent)}
.report-export-message{padding:9px 20px;border-bottom:1px solid var(--line);background:color-mix(in srgb,var(--success) 8%,transparent);color:var(--success);word-break:break-all}.report-export-message.error{background:color-mix(in srgb,var(--danger) 8%,transparent);color:var(--danger)}
.report-overview-v2{grid-template-columns:repeat(4,minmax(0,1fr));padding:14px 18px}.report-overview-v2 article{min-height:96px}.report-overview-v2 b.blocked,.report-overview-v2 b.cancelled{color:var(--warning)}.report-overview-v2 b.error{color:var(--danger)}.report-overview-v2 span{max-height:34px;overflow:hidden;line-height:1.5}
.report-body-v2{flex:1;overflow:auto;padding:0 18px 22px}.problem-section{margin-top:4px}.problem-section>header,.completed-section>header{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:14px 0 10px}.problem-section h3,.completed-section h3{margin:4px 0}.problem-section>header p{margin:0;color:var(--muted)}
.problem-card{margin-bottom:12px;border:1px solid color-mix(in srgb,var(--danger) 42%,var(--line));border-radius:11px;overflow:hidden;background:color-mix(in srgb,var(--danger) 3%,var(--surface-2))}.problem-card>header{display:flex;align-items:flex-start;gap:11px;padding:13px 14px;border-bottom:1px solid var(--line)}.problem-card>header i{display:grid;place-items:center;width:28px;height:28px;border-radius:7px;background:color-mix(in srgb,var(--danger) 12%,transparent);color:var(--danger);font-style:normal}.problem-card>header div{min-width:0;flex:1}.problem-card h4{margin:0 0 5px}.problem-card header p{margin:0;color:var(--muted)}.problem-card>header b{padding:5px 8px;border-radius:99px;color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.problem-card>header b.blocked{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}
.scenario-error{margin:12px 14px 0;padding:11px;border-radius:8px;background:#161923;color:#ffb8be;white-space:pre-wrap;word-break:break-word;font:11px/1.6 Consolas,monospace}.scenario-detail-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:13px 14px}.scenario-detail-grid section{padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface)}.scenario-detail-grid h5,.scenario-screenshots h5{margin:0 0 8px}.scenario-detail-grid ol,.scenario-detail-grid ul{margin:0;padding-left:19px;line-height:1.8;color:var(--muted)}
.scenario-screenshots{padding:0 14px 14px}.scenario-screenshots figure{margin:9px 0 0;padding:10px;border:1px solid var(--line);border-radius:9px;background:var(--surface)}.scenario-screenshots img{display:block;max-width:100%;max-height:560px;margin:auto;border-radius:6px}.scenario-screenshots figcaption{display:flex;flex-direction:column;gap:4px;margin-top:8px}.scenario-screenshots figcaption small{color:var(--muted);word-break:break-all}.screenshot-unavailable{display:grid;place-items:center;min-height:120px;color:var(--muted);background:var(--surface-2)}
.report-success-state{display:flex;align-items:center;gap:12px;margin:15px 0;padding:15px;border:1px solid color-mix(in srgb,var(--success) 35%,var(--line));border-radius:10px;background:color-mix(in srgb,var(--success) 7%,transparent)}.report-success-state i{display:grid;place-items:center;width:32px;height:32px;border-radius:50%;background:var(--success);color:white;font-style:normal}.report-success-state h3,.report-success-state p{margin:0}.report-success-state p{margin-top:4px;color:var(--muted)}
.completed-section>header span{color:var(--muted)}.scenario-row{border-top:1px solid var(--line)}.scenario-row summary{display:grid;grid-template-columns:9px minmax(0,1fr) auto;align-items:center;gap:9px;padding:12px 4px;cursor:pointer}.scenario-row summary>span{width:8px;height:8px;border-radius:50%;background:var(--muted)}.scenario-row summary>span.passed{background:var(--success)}.scenario-row summary>span.skipped{background:var(--warning)}.scenario-row summary em{color:var(--muted);font-style:normal}.scenario-row>p{margin:0 24px 9px;color:var(--muted)}.scenario-row>div{display:grid;grid-template-columns:1fr 1fr;gap:14px;margin:0 24px 12px;padding:10px;border-radius:8px;background:var(--surface-2)}.scenario-row ol,.scenario-row ul{margin:0;padding-left:18px;line-height:1.7}
.raw-output{margin-top:15px;border:1px solid var(--line);border-radius:9px;overflow:hidden}.raw-output summary{padding:11px 13px;cursor:pointer;color:var(--primary)}.raw-output pre,.legacy-report{margin:0;padding:14px;background:#0d1118;color:#c9d3e7;white-space:pre-wrap;word-break:break-word;font:11px/1.65 Consolas,monospace}.legacy-report{flex:1;overflow:auto}
@media(max-width:900px){.test-report-v2{width:calc(100vw - 24px)}.report-overview-v2{grid-template-columns:1fr 1fr}.scenario-detail-grid,.scenario-row>div{grid-template-columns:1fr}.report-result-pill{display:none}}
</style>
