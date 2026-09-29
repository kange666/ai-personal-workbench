<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useGameSave } from '../../composables/useGameSave'
import { useGameLoop } from '../../composables/useGameLoop'
import { TOWER_BASE_Y, TOWER_BLOCK_HEIGHT, TOWER_HEIGHT, TOWER_MAX_BLOCKS, TOWER_WIDTH, createTower, dropTower, isTowerState, startTower, stepTower, type TowerBlock } from '../../utils/rest/tower'

const props = defineProps<{ paused: boolean }>()
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave('tower', createTower, isTowerState)
const surface = ref<HTMLElement | null>(null)
const feedback = ref('')
const debris = ref<(TowerBlock & { y: number; age: number }) | null>(null)
let sinceSave = 0
if (state.value.blocks.length === TOWER_MAX_BLOCKS) state.value.phase = 'complete'
else if (state.value.phase === 'moving') state.value.phase = 'ready'
const highScore = computed(() => Math.max(best.value, state.value.score))
const height = computed(() => state.value.blocks.length - 1)
const top = computed(() => state.value.blocks[state.value.blocks.length - 1]!)
const camera = computed(() => Math.max(0, (height.value - 7) * TOWER_BLOCK_HEIGHT))
const movingY = computed(() => TOWER_BASE_Y - (height.value + 1) * TOWER_BLOCK_HEIGHT + camera.value)
const visibleBlocks = computed(() => {
  const start = Math.max(0, state.value.blocks.length - Math.ceil(TOWER_HEIGHT / TOWER_BLOCK_HEIGHT) - 2)
  return state.value.blocks.slice(start).map((block, offset) => ({ ...block, index: start + offset, y: TOWER_BASE_Y - (start + offset) * TOWER_BLOCK_HEIGHT + camera.value })).filter(block => block.y >= -TOWER_BLOCK_HEIGHT && block.y < TOWER_HEIGHT)
})
const finished = computed(() => state.value.phase === 'over' || state.value.phase === 'complete')
const buttonText = computed(() => finished.value ? '再来一局' : state.value.phase === 'ready' ? height.value ? '继续搭建' : '开始搭建' : '放下方块')

function persist() {
  if (state.value.score > best.value) recordScore(state.value.score)
  else save()
  sinceSave = 0
}
function restart() {
  if (props.paused) return
  recordScore(state.value.score)
  resetSave()
  feedback.value = ''
  debris.value = null
  sinceSave = 0
  surface.value?.focus()
}
defineExpose({ restart })
function action() {
  if (props.paused) return
  if (finished.value) { restart(); return }
  if (state.value.phase === 'ready') { startTower(state.value); save(); surface.value?.focus(); return }
  const dropY = movingY.value - camera.value
  const result = dropTower(state.value)
  if (!result) return
  feedback.value = result.kind === 'perfect' ? `完美对齐！${state.value.perfectStreak > 1 ? `连续 ${state.value.perfectStreak} 次` : '+15 分'}` : result.kind === 'miss' ? '这次没有接住' : '稳稳接住，继续向上'
  debris.value = result.cut ? { ...result.cut, y: dropY, age: 0 } : null
  persist()
  surface.value?.focus()
}
watch(() => props.paused, persist)
useGameLoop(() => props.paused, dt => {
  stepTower(state.value, dt)
  if (debris.value) {
    debris.value.age += dt
    if (debris.value.age > 0.65) debris.value = null
  }
  if (state.value.phase === 'moving') {
    sinceSave += dt
    if (sinceSave >= 1) persist()
  }
})
function keydown(event: KeyboardEvent) {
  if (event.key !== ' ' && event.key !== 'Enter') return
  event.preventDefault()
  if (!event.repeat) action()
}
</script>

<template>
  <div class="tower-game">
    <div class="game-stats" aria-live="polite">
      <span>分数 <strong>{{ state.score }}</strong></span>
      <span>最高 <strong>{{ highScore }}</strong></span>
      <span>高度 <strong>{{ height }}</strong> 层</span>
      <span>连击 <strong>{{ state.perfectStreak }}</strong></span>
    </div>
    <div ref="surface" class="game-surface tower-surface" tabindex="0" data-game-focus aria-label="堆塔挑战游戏区域，点击或按空格放下方块" @keydown="keydown" @pointerdown.prevent="action">
      <svg :viewBox="`0 0 ${TOWER_WIDTH} ${TOWER_HEIGHT}`" role="img" :aria-label="`当前 ${height} 层，${state.phase === 'complete' ? '挑战完成' : state.phase === 'over' ? '挑战结束' : state.phase === 'ready' ? '等待开始' : '点击放下方块'}`">
        <path d="M32 74H448M32 154H448M32 234H448M32 314H448" class="height-guide" />
        <path d="M240 24V322" class="center-guide" />
        <text x="28" y="27" class="stage-label">向上，再高一点</text>
        <text x="450" y="27" class="stage-label stage-width">宽度 {{ Math.round(top.width) }}</text>
        <g v-for="block in visibleBlocks" :key="block.index" :class="['tower-block', `block-tone-${block.index % 3}`, { 'perfect-block': block.index === height && state.lastPerfect }]">
          <rect :x="block.x" :y="block.y" :width="block.width" :height="TOWER_BLOCK_HEIGHT - 1" rx="2" />
          <path v-if="block.width > 16" :d="`M${block.x + 6} ${block.y + 4}h${block.width - 12}`" />
        </g>
        <rect v-if="state.phase === 'moving' || state.phase === 'ready'" :x="state.movingX" :y="movingY" :width="top.width" :height="TOWER_BLOCK_HEIGHT - 1" rx="3" class="moving-block" />
        <rect v-if="debris" :x="debris.x" :y="debris.y + camera + debris.age * debris.age * 320" :width="debris.width" :height="TOWER_BLOCK_HEIGHT - 1" rx="2" :opacity="1 - debris.age / 0.65" class="falling-block" />
        <g v-if="state.phase !== 'moving'" class="stage-message">
          <rect x="100" y="91" width="280" height="78" rx="14" />
          <text x="240" y="121" class="stage-title">{{ state.phase === 'complete' ? '高塔已建成' : state.phase === 'over' ? '挑战结束' : height ? '继续向上搭建' : '搭起属于你的高塔' }}</text>
          <text x="240" y="146" class="stage-subtitle">{{ finished ? `${height} 层 · ${state.score} 分，点击再来一局` : '点击或按空格开始，找准时机再放下' }}</text>
        </g>
      </svg>
    </div>
    <div class="game-actions">
      <button class="button primary" :disabled="paused" @click="action">{{ buttonText }}</button>
      <span :class="['tower-feedback', { perfect: state.lastPerfect && state.phase !== 'over' }]" aria-live="polite">{{ state.phase === 'complete' ? '挑战完成，完整成绩已保留' : feedback || '一层一层，慢慢来' }}</span>
    </div>
    <p class="game-help">点击舞台 / 空格放下方块，超出的部分会掉落。每层 +10 分，完美对齐连续加分。</p>
    <p v-if="storageError" class="game-help save-warning">本机存档暂时不可用，本次仍可继续游玩。</p>
  </div>
</template>

<style scoped>
.tower-game{display:grid;gap:12px}.tower-surface{width:100%;max-width:480px;margin:0 auto;overflow:hidden;touch-action:manipulation;border:1px solid var(--line);border-radius:14px;background:linear-gradient(180deg,var(--surface-2),var(--primary-soft));outline:none;cursor:pointer}.tower-surface:focus-visible{outline:2px solid var(--primary);outline-offset:3px}.tower-surface svg{display:block;width:100%;height:auto}.height-guide{stroke:var(--line);stroke-width:1;fill:none;opacity:.6}.center-guide{stroke:var(--muted);stroke-width:1;stroke-dasharray:3 9;opacity:.15}.stage-label{font-family:inherit;fill:var(--muted);font-size:11px}.stage-width{text-anchor:end}.tower-block rect{fill:var(--primary);opacity:.75}.tower-block path{stroke:var(--surface);stroke-width:1;opacity:.28}.block-tone-1 rect{fill:var(--success);opacity:.65}.block-tone-2 rect{fill:var(--warning);opacity:.7}.perfect-block rect{stroke:var(--success);stroke-width:1.5;opacity:1}.moving-block{fill:var(--primary);stroke:var(--text);stroke-width:1;stroke-opacity:.5}.falling-block{fill:var(--danger)}.stage-message rect{fill:var(--surface);stroke:var(--line);opacity:.97}.stage-message text{text-anchor:middle;fill:var(--text);font-family:inherit}.stage-title{font-size:17px;font-weight:700}.stage-subtitle{font-size:11px;fill:var(--muted)!important}.tower-feedback{font-size:12px;color:var(--muted)}.tower-feedback.perfect{color:var(--success);font-weight:700}.save-warning{color:var(--warning)}
</style>
