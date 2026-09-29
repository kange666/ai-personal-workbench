<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, onUnmounted, ref, watch } from 'vue'
import { useGameSave } from '../../composables/useGameSave'
import { useGameLoop } from '../../composables/useGameLoop'
import { BALL_RADIUS, BREAKOUT_HEIGHT, BREAKOUT_WIDTH, PADDLE_WIDTH, PADDLE_Y, brickRect, createBreakout, isBreakoutState, launchBreakout, moveBreakoutPaddle, stepBreakout } from '../../utils/rest/breakout'

const props = defineProps<{ paused: boolean }>()
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave('breakout', createBreakout, isBreakoutState)
const surface = ref<HTMLElement | null>(null)
const held = new Set<string>()
let sinceSave = 0
// 保留球的原位置与速度，用户主动继续后再运行，避免重开弹框成为免费复活。
const awaitingResume = ref(state.value.phase === 'playing')
best.value = Math.max(best.value, state.value.score)
const highScore = computed(() => Math.max(best.value, state.value.score))
const remaining = computed(() => state.value.bricks.filter(hp => hp > 0).length)
const bricks = computed(() => state.value.bricks.map((hp, index) => ({ hp, index, ...brickRect(index) })).filter(brick => brick.hp > 0))
const status = computed(() => awaitingResume.value ? '继续刚才的这一球' : state.value.phase === 'over' ? '本次挑战结束' : state.value.phase === 'ready' ? '准备好，发球吧' : '接住小球，清空砖块')

function persist() {
  if (state.value.score > best.value) recordScore(state.value.score)
  else save()
  sinceSave = 0
}
function launch() {
  if (props.paused) return
  if (awaitingResume.value) awaitingResume.value = false
  else launchBreakout(state.value)
  save()
  surface.value?.focus()
}
function restart() {
  if (props.paused) return
  recordScore(state.value.score)
  resetSave()
  awaitingResume.value = false
  held.clear()
  sinceSave = 0
  surface.value?.focus()
}
defineExpose({ restart })
watch(() => props.paused, () => { held.clear(); persist() })
function clearKeys() { held.clear(); persist() }
onMounted(() => window.addEventListener('blur', clearKeys))
onBeforeUnmount(persist)
onUnmounted(() => window.removeEventListener('blur', clearKeys))
useGameLoop(() => props.paused || awaitingResume.value, dt => {
  if (state.value.phase !== 'playing' && held.size === 0) return
  const priorLives = state.value.lives
  const priorLevel = state.value.level
  stepBreakout(state.value, dt, Number(held.has('ArrowRight')) - Number(held.has('ArrowLeft')))
  // 先更新内存最高分，页面关闭事件立即保存时不会漏掉最后一次命中。
  best.value = Math.max(best.value, state.value.score)
  sinceSave += dt
  if (priorLives !== state.value.lives || priorLevel !== state.value.level || sinceSave >= 1) persist()
})
function keydown(event: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', ' '].includes(event.key)) return
  event.preventDefault()
  if (props.paused) return
  if (event.key === ' ') { if (!event.repeat) launch(); return }
  if (awaitingResume.value) return
  held.add(event.key)
}
function keyup(event: KeyboardEvent) {
  if (['ArrowLeft', 'ArrowRight', ' '].includes(event.key)) event.preventDefault()
  held.delete(event.key)
}
function pointerMove(event: PointerEvent) {
  if (props.paused || awaitingResume.value || !surface.value) return
  const rect = surface.value.getBoundingClientRect()
  moveBreakoutPaddle(state.value, (event.clientX - rect.left) / rect.width * BREAKOUT_WIDTH)
}
function pointerDown(event: PointerEvent) {
  if (props.paused) return
  surface.value?.focus()
  surface.value?.setPointerCapture(event.pointerId)
  pointerMove(event)
}
</script>

<template>
  <div class="breakout-game">
    <div class="game-stats" aria-live="polite">
      <span>分数 <strong>{{ state.score }}</strong></span>
      <span>最高 <strong>{{ highScore }}</strong></span>
      <span>第 <strong>{{ state.level }}</strong> 关</span>
      <span>生命 <strong>{{ state.lives }}</strong></span>
    </div>
    <div ref="surface" class="game-surface breakout-surface" tabindex="0" data-game-focus aria-label="打砖块游戏区域，左右方向键移动，空格发球" @keydown="keydown" @keyup="keyup" @blur="held.clear()" @pointerdown.prevent="pointerDown" @pointermove="pointerMove">
      <svg :viewBox="`0 0 ${BREAKOUT_WIDTH} ${BREAKOUT_HEIGHT}`" role="img" :aria-label="`${status}，剩余 ${remaining} 块砖`">
        <path d="M24 16H456M24 307H456" class="court-line" />
        <circle v-for="i in 9" :key="`dot-${i}`" :cx="i * 48" cy="180" r="1" class="court-dot" />
        <g v-for="brick in bricks" :key="brick.index" :class="['brick', `brick-row-${Math.floor(brick.index / 7)}`, { armored: brick.hp > 1 }]">
          <rect :x="brick.x" :y="brick.y" :width="brick.width" :height="brick.height" rx="4" />
          <path v-if="brick.hp > 1" :d="`M${brick.x + 21} ${brick.y + 8.5}h14`" />
        </g>
        <ellipse :cx="state.ball.x" :cy="PADDLE_Y + 17" :rx="BALL_RADIUS + Math.max(0, PADDLE_Y - state.ball.y) / 30" ry="2" class="ball-shadow" />
        <rect :x="state.paddleX - PADDLE_WIDTH / 2" :y="PADDLE_Y" :width="PADDLE_WIDTH" height="10" rx="5" class="paddle" />
        <rect :x="state.paddleX - 12" :y="PADDLE_Y + 3" width="24" height="2" rx="1" class="paddle-detail" />
        <circle v-if="state.phase !== 'over'" :cx="state.ball.x" :cy="state.ball.y" :r="BALL_RADIUS" class="ball" />
        <g v-if="state.phase !== 'playing' || awaitingResume" class="stage-message">
          <rect x="108" y="154" width="264" height="76" rx="14" />
          <text x="240" y="184" class="stage-title">{{ status }}</text>
          <text x="240" y="207" class="stage-subtitle">{{ awaitingResume ? '按空格或点击继续，球从原处出发' : state.phase === 'over' ? `获得 ${state.score} 分 · 再来一次？` : '移动挡板后，按空格或点击发球' }}</text>
        </g>
      </svg>
    </div>
    <div class="game-actions">
      <button v-if="state.phase !== 'over'" class="button primary" :disabled="paused || (state.phase === 'playing' && !awaitingResume)" @click="launch">{{ awaitingResume ? '继续游戏' : state.phase === 'playing' ? '小球飞行中' : '发球' }}</button>
      <button v-else class="button primary" :disabled="paused" @click="restart">再来一局</button>
      <span class="game-help">{{ remaining }} 块砖 · 每块命中 +10，过关 +100</span>
    </div>
    <p class="game-help">鼠标移动 / 触屏拖动 / ← → 控制挡板，空格发球。击中挡板边缘可改变反弹方向。</p>
    <p v-if="storageError" class="game-help save-warning">本机存档暂时不可用，本次仍可继续游玩。</p>
  </div>
</template>

<style scoped>
.breakout-game{display:grid;gap:12px}.breakout-surface{width:100%;max-width:480px;margin:0 auto;touch-action:none;overflow:hidden;border:1px solid var(--line);border-radius:14px;background:radial-gradient(ellipse at 50% 90%,var(--primary-soft),var(--surface-2) 65%);outline:none}.breakout-surface:focus-visible{outline:2px solid var(--primary);outline-offset:3px}.breakout-surface svg{display:block;width:100%;height:auto}.court-line{fill:none;stroke:var(--line);stroke-width:1}.court-dot{fill:var(--muted);opacity:.25}.brick rect{fill:var(--primary);opacity:.8}.brick-row-1 rect{fill:var(--success);opacity:.75}.brick-row-2 rect{fill:var(--warning);opacity:.8}.brick-row-3 rect{fill:var(--primary);opacity:.55}.brick.armored rect{stroke:var(--text);stroke-opacity:.45;stroke-width:1}.brick path{stroke:var(--surface);stroke-width:2;stroke-linecap:round}.paddle{fill:var(--primary)}.paddle-detail{fill:var(--surface);opacity:.7}.ball{fill:var(--text)}.ball-shadow{fill:var(--primary);opacity:.12}.stage-message rect{fill:var(--surface);stroke:var(--line);opacity:.97}.stage-message text{text-anchor:middle;fill:var(--text);font-family:inherit}.stage-title{font-size:17px;font-weight:700}.stage-subtitle{font-size:11px;fill:var(--muted)!important}.save-warning{color:var(--warning)}
</style>
