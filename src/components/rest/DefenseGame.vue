<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { useGameSave } from '../../composables/useGameSave';
import { useGameLoop } from '../../composables/useGameLoop';
import { closeDefenseOffers, createDefense, createDefenseMeta, DEFENSE_CARDS, DEFENSE_UPGRADES, defenseOffers, defenseStats, defenseUpgradeCost, enterDefenseEndless, migrateDefense, pickDefenseCard, settleDefense, startDefense, stepDefense, upgradeDefense, validDefense, validDefenseMeta, type DefenseEnemy, type DefenseState } from '../../utils/rest/defense';
import { defenseCardDetails } from '../../utils/rest/defenseCardDetails';
import { addBattleEffect, effectProgress, stepBattleEffects, type BattleEffect } from '../../utils/rest/battleEffects';
import DefenseSprites from './DefenseSprites.vue';

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore } = useGameSave('defense', createDefense, validDefense, migrateDefense);
const { state: meta, save: saveMeta } = useGameSave('defense-meta', createDefenseMeta, validDefenseMeta);
const stats = computed(() => defenseStats(state.value));
const waiting = ref(state.value.phase === 'playing'), surface = ref<HTMLElement>();
const effects = ref<BattleEffect[]>([]), recoil = ref(0), towerHurt = ref(0), direction = ref(-25);
const boss = computed(() => state.value.enemies.find(e => e.kind === 'boss'));
const card = (id: string) => DEFENSE_CARDS.find(c => c.id === id)!;
const upgradeIcons = ['M5 19 18 6m-5 0h5v5M4 15l5 5', 'm5 5 6 7-6 7m8-14 6 7-6 7', 'M5 20V9h3V5h3v4h3V5h3v4h2v11ZM10 20v-6h4v6', 'M12 2v20M2 12h20M19 12a7 7 0 1 1-14 0 7 7 0 0 1 14 0', 'm6 18 8-8M14 10q-5-8 3-7l-2 4 3 2 3-3q3 9-7 6L8 21Z', 'M8 5h8l-2 4q8 5 6 10H4Q2 14 10 9ZM10 2h4m-2 10v6m-3-4h5m-5 3h5'];
let elapsed = 0, effectId = 0;
function effect(kind: BattleEffect['kind'], x: number, y: number, text = '', target?: number, color = '#ffe0a0', fromX?: number, fromY?: number) {
  const duration = kind === 'shot' || kind === 'chain' ? .16 : kind === 'burst' ? .4 : .7;
  addBattleEffect(effects.value, { id: ++effectId, kind, x, y, text, target, color, fromX, fromY, duration, life: duration });
}
// 特效只观察已有战斗结果，不修改伤害、敌人位置或随机数；也不写入存档。
useGameLoop(() => props.paused || waiting.value || (state.value.phase !== 'playing' && !effects.value.length && recoil.value === 0 && towerHurt.value === 0), dt => {
  effects.value = stepBattleEffects(effects.value, dt);
  recoil.value = Math.max(0, recoil.value - dt * 7);
  towerHurt.value = Math.max(0, towerHurt.value - dt * 5);
  const s = state.value, phase = s.phase, wave = s.wave, traces = s.traces.slice(), hp = s.hp + s.shield, time = s.time;
  const previous = new Map(s.enemies.map(e => [e.id, { hp: e.hp, shield: e.shield, x: e.x, y: e.y }]));
  stepDefense(s, dt);
  const fresh = s.traces.filter(t => !traces.some(old => old.x === t.x && old.y === t.y && old.color === t.color && old.life >= t.life));
  let origin = { x: 300, y: 177 };
  for (const trace of fresh) {
    const chain = trace.color === '#c2a5ff';
    if (!chain) {
      direction.value = Math.atan2(trace.y - 177, trace.x - 300) * 180 / Math.PI;
      const rad = direction.value * Math.PI / 180;
      effect('shot', trace.x, trace.y, '', undefined, '#f0c47b', 300 + Math.cos(rad) * 30, 177 + Math.sin(rad) * 30);
      origin = trace; recoil.value = 1;
      if (s.cards.blast) effect('burst', trace.x, trace.y, '', undefined, '#edbb7c');
    } else effect('chain', trace.x, trace.y, '', undefined, '#a6dcf0', origin.x, origin.y);
  }
  for (const enemy of s.enemies) {
    const old = previous.get(enemy.id);
    if (old) {
      const damage = old.hp + old.shield - enemy.hp - enemy.shield;
      if (damage > .1 && !effects.value.some(e => e.kind === 'hit' && e.target === enemy.id && e.life > .4)) effect('hit', enemy.x, enemy.y - 20, `−${Math.ceil(damage)}`, enemy.id);
      previous.delete(enemy.id);
    }
  }
  for (const old of previous.values()) effect('coin', old.x, old.y);
  if (s.hp + s.shield < hp - .1) towerHurt.value = 1;
  if (s.cards.nova && Math.floor(time / 5) !== Math.floor(s.time / 5)) effect('burst', 300, 200, '', undefined, '#c8b6ef');
  if (phase !== s.phase || wave !== s.wave) { settleDefense(s, meta.value); saveMeta(); recordScore(s.score); }
  elapsed += dt;
  if (elapsed >= 1 || phase !== s.phase || wave !== s.wave) { save(); elapsed = 0; }
});
const focusSurface = () => nextTick(() => surface.value?.focus());
function clearEffects() { effects.value = []; recoil.value = 0; towerHurt.value = 0; }
function restart() {
  if (props.paused) return;
  state.value = createDefense(Date.now() >>> 0, state.value.mode); waiting.value = false; clearEffects(); save(); void focusSurface();
}
function chooseMode(mode: DefenseState['mode']) {
  if (props.paused || state.value.phase !== 'ready') return;
  state.value.mode = mode; save();
}
function begin() {
  if (props.paused) return;
  startDefense(state.value, meta.value); waiting.value = false; save(); void focusSurface();
}
function pick(id: string) {
  if (props.paused || !pickDefenseCard(state.value, id)) return;
  effect('upgrade', 300, 200, card(id).name); save(); void focusSurface();
}
function openUpgrades(refresh=false) {
  if (props.paused || waiting.value) return;
  defenseOffers(state.value, refresh); save();
}
function deferUpgrades() {
  if (props.paused) return;
  closeDefenseOffers(state.value); save(); void focusSurface();
}
function improve(id: typeof DEFENSE_UPGRADES[number]['id']) {
  if (props.paused || waiting.value || !upgradeDefense(state.value, id)) return;
  effect('upgrade', 300, 200, DEFENSE_UPGRADES.find(u => u.id === id)!.text); save();
}
function talent(id: 'power' | 'guard' | 'wealth') {
  const cost = 3 + meta.value.talents[id] * 3;
  if (props.paused || meta.value.talents[id] >= 5 || meta.value.shards < cost) return;
  meta.value.shards -= cost; meta.value.talents[id]++; saveMeta();
}
function endless() {
  if (props.paused || !enterDefenseEndless(state.value)) return;
  save(); void focusSurface();
}
function enemyPose(e: DefenseEnemy) {
  const hit = effects.value.find(f => f.kind === 'hit' && f.target === e.id);
  const impact = hit ? Math.max(0, (hit.life - .45) * 4) : 0;
  const bob = state.value.phase === 'playing' && e.freeze <= 0 ? Math.sin(state.value.time * (e.kind === 'swarm' ? 12 : 7) + e.id) * (e.kind === 'boss' ? .7 : 1.3) : 0;
  return `translate(${e.x} ${e.y + bob}) rotate(${impact * 10}) scale(${1 + impact * .12} ${1 - impact * .12})`;
}
function beamPath(e: BattleEffect) {
  const fromX = e.fromX ?? 300, fromY = e.fromY ?? 177, dx = e.x - fromX, dy = e.y - fromY;
  if (e.kind === 'chain') return `M${fromX} ${fromY}l${dx*.25-dy*.06} ${dy*.25+dx*.06} ${dx*.25+dy*.1} ${dy*.25-dx*.1} ${dx*.25-dy*.08} ${dy*.25+dx*.08} ${dx*.25+dy*.04} ${dy*.25-dx*.04}`;
  const t = effectProgress(e), tail = Math.max(0, t - .23);
  return `M${fromX+dx*tail} ${fromY+dy*tail}L${fromX+dx*t} ${fromY+dy*t}`;
}
defineExpose({ restart });
</script>
<template>
  <div class="defense-game battle-game" :class="{ 'battle-paused': paused || waiting }">
    <div class="defense-topline"><span>{{ state.mode==='endless'?'无尽 · ':'' }}第 <b>{{ state.wave }}</b> / {{ state.mode==='campaign'?'30':'∞' }} 波 <small>{{ state.wave%10===0?'· 首领波':'' }}</small><button v-if="state.mode==='campaign'&&(state.phase==='playing'||state.phase==='choice')" class="button defense-endless-link" :disabled="paused" title="保留当前血量、金币和强化，转为无尽模式" @click="endless">转为无尽</button></span><button class="button defense-strengthen" :class="{primary:state.pendingUpgrades>0}" :disabled="paused||waiting||state.pendingUpgrades===0||!['playing','won'].includes(state.phase)" @click="openUpgrades()">强化 <b>{{ state.pendingUpgrades }}</b> 次</button><span class="defense-coins">◈ {{ Math.floor(state.coins) }}</span><span>最高 {{ best }} <small>· 历史 {{ meta.bestWave }} 波</small></span></div>
    <div ref="surface" class="defense-stage" tabindex="0" data-game-focus aria-label="单塔守卫战场，塔自动攻击，可用下方按钮升级">
      <svg viewBox="-100 -35 800 450" aria-hidden="true" class="defense-surface">
        <DefenseSprites v-once />
        <g v-once>
          <rect x="-100" y="-35" width="800" height="450" fill="var(--defense-ground)"/>
          <path d="M-100 75Q50 36 99 105T245 65 400 92 580 47 700 84V-35H-100ZM-100 360Q33 315 155 368T368 349 546 352 700 331V415H-100Z" fill="var(--defense-edge)"/>
          <path d="M-100 43Q43 89 141 156T304 210 510 326 700 353M170-35Q182 98 278 174T405 279 462 415" fill="none" stroke="var(--defense-path)" stroke-width="18" opacity=".45"/>
          <g color="var(--defense-tree)"><use v-for="(p,i) in [[-77,48],[-54,6],[-9,-8],[61,-7],[132,-12],[226,-15],[383,-9],[480,-10],[564,-9],[654,3],[685,70],[-81,192],[677,217],[-65,336],[-30,395],[56,398],[148,405],[262,409],[382,406],[482,403],[587,395],[665,355]]" :key="i" href="#defense-pine" :transform="`translate(${p[0]} ${p[1]}) scale(${1.1+i%3*.3})`"/></g>
          <g fill="var(--defense-grass)" opacity=".65"><path v-for="(p,i) in [[16,85],[123,44],[197,79],[420,32],[543,93],[614,167],[38,258],[116,320],[208,355],[430,345],[563,322],[620,245],[-23,164],[473,147],[166,258]]" :key="i" :d="`M${p[0]} ${p[1]}l-4-8 2 8 5-11-2 11 7-5-3 5Z`"/></g>
          <g fill="#84958e" stroke="#647d78" stroke-width="1"><path d="m38 137 17-5 10 12-8 15H37Z"/><path d="m536 256 15-5 11 12-9 14h-18Z"/><path d="m215 320 11-3 8 8-7 8h-14Z"/></g>
          <circle cx="300" cy="200" r="51" fill="var(--defense-stone)" stroke="#9eaf9a" stroke-width="1"/><circle cx="300" cy="200" r="37" fill="none" stroke="#9eaf9a" stroke-dasharray="19 2" stroke-width="11" opacity=".55"/>
        </g>
        <circle cx="300" cy="200" :r="stats.range" fill="none" stroke="var(--defense-range)" opacity=".65" stroke-dasharray="5 7" stroke-width="1.2"/>
        <g v-for="e in state.enemies" :key="e.id" :data-enemy="e.kind" :data-enemy-id="e.id">
          <g :transform="enemyPose(e)"><use :href="`#defense-${e.kind}`"/><circle v-if="e.shield>0" r="18" cy="-2" fill="none" stroke="#c2eef4" stroke-width="1.3" opacity=".8"/><path v-if="e.freeze>0" d="M-15-16H15V11H-15Z" fill="#d1f2fc" opacity=".45" stroke="#e1faff"/><ellipse v-else-if="e.slow>0" cy="9" rx="18" ry="6" fill="none" stroke="#b2e9ee" stroke-width="1.5"/><path v-if="e.burnTime>0" d="M-5 6Q-8-1 0-8L1 0 5-3Q10 8-1 10Z" fill="#f0b363" opacity=".9"/></g>
          <g :transform="`translate(${e.x} ${e.y-(e.kind==='boss'?40:23)})`"><rect :x="e.kind==='boss'?-27:-12" :width="e.kind==='boss'?54:24" height="3" rx="1.5" fill="#344b50"/><rect :x="e.kind==='boss'?-27:-12" :width="(e.kind==='boss'?54:24)*Math.max(0,e.hp/e.maxHP)" height="3" rx="1.5" fill="#e58d83"/></g>
        </g>
        <g data-central-tower transform="translate(300 200)">
          <ellipse cy="20" rx="35" ry="9" fill="#28494b" opacity=".18"/>
          <ellipse cy="13" rx="30" ry="14" fill="#68757d" stroke="#465363" stroke-width="1.7"/>
          <ellipse cy="9" rx="29" ry="12" fill="#99a9a4" stroke="#d6d7be" stroke-width="1.6"/>
          <ellipse v-if="state.shield>0" cy="8" rx="32" ry="15" fill="none" stroke="#9bdee0" stroke-width="1.8" :opacity=".4+state.shield/stats.maxShield*.5"/>
          <g :transform="`rotate(${towerHurt*4} 0 12)`">
            <path d="M-21-14V9Q0 24 21 9V-14Z" fill="#c6c9b5" stroke="#526071" stroke-width="1.7"/>
            <path d="M5-14V16L17 12 18-12Z" fill="#8f9da0"/>
            <path d="M-20-4 0 2 21-4M-10-11V-2M-3 3v13m15-20v12" fill="none" stroke="#708087" stroke-width="1.3"/>
            <path d="M-6-8 2-6 2 10-6 8Z" fill="#f2ca78" stroke="#6a6a5e" stroke-width="1.2"/>
            <ellipse cy="-15" rx="23" ry="11" fill="#677584" stroke="#465363" stroke-width="1.7"/>
            <path v-for="i in Math.min(3,Math.floor((state.upgrades.health+state.upgrades.attack)/3))" :key="i" :d="`M${-22+i*9} -6v15h5V-5Z`" fill="#9c8bbd" stroke="#59647a" stroke-width="1"/>
            <path d="M-12-24-10-13Q0-8 11-14L12-24Z" fill="#8b75ad" stroke="#465363" stroke-width="1.5"/>
            <ellipse cy="-24" rx="12" ry="6" fill="#a697b9" stroke="#465363" stroke-width="1.5"/>
          </g>
          <g :transform="`translate(0 -23) rotate(${direction})`" data-tower-barrel>
            <g :transform="`translate(${-recoil*4} 0)`"><rect x="-9" y="-7" width="34" height="14" rx="4" fill="#526171" stroke="#344555" stroke-width="1.5"/><path d="M-4-6v12m12-12v12" stroke="#c4c9b3" stroke-width="3"/><path d="M23-8H30V8H23Z" fill="#c3b58e" stroke="#6b6a62" stroke-width="1.4"/><ellipse cx="30" rx="3" ry="7" fill="#334655" stroke="#e3c88e" stroke-width="1.2"/></g>
            <path v-if="recoil>.3" d="m31 0 8-3-2-5 7 5 6-1-5 4 6 3-7-1-5 5 1-5Z" fill="#ffe0a0" :opacity="recoil"/>
          </g>
          <ellipse v-if="towerHurt>0" cy="7" rx="33" ry="17" fill="none" stroke="#f4be94" stroke-width="2" :opacity="towerHurt"/>
        </g>
        <g v-for="fx in effects" :key="fx.id" data-battle-effect :data-effect-kind="fx.kind" :opacity="Math.min(1,fx.life*5)">
          <path v-if="fx.kind==='shot'||fx.kind==='chain'" :d="beamPath(fx)" fill="none" :stroke="fx.color" :stroke-width="fx.kind==='chain'?1.5:2.5" stroke-linecap="round"/>
          <g v-else-if="fx.kind==='coin'" :transform="`translate(${fx.x+(300-fx.x)*effectProgress(fx)} ${fx.y+(200-fx.y)*effectProgress(fx)-Math.sin(effectProgress(fx)*Math.PI)*18})`"><circle r="4.5" fill="#efc477" stroke="#fff0bc"/><path d="M0-2V2" stroke="#b28a4c" stroke-width="1.5"/></g>
          <g v-else-if="fx.kind==='hit'" :transform="`translate(${fx.x} ${fx.y-effectProgress(fx)*12})`"><text text-anchor="middle" :fill="fx.color" font-size="11" font-weight="700" stroke="#6b6050" stroke-width=".7" paint-order="stroke">{{ fx.text }}</text><path v-if="effectProgress(fx)<.3" d="M-5 13H5m-5-5v10m-4-9 8 8" :stroke="fx.color" stroke-width="1.3"/></g>
          <g v-else-if="fx.kind==='upgrade'"><ellipse :cx="fx.x" :cy="fx.y+5" :rx="28+effectProgress(fx)*28" :ry="14+effectProgress(fx)*10" fill="none" stroke="#c2a5ee" stroke-width="2"/><text :x="fx.x" :y="fx.y-49-effectProgress(fx)*12" text-anchor="middle" fill="#e8d3ff" stroke="#56685f" stroke-width=".6" paint-order="stroke" font-size="11" font-weight="700">{{ fx.text }}</text></g>
          <circle v-else :cx="fx.x" :cy="fx.y" :r="8+effectProgress(fx)*38" fill="none" :stroke="fx.color" stroke-width="2"/>
        </g>
      </svg>
      <div class="defense-hud" aria-label="塔身与护盾"><div><span>♥ {{ Math.ceil(state.hp) }} / {{ Math.round(stats.maxHP) }}</span><i><em :style="{width:`${Math.max(0,state.hp/stats.maxHP)*100}%`}"/></i></div><div class="defense-shield"><span>◇ {{ Math.ceil(state.shield) }} / {{ stats.maxShield }}</span><i><em :style="{width:`${state.shield/stats.maxShield*100}%`}"/></i></div><span v-if="boss" class="defense-boss-tag">区域首领</span></div>
      <span v-if="state.phase==='playing'&&!waiting" class="defense-wave-detail">场上 {{ state.enemies.length }} · 待入场 {{ state.spawnLeft }} · 击败 {{ state.kills }}</span>
      <div v-if="state.phase==='ready'||waiting||state.phase==='won'||state.phase==='lost'" class="defense-overlay">
        <h3>{{ waiting?'守卫进度已保存':state.phase==='ready'?'今晚，只守这一座塔':state.phase==='won'?'守住三十波！':'这次已经很棒了' }}</h3>
        <template v-if="state.phase==='ready'"><div class="game-actions defense-modes" aria-label="选择守卫模式"><button v-for="mode in [{id:'campaign',name:'30波闯关'},{id:'endless',name:'无尽守卫'}] as const" :key="mode.id" class="button" :class="{primary:state.mode===mode.id}" :aria-pressed="state.mode===mode.id" :disabled="paused" @click="chooseMode(mode.id)">{{ mode.name }}</button></div><p>{{ state.mode==='endless'?'不限波数 · 升级不封顶 · 每波三选一':'自动攻击 · 每波三选一 · 初期更宽松' }}</p><div class="game-actions"><button v-for="entry in [{id:'power',name:'火力 +10%'},{id:'guard',name:'守护 +30生命'},{id:'wealth',name:'补给 +25金币'}] as const" :key="entry.id" class="button" :class="{primary:state.perk===entry.id}" :disabled="paused" @click="state.perk=entry.id;save()">{{ entry.name }}</button></div><div class="game-actions"><button class="button primary" :disabled="paused" @click="begin">开始守卫</button></div></template>
        <button v-else-if="waiting" class="button primary" :disabled="paused" @click="waiting=false;focusSurface()">继续守卫</button>
        <template v-else><p>第 {{ state.wave }} 波 · {{ state.score }} 分 · 研究碎片 {{ meta.shards }}</p><button v-if="state.phase==='won'" class="button primary" :disabled="paused" @click="endless">继续无尽挑战</button><button v-else class="button primary" :disabled="paused" @click="restart">再试一种组合</button></template>
      </div>
      <div v-if="state.phase==='choice'" class="defense-choice-overlay"><h3>选择强化 · 剩余 {{ state.pendingUpgrades }} 次</h3><p>战斗已暂停。刷新免费且不限次数；也可以留待以后选择。</p><div class="game-actions"><button class="button" :disabled="paused" @click="openUpgrades(true)">刷新三选一</button><button class="button" :disabled="paused" @click="deferUpgrades">暂不选择，继续战斗</button></div><div class="defense-choices" aria-label="选择强化"><button v-for="id in state.offers" :key="id" class="defense-card" :disabled="paused" @click="pick(id)"><small>{{ card(id).family }} · {{ state.cards[id]||0 }} → {{ (state.cards[id]||0)+1 }} 级</small><strong>{{ card(id).name }}</strong><span>{{ defenseCardDetails(state,id).description }}</span><small v-if="state.mode==='endless'&&(state.cards[id]||0)>=6">精研：全局伤害加成再加4个百分点</small></button></div><p class="defense-card-context">{{ defenseCardDetails(state,state.offers[0]||'power').context }}</p></div>
    </div>
    <div class="defense-upgrades"><button v-for="(upgrade,i) in DEFENSE_UPGRADES" :key="upgrade.id" class="button" :disabled="paused||waiting||state.coins<defenseUpgradeCost(state,upgrade.id)||['won','lost'].includes(state.phase)" :title="upgrade.text" @click="improve(upgrade.id)"><svg viewBox="0 0 24 24" aria-hidden="true"><path :d="upgradeIcons[i]" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg><span>{{ upgrade.name }} <small>{{ state.upgrades[upgrade.id] }}级</small><b>{{ defenseUpgradeCost(state,upgrade.id) }} ◈</b></span></button></div>
    <details v-if="Object.keys(state.cards).length||state.mode==='endless'" class="defense-build"><summary>{{ state.mode==='endless'?'无尽规则 · ':'' }}当前强化 {{ Object.keys(state.cards).length }} 项 · 攻击 {{ Math.round(stats.damage) }} · 攻速 {{ stats.rate.toFixed(1) }} · 范围 {{ Math.round(stats.range) }}</summary><p v-if="state.mode==='endless'">波数、局内升级与强化牌等级不封顶。攻速、射程超出上限的部分转为伤害；强化牌6级后每级额外增伤4%。</p><div><span v-for="(count,id) in state.cards" :key="id" :title="card(String(id)).description">{{ card(String(id)).name }} ×{{ count }}</span></div></details>
    <div v-if="state.phase==='ready'||state.phase==='lost'||state.phase==='won'" class="defense-research"><span>研究碎片 {{ meta.shards }} · 天赋永久解锁</span><button v-for="id in ['power','guard','wealth'] as const" :key="id" class="button" :disabled="paused||meta.talents[id]>=5||meta.shards<3+meta.talents[id]*3" @click="talent(id)">{{ {power:'火力',guard:'守护',wealth:'补给'}[id] }} {{ meta.talents[id] }}/5 · {{ 3+meta.talents[id]*3 }}碎片</button></div>
    <p class="game-help defense-help" role="status">{{ state.last }} <span>波次自动继续 · 强化可累计 · 选卡暂停</span></p>
  </div>
</template>
<style scoped>
.defense-topline{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:7px;font-size:11px;color:var(--muted)}.defense-topline b{color:var(--text)}.defense-topline small{font-size:10px}.defense-coins{color:var(--warning);font-weight:600}
.defense-topline .defense-endless-link{margin-left:8px;padding:3px 7px;font-size:10px}.defense-build p{line-height:1.7;margin:5px 0}.defense-overlay .button:not(.primary){color:var(--text)}
.defense-strengthen{padding:5px 9px;font-size:11px;white-space:nowrap}.defense-stage>.defense-choice-overlay{z-index:2;overflow-y:auto;justify-content:safe center}.defense-choice-overlay .button{color:var(--text)}.defense-choice-overlay .defense-card-context{font-size:10px}.defense-choice-overlay .defense-card{gap:8px;padding:14px 12px;white-space:normal;overflow-wrap:anywhere}.defense-choice-overlay .defense-card span{line-height:1.7}
.defense-stage{--defense-ground:#91af98;--defense-edge:#769d87;--defense-path:#c1c5a0;--defense-tree:#3d7467;--defense-grass:#6c9b77;--defense-stone:#b4bda1;--defense-range:#d9e4c4;position:relative;border:1px solid var(--line);border-radius:12px;overflow:hidden;outline-offset:3px}.defense-surface{display:block;width:100%;aspect-ratio:16/9}
.defense-hud{position:absolute;inset:10px 12px auto;display:flex;align-items:flex-start;gap:9px;pointer-events:none;font-size:10px;color:#f2f5ec}.defense-hud>div{background:#28464add;padding:6px 9px;border-radius:7px;min-width:120px}.defense-hud span{font-variant-numeric:tabular-nums}.defense-hud i{display:block;height:4px;background:#193d42;border-radius:5px;margin-top:4px;overflow:hidden}.defense-hud em{display:block;height:100%;background:#86d4a5;transition:width .15s}.defense-shield em{background:#94d7e4}.defense-boss-tag{margin-left:auto;background:#7c4e59dc;border:1px solid #e7a3a2;padding:6px 9px;border-radius:7px}.defense-wave-detail{position:absolute;left:12px;bottom:9px;color:#e1ebd9;font-size:10px;border-radius:5px;background:#304f4abf;padding:4px 7px;pointer-events:none}
.defense-overlay,.defense-choice-overlay{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;padding:20px;text-align:center;background:#1b3139cc;color:#f2f3e9}.defense-overlay h3,.defense-choice-overlay h3{font-size:21px;margin:0}.defense-overlay p,.defense-choice-overlay p{font-size:12px;color:#d0dcd5;margin:0}.defense-stage .game-actions{margin-top:0}.defense-choices{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px;width:min(650px,100%);margin-top:8px}.defense-card{display:flex;flex-direction:column;gap:12px;text-align:left;color:var(--text);background:var(--surface);border:1px solid var(--primary);border-radius:12px;padding:18px 14px;cursor:pointer;min-width:0;min-height:140px;transition:transform .15s,border-color .15s}.defense-card:hover{transform:translateY(-3px);border-color:var(--text)}.defense-card small,.defense-card span{font-size:11px;color:var(--muted);line-height:1.6}.defense-card strong{font-size:15px;color:var(--primary)}
.defense-upgrades{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:7px;margin-top:9px}.defense-upgrades .button{display:flex;align-items:center;justify-content:center;gap:10px;padding:8px 6px;min-width:0;font-size:12px}.defense-upgrades svg{width:27px;height:27px;flex:none;color:var(--primary)}.defense-upgrades span{text-align:left;white-space:nowrap}.defense-upgrades small{font-size:10px;color:var(--muted)}.defense-upgrades b{display:block;color:var(--warning);font-size:10px;font-weight:500;margin-top:2px}.defense-build{margin-top:7px;font-size:10px;color:var(--muted)}.defense-build summary{cursor:pointer;line-height:1.8}.defense-build>div{display:flex;flex-wrap:wrap;gap:5px;margin-top:5px}.defense-build span{border:1px solid var(--line);border-radius:5px;padding:3px 5px}.defense-research{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin-top:8px;font-size:10px;color:var(--muted)}.defense-research .button{font-size:10px;padding:5px 7px}.defense-game .defense-help{display:flex;justify-content:space-between;gap:10px;margin:7px 0 0;font-size:10px;text-align:left}
:global(:root[data-theme="warm"] .defense-stage){--defense-ground:#b6b495;--defense-edge:#9eaa83;--defense-path:#d6caab;--defense-tree:#738666;--defense-stone:#d0c5a5;--defense-range:#f1e7cb}
@media(max-width:850px){.defense-upgrades{grid-template-columns:repeat(3,minmax(0,1fr))}.defense-upgrades .button{padding:6px}.defense-topline small{display:none}.defense-hud>div{min-width:90px}.defense-card{padding:12px 8px;min-height:120px;gap:8px}.defense-card strong{font-size:13px}.defense-game .defense-help{display:block}}
@media(prefers-reduced-motion:reduce){.defense-hud em,.defense-card{transition:none}.defense-card:hover{transform:none}}
</style>
