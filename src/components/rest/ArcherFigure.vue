<script setup lang="ts">
import { computed } from 'vue';
const props = defineProps<{ enemy?: boolean; angle: number; draw: number; release: number; hurt: number; holding: boolean }>();
const pull = computed(() => 5 + props.draw * 8);
const hand = computed(() => {
  const radians = props.angle * Math.PI / 180;
  return { x: 17 - Math.cos(radians) * pull.value, y: 16 + Math.sin(radians) * pull.value };
});
</script>
<template>
  <g class="archer-figure" :class="{ 'archer-enemy': enemy }" :transform="`scale(${enemy ? -1 : 1} 1)`">
    <ellipse cx="0" cy="49" rx="19" ry="3" fill="#182d43" opacity=".2" />
    <g :transform="`rotate(${hurt * -13 + release * -4} 0 46)`">
      <path :d="`M-3 10Q-13 ${5-release*5} -20 12L-32 ${8+release*8} -25 19 -13 17Z`" fill="var(--archer-scarf)" />
      <path d="M-2 25 -10 38 -14 47M1 26 10 35 13 47" fill="none" stroke="#213042" stroke-width="5.5" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M-18 48h7m0 0h-4m25 0h7" stroke="#182331" stroke-width="3" stroke-linecap="round" />
      <path d="M-1 9 1 28" stroke="#263747" stroke-width="7" stroke-linecap="round" />
      <path d="M-4 23 5 24" stroke="#977150" stroke-width="3" />
      <circle cy="0" r="9" fill="#203042" />
      <path v-if="!enemy" d="M-9 5Q-15-5-4-11Q5-13 10-5L3-7-4 4Z" fill="#eee7d3" stroke="#c7c7b8" stroke-width=".7" />
      <path v-else d="M-8-4Q-3-13 7-6L4-10Q-6-12-9-2Z" fill="#4a4b50" />
      <path d="m2 0 4-1m-5 3 3-1" stroke="#f5e9ce" stroke-width="1.5" stroke-linecap="round" />
      <path d="M-5 9 4 11 2 15-6 13Z" fill="var(--archer-scarf)" />
      <path d="M0 14 9 16 17 16" fill="none" stroke="#263747" stroke-width="4" stroke-linecap="round" />
      <path :d="holding ? `M0 17 -3 23 ${hand.x} ${hand.y}` : 'M0 17 -9 15 -12 9'" fill="none" stroke="#263747" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M11 15 15 16" stroke="#b28a5b" stroke-width="5" />
      <g :transform="`translate(17 16) rotate(${-angle})`" fill="none" stroke-linecap="round" stroke-linejoin="round">
        <path d="M0-17Q15 0 0 17" stroke="#704a32" stroke-width="2.6" />
        <path d="M1-15Q13 0 1 15" stroke="#d3aa67" stroke-width="1" />
        <path :d="holding ? `M0-17 -${pull} 0 0 17` : 'M0-17 0 17'" stroke="#e7e7d8" stroke-width=".8" />
        <g v-if="holding" stroke="#e9d4a4" stroke-width="1.1"><path :d="`M-${pull+3} 0H13m-4-2 5 2-5 2`" /><path :d="`m-${pull+1} 0 -3-2m3 2-3 2`" stroke="#a5a8dd" /></g>
      </g>
      <path v-if="hurt > .1" d="m-9-12-4-4m26 5 4-3" stroke="#ffe2a4" stroke-width="1.5" />
    </g>
  </g>
</template>
<style scoped>
.archer-figure { --archer-scarf:#9b80db; }
.archer-enemy { --archer-scarf:#cf7255; }
</style>
