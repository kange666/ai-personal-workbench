<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { useGameLoop } from "../../composables/useGameLoop";
import { createDefense, createDefenseMeta, DEFENSE_CARDS, DEFENSE_ENEMY_NAMES, DEFENSE_UPGRADES, defenseStats, defenseUpgradeCost, pickDefenseCard, settleDefense, startDefense, startDefenseWave, stepDefense, upgradeDefense, validDefense, validDefenseMeta } from "../../utils/rest/defense";
const props=defineProps<{paused:boolean}>();
const {state,best,save,recordScore}=useGameSave("defense",createDefense,validDefense);
const {state:meta,save:saveMeta}=useGameSave("defense-meta",createDefenseMeta,validDefenseMeta);
const stats=computed(()=>defenseStats(state.value));const waiting=ref(state.value.phase==="playing");let elapsed=0;
const colors={normal:"#d78387",swarm:"#dca96d",runner:"#eab58b",armored:"#8c96b2",splitter:"#9c91d9",healer:"#77c59e",shooter:"#bb9fcf",shielded:"#78b8d2",boss:"#e66e88"};
const card=(id:string)=>DEFENSE_CARDS.find(c=>c.id===id)!;
useGameLoop(()=>props.paused||waiting.value,dt=>{const phase=state.value.phase;stepDefense(state.value,dt);elapsed+=dt;if(phase!==state.value.phase){settleDefense(state.value,meta.value);saveMeta();recordScore(state.value.score);}if(elapsed>=1||phase!==state.value.phase){save();elapsed=0;}});
function restart(){if(props.paused)return;state.value=createDefense();waiting.value=false;save();}
function begin(){if(props.paused)return;startDefense(state.value,meta.value);waiting.value=false;save();}
function pick(id:string){if(!props.paused&&pickDefenseCard(state.value,id))save();}
function improve(id:typeof DEFENSE_UPGRADES[number]["id"]){if(!props.paused&&upgradeDefense(state.value,id))save();}
function talent(id:"power"|"guard"|"wealth"){const cost=3+meta.value.talents[id]*3;if(props.paused||meta.value.talents[id]>=5||meta.value.shards<cost)return;meta.value.shards-=cost;meta.value.talents[id]++;saveMeta();}
function endless(){if(props.paused||state.value.phase!=='won')return;state.value.mode="endless";state.value.wave++;startDefenseWave(state.value);save();}
defineExpose({restart});
</script>
<template>
  <div class="defense-game">
    <div class="game-stats"><span>第 <b>{{ state.wave }}</b> 波 / {{ state.mode==='campaign'?'30':'∞' }}</span><span>金币 <b>{{ Math.floor(state.coins) }}</b></span><span>最高 <b>{{ best }}</b></span><span>历史 {{ meta.bestWave }} 波</span></div>
    <div class="defense-meters"><label>塔身 {{ Math.ceil(state.hp) }}/{{ Math.round(stats.maxHP) }}<meter :value="state.hp" :max="stats.maxHP" /></label><label>护盾 {{ Math.ceil(state.shield) }}/{{ stats.maxShield }}<meter :value="state.shield" :max="stats.maxShield" /></label></div>
    <div class="defense-stage" tabindex="0" data-game-focus aria-label="单塔守卫战场，塔自动攻击，可用下方按钮升级">
      <svg viewBox="0 0 600 400" aria-hidden="true"><defs><radialGradient id="defense-field"><stop stop-color="var(--primary-soft)"/><stop offset="1" stop-color="var(--surface-2)"/></radialGradient></defs><rect width="600" height="400" rx="12" fill="url(#defense-field)"/>
        <g stroke="var(--line)" fill="none" opacity=".4"><circle cx="300" cy="200" r="45"/><circle cx="300" cy="200" r="100"/><circle cx="300" cy="200" r="155"/><path d="M0 200H600M300 0V400"/></g>
        <circle cx="300" cy="200" :r="stats.range" fill="var(--primary)" opacity=".035" stroke="var(--primary)" stroke-dasharray="3 8"/>
        <g v-for="e in state.enemies" :key="e.id" :transform="`translate(${e.x} ${e.y})`"><circle v-if="e.shield>0" :r="e.kind==='boss'?24:13" fill="none" stroke="#8dd2f7" stroke-width="2"/><path :d="e.kind==='boss'?'M0-21 20-6 13 18H-13L-20-6Z':e.kind==='runner'?'M0-10 11 8H-11Z':'M-8-8H8V8H-8Z'" :fill="colors[e.kind]" :stroke="e.freeze>0?'#bdeaff':'none'" stroke-width="3"/><rect x="-12" :y="e.kind==='boss'?-29:-17" width="24" height="3" rx="1" fill="var(--line)"/><rect x="-12" :y="e.kind==='boss'?-29:-17" :width="24*Math.max(0,e.hp/e.maxHP)" height="3" fill="var(--success)"/><circle v-if="e.burnTime>0" cy="2" r="4" fill="#ffb867"/><title>{{ DEFENSE_ENEMY_NAMES[e.kind] }}</title></g>
        <g v-for="(trace,i) in state.traces" :key="i"><path :d="`M300 200L${trace.x} ${trace.y}`" :stroke="trace.color" :stroke-width="state.cards.chain?2:1.5" :opacity="trace.life*4"/><circle :cx="trace.x" :cy="trace.y" r="6" :fill="trace.color" :opacity="trace.life*3"/></g>
        <circle v-if="state.shield>0" cx="300" cy="200" r="31" fill="var(--primary)" opacity=".12" stroke="var(--primary)" stroke-width="2"/><g transform="translate(300 200)" fill="var(--primary)" stroke="var(--text)" stroke-width="1"><path d="m0-24 20 12-4 31H-16L-20-12Z"/><circle r="10" fill="var(--surface)"/><circle r="4" fill="var(--warning)"/></g>
        <text x="14" y="24" fill="var(--muted)" font-size="11">攻击 {{ Math.round(stats.damage) }} · 攻速 {{ stats.rate.toFixed(1) }} · 范围 {{ Math.round(stats.range) }}</text><text x="14" y="382" fill="var(--muted)" font-size="11">{{ state.enemies.length }} 个敌人 · 本波待入场 {{ state.spawnLeft }} · 已击败 {{ state.kills }}</text>
      </svg>
      <div v-if="state.phase==='ready'||waiting||state.phase==='won'||state.phase==='lost'" class="defense-overlay">
        <h3>{{ waiting?'守卫进度已保存':state.phase==='ready'?'今晚，只守这一座塔':state.phase==='won'?'守住三十波！':'这次已经很棒了' }}</h3>
        <template v-if="state.phase==='ready'"><p>自动攻击 · 每波三选一 · 初期更宽松</p><div class="game-actions"><button v-for="entry in [{id:'power',name:'火力 +10%'},{id:'guard',name:'守护 +30生命'},{id:'wealth',name:'补给 +25金币'}] as const" :key="entry.id" class="button" :class="{primary:state.perk===entry.id}" :disabled="paused" @click="state.perk=entry.id">{{ entry.name }}</button></div><div class="game-actions"><button class="button primary" :disabled="paused" @click="begin">开始守卫</button></div></template>
        <button v-else-if="waiting" class="button primary" :disabled="paused" @click="waiting=false">继续守卫</button>
        <template v-else><p>第 {{ state.wave }} 波 · {{ state.score }} 分 · 研究碎片 {{ meta.shards }}</p><button v-if="state.phase==='won'" class="button primary" :disabled="paused" @click="endless">继续无尽挑战</button><button v-else class="button primary" :disabled="paused" @click="restart">再试一种组合</button></template>
      </div>
    </div>
    <div v-if="state.phase==='choice'" class="defense-choices" aria-label="选择本波强化"><button v-for="id in state.offers" :key="id" class="defense-card" :disabled="paused" @click="pick(id)"><small>{{ card(id).family }} · 等级 {{ (state.cards[id]||0)+1 }}</small><strong>{{ card(id).name }}</strong><span>{{ card(id).description }}</span></button></div>
    <div class="defense-upgrades"><button v-for="upgrade in DEFENSE_UPGRADES" :key="upgrade.id" class="button" :disabled="paused||waiting||state.coins<defenseUpgradeCost(state,upgrade.id)||['won','lost'].includes(state.phase)" :title="upgrade.text" @click="improve(upgrade.id)">{{ upgrade.name }} {{ state.upgrades[upgrade.id] }}级 <b>{{ defenseUpgradeCost(state,upgrade.id) }} ◈</b></button></div>
    <div v-if="Object.keys(state.cards).length" class="defense-build"><span v-for="(count,id) in state.cards" :key="id" :title="card(String(id)).description">{{ card(String(id)).name }} ×{{ count }}</span></div>
    <div v-if="state.phase==='ready'||state.phase==='lost'||state.phase==='won'" class="defense-research"><span>研究碎片 {{ meta.shards }} · 天赋永久解锁</span><button v-for="id in ['power','guard','wealth'] as const" :key="id" class="button" :disabled="paused||meta.talents[id]>=5||meta.shards<3+meta.talents[id]*3" @click="talent(id)">{{ {power:'火力',guard:'守护',wealth:'补给'}[id] }} {{ meta.talents[id] }}/5 · {{ 3+meta.talents[id]*3 }}碎片</button></div>
    <p class="game-help" role="status">{{ state.last }}<br>前几波推荐先点攻击与攻速。选卡时战场暂停，随时关闭都会保存进度。</p>
  </div>
</template>
<style scoped>
.defense-stage{position:relative;border:1px solid var(--line);border-radius:12px;overflow:hidden;outline-offset:3px}.defense-stage svg{display:block;width:100%;aspect-ratio:3/2}.defense-meters{display:flex;gap:14px;margin-bottom:10px;color:var(--muted);font-size:11px}.defense-meters label{display:flex;flex:1;gap:7px;align-items:center}.defense-meters meter{min-width:30px;flex:1;height:8px;accent-color:var(--success)}.defense-overlay{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;background:color-mix(in srgb,var(--surface) 94%,transparent);text-align:center;padding:16px;gap:10px}.defense-overlay h3{font-size:20px;margin:0}.defense-overlay p{font-size:12px;color:var(--muted);margin:0}.defense-choices{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:8px;margin-top:12px}.defense-card{display:flex;flex-direction:column;gap:8px;text-align:left;color:var(--text);background:var(--surface-2);border:1px solid var(--primary);border-radius:10px;padding:12px;cursor:pointer;min-width:0}.defense-card small,.defense-card span{font-size:10px;color:var(--muted);line-height:1.5}.defense-card strong{font-size:13px}.defense-upgrades{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:7px;margin-top:12px}.defense-upgrades .button{font-size:11px;display:flex;justify-content:space-between;gap:5px;padding:8px}.defense-upgrades b{color:var(--warning);font-weight:500}.defense-build{display:flex;flex-wrap:wrap;gap:5px;margin-top:10px}.defense-build span{font-size:10px;border:1px solid var(--line);padding:4px 6px;border-radius:5px;color:var(--muted)}.defense-research{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin-top:12px;font-size:10px;color:var(--muted)}.defense-research .button{font-size:10px;padding:6px}
</style>
