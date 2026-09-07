<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { SystemPerformanceSnapshot } from "../services/backend";
import {
  findPerformanceGpu,
  formatPerformanceBytes,
  formatPerformancePercent,
  formatPerformanceRate,
  type SystemPerformanceHistoryPoint,
} from "../utils/systemPerformance";

type ChartTab = "utilization" | "network";
const props = defineProps<{
  open: boolean;
  snapshot: SystemPerformanceSnapshot;
  history: SystemPerformanceHistoryPoint[];
}>();
const emit = defineEmits<{ close: [] }>();
const chartTab = ref<ChartTab>("utilization");
const closeButton = ref<HTMLButtonElement | null>(null);
const integratedGpu = computed(() => findPerformanceGpu(props.snapshot, "integrated"));
const discreteGpu = computed(() => findPerformanceGpu(props.snapshot, "discrete"));

const utilizationSeries = computed(() => [
  { key:"cpu", label:"CPU", color:"#5aa8ff", values:props.history.map(point => point.cpu) },
  { key:"memory", label:"内存", color:"#8c68ee", values:props.history.map(point => point.memory) },
  { key:"disk", label:"磁盘", color:"#43cf8d", values:props.history.map(point => point.disk) },
  { key:"integrated", label:"集显", color:"#36bfd1", values:props.history.map(point => point.integratedGpu) },
  { key:"discrete", label:"独显", color:"#9b61ec", values:props.history.map(point => point.discreteGpu) },
]);
const networkSeries = computed(() => [
  { key:"download", label:"下载", color:"#5aa8ff", values:props.history.map(point => point.networkDownload) },
  { key:"upload", label:"上传", color:"#43cf8d", values:props.history.map(point => point.networkUpload) },
]);
const activeSeries = computed(() => chartTab.value === "utilization" ? utilizationSeries.value : networkSeries.value);
const chartMaximum = computed(() => {
  if (chartTab.value === "utilization") return 100;
  const values = networkSeries.value.flatMap(series => series.values).filter((value): value is number => value !== null);
  return Math.max(1024, ...values) * 1.08;
});
const axisLabels = computed(() => chartTab.value === "utilization"
  ? ["100%","75%","50%","25%","0%"]
  : [1,.75,.5,.25,0].map(value => formatPerformanceRate(chartMaximum.value * value)));

function seriesPoints(values:(number | null)[]) {
  const samples = values.slice(-60);
  if (!samples.length) return "";
  const safe = samples.length === 1 ? [samples[0],samples[0]] : samples;
  return safe.map((value,index) => {
    const x = 58 + index / (safe.length - 1) * 702;
    const y = 22 + (1 - Math.min(chartMaximum.value,Math.max(0,value || 0)) / chartMaximum.value) * 212;
    return `${x},${y}`;
  }).join(" ");
}
function gpuTemperature(value:number | null | undefined) {
  return value === null || value === undefined ? "--" : `${Math.round(value)}°C`;
}
function gpuMemory(gpu:ReturnType<typeof findPerformanceGpu>, kind:"shared"|"dedicated") {
  if (!gpu) return "暂无显存数据";
  const used = kind === "shared" ? gpu.sharedUsedBytes : gpu.dedicatedUsedBytes;
  const total = kind === "shared" ? gpu.sharedTotalBytes : gpu.dedicatedTotalBytes;
  if (used === null || used === undefined) return "暂无显存数据";
  return total ? `${formatPerformanceBytes(used)} / ${formatPerformanceBytes(total)}` : formatPerformanceBytes(used);
}
function handleKeydown(event:KeyboardEvent) {
  if (event.key === "Escape" && props.open) emit("close");
}
watch(() => props.open, open => {
  if (open) {
    chartTab.value="utilization";
    document.addEventListener("keydown",handleKeydown);
    void nextTick(() => closeButton.value?.focus());
  } else document.removeEventListener("keydown",handleKeydown);
});
onBeforeUnmount(() => document.removeEventListener("keydown",handleKeydown));
</script>

<template>
  <Teleport to="body">
    <Transition name="performance-dialog">
      <div v-if="open" class="performance-dialog-backdrop" role="presentation" @mousedown.self="$emit('close')">
        <section class="performance-dialog" role="dialog" aria-modal="true" aria-label="性能监测详情">
          <header class="performance-dialog-header"><div><h2>性能监测</h2><p>实时查看本机资源占用</p></div><span class="performance-dialog-live"><i></i>{{ snapshot.available ? '实时' : '暂不可用' }}</span><span class="performance-dialog-refresh">1 秒刷新⌄</span><button ref="closeButton" class="icon-button" type="button" title="关闭" aria-label="关闭性能监测" @click="$emit('close')">×</button></header>
          <div class="performance-summary">
            <span><small>CPU</small><b>{{ formatPerformancePercent(snapshot.cpuUsagePercent) }}</b></span>
            <span><small>内存</small><b>{{ formatPerformancePercent(snapshot.memoryUsagePercent) }}</b></span>
            <span><small>磁盘</small><b>{{ formatPerformancePercent(snapshot.diskUsagePercent) }}</b></span>
            <span><small>网络</small><b class="network">↓ {{ formatPerformanceRate(snapshot.networkDownloadBytesPerSecond) }}　↑ {{ formatPerformanceRate(snapshot.networkUploadBytesPerSecond) }}</b></span>
            <span><small>GPU 0 · 集显</small><b>{{ formatPerformancePercent(integratedGpu?.usagePercent) }}</b></span>
            <span><small>GPU 1 · 独显</small><b>{{ formatPerformancePercent(discreteGpu?.usagePercent) }}</b></span>
          </div>
          <article class="performance-chart-panel">
            <header><h3>{{ chartTab === 'utilization' ? '系统负载' : '网络吞吐' }} <small>· 最近 60 秒</small></h3><nav aria-label="性能图表类型"><button :class="{active:chartTab==='utilization'}" @click="chartTab='utilization'">利用率</button><button :class="{active:chartTab==='network'}" @click="chartTab='network'">网络</button></nav></header>
            <div class="performance-chart-legend"><span v-for="series in activeSeries" :key="series.key"><i :style="{background:series.color}"></i>{{ series.label }}</span></div>
            <div class="performance-chart">
              <svg viewBox="0 0 780 255" preserveAspectRatio="none" aria-label="最近六十秒性能趋势">
                <g class="performance-chart-grid"><line v-for="index in 5" :key="index" x1="58" x2="760" :y1="22+(index-1)*53" :y2="22+(index-1)*53" /></g>
                <polyline v-for="series in activeSeries" :key="series.key" :points="seriesPoints(series.values)" :stroke="series.color" />
              </svg>
              <div class="performance-y-axis"><span v-for="label in axisLabels" :key="label">{{ label }}</span></div>
              <div class="performance-x-axis"><span>-60s</span><span>-45s</span><span>-30s</span><span>-15s</span><span>现在</span></div>
            </div>
          </article>
          <div class="performance-details">
            <span><small>当前速度</small><b>{{ snapshot.cpuFrequencyMhz === null ? '--' : `${(snapshot.cpuFrequencyMhz/1000).toFixed(1)} GHz` }}</b></span>
            <span><small>可用内存</small><b>{{ formatPerformanceBytes(snapshot.memoryAvailableBytes) }}</b></span>
            <span><small>磁盘读写</small><b>{{ formatPerformanceRate((snapshot.diskReadBytesPerSecond || 0)+(snapshot.diskWriteBytesPerSecond || 0)) }}</b></span>
            <span><small>网络</small><b class="network">↓ {{ formatPerformanceRate(snapshot.networkDownloadBytesPerSecond) }}　↑ {{ formatPerformanceRate(snapshot.networkUploadBytesPerSecond) }}</b></span>
            <span><small>集显温度</small><b>{{ gpuTemperature(integratedGpu?.temperatureCelsius) }}</b><em>共享显存 {{ gpuMemory(integratedGpu,'shared') }}</em></span>
            <span><small>独显温度</small><b>{{ gpuTemperature(discreteGpu?.temperatureCelsius) }}</b><em>专用显存 {{ gpuMemory(discreteGpu,'dedicated') }}</em></span>
          </div>
          <p v-if="!snapshot.available" class="performance-dialog-error">{{ snapshot.message }}</p>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.performance-dialog-backdrop{position:fixed;inset:0;z-index:480;display:grid;place-items:center;padding:32px;background:rgba(3,7,12,.68);backdrop-filter:blur(2px)}.performance-dialog{width:min(1040px,calc(100vw - 64px));max-height:calc(100vh - 64px);padding:20px 24px;border:1px solid color-mix(in srgb,var(--primary) 28%,var(--line));border-radius:14px;background:color-mix(in srgb,var(--surface) 97%,#06101b);color:var(--text);box-shadow:0 28px 80px rgba(0,0,0,.5);overflow:auto;display:grid;gap:16px}.performance-dialog-header{min-height:50px;display:flex;align-items:center;gap:13px}.performance-dialog-header>div{margin-right:auto}.performance-dialog-header h2{margin:0;font-size:22px}.performance-dialog-header p{margin:5px 0 0;color:var(--muted);font-size:10px}.performance-dialog-live{display:flex;align-items:center;gap:6px;color:var(--success);font-size:10px}.performance-dialog-live i{width:7px;height:7px;border-radius:50%;background:currentColor;box-shadow:0 0 0 4px color-mix(in srgb,currentColor 12%,transparent)}.performance-dialog-refresh{height:34px;padding:0 12px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);display:flex;align-items:center;color:var(--muted);font-size:10px}.performance-summary{min-height:96px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);display:grid;grid-template-columns:repeat(6,minmax(0,1fr))}.performance-summary>span{min-width:0;padding:20px 14px;border-left:1px solid var(--line);display:flex;flex-direction:column;justify-content:center}.performance-summary>span:first-child{border-left:0}.performance-summary small,.performance-details small{color:var(--muted);font-size:10px}.performance-summary b{margin-top:7px;font-size:23px;font-variant-numeric:tabular-nums;white-space:nowrap}.performance-summary b.network,.performance-details b.network{font-size:11px}.performance-chart-panel{height:350px;padding:0 18px 14px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);display:grid;grid-template-rows:48px 24px minmax(0,1fr)}.performance-chart-panel>header{display:flex;align-items:center}.performance-chart-panel h3{margin:0;font-size:13px}.performance-chart-panel h3 small{color:var(--muted);font-weight:400}.performance-chart-panel nav{height:32px;margin-left:auto;border:1px solid var(--line);border-radius:7px;display:flex;overflow:hidden}.performance-chart-panel nav button{width:70px;border:0;border-left:1px solid var(--line);background:transparent;color:var(--muted);font-size:10px}.performance-chart-panel nav button:first-child{border-left:0}.performance-chart-panel nav button.active{background:var(--primary-soft);color:var(--primary)}.performance-chart-legend{display:flex;justify-content:center;gap:24px;color:var(--muted);font-size:9px}.performance-chart-legend span{display:flex;align-items:center;gap:5px}.performance-chart-legend i{width:14px;height:2px;border-radius:2px}.performance-chart{position:relative;min-height:0;padding-bottom:22px}.performance-chart svg{width:100%;height:100%;overflow:visible}.performance-chart-grid line{stroke:var(--line);stroke-dasharray:3 5;vector-effect:non-scaling-stroke}.performance-chart polyline{fill:none;stroke-width:1.7;vector-effect:non-scaling-stroke}.performance-y-axis{position:absolute;left:0;top:5px;bottom:27px;display:flex;flex-direction:column;justify-content:space-between;color:var(--muted);font-size:8px}.performance-x-axis{position:absolute;left:7.5%;right:1%;bottom:2px;display:flex;justify-content:space-between;color:var(--muted);font-size:8px}.performance-details{min-height:100px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);display:grid;grid-template-columns:repeat(6,minmax(0,1fr))}.performance-details>span{min-width:0;padding:16px 14px;border-left:1px solid var(--line);display:flex;flex-direction:column;justify-content:center}.performance-details>span:first-child{border-left:0}.performance-details b{margin-top:7px;font-size:18px;font-variant-numeric:tabular-nums;white-space:nowrap}.performance-details em{margin-top:6px;color:var(--muted);font-size:8px;font-style:normal;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.performance-dialog-error{margin:0;padding:8px 10px;border-radius:7px;background:color-mix(in srgb,var(--warning) 10%,transparent);color:var(--warning);font-size:10px}.performance-dialog-enter-active,.performance-dialog-leave-active{transition:opacity .18s ease}.performance-dialog-enter-active .performance-dialog,.performance-dialog-leave-active .performance-dialog{transition:transform .18s ease}.performance-dialog-enter-from,.performance-dialog-leave-to{opacity:0}.performance-dialog-enter-from .performance-dialog,.performance-dialog-leave-to .performance-dialog{transform:translateY(8px) scale(.99)}@media(max-width:1180px){.performance-summary,.performance-details{grid-template-columns:repeat(3,1fr)}.performance-summary>span:nth-child(4),.performance-details>span:nth-child(4){border-left:0}.performance-summary>span:nth-child(n+4),.performance-details>span:nth-child(n+4){border-top:1px solid var(--line)}}@media(prefers-reduced-motion:reduce){.performance-dialog-enter-active,.performance-dialog-leave-active,.performance-dialog-enter-active .performance-dialog,.performance-dialog-leave-active .performance-dialog{transition:none}}
</style>
