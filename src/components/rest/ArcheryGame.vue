<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useGameSave } from '../../composables/useGameSave';
import { useGameLoop } from '../../composables/useGameLoop';
import { ARCHERY_LEVELS, ARROWS, archerPosition, archeryTrajectory, createArchery, obstacleAt, shootArchery, stepArchery, suggestArcheryAim, validArchery } from '../../utils/rest/archery';
import { addBattleEffect, effectProgress, stepBattleEffects, type BattleEffect } from '../../utils/rest/battleEffects';
import { clamp, createProgress, finishLevel, validProgress } from '../../utils/rest/progression';
import CampaignToolbar from './CampaignToolbar.vue';
import ArcherFigure from './ArcherFigure.vue';

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore } = useGameSave('archery', () => createArchery(), validArchery);
const { state: progress, save: saveProgress } = useGameSave('archery-progress', () => createProgress(30), (v): v is ReturnType<typeof createProgress> => validProgress(v, 30));
const mode = computed({ get: () => state.value.mode, set: v => { state.value.mode = v as 'campaign' | 'endless'; } });
const surface = ref<SVGSVGElement>();
const awaiting = ref(state.value.phase === 'flight' || state.value.phase === 'ai');
const positions = computed(() => [archerPosition(state.value.level, 'player'), archerPosition(state.value.level, 'enemy')]);
const preview = computed(() => state.value.phase === 'aim' && state.value.assist ? archeryTrajectory(state.value).points : []);
const canAim = computed(() => !props.paused && !awaiting.value && state.value.phase === 'aim');
const effects = ref<BattleEffect[]>([]);
const release = ref([0, 0]), hurt = ref([0, 0]);
const enemyAngle = ref(42);
const path = (points: { x: number; y: number }[]) => points.map((p, i) => `${i ? 'L' : 'M'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(' ');
let saved = 0, effectId = 0, drag: { x: number; y: number; id: number } | null = null;
const emitEffect = (kind: BattleEffect['kind'], x: number, y: number, text = '', color = '#ffe2a4') => {
  const duration = kind === 'hit' ? .8 : .35;
  addBattleEffect(effects.value, { id: ++effectId, kind, x, y, text, color, duration, life: duration });
};
// 瞄准待机没有动画帧；飞行和短反馈结束后自动停止，暂停也冻结反馈。
useGameLoop(() => props.paused || awaiting.value || (!['flight', 'ai'].includes(state.value.phase) && !effects.value.length && !release.value.some(Boolean) && !hurt.value.some(Boolean)), dt => {
  effects.value = stepBattleEffects(effects.value, dt);
  release.value = release.value.map(value => Math.max(0, value - dt * 5));
  hurt.value = hurt.value.map(value => Math.max(0, value - dt * 4));
  const s = state.value, phase = s.phase, shot = s.shot, hp = [s.playerHP, s.enemyHP];
  stepArchery(s, dt);
  if (!shot && s.shot) {
    release.value[1] = 1;
    enemyAngle.value = Math.atan2(-s.shot.vy, Math.abs(s.shot.vx)) * 180 / Math.PI;
  }
  if (shot && !s.shot) {
    emitEffect('burst', shot.x, shot.y, '', shot.kind === 'blast' ? '#ffbf73' : '#fff0c9');
    [s.playerHP, s.enemyHP].forEach((value, i) => {
      if (value >= hp[i]!) return;
      hurt.value[i] = 1;
      emitEffect('hit', positions.value[i]!.x, positions.value[i]!.y - 14, `−${hp[i]! - value}`);
    });
  }
  if (s.phase === 'won' && phase !== 'won') {
    if (mode.value === 'campaign') {
      finishLevel(progress.value, s.level, s.playerHP >= 80 ? 3 : s.playerHP >= 35 ? 2 : 1, s.shots);
      saveProgress();
    }
    recordScore(s.score);
  }
  saved += dt;
  if (saved >= 1 || phase !== s.phase) { save(); saved = 0; }
});
const focusSurface = () => nextTick(() => surface.value?.focus());
function cancelDrag() {
  if (drag && surface.value?.hasPointerCapture?.(drag.id)) surface.value.releasePointerCapture(drag.id);
  drag = null;
}
watch(() => props.paused, paused => { if (paused) cancelDrag(); });
function start(level = state.value.level) {
  if (props.paused) return;
  cancelDrag();
  const selected = mode.value;
  state.value = createArchery(level); state.value.mode = selected;
  effects.value = []; release.value = [0, 0]; hurt.value = [0, 0];
  awaiting.value = false; save(); void focusSurface();
}
function fire() {
  if (!canAim.value || !shootArchery(state.value)) return;
  release.value[0] = 1; save(); void focusSurface();
}
function aim() {
  if (!canAim.value) return;
  const solution = suggestArcheryAim(state.value, 'player', state.value.kind);
  state.value.angle = solution.angle; state.value.power = solution.power; save(); void focusSurface();
}
function pointer(event: PointerEvent) {
  const bounds = surface.value!.getBoundingClientRect();
  if (!bounds.width || !bounds.height) return { x: 0, y: 0 };
  return { x: (event.clientX - bounds.left) / bounds.width * 640, y: (event.clientY - bounds.top) / bounds.height * 360 };
}
function down(event: PointerEvent) {
  if (!canAim.value || event.button !== 0) return;
  drag = { ...pointer(event), id: event.pointerId };
  surface.value?.setPointerCapture?.(event.pointerId);
}
function move(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.id || !canAim.value) return;
  const p = pointer(event), dx = drag.x - p.x, dy = p.y - drag.y;
  state.value.angle = clamp(Math.atan2(Math.max(0, dy), Math.max(1, dx)) * 180 / Math.PI, 15, 80);
  state.value.power = clamp(Math.hypot(dx, dy) * 2.5 + 180, 180, 450);
}
function up(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.id) return;
  move(event); cancelDrag(); fire();
}
defineExpose({ restart: () => start() });
</script>
<template>
  <div class="archery-game battle-game" :class="{ 'battle-paused': paused || awaiting }">
    <div class="archery-topline">
      <CampaignToolbar :mode="mode" :level="state.level" :levels="ARCHERY_LEVELS" :stars="progress.stars" :unlocked="progress.unlocked" :disabled="paused" :modes="[{id:'campaign',label:'章节闯关'},{id:'endless',label:'随机遭遇'}]" @mode="value=>{mode=value==='endless'?'endless':'campaign';start(value==='endless'?Math.floor(Math.random()*30):Math.min(state.level,progress.unlocked))}" @select="level=>{if(level<=progress.unlocked)start(level)}" />
      <span class="archery-record">最高 {{ best }}</span>
    </div>
    <div class="archery-stage">
      <div class="archery-hud" aria-label="对决状态">
        <div class="archer-health"><span>你 <b>{{ state.playerHP }} / 130</b></span><i><em :style="{width:`${state.playerHP/130*100}%`}" /></i></div>
        <span class="archery-wind">{{ state.wind > 0 ? '⚑ → 顺风' : state.wind < 0 ? '⚑ ← 逆风' : '⚑ 无风' }}<small v-if="state.wind">{{ Math.abs(state.wind) }}</small></span>
        <div class="archer-health enemy-health"><span>对手 <b>{{ state.enemyHP }} / {{ state.enemyMax }}</b></span><i><em :style="{width:`${state.enemyHP/state.enemyMax*100}%`}" /></i></div>
      </div>
      <svg ref="surface" class="archery-surface" viewBox="0 0 640 360" tabindex="0" data-game-focus aria-label="火柴人射箭战场，向左下拖动拉弓，松开射击，空格也可射击" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="cancelDrag" @lostpointercapture="cancelDrag" @keydown.space.prevent="event=>{if(!event.repeat)fire()}">
        <defs><linearGradient id="archer-sky" x2="0" y2="1"><stop stop-color="var(--archery-sky)"/><stop offset="1" stop-color="var(--archery-horizon)"/></linearGradient><g id="archery-pine"><path d="M0-25-12-6H-7L-17 7H-3V14H3V7H17L7-6H12Z" fill="currentColor"/></g></defs>
        <g v-once aria-hidden="true">
          <rect width="640" height="360" fill="url(#archer-sky)"/>
          <circle cx="491" cy="83" r="15" fill="#f2e9ca" opacity=".7"/>
          <path d="M-20 108Q-9 85 7 100 21 75 34 98 53 96 55 108ZM452 124Q465 109 480 119 491 95 507 117 527 113 535 124Z" fill="#e9f3ed" opacity=".4"/>
          <path d="M0 190 58 128 127 194 189 142 272 209 330 155 415 203 496 137 566 188 640 145V360H0Z" fill="#689dbc" opacity=".45"/>
          <path d="m0 254 92-48 74 29 109-30 87 51 90-28 110-34 78 64V360H0Z" fill="#5488a4" opacity=".6"/>
          <path d="M235 284H409v5H235Zm18 4v35h12v-23q14-18 28 0v23h12v-23q14-18 28 0v23h12v-23q14-18 28 0v23h12v-35Z" fill="#abc9cc" opacity=".65"/>
          <g color="#376f8c" opacity=".7"><use v-for="(x,i) in [10,38,86,163,205,436,478,560,612]" :key="i" href="#archery-pine" :transform="`translate(${x} ${286+i%3*12}) scale(${.7+i%2*.3})`"/></g>
          <path d="M0 337q70-26 130 0t130 0 130 0 130 0 120 0v23H0Z" fill="#305c75"/>
        </g>
        <g v-for="(p,i) in positions" :key="i">
          <path :d="`M${p.x-49} ${p.y+51}H${p.x+47}l-7 35 10 74H${p.x-49}Z`" fill="#63748b" stroke="#4a5d76" stroke-width="1"/>
          <path :d="`M${p.x-35} ${p.y+54}l12 27-4 28 21 17M${p.x+27} ${p.y+56}l-9 26 12 20`" fill="none" stroke="#435770" stroke-width="2"/>
          <path :d="`M${p.x-49} ${p.y+48}H${p.x+47}v6l-22 3-9-4-29 3-17-3H${p.x-49}Z`" fill="#98b76e"/>
          <path :d="`M${p.x-38} ${p.y+49}l2-5 3 5m43 0 4-7 1 7m15 0 2-4 3 4`" fill="none" stroke="#a9c57d" stroke-width="2"/>
          <g :transform="`translate(${p.x} ${p.y})`"><ArcherFigure :enemy="Boolean(i)" :angle="i ? enemyAngle : state.angle" :draw="i ? .75 : (state.power-180)/270" :release="release[i]!" :hurt="hurt[i]!" :holding="i ? state.phase==='ai' : state.phase==='aim'" /></g>
        </g>
        <g v-for="(obstacle,i) in state.obstacles" :key="i">
          <g v-if="obstacle.hp>0" :transform="`translate(${obstacle.x} ${obstacleAt(obstacle,state.time+(state.shot?.age||0)).y})`" :data-obstacle="obstacle.kind">
            <rect :width="obstacle.width" :height="obstacle.height" rx="2" :fill="obstacle.kind==='wood'?'#b08755':obstacle.kind==='reflect'?'#88bec3':obstacle.kind==='moving'?'#baab7d':'#7c8ba2'" stroke="#526479" stroke-width="1"/>
            <template v-if="obstacle.kind==='wood'"><path :d="`M${obstacle.width*.3} 0v${obstacle.height}m${obstacle.width*.4}-${obstacle.height}v${obstacle.height}M0 ${obstacle.height*.3}h${obstacle.width}M0 ${obstacle.height*.7}h${obstacle.width}`" stroke="#755d43" stroke-width="1.5"/><path v-if="obstacle.hp===1" :d="`m0 8 ${obstacle.width*.6} 13 -6 14 10 12`" stroke="#e1c6a0" fill="none" stroke-width="2"/></template>
            <template v-else-if="obstacle.kind==='stone'"><path :d="`m${obstacle.width*.65} 0 -5 17 8 15 -12 17 4 23`" stroke="#53667f" fill="none" stroke-width="1.5"/><path :d="`M0 0h${obstacle.width}v4l-6-1-8 3H0Z`" fill="#a1b87c"/></template>
            <path v-else-if="obstacle.kind==='reflect'" :d="`M3 ${obstacle.height/2}h${obstacle.width-6}`" stroke="#e1f7f0" stroke-width="2"/>
            <path v-else :d="`M${obstacle.width/2} 6v${obstacle.height-12}m-3-3 3 3 3-3`" stroke="#f2e3af" fill="none"/>
          </g>
        </g>
        <path v-if="preview.length" :d="path(preview)" fill="none" stroke="#e8dcff" stroke-width="1.5" stroke-dasharray="3 6" opacity=".85"/>
        <g v-if="state.shot" data-arrow-flight>
          <path :d="path(state.shot.trail)" fill="none" :stroke="state.shot.kind==='blast'?'#ffc175':'#f0e6bf'" opacity=".45" stroke-width="1.5"/>
          <g :transform="`translate(${state.shot.x} ${state.shot.y}) rotate(${Math.atan2(state.shot.vy,state.shot.vx)*180/Math.PI})`" stroke="#f8e3b5" stroke-width="1.4"><path d="M-15 0H6m-5-3 6 3-6 3"/><path d="m-11 0-4-3m4 3-4 3" stroke="#ac9cd6"/></g>
        </g>
        <g v-for="effect in effects" :key="effect.id" :opacity="Math.min(1,effect.life*4)" data-battle-effect>
          <text v-if="effect.kind==='hit'" :x="effect.x" :y="effect.y-effectProgress(effect)*18" text-anchor="middle" :fill="effect.color" font-size="13" font-weight="700" stroke="#695544" stroke-width=".6" paint-order="stroke">{{ effect.text }}</text>
          <g v-else :transform="`translate(${effect.x} ${effect.y}) scale(${.6+effectProgress(effect)})`" :stroke="effect.color" stroke-width="2"><path d="M-8 0H8M0-8V8m-5-5 10 10m0-10L-5 5"/></g>
        </g>
      </svg>
      <div v-if="awaiting || (['won','lost'].includes(state.phase) && !effects.length)" class="archery-result">
        <strong>{{ awaiting ? '本次对决已保存' : state.phase==='won' ? '漂亮，赢下这一局！' : '再修正一次弧线试试' }}</strong>
        <span>{{ awaiting ? '从离开时的进度继续' : `本局 ${state.shots} 箭 · 命中 ${state.hits} 次${state.phase==='won' ? ` · ${state.score} 分` : ''}` }}</span>
        <button v-if="awaiting" class="button primary" :disabled="paused" @click="awaiting=false;focusSurface()">继续对决</button>
        <button v-else-if="state.phase==='won' && state.level<29 && mode==='campaign'" class="button primary" :disabled="paused" @click="start(state.level+1)">下一关</button>
        <button v-else class="button primary" :disabled="paused" @click="start(mode==='endless'?Math.floor(Math.random()*30):state.level)">再战一局</button>
      </div>
    </div>
    <div class="archery-controls">
      <div class="archery-sliders"><label>角度 <b>{{ Math.round(state.angle) }}°</b><input v-model.number="state.angle" aria-label="射箭角度" type="range" min="15" max="80" :disabled="!canAim"></label><label>力度 <b>{{ Math.round(state.power) }}</b><input v-model.number="state.power" aria-label="射箭力度" type="range" min="180" max="450" :disabled="!canAim"></label></div>
      <div class="archery-arrows" aria-label="选择箭种"><button v-for="(arrow,i) in ARROWS" :key="arrow.id" class="button archery-arrow" :class="{selected:state.kind===arrow.id}" :aria-pressed="state.kind===arrow.id" :disabled="!canAim || (arrow.id!=='normal' && state.ammo[arrow.id]<=0)" @click="state.kind=arrow.id;save()"><svg viewBox="0 0 38 16" aria-hidden="true" :class="`arrow-${arrow.id}`"><path d="m6 12 25-8m-8-1 9 1-6 6M11 10 5 8m6 2-3 5" fill="none" stroke="currentColor" :stroke-width="i===1?2.8:1.8"/><path v-if="i===2" d="m28 1 3 4 5-2-3 5" fill="none" stroke="#edac62"/><path v-if="i===3" d="m30 9 4-1-1 5" fill="none"/></svg><span>{{ arrow.name }} <small>{{ arrow.id==='normal'?'∞':state.ammo[arrow.id] }}</small></span></button></div>
      <div class="archery-fire"><button class="button" :disabled="!canAim" @click="aim">瞄准辅助</button><button class="button primary" :disabled="!canAim" @click="fire">↗ 放箭</button></div>
    </div>
    <p class="game-help archery-help" role="status">{{ state.last }} <span>拖动拉弓 · 松开放箭 · 空格也可发射</span></p>
  </div>
</template>
<style scoped>
.archery-topline{display:flex;align-items:center;gap:12px}.archery-topline :deep(.campaign-toolbar){flex:1;margin-bottom:7px;flex-wrap:nowrap}.archery-record{flex:none;color:var(--muted);font-size:11px;margin-bottom:7px}
.archery-stage{--archery-sky:#78b8e0;--archery-horizon:#c1e0df;position:relative;border:1px solid var(--line);border-radius:12px;overflow:hidden}.archery-surface{display:block;width:100%;aspect-ratio:16/9;outline-offset:-3px;touch-action:none;user-select:none;cursor:crosshair}
.archery-hud{position:absolute;z-index:1;inset:12px 14px auto;display:flex;align-items:flex-start;justify-content:space-between;gap:12px;pointer-events:none;color:#f3f6f4;font-size:11px}.archer-health{width:27%;max-width:240px;padding:7px 9px;border-radius:8px;background:#1c3444d9}.archer-health span{display:flex;justify-content:space-between;gap:8px}.archer-health b{font-variant-numeric:tabular-nums;font-weight:600}.archer-health i{display:block;height:5px;margin-top:5px;background:#12303c;border-radius:5px;overflow:hidden}.archer-health em{display:block;height:100%;background:#73d2b3;border-radius:5px;transition:width .18s}.enemy-health em{background:#e68b80}.archery-wind{display:flex;gap:7px;align-items:center;padding:6px 9px;border-radius:7px;background:#24445a99}.archery-wind small{opacity:.8}
.archery-result{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:13px;background:#132436b8;color:#f2f5ef;backdrop-filter:none}.archery-result strong{font-size:23px}.archery-result span{font-size:12px;color:#cbd7e0}
.archery-controls{display:grid;grid-template-columns:1.05fr 1.45fr .8fr;gap:12px;align-items:center;margin-top:10px}.archery-sliders{display:grid;gap:8px;font-size:12px;color:var(--muted)}.archery-sliders label{display:flex;align-items:center;gap:6px;min-width:0;white-space:nowrap}.archery-sliders b{min-width:31px;color:var(--text);font-weight:500}.archery-sliders input{width:100%;min-width:35px;accent-color:var(--primary)}.archery-arrows{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:5px}.archery-arrow{min-width:0;display:flex;flex-direction:column;align-items:center;gap:3px;padding:5px 3px;font-size:11px}.archery-arrow svg{width:38px;height:16px;color:#c4af87}.archery-arrow .arrow-blast{color:#d8a561}.archery-arrow .arrow-bounce{color:#8eb9d6}.archery-arrow span{white-space:nowrap}.archery-arrow small{color:var(--muted)}.archery-arrow.selected{border-color:var(--primary);background:var(--primary-soft)}.archery-fire{display:flex;gap:6px}.archery-fire .button{flex:1;min-width:0;padding:9px 6px;font-size:12px;white-space:nowrap}.archery-game .archery-help{display:flex;justify-content:space-between;gap:8px;margin:7px 0 0;text-align:left;font-size:10px}.archery-help span{color:var(--muted)}
:global(:root[data-theme="warm"] .archery-stage){--archery-sky:#abc9d1;--archery-horizon:#eadfc8}
@media(max-width:850px){.archery-controls{grid-template-columns:1fr 1.5fr}.archery-fire{grid-column:1/-1;justify-self:end}.archery-fire .button{min-width:100px}.archery-record{display:none}.archery-topline :deep(.campaign-toolbar){flex-wrap:wrap}.archery-hud{inset:8px}.archery-game .archery-help{display:block}}
@media(prefers-reduced-motion:reduce){.archer-health em{transition:none}}
</style>
