<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { useGameLoop } from "../../composables/useGameLoop";
import { chooseGomokuMove, createGomoku, isGomokuState, placeGomoku, undoGomoku, winningLine } from "../../utils/rest/gomoku";

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave("gomoku", createGomoku, isGomokuState);
const cursor = ref(112);
const surface = ref<HTMLElement>();
const aiTurn = computed(() => state.value.status === "playing" && state.value.moves.length % 2 === 1);
const lastMove = computed(() => state.value.moves.at(-1)?.index ?? -1);
const line = computed(() => state.value.status === "won" || state.value.status === "lost" ? winningLine(state.value.board, lastMove.value) : []);
const message = computed(() => ({ playing: aiTurn.value ? "电脑正在思考…" : "轮到你了 · 黑棋先行", won: `五子连珠，你赢了！+${state.value.score} 分`, lost: "电脑五子连珠，下局再试试。", draw: "棋盘已满，平局。+50 分" }[state.value.status]));
let aiElapsed = 0;

useGameLoop(() => props.paused, dt => {
  if (!aiTurn.value) { aiElapsed = 0; return; }
  aiElapsed += dt;
  if (aiElapsed < 0.32) return;
  aiElapsed = 0;
  const index = chooseGomokuMove([...state.value.board]);
  if (index !== null && placeGomoku(state.value, index, 2)) commit();
});

function commit() {
  if (state.value.status !== "playing") recordScore(state.value.score);
  else save();
}
function place(index: number) {
  if (props.paused || aiTurn.value) return;
  cursor.value = index;
  if (placeGomoku(state.value, index, 1)) { aiElapsed = 0; commit(); }
}
function clickBoard(event: MouseEvent) {
  if (props.paused) return;
  const rect = (event.currentTarget as SVGElement).getBoundingClientRect();
  if (!rect.width || !rect.height) return;
  const x = Math.round(((event.clientX - rect.left) / rect.width * 360 - 19) / 23);
  const y = Math.round(((event.clientY - rect.top) / rect.height * 360 - 19) / 23);
  if (x >= 0 && x < 15 && y >= 0 && y < 15) place(y * 15 + x);
  surface.value?.focus();
}
function keydown(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", " ", "Enter"].includes(event.key)) return;
  event.preventDefault();
  if (props.paused) return;
  const x = cursor.value % 15, y = Math.floor(cursor.value / 15);
  if (event.key === "ArrowLeft") cursor.value = y * 15 + Math.max(0, x - 1);
  if (event.key === "ArrowRight") cursor.value = y * 15 + Math.min(14, x + 1);
  if (event.key === "ArrowUp") cursor.value = Math.max(0, y - 1) * 15 + x;
  if (event.key === "ArrowDown") cursor.value = Math.min(14, y + 1) * 15 + x;
  if ([" ", "Enter"].includes(event.key) && !event.repeat) place(cursor.value);
}
function undo() { if (!props.paused && undoGomoku(state.value)) { aiElapsed = 0; save(); surface.value?.focus({ preventScroll: true }); } }
function restart() { if (props.paused) return; resetSave(); aiElapsed = 0; cursor.value = 112; surface.value?.focus({ preventScroll: true }); }
defineExpose({ restart });
</script>

<template>
  <div class="gomoku-game">
    <div class="game-stats"><span>你的步数 <strong>{{ Math.ceil(state.moves.length / 2) }}</strong></span><span>本局得分 <strong>{{ state.score }}</strong></span><span>最高分 <strong>{{ best }}</strong></span></div>
    <div class="gomoku-players"><span><i class="black"/>你 · 黑棋</span><b>VS</b><span><i class="white"/>电脑 · 白棋</span></div>
    <div ref="surface" class="game-surface gomoku-surface" tabindex="0" data-game-focus :aria-label="`五子棋棋盘，光标第 ${Math.floor(cursor / 15) + 1} 行第 ${cursor % 15 + 1} 列。方向键移动，空格或回车落子。${message}`" @keydown="keydown">
      <svg viewBox="0 0 360 360" class="gomoku-board" aria-hidden="true" @click="clickBoard">
        <defs><radialGradient id="gomoku-black" cx="30%" cy="25%"><stop stop-color="#68717b"/><stop offset=".65" stop-color="#252e39"/><stop offset="1" stop-color="#131c27"/></radialGradient><radialGradient id="gomoku-white" cx="30%" cy="25%"><stop stop-color="#fffefa"/><stop offset=".7" stop-color="#ebe8de"/><stop offset="1" stop-color="#c7c9c4"/></radialGradient></defs>
        <rect width="360" height="360" rx="12" fill="var(--board-bg)"/>
        <g stroke="var(--board-line)" stroke-width=".65"><template v-for="n in 15" :key="n"><path :d="`M19 ${19 + (n - 1) * 23}H341`"/><path :d="`M${19 + (n - 1) * 23} 19V341`"/></template></g>
        <g fill="var(--board-line)"><circle v-for="point in [48, 56, 112, 168, 176]" :key="point" :cx="19 + point % 15 * 23" :cy="19 + Math.floor(point / 15) * 23" r="2.4"/></g>
        <g v-for="(stone, index) in state.board" :key="index">
          <template v-if="stone"><circle :cx="20 + index % 15 * 23" :cy="21 + Math.floor(index / 15) * 23" r="10" fill="#0c17272b"/><circle :cx="19 + index % 15 * 23" :cy="19 + Math.floor(index / 15) * 23" r="9.7" :fill="stone === 1 ? 'url(#gomoku-black)' : 'url(#gomoku-white)'" :stroke="stone === 1 ? '#161f2c' : '#bcbdba'" stroke-width=".5"/></template>
          <circle v-if="line.includes(index)" :cx="19 + index % 15 * 23" :cy="19 + Math.floor(index / 15) * 23" r="11.2" fill="none" stroke="#de9553" stroke-width="1.8"/>
        </g>
        <rect v-if="lastMove >= 0" :x="16 + lastMove % 15 * 23" :y="16 + Math.floor(lastMove / 15) * 23" width="6" height="6" rx="1" fill="none" :stroke="state.board[lastMove] === 1 ? '#e1bd80' : '#536b80'" stroke-width="1.1"/>
        <rect class="gomoku-cursor" :x="8 + cursor % 15 * 23" :y="8 + Math.floor(cursor / 15) * 23" width="22" height="22" rx="4" fill="none" stroke="var(--primary)" stroke-width="1.5"/>
      </svg>
    </div>
    <p class="gomoku-status" role="status" :class="{ won: state.status === 'won' }">{{ message }}</p>
    <div class="game-actions"><button class="button" :disabled="paused || !state.moves.length || state.status !== 'playing'" @click="undo">悔一步</button><button v-if="state.status !== 'playing'" class="button primary" :disabled="paused" @click="restart">再来一局</button></div>
    <p class="game-help">点击交叉点落子，或方向键移动、空格落子。无禁手，先连成五子获胜。胜局 500 − 你的步数 × 10 分（最低 50）；平局 50，败局 0。</p>
    <p v-if="storageError" class="game-help" role="alert">存档空间不可用，本局进度暂时无法保存。</p>
  </div>
</template>

<style scoped>
.gomoku-game{--board-bg:#c8b48e;--board-line:#73664d}:global(:root[data-theme="warm"]) .gomoku-game{--board-bg:#e0cda8;--board-line:#9a835d}
.gomoku-players{display:flex;align-items:center;justify-content:center;gap:20px;margin:-2px 0 10px;font-size:10px;color:var(--muted)}.gomoku-players span{display:flex;align-items:center;gap:6px}.gomoku-players b{font-weight:400;font-size:9px;opacity:.6}.gomoku-players i{width:13px;height:13px;border-radius:50%;box-shadow:0 1px 3px #0003}.gomoku-players .black{background:linear-gradient(135deg,#64707d,#17212d);border:1px solid #171f2b}.gomoku-players .white{background:linear-gradient(135deg,#fffdf6,#d7d5cc);border:1px solid #bebfb7}
.gomoku-surface{width:min(100%,350px);aspect-ratio:1;margin:0 auto;outline-offset:3px;border-radius:12px;box-shadow:inset 0 0 0 1px #fff2,0 8px 22px #0002}.gomoku-board{display:block;width:100%;height:100%;cursor:crosshair;touch-action:manipulation;border-radius:12px}.gomoku-cursor{opacity:0}.gomoku-surface:focus-visible .gomoku-cursor{opacity:1}.gomoku-status{text-align:center;min-height:20px;font-size:12px;color:var(--text);margin:11px 0 8px}.gomoku-status.won{color:var(--success)}
</style>
