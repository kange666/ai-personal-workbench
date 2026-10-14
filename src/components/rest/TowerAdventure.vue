<script setup lang="ts">
import { computed, ref } from 'vue';
import { useGameSave, gameBest } from '../../composables/useGameSave';
import { useGameLoop } from '../../composables/useGameLoop';
import { startTower } from '../../utils/rest/tower';
import { createTowerAdventure, validTowerAdventure, stepTowerAdventure, dropTowerAdventure, towerTool, TOWER_LEVELS } from '../../utils/rest/towerCampaign';
import { createProgress, validProgress, finishLevel } from '../../utils/rest/progression';
import CampaignToolbar from './CampaignToolbar.vue';
import TowerGame from './TowerGame.vue';
const props = defineProps<{ paused: boolean }>();
const saved = useGameSave('tower-v2', () => createTowerAdventure(), validTowerAdventure);
const state = saved.state, legacy = ref(false), oldGame = ref<InstanceType<typeof TowerGame> | null>(null);
const progress = useGameSave('tower-progress', () => createProgress(40), v => validProgress(v,40));
const waiting = ref(state.value.run.phase === 'moving'), message = ref(''), ticks = ref(0);
const surface = ref<HTMLElement | null>(null);
function focusGame() { surface.value?.focus({ preventScroll: true }); }
function resume() { if (!props.paused) { waiting.value = false; focusGame(); } }
const best = computed(() => Math.max(saved.best.value, gameBest('tower')));
const goal = computed(() => TOWER_LEVELS[state.value.level]);
const top = computed(() => state.value.run.blocks.at(-1)!);
const blocks = computed(() => { const all = state.value.run.blocks; return all.slice(-13).map((b, i) => ({ ...b, y: 300 - i * 20, index: Math.max(0, all.length - 13) + i })); });
const movingY = computed(() => 300 - Math.min(state.value.run.blocks.length,13) * 20);
useGameLoop(() => props.paused || legacy.value || waiting.value, dt => { stepTowerAdventure(state.value,dt); ticks.value += dt; if (ticks.value >= 1) { saved.save(); ticks.value = 0; } });
function select(level: number) { if (!props.paused && level <= progress.state.value.unlocked) { state.value = createTowerAdventure(level); waiting.value = false; saved.save(); } }
function mode(value: string) { if (props.paused) return; legacy.value = value === 'legacy'; if (!legacy.value) { state.value = createTowerAdventure(state.value.level, value === 'endless' ? 'endless' : 'campaign'); waiting.value = false; saved.save(); } }
function drop() {
  if (props.paused || waiting.value || state.value.won) return;
  focusGame();
  if (state.value.run.phase === 'ready') { startTower(state.value.run); saved.save(); return; }
  const result = dropTowerAdventure(state.value);
  message.value = result?.kind === 'perfect' ? `完美！${state.value.run.perfectStreak} 连击` : result?.kind === 'trimmed' ? '稳住，还有机会' : '没有接住，可以使用回退';
  saved.recordScore(state.value.run.score);
  if (state.value.won) { finishLevel(progress.state.value,state.value.level,state.value.used === 0 && top.value.width >= 130 ? 3 : state.value.used <= 2 ? 2 : 1,state.value.run.blocks.length - 1); progress.save(); }
  saved.save();
}
function tool(kind: 'slow' | 'repair' | 'undo') { if (!props.paused && !waiting.value && towerTool(state.value,kind)) { message.value = '工具已生效'; saved.save(); focusGame(); } }
function restart() { if (props.paused) return; if (legacy.value) oldGame.value?.restart(); else { state.value = createTowerAdventure(state.value.level,state.value.mode); waiting.value = false; message.value = ''; saved.save(); focusGame(); } }
function key(event: KeyboardEvent) { if ((event.code === 'Space' || event.key === ' ') && !event.repeat) { event.preventDefault(); event.stopPropagation(); drop(); } }
defineExpose({ restart });
</script>
<template>
  <div class="tower-adventure">
    <CampaignToolbar :mode="legacy ? 'legacy' : state.mode" :modes="[{id:'campaign',name:'目标挑战'},{id:'endless',name:'无尽高塔'},{id:'legacy',name:'经典 · 旧进度'}]" :level="state.level" :levels="TOWER_LEVELS" :stars="progress.state.value.stars" :unlocked="progress.state.value.unlocked" :disabled="paused" @mode="mode" @select="select" />
    <TowerGame v-if="legacy" ref="oldGame" :paused="paused" />
    <template v-else>
      <div class="game-stats"><span>高度 <b>{{ state.run.blocks.length - 1 }}</b></span><span>宽度 <b>{{ Math.round(top.width) }}</b></span><span>得分 <b>{{ state.run.score }}</b> / {{ best }}</span></div>
      <p class="game-help">{{ state.mode === 'campaign' ? `目标 ${goal.height} 层 · 宽度 ≥ ${goal.width} · 最长完美连击 ≥ ${goal.perfect}` : '不断向上，挑战最高纪录' }}<span v-if="goal.pattern === 3 && state.mode === 'campaign'"> · 每块会反向一次</span></p>
      <div ref="surface" class="game-surface" data-game-focus tabindex="0" @keydown="key">
        <svg viewBox="0 0 480 340" class="tower-scene" aria-label="高塔，点击或按空格落下" role="button" @click="drop">
          <defs><linearGradient id="tower-sky" x2="0" y2="1"><stop stop-color="var(--surface-2)"/><stop offset="1" stop-color="var(--surface)"/></linearGradient></defs>
          <rect width="480" height="340" fill="url(#tower-sky)"/>
          <g fill="var(--primary)" opacity=".12"><path v-if="state.run.blocks.length < 15" d="M0 330V245h30v35h25v-65h35v115h50v-45h35v45h305v10H0"/><g v-else-if="state.run.blocks.length < 24"><ellipse cx="75" cy="85" rx="65" ry="13"/><ellipse cx="370" cy="150" rx="70" ry="15"/></g><g v-else><circle v-for="i in 20" :key="i" :cx="(i*73)%470" :cy="(i*47)%290" r="2"/></g></g>
          <rect v-for="b in blocks" :key="b.index" :x="b.x" :y="b.y" :width="b.width" height="18" rx="3" :fill="b.index % 3 === 0 ? 'var(--primary)' : b.index % 3 === 1 ? 'var(--warning)' : 'var(--success)'"/>
          <rect v-if="!state.won && state.run.phase !== 'over'" :x="state.run.movingX" :y="movingY" :width="top.width" height="18" rx="3" fill="var(--primary)" opacity=".9"/>
          <g v-if="waiting || state.run.phase === 'ready' || state.run.phase === 'over' || state.run.phase === 'complete' || state.won"><rect width="480" height="340" fill="var(--surface)" opacity=".7"/><text x="240" y="155" text-anchor="middle" fill="var(--text)" font-size="22">{{ waiting ? '进度已保留' : state.run.phase === 'complete' ? '高塔直达星空！' : state.won ? '目标达成！' : state.run.phase === 'over' ? '高塔暂时停工' : '准备好，稳稳落下' }}</text><text x="240" y="185" text-anchor="middle" fill="var(--muted)" font-size="13">{{ waiting ? '点击下方继续' : state.won ? '继续挑战下一座高塔' : state.run.phase === 'over' ? '可回退刚才的落点，或重新挑战' : '点击画面 / 空格开始，再次点击落下' }}</text></g>
        </svg>
      </div>
      <div class="tower-tools"><button v-if="waiting" class="button primary" :disabled="paused" @click="resume">继续建造</button><template v-else><button v-for="t in (['slow','repair','undo'] as const)" :key="t" class="button" :disabled="paused || state[t] === 0 || state.won" @click="tool(t)">{{ {slow:'慢速 6 秒',repair:'修补宽度',undo:'回退一层'}[t] }} ×{{ state[t] }}</button></template><button v-if="state.won && state.level < 39" class="button primary" :disabled="paused" @click="select(state.level+1)">下一关</button><button class="button" :disabled="paused" @click="restart">重开本关</button></div>
      <p class="game-help" aria-live="polite">{{ message || '每关提供两次慢速、一次修补和一次回退。完美落点不会损失宽度。' }}</p>
      <p v-if="saved.storageError.value || progress.storageError.value" class="game-help">本机存档暂不可用，当前仍可游玩。</p>
    </template>
  </div>
</template>
<style scoped>.tower-adventure{display:grid;gap:10px}.tower-adventure>.game-surface{width:min(100%,480px)}.tower-scene{display:block;width:100%;touch-action:manipulation;cursor:pointer}.tower-tools{display:flex;gap:6px;flex-wrap:wrap}.game-help{margin:0;line-height:1.6}</style>
