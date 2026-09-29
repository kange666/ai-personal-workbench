<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { createWiringState, nextWiringRound, turnWire, validateWiringState, wiringLeaks, wiringPower, WIRING_SIZE, WIRING_SOURCE, type WiringState } from "../../utils/rest/wiring";

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave<WiringState>("wiring", createWiringState, validateWiringState);
const selected = ref(WIRING_SOURCE);
const powered = computed(() => wiringPower(state.value.tiles));
const leaks = computed(() => wiringLeaks(state.value.tiles));
const lines = [
  { bit: 1, x: 30, y: 0 }, { bit: 2, x: 60, y: 30 },
  { bit: 4, x: 30, y: 60 }, { bit: 8, x: 0, y: 30 },
];

function rotate(index: number) {
  if (props.paused) return;
  selected.value = index;
  const next = turnWire(state.value, index);
  if (next === state.value) return;
  state.value = next;
  if (next.won) recordScore(next.score);
  save();
}
function nextRound() {
  if (props.paused || !state.value.won) return;
  state.value = nextWiringRound(state.value);
  selected.value = WIRING_SOURCE;
  save();
}
function restart() {
  if (props.paused) return;
  resetSave();
  selected.value = WIRING_SOURCE;
}
function onKeydown(event: KeyboardEvent) {
  if (props.paused || event.altKey || event.ctrlKey || event.metaKey) return;
  const offsets: Record<string, number> = { ArrowUp: -WIRING_SIZE, ArrowDown: WIRING_SIZE, ArrowLeft: -1, ArrowRight: 1 };
  if (event.key in offsets) {
    event.preventDefault();
    event.stopPropagation();
    const next = selected.value + offsets[event.key];
    if (next >= 0 && next < state.value.tiles.length && (Math.abs(offsets[event.key]) !== 1 || Math.floor(next / WIRING_SIZE) === Math.floor(selected.value / WIRING_SIZE))) selected.value = next;
    (event.currentTarget as HTMLElement).focus();
  } else if ((event.key === "Enter" || event.key === " ") && event.target === event.currentTarget) {
    event.preventDefault();
    event.stopPropagation();
    rotate(selected.value);
  }
}
defineExpose({ restart });
</script>

<template>
  <div class="wiring-game">
    <div class="game-stats"><span>总分 <b>{{ state.score }}</b></span><span>最高 <b>{{ best }}</b></span><span>第 <b>{{ state.round }}</b> 局</span></div>
    <div class="wiring-readout"><span><i /> 已通电 {{ powered.size }}/25</span><span>旋转 {{ state.turns }} 次 · 断口 {{ leaks }}</span></div>
    <div class="game-surface wiring-board" tabindex="0" data-game-focus aria-label="接线棋盘，方向键选择，回车或空格旋转" @keydown="onKeydown">
      <button v-for="(mask, index) in state.tiles" :key="index" class="wire-tile" :class="{ powered: powered.has(index), source: index === WIRING_SOURCE, selected: selected === index }" :disabled="paused || state.won" tabindex="-1" :aria-label="`第 ${Math.floor(index / WIRING_SIZE) + 1} 行第 ${index % WIRING_SIZE + 1} 列${index === WIRING_SOURCE ? '电源' : '线路'}，${powered.has(index) ? '已通电' : '未通电'}，点击顺时针旋转`" @click="rotate(index)">
        <svg viewBox="0 0 60 60" aria-hidden="true">
          <template v-for="line in lines" :key="line.bit"><line v-if="mask & line.bit" x1="30" y1="30" :x2="line.x" :y2="line.y" /></template>
          <circle cx="30" cy="30" :r="index === WIRING_SOURCE ? 10 : 5" />
          <path v-if="index === WIRING_SOURCE" d="M31 23 26 31h5l-2 6 6-9h-5z" />
        </svg>
      </button>
    </div>
    <div class="wiring-result" aria-live="polite"><template v-if="state.won"><b>整座星站已通电！+{{ state.lastAward }} 分</b><span>继续连接下一座星站，累计本轮总分。</span></template><template v-else><b>从中心电源点亮所有节点</b><span>线路需要两端对齐，全部接通且没有断口即可过关。</span></template></div>
    <div v-if="state.won" class="game-actions"><button class="button primary" :disabled="paused" @click="nextRound">下一组接线 →</button></div>
    <p class="game-help">点击方格顺时针旋转；也可用方向键选格，回车旋转。每局 1,000 分，每转一次扣 10 分，最低 100 分。</p>
    <p v-if="storageError" class="game-help" role="status">本机存档暂时不可用，当前仍可继续游玩。</p>
  </div>
</template>

<style scoped>
.wiring-game{display:grid;gap:12px}.wiring-readout{display:flex;justify-content:space-between;gap:8px;color:var(--muted);font-size:11px}.wiring-readout>span:first-child{display:flex;align-items:center;gap:6px;color:var(--primary)}.wiring-readout i{width:6px;height:6px;border-radius:50%;background:var(--primary)}.wiring-board{display:grid;grid-template-columns:repeat(5,minmax(0,1fr));width:min(100%,340px);aspect-ratio:1;margin:auto;padding:9px;gap:2px;border:1px solid var(--line);border-radius:14px;background:var(--surface-2);outline:none}.wiring-board:focus-visible{outline:2px solid var(--primary);outline-offset:3px}.wire-tile{min-width:0;padding:0;border:1px solid transparent;border-radius:8px;background:var(--surface);color:var(--muted);transition:background .15s,color .15s}.wire-tile svg{display:block;width:100%;height:100%}.wire-tile line{stroke:currentColor;stroke-width:5;stroke-linecap:round}.wire-tile circle{fill:var(--surface);stroke:currentColor;stroke-width:3}.wire-tile path{fill:var(--surface)}.wire-tile.powered{color:var(--primary);background:color-mix(in srgb,var(--primary) 10%,var(--surface))}.wire-tile.source circle{fill:var(--primary);stroke:var(--primary)}.wire-tile.source{color:var(--primary)}.wire-tile:hover:not(:disabled),.wiring-board:focus-within .wire-tile.selected{border-color:var(--primary);background:color-mix(in srgb,var(--primary) 16%,var(--surface))}.wire-tile:disabled{opacity:1}.wiring-result{display:grid;gap:5px;text-align:center}.wiring-result b{font-size:13px;color:var(--text)}.wiring-result span{font-size:11px;color:var(--muted);line-height:1.5}.game-help{margin:0}.game-actions{justify-content:center}@media(prefers-reduced-motion:reduce){.wire-tile{transition:none}}
</style>
