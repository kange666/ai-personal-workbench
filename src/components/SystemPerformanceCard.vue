<script setup lang="ts">
import { computed } from "vue";
import type { SystemPerformanceSnapshot } from "../services/backend";
import {
  findPerformanceGpu,
  formatPerformancePercent,
  formatPerformanceRate,
  type SystemPerformanceHistoryPoint,
} from "../utils/systemPerformance";

const props = defineProps<{
  snapshot: SystemPerformanceSnapshot;
  history: SystemPerformanceHistoryPoint[];
  optimizationState: "idle" | "loading" | "done" | "error";
  optimizationMessage: string;
}>();
defineEmits<{ open: []; optimize: [] }>();

const integratedGpu = computed(() => findPerformanceGpu(props.snapshot, "integrated"));
const discreteGpu = computed(() => findPerformanceGpu(props.snapshot, "discrete"));
const cpuPoints = computed(() => {
  const values = props.history.slice(-20).map(point => point.cpu).filter((value): value is number => value !== null);
  if (!values.length) return "";
  const samples = values.length === 1 ? [values[0], values[0]] : values;
  return samples.map((value, index) => `${index / (samples.length - 1) * 100},${28 - Math.min(100, Math.max(0, value)) * .24}`).join(" ");
});
const networkText = computed(() => {
  const down = formatPerformanceRate(props.snapshot.networkDownloadBytesPerSecond, true);
  const up = formatPerformanceRate(props.snapshot.networkUploadBytesPerSecond, true);
  return `↓${down} ↑${up}`;
});
</script>

<template>
  <article class="rail-performance-card" :class="{ unavailable:!snapshot.available }">
    <header class="rail-performance-head"><b>系统性能</b><em><i></i>{{ snapshot.available ? '实时' : '暂不可用' }}</em><button type="button" :class="optimizationState" :disabled="optimizationState==='loading'" :title="optimizationMessage || '安全释放星枢工作台暂时不用的内存'" @click.stop="$emit('optimize')">{{ optimizationState === 'loading' ? '优化中…' : optimizationState === 'done' ? (optimizationMessage || '已优化') : optimizationState === 'error' ? '重试' : '优化内存' }}</button><strong>›</strong></header>
    <button class="rail-performance-grid" type="button" aria-label="打开系统性能详情" @click="$emit('open')">
      <span class="rail-performance-cell cpu"><small>CPU</small><b>{{ formatPerformancePercent(snapshot.cpuUsagePercent) }}</b><svg v-if="cpuPoints" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true"><polyline :points="cpuPoints" /></svg></span>
      <span class="rail-performance-cell memory"><small>内存</small><b>{{ formatPerformancePercent(snapshot.memoryUsagePercent) }}</b><i><em :style="{width:`${snapshot.memoryUsagePercent || 0}%`}"></em></i></span>
      <span class="rail-performance-cell discrete"><small>GPU 1 · 独显</small><b>{{ formatPerformancePercent(discreteGpu?.usagePercent) }}</b><i><em :style="{width:`${discreteGpu?.usagePercent || 0}%`}"></em></i></span>
      <span class="rail-performance-cell network"><small>网络</small><b>{{ networkText }}</b></span>
      <span class="rail-performance-cell integrated"><small>GPU 0 · 集显</small><b>{{ formatPerformancePercent(integratedGpu?.usagePercent) }}</b></span>
      <span class="rail-performance-cell disk"><small>磁盘</small><b>{{ formatPerformancePercent(snapshot.diskUsagePercent) }}</b></span>
    </button>
  </article>
</template>

<style scoped>
.rail-performance-card{width:100%;padding:0;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);color:var(--text);overflow:hidden;text-align:left;transition:border-color .16s ease,background .16s ease}.rail-performance-card:hover{border-color:color-mix(in srgb,var(--primary) 48%,var(--line));background:color-mix(in srgb,var(--primary) 4%,var(--surface-2))}.rail-performance-card.unavailable{opacity:.72}.rail-performance-head{height:36px;padding:0 8px 0 10px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:7px}.rail-performance-head>b{margin-right:auto;font-size:11px}.rail-performance-head>em{display:flex;align-items:center;gap:4px;color:var(--success);font-size:8px;font-style:normal}.rail-performance-head>em>i{width:5px;height:5px;border-radius:50%;background:currentColor}.rail-performance-head>button{height:23px;padding:0 7px;border:1px solid color-mix(in srgb,var(--primary) 35%,var(--line));border-radius:6px;background:color-mix(in srgb,var(--primary) 8%,transparent);color:var(--primary);font-size:8px;cursor:pointer;white-space:nowrap}.rail-performance-head>button:hover,.rail-performance-head>button:focus-visible{border-color:var(--primary);outline:0}.rail-performance-head>button.done{border-color:color-mix(in srgb,var(--success) 38%,var(--line));color:var(--success)}.rail-performance-head>button.error{border-color:color-mix(in srgb,var(--danger) 38%,var(--line));color:var(--danger)}.rail-performance-head>button:disabled{cursor:wait;opacity:.7}.rail-performance-head strong{color:var(--muted);font-size:14px;font-weight:400}.rail-performance-grid{width:100%;height:116px;padding:0;border:0;background:transparent;color:inherit;display:grid;grid-template-columns:repeat(3,minmax(0,1fr));grid-template-rows:repeat(2,1fr);text-align:left;cursor:pointer}.rail-performance-grid:focus-visible{outline:2px solid color-mix(in srgb,var(--primary) 55%,transparent);outline-offset:-2px}.rail-performance-cell{min-width:0;padding:7px 8px 6px;border-right:1px solid var(--line);border-bottom:1px solid var(--line);display:flex;flex-direction:column;overflow:hidden}.rail-performance-cell:nth-child(3n){border-right:0}.rail-performance-cell:nth-child(n+4){border-bottom:0}.rail-performance-cell small,.rail-performance-cell>b{display:block;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.rail-performance-cell small{color:var(--muted);font-size:8px;line-height:11px}.rail-performance-cell>b{margin-top:2px;font-size:13px;line-height:16px;font-variant-numeric:tabular-nums}.rail-performance-cell.network>b{font-size:8px;letter-spacing:-.25px}.rail-performance-cell>i{display:block;flex:0 0 3px;width:100%;height:3px;margin-top:7px;border-radius:2px;background:var(--surface-3);overflow:hidden}.rail-performance-cell>i>em{display:block;height:100%;border-radius:inherit;background:var(--primary)}.rail-performance-cell.discrete>i>em{background:color-mix(in srgb,var(--primary) 78%,#9e7cff)}.rail-performance-cell svg{display:block;flex:0 0 9px;width:100%;height:9px;margin-top:5px}.rail-performance-cell polyline{fill:none;stroke:var(--primary);stroke-width:1.4;vector-effect:non-scaling-stroke}
</style>
