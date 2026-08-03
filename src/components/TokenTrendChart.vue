<script setup lang="ts">
import { computed, ref } from "vue";
import type { TokenTrendPoint } from "../services/backend";

const props = withDefaults(defineProps<{ points: TokenTrendPoint[]; small?: boolean }>(), { small: false });
const emit = defineEmits<{ select: [point: TokenTrendPoint] }>();
const hoveredIndex = ref<number | null>(null);
const dimensions = computed(() => props.small ? { width: 560, height: 150, left: 56, right: 8, top: 9, bottom: 24 } : { width: 800, height: 260, left: 66, right: 12, top: 14, bottom: 30 });

function compact(value: number) {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(value >= 100_000_000 ? 0 : 1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(value >= 100_000 ? 0 : 1)}K`;
  return String(Math.round(value));
}
function niceStep(max: number) {
  if (max <= 0) return 1;
  const raw = max / 4;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const normalized = raw / magnitude;
  return (normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10) * magnitude;
}
const maxValue = computed(() => Math.max(...props.points.map(point => point.totalTokens), 0));
const yStep = computed(() => niceStep(maxValue.value));
const yMax = computed(() => Math.max(yStep.value * 4, Math.ceil(maxValue.value / yStep.value) * yStep.value));
const yTicks = computed(() => Array.from({ length: 5 }, (_, index) => {
  const value = yMax.value - index * yMax.value / 4;
  const d = dimensions.value;
  return { value, label: compact(value), y: d.top + index * (d.height - d.top - d.bottom) / 4 };
}));
const dots = computed(() => {
  const d = dimensions.value;
  const plotWidth = d.width - d.left - d.right;
  const plotHeight = d.height - d.top - d.bottom;
  return props.points.map((point, index) => ({ point, x: d.left + (props.points.length === 1 ? plotWidth / 2 : index * plotWidth / (props.points.length - 1)), y: d.top + plotHeight - point.totalTokens / yMax.value * plotHeight }));
});
const linePoints = computed(() => dots.value.map(dot => `${dot.x},${dot.y}`).join(" "));
const areaPoints = computed(() => dots.value.length ? `${dots.value[0].x},${dimensions.value.height - dimensions.value.bottom} ${linePoints.value} ${dots.value.at(-1)!.x},${dimensions.value.height - dimensions.value.bottom}` : "");
const labelIndexes = computed(() => {
  const count = props.points.length;
  const limit = props.small ? 4 : 8;
  if (!count) return [];
  if (count <= limit) return Array.from({ length: count }, (_, index) => index);
  return Array.from(new Set(Array.from({ length: limit }, (_, index) => Math.round(index * (count - 1) / (limit - 1)))));
});
const hovered = computed(() => hoveredIndex.value === null ? null : dots.value[hoveredIndex.value]);
const hitWidth = computed(() => props.points.length <= 1 ? dimensions.value.width - dimensions.value.left - dimensions.value.right : (dimensions.value.width - dimensions.value.left - dimensions.value.right) / (props.points.length - 1));
function tooltipClass(index: number) { return index <= 1 ? "align-left" : index >= props.points.length - 2 ? "align-right" : ""; }
</script>

<template>
  <div class="token-trend-chart" :class="{ small }" @mouseleave="hoveredIndex = null">
    <svg :viewBox="`0 0 ${dimensions.width} ${dimensions.height}`" preserveAspectRatio="none" aria-label="Token 趋势图">
      <g class="chart-grid"><line v-for="tick in yTicks" :key="tick.value" :x1="dimensions.left" :x2="dimensions.width - dimensions.right" :y1="tick.y" :y2="tick.y" /></g>
      <polygon v-if="dots.length" class="chart-area" :points="areaPoints" /><polyline v-if="dots.length" class="chart-line" :points="linePoints" />
      <g v-if="hovered" class="chart-hover"><line :x1="hovered.x" :x2="hovered.x" :y1="dimensions.top" :y2="dimensions.height - dimensions.bottom" /><circle :cx="hovered.x" :cy="hovered.y" :r="small ? 4 : 5" /></g>
      <rect v-for="(dot,index) in dots" :key="dot.point.date" class="chart-hit" :x="Math.max(dimensions.left, dot.x - hitWidth / 2)" :y="dimensions.top" :width="Math.min(hitWidth, dimensions.width - dimensions.right - Math.max(dimensions.left, dot.x - hitWidth / 2))" :height="dimensions.height - dimensions.top - dimensions.bottom" tabindex="0" @mouseenter="hoveredIndex = index" @focus="hoveredIndex = index" @blur="hoveredIndex = null" @click.stop="emit('select', dot.point)" />
    </svg>
    <span v-for="tick in yTicks" :key="`label-${tick.value}`" class="chart-y-label" :style="{ top: `${tick.y / dimensions.height * 100}%` }">{{ tick.label }}</span>
    <div class="chart-x-labels"><button v-for="index in labelIndexes" :key="points[index].date" @mouseenter="hoveredIndex = index" @focus="hoveredIndex = index" @blur="hoveredIndex = null" @click.stop="emit('select', points[index])">{{ points[index].date.slice(5) }}</button></div>
    <div v-if="hovered" class="chart-tooltip" :class="tooltipClass(hoveredIndex!)" :style="{ left: `${hovered.x / dimensions.width * 100}%`, top: `${hovered.y / dimensions.height * 100}%` }"><b>{{ hovered.point.date }}</b><strong>{{ hovered.point.totalTokens.toLocaleString() }} Token</strong><span>输入 {{ compact(hovered.point.inputTokens) }}</span><span>缓存 {{ compact(hovered.point.cachedInputTokens) }}</span><span>输出 {{ compact(hovered.point.outputTokens) }}</span><span>推理 {{ compact(hovered.point.reasoningOutputTokens) }}</span></div>
    <p v-if="!points.length" class="chart-empty">扫描 Codex 后显示真实趋势。</p>
  </div>
</template>
