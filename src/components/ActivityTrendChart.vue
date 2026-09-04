<script setup lang="ts">
import { computed, ref } from "vue";
import type { DailyActivity } from "../services/backend";

const props = defineProps<{ points: DailyActivity[] }>();
const emit = defineEmits<{ select: [point: DailyActivity] }>();
const hoveredIndex = ref<number | null>(null);
const dimensions = { width: 560, height: 150, left: 56, right: 8, top: 9, bottom: 24 };

function niceStep(max: number) {
  if (max <= 4) return 1;
  const raw = max / 4;
  const magnitude = 10 ** Math.floor(Math.log10(raw));
  const normalized = raw / magnitude;
  return (normalized <= 1 ? 1 : normalized <= 2 ? 2 : normalized <= 5 ? 5 : 10) * magnitude;
}
const maxValue = computed(() => Math.max(...props.points.map(point => point.conversationCount), 0));
const yStep = computed(() => niceStep(maxValue.value));
const yMax = computed(() => Math.max(yStep.value * 4, Math.ceil(maxValue.value / yStep.value) * yStep.value));
const yTicks = computed(() => Array.from({ length: 5 }, (_, index) => {
  const value = yMax.value - index * yMax.value / 4;
  return { value, label: String(Math.round(value)), y: dimensions.top + index * (dimensions.height - dimensions.top - dimensions.bottom) / 4 };
}));
const bars = computed(() => {
  const plotWidth = dimensions.width - dimensions.left - dimensions.right;
  const plotHeight = dimensions.height - dimensions.top - dimensions.bottom;
  const slot = plotWidth / Math.max(props.points.length, 1);
  const width = Math.min(38, slot * 0.55);
  return props.points.map((point, index) => {
    const height = point.conversationCount / yMax.value * plotHeight;
    return { point, x: dimensions.left + index * slot + (slot - width) / 2, y: dimensions.top + plotHeight - height, width, height, slot };
  });
});
const hovered = computed(() => hoveredIndex.value === null ? null : bars.value[hoveredIndex.value]);
function tooltipClass(index: number) { return index <= 1 ? "align-left" : index >= props.points.length - 2 ? "align-right" : ""; }
</script>

<template>
  <div class="token-trend-chart small activity-trend-chart" @mouseleave="hoveredIndex=null">
    <svg :viewBox="`0 0 ${dimensions.width} ${dimensions.height}`" preserveAspectRatio="none" aria-label="近 7 天 Codex 活跃趋势图">
      <g class="chart-grid"><line v-for="tick in yTicks" :key="tick.value" :x1="dimensions.left" :x2="dimensions.width-dimensions.right" :y1="tick.y" :y2="tick.y" /></g>
      <rect v-for="(bar,index) in bars" :key="bar.point.date" class="activity-bar" :class="{ hovered:hoveredIndex===index }" :x="bar.x" :y="bar.y" :width="bar.width" :height="Math.max(bar.height,1)" rx="4" />
      <rect v-for="(bar,index) in bars" :key="`hit-${bar.point.date}`" class="chart-hit" :x="dimensions.left+index*bar.slot" :y="dimensions.top" :width="bar.slot" :height="dimensions.height-dimensions.top-dimensions.bottom" tabindex="0" @mouseenter="hoveredIndex=index" @focus="hoveredIndex=index" @blur="hoveredIndex=null" @click.stop="emit('select',bar.point)" />
    </svg>
    <span v-for="tick in yTicks" :key="`label-${tick.value}`" class="chart-y-label" :style="{top:`${tick.y/dimensions.height*100}%`}">{{ tick.label }}</span>
    <div class="chart-x-labels"><button v-for="(point,index) in points" :key="point.date" @mouseenter="hoveredIndex=index" @focus="hoveredIndex=index" @blur="hoveredIndex=null" @click.stop="emit('select',point)">{{ point.date.slice(5) }}</button></div>
    <div v-if="hovered" class="chart-tooltip" :class="tooltipClass(hoveredIndex!)" :style="{left:`${(hovered.x+hovered.width/2)/dimensions.width*100}%`,top:`${hovered.y/dimensions.height*100}%`}"><b>{{ hovered.point.date }}</b><strong>{{ hovered.point.conversationCount }} 次 Codex 对话</strong><span>普通 {{ hovered.point.conversationCount-hovered.point.archivedConversationCount }}</span><span>归档 {{ hovered.point.archivedConversationCount }}</span><span>消息 {{ hovered.point.messageCount }}</span><span>Token {{ hovered.point.totalTokens.toLocaleString() }}</span></div>
    <p v-if="!points.length" class="chart-empty">扫描 Codex 后显示真实趋势。</p>
  </div>
</template>

<style scoped>
/* 半透明柱形叠加卡片底色后，两主题仍保持至少 3:1 的图形对比。 */
.activity-bar{fill:color-mix(in srgb,var(--primary) 72%,transparent);transition:opacity .15s}.activity-bar.hovered{fill:var(--primary)}
</style>
