<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { createSokobanState, getSokobanBoard, moveSokoban, nextSokobanLevel, restartSokobanLevel, SOKOBAN_LEVELS, undoSokoban, validateSokobanState, type SokobanDirection, type SokobanState } from "../../utils/rest/sokoban";

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave<SokobanState>("sokoban", createSokobanState, validateSokobanState);
const gameElement = ref<HTMLElement | null>(null);
const board = computed(() => getSokobanBoard(state.value.level));
const cells = computed(() => Array.from({ length: board.value.width * board.value.height }, (_, index) => ({ index, wall: board.value.walls.includes(index), goal: board.value.goals.includes(index), box: state.value.boxes.includes(index), player: state.value.player === index })));
const parked = computed(() => state.value.boxes.filter(box => board.value.goals.includes(box)).length);
const controls: { direction: SokobanDirection; arrow: string; name: string }[] = [
  { direction: "up", arrow: "↑", name: "向上移动" }, { direction: "left", arrow: "←", name: "向左移动" },
  { direction: "down", arrow: "↓", name: "向下移动" }, { direction: "right", arrow: "→", name: "向右移动" },
];

function update(next: SokobanState) {
  if (props.paused || next === state.value) return;
  state.value = next;
  if (next.completed) recordScore(next.score);
  save();
  gameElement.value?.focus();
}
function move(direction: SokobanDirection) {
  if (!props.paused) update(moveSokoban(state.value, direction));
}
function undo() { if (!props.paused) update(undoSokoban(state.value)); }
function retry() { if (!props.paused) update(restartSokobanLevel(state.value)); }
function nextLevel() { if (!props.paused) update(nextSokobanLevel(state.value)); }
function restart() { if (!props.paused) resetSave(); }
function onKeydown(event: KeyboardEvent) {
  if (props.paused || event.altKey || event.ctrlKey || event.metaKey) return;
  const keys: Record<string, SokobanDirection> = { ArrowUp: "up", w: "up", ArrowRight: "right", d: "right", ArrowDown: "down", s: "down", ArrowLeft: "left", a: "left" };
  const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
  if (keys[key]) { event.preventDefault(); event.stopPropagation(); move(keys[key]); }
  else if (key === "z") { event.preventDefault(); event.stopPropagation(); undo(); }
}
defineExpose({ restart });
</script>

<template>
  <div class="sokoban-game">
    <div class="game-stats"><span>总分 <b>{{ state.score }}</b></span><span>最高 <b>{{ best }}</b></span><span>关卡 <b>{{ state.level + 1 }}/{{ SOKOBAN_LEVELS.length }}</b></span></div>
    <div class="sokoban-readout"><span>{{ SOKOBAN_LEVELS[state.level].name }} · 就位 {{ parked }}/{{ board.goals.length }}</span><span>{{ state.moves }} 步 · {{ state.pushes }} 次推动</span></div>
    <div ref="gameElement" class="game-surface sokoban-board" :style="{ gridTemplateColumns: `repeat(${board.width}, minmax(0, 1fr))`, aspectRatio: `${board.width} / ${board.height}` }" tabindex="0" data-game-focus aria-label="推箱子棋盘，用方向键或 WASD 移动，Z 撤销" @keydown="onKeydown">
      <div v-for="cell in cells" :key="cell.index" class="sokoban-cell" :class="{ wall: cell.wall, goal: cell.goal, parked: cell.box && cell.goal }" aria-hidden="true">
        <i v-if="cell.goal" class="sokoban-goal" />
        <svg v-if="cell.box" class="sokoban-box" viewBox="0 0 40 40"><rect x="5" y="5" width="30" height="30" rx="5" /><path d="m9 9 22 22M31 9 9 31" /><rect x="15" y="15" width="10" height="10" rx="2" /></svg>
        <svg v-if="cell.player" class="sokoban-player" viewBox="0 0 40 40"><circle cx="20" cy="20" r="15" /><rect x="10" y="11" width="20" height="15" rx="7" /><path d="M15 18v2m10-2v2M16 31h8" /></svg>
      </div>
    </div>
    <div class="sokoban-result" aria-live="polite"><template v-if="state.completed"><b>{{ state.level === SOKOBAN_LEVELS.length - 1 ? '六座货舱全部归位！' : '货物全部就位！' }} +{{ state.awards[state.level] }} 分</b></template><span v-else>把每个货箱推到圆形泊位；箱子只能推，不能拉。</span></div>
    <div class="sokoban-controls">
      <div class="direction-pad"><button v-for="control in controls" :key="control.direction" class="button" :class="`direction-${control.direction}`" :aria-label="control.name" :disabled="paused || state.completed" @click="move(control.direction)">{{ control.arrow }}</button></div>
      <div class="game-actions"><button class="button" :disabled="paused || !state.history.length" @click="undo">撤销一步</button><button class="button" :disabled="paused" @click="retry">重开本关</button><button v-if="state.completed && state.level < SOKOBAN_LEVELS.length - 1" class="button primary" :disabled="paused" @click="nextLevel">下一关 →</button><button v-else-if="state.completed" class="button primary" :disabled="paused" @click="restart">重新挑战</button></div>
    </div>
    <p class="game-help">方向键 / WASD 移动，Z 撤销（保留最近 200 步）。每关 1,000 分，每步扣 5 分，每次推动另扣 10 分，最低 100 分。重开本关会重算本关得分。</p>
    <p v-if="storageError" class="game-help" role="status">本机存档暂时不可用，当前仍可继续游玩。</p>
  </div>
</template>

<style scoped>
.sokoban-game{display:grid;gap:12px}.sokoban-readout{display:flex;justify-content:space-between;gap:8px;color:var(--muted);font-size:11px}.sokoban-readout>span:first-child{color:var(--primary)}.sokoban-board{display:grid;width:min(100%,350px);max-height:350px;margin:auto;padding:8px;gap:2px;border:1px solid var(--line);border-radius:14px;background:var(--surface-2);outline:none}.sokoban-board:focus-visible{outline:2px solid var(--primary);outline-offset:3px}.sokoban-cell{position:relative;min-width:0;min-height:0;border-radius:4px;background:color-mix(in srgb,var(--surface) 80%,transparent);display:grid;place-items:center}.sokoban-cell.wall{border:1px solid color-mix(in srgb,var(--primary) 12%,var(--line));background:linear-gradient(145deg,color-mix(in srgb,var(--primary) 13%,var(--surface-2)),var(--surface-2));box-shadow:inset 0 2px 0 color-mix(in srgb,var(--text) 5%,transparent)}.sokoban-goal{position:absolute;width:39%;aspect-ratio:1;border:2px dashed var(--primary);border-radius:50%;opacity:.7}.sokoban-cell.goal{background:color-mix(in srgb,var(--primary) 8%,var(--surface))}.sokoban-box,.sokoban-player{position:relative;width:100%;height:100%;max-height:48px}.sokoban-box>rect:first-child{fill:color-mix(in srgb,var(--warning) 18%,var(--surface));stroke:var(--warning);stroke-width:2}.sokoban-box path{stroke:var(--warning);stroke-width:1.5;opacity:.65}.sokoban-box>rect:last-child{fill:var(--warning);stroke:var(--surface);stroke-width:1}.parked .sokoban-box>rect:first-child{fill:color-mix(in srgb,var(--primary) 23%,var(--surface));stroke:var(--primary)}.parked .sokoban-box path{stroke:var(--primary)}.parked .sokoban-box>rect:last-child{fill:var(--primary)}.sokoban-player circle{fill:var(--primary);stroke:color-mix(in srgb,var(--primary) 55%,var(--text));stroke-width:1.5}.sokoban-player rect{fill:var(--surface)}.sokoban-player path{stroke:var(--text);stroke-width:2;stroke-linecap:round}.sokoban-result{min-height:20px;text-align:center;color:var(--muted);font-size:12px}.sokoban-result b{color:var(--text)}.sokoban-controls{display:flex;align-items:center;justify-content:space-between;gap:14px}.direction-pad{display:grid;grid-template-columns:repeat(3,34px);grid-template-rows:repeat(2,32px);gap:4px;flex-shrink:0}.direction-pad .button{min-width:0;height:32px;padding:0;font-size:18px}.direction-up{grid-column:2;grid-row:1}.direction-left{grid-column:1;grid-row:2}.direction-down{grid-column:2;grid-row:2}.direction-right{grid-column:3;grid-row:2}.game-actions{display:flex;flex-wrap:wrap;justify-content:flex-end;gap:6px}.game-help{margin:0;line-height:1.6}@media(max-width:480px){.sokoban-controls{gap:8px}.game-actions .button{padding-inline:9px}}
</style>
