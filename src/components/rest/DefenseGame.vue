<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import { useGameSave } from '../../composables/useGameSave';
import { useGameLoop } from '../../composables/useGameLoop';
import { closeDefenseOffers, configureDefense, createDefense, createDefenseMeta, DEFENSE_BOSSES, DEFENSE_CARDS, DEFENSE_CORES, DEFENSE_EVOLUTIONS, DEFENSE_UPGRADES, defenseFeatures, defenseOffers, defensePurchaseQuote, defenseRegion, defenseRunReview, defenseStats, defenseTier, defenseUpgradeCost, defenseWaveTheme, enterDefenseEndless, migrateDefense, pickDefenseCard, purchaseDefenseLevels, selectDefenseCore, settleDefense, startDefense, stepDefense, validDefense, validDefenseMeta, type DefenseCoreId, type DefenseEnemy, type DefensePreference, type DefenseState, type DefenseTrace, type UpgradeId } from '../../utils/rest/defense';
import { defenseNumber } from '../../utils/rest/defenseCardDetails';
import { defenseProjectileOrigin } from '../../utils/rest/defenseContent';
import { addBattleEffect, stepBattleEffects, type BattleEffect } from '../../utils/rest/battleEffects';
import DefenseSprites from './DefenseSprites.vue';
import DefenseTower from './DefenseTower.vue';
import DefenseEffects from './DefenseEffects.vue';
import DefenseUpgradePanel from './DefenseUpgradePanel.vue';
import DefenseDetails from './DefenseDetails.vue';

const props=defineProps<{paused:boolean}>();
const {state,best,save,recordScore}=useGameSave('defense',createDefense,validDefense,migrateDefense);
const {state:meta,save:saveMeta}=useGameSave('defense-meta',createDefenseMeta,validDefenseMeta);
const stats=computed(()=>defenseStats(state.value)),features=computed(()=>defenseFeatures(state.value));
const tier=computed(()=>defenseTier(state.value)),region=computed(()=>defenseRegion(state.value));
const core=computed(()=>DEFENSE_CORES.find(c=>c.id===features.value.core));
const evolved=computed(()=>Object.values(features.value.evolutions).some(n=>!!n&&n>0));
const visualSkills=computed(()=>({fire:state.value.cards.burn||0,ice:state.value.cards.slow||0,chain:state.value.cards.chain||0,blast:state.value.cards.blast||0}));
const waiting=ref(state.value.phase==='playing'),surface=ref<HTMLElement>();
const effects=ref<BattleEffect[]>([]),recoil=ref(0),towerHurt=ref(0),direction=ref(-25),revision=ref(0),volleyFlash=ref(0);
const volleyCharge=computed(()=>{void revision.value;return features.value.log.shots%12;});
const boss=computed(()=>state.value.enemies.find(e=>e.kind==='boss'));
const bossInfo=computed(()=>boss.value?.boss?DEFENSE_BOSSES[boss.value.boss]:undefined);
const quotes=computed(()=>Object.fromEntries(DEFENSE_UPGRADES.map(u=>[u.id,defensePurchaseQuote(state.value,u.id,features.value.auto.batch)])));
const upgradeIcons=['M5 19 18 6m-5 0h5v5M4 15l5 5','m5 5 6 7-6 7m8-14 6 7-6 7','M5 20V9h3V5h3v4h3V5h3v4h2v11ZM10 20v-6h4v6','M12 2v20M2 12h20M19 12a7 7 0 1 1-14 0 7 7 0 0 1 14 0','m6 18 8-8M14 10q-5-8 3-7l-2 4 3 2 3-3q3 9-7 6L8 21Z','M8 5h8l-2 4q8 5 6 10H4Q2 14 10 9ZM10 2h4m-2 10v6m-3-4h5m-5 3h5'];
let elapsed=0,uiElapsed=0,effectId=0;
function effect(kind:BattleEffect['kind'],x:number,y:number,text='',target?:number,color='#ffe0a0',trace?:DefenseTrace){
  const duration=trace?.variant==='volley'?1:trace?.variant==='beam'?.5:kind==='shot'?.2:kind==='chain'?.24:kind==='burst'?.55:.7;
  const delay=kind==='hit'?.12:kind==='chain'?.06:kind==='burst'&&(trace?.variant==='shell'||trace?.variant==='ice')?.13:0;
  addBattleEffect(effects.value,{id:++effectId,kind,x,y,text,target,color,duration,life:duration,delay,fromX:trace?.fromX,fromY:trace?.fromY,variant:trace?.variant,radius:trace?.radius,amount:trace?.amount});
}
useGameLoop(()=>props.paused||waiting.value||state.value.phase==='choice'||(state.value.phase!=='playing'&&!effects.value.length&&recoil.value===0&&towerHurt.value===0),dt=>{
  effects.value=stepBattleEffects(effects.value,dt);recoil.value=Math.max(0,recoil.value-dt*7);towerHurt.value=Math.max(0,towerHurt.value-dt*5);volleyFlash.value=Math.max(0,volleyFlash.value-dt);
  const s=state.value,phase=s.phase,wave=s.wave,hp=s.hp+s.shield,traceIds=new Set(s.traces.map(t=>t.id));
  const previous=new Map(s.enemies.map(e=>[e.id,{hp:e.hp,shield:e.shield,x:e.x,y:e.y}]));
  stepDefense(s,dt);
  // 绘制预算只影响短特效；规则层的完整攻击和技能次数已经结算。
  const traces=s.traces.filter(t=>t.id!==undefined&&!traceIds.has(t.id));
  // 高连锁时也保留主弹道的位置，避免短电弧挤掉全部发射反馈。
  const special=traces.filter(t=>t.variant==='volley'||t.variant==='beam'||t.variant==='pulse');
  if(special.some(t=>t.variant==='volley'))volleyFlash.value=1.5;
  const normal=traces.filter(t=>!special.includes(t));
  const shots=normal.filter(t=>t.kind==='shot').slice(0,3),secondary=normal.filter(t=>t.kind!=='shot').slice(-5);
  // 齐射最后入绘制队列，普通命中反馈不会把七束弹幕全部挤走。
  for(const original of [...shots,...secondary,...special]){
    let trace=original;
    if(trace.kind==='shot'&&trace.variant!=='beam'){
      const origin=defenseProjectileOrigin(features.value.core,tier.value,trace);
      direction.value=origin.angle*180/Math.PI;recoil.value=1;
      trace={...trace,fromX:origin.x,fromY:origin.y};
    }
    effect(trace.kind==='enemy'?'shot':trace.kind||'shot',trace.x,trace.y,'',undefined,trace.color,trace);
  }
  let hitLabels=0;
  for(const enemy of s.enemies){
    const before=previous.get(enemy.id);
    if(before){
      const damage=before.hp+before.shield-enemy.hp-enemy.shield;
      if(damage>.1&&hitLabels<3&&!effects.value.some(e=>e.kind==='hit'&&e.target===enemy.id&&e.life>.4)){effect('hit',enemy.x,enemy.y-22,'−'+defenseNumber(damage),enemy.id);hitLabels++;}
      previous.delete(enemy.id);
    }
  }
  for(const before of [...previous.values()].slice(-3))effect('coin',before.x,before.y);
  if(s.hp+s.shield<hp-.1)towerHurt.value=1;
  if(phase!==s.phase||wave!==s.wave){settleDefense(s,meta.value);saveMeta();recordScore(s.score);}
  elapsed+=dt;uiElapsed+=dt;
  if(uiElapsed>=.25||phase!==s.phase||wave!==s.wave){revision.value++;uiElapsed=0;}
  if(elapsed>=1||phase!==s.phase||wave!==s.wave){save();elapsed=0;}
},{pauseOnBlur:false});
const focusSurface=()=>nextTick(()=>surface.value?.focus());
function persist(){revision.value++;save();}
function clearEffects(){effects.value=[];recoil.value=0;towerHurt.value=0;volleyFlash.value=0;elapsed=0;}
function restart(){
  if(props.paused)return;
  const previous=features.value,next=createDefense(Date.now()>>>0,state.value.mode);
  selectDefenseCore(next,previous.core==='classic'?'rapid':previous.core);configureDefense(next,{preference:previous.preference,auto:previous.auto});
  state.value=next;waiting.value=false;clearEffects();persist();void focusSurface();
}
function chooseMode(mode:DefenseState['mode']){if(props.paused||state.value.phase!=='ready')return;state.value.mode=mode;persist();}
function chooseCore(id:DefenseCoreId){if(props.paused||!selectDefenseCore(state.value,id))return;persist();}
function begin(){if(props.paused)return;startDefense(state.value,meta.value);waiting.value=false;persist();void focusSurface();}
function pick(id:string,catalog=false){
  if(props.paused||!pickDefenseCard(state.value,id,catalog))return;
  const name=DEFENSE_CARDS.find(c=>c.id===id)?.name||DEFENSE_EVOLUTIONS.find(e=>'evo:'+e.id===id)?.name||'强化';
  effect('upgrade',300,200,name);persist();if(state.value.phase!=='choice')void focusSurface();
}
function openUpgrades(refresh=false){if(props.paused||waiting.value)return;defenseOffers(state.value,refresh);persist();}
function deferUpgrades(){if(props.paused)return;closeDefenseOffers(state.value);persist();void focusSurface();}
function preference(value:DefensePreference){if(props.paused)return;configureDefense(state.value,{preference:value});defenseOffers(state.value,true);persist();}
function improve(id:UpgradeId){
  if(props.paused||waiting.value||!purchaseDefenseLevels(state.value,id,features.value.auto.batch))return;
  effect('upgrade',300,200,DEFENSE_UPGRADES.find(u=>u.id===id)!.name+'升级');persist();
}
function changeAuto(key:'enabled'|'priority'|'reserve'|'batch',event:Event){
  if(props.paused||waiting.value)return;
  const input=event.target as HTMLInputElement;
  const value=key==='enabled'?{enabled:input.checked}:key==='priority'?{priority:input.value as UpgradeId|'balanced'}:key==='batch'?{batch:Number(input.value) as 1|5|10}:{reserve:Number(input.value)};
  configureDefense(state.value,{auto:value});persist();
}
function talent(id:'power'|'guard'|'wealth'){
  const cost=3+meta.value.talents[id]*3;if(props.paused||meta.value.talents[id]>=5||meta.value.shards<cost)return;
  meta.value.shards-=cost;meta.value.talents[id]++;saveMeta();
}
function endless(){if(props.paused||!enterDefenseEndless(state.value))return;persist();void focusSurface();}
function enemyPose(e:DefenseEnemy){
  const hit=effects.value.find(f=>f.kind==='hit'&&f.target===e.id),impact=hit?Math.max(0,(hit.life-.45)*4):0;
  const bob=state.value.phase==='playing'&&e.freeze<=0?Math.sin(state.value.time*(e.kind==='swarm'?12:7)+e.id)*(e.kind==='boss'?.7:1.3):0;
  return 'translate('+e.x+' '+(e.y+bob)+') rotate('+(impact*10)+') scale('+(1+impact*.12)+' '+(1-impact*.12)+')';
}
defineExpose({restart});
</script>
<template>
  <div class="defense-game battle-game" :class="{ 'battle-paused': paused || waiting }">
    <div class="defense-topline"><span>{{ state.mode==='endless'?'无尽 · ':'' }}第 <b>{{ state.wave }}</b> / {{ state.mode==='campaign'?'30':'∞' }} 波 <small>{{ state.wave%10===0?'· 首领波':'' }}</small><button v-if="state.mode==='campaign'&&(state.phase==='playing'||state.phase==='choice')" class="button defense-endless-link" :disabled="paused" title="保留当前血量、金币和强化，转为无尽模式" @click="endless">转为无尽</button></span><button class="button defense-strengthen" :class="{primary:state.pendingUpgrades>0}" :disabled="paused||waiting||state.pendingUpgrades===0||!['playing','won'].includes(state.phase)" @click="openUpgrades()">强化 <b>{{ state.pendingUpgrades }}</b> 次</button><span class="defense-coins">◈ {{ defenseNumber(state.coins) }}</span><span>最高 {{ defenseNumber(best) }} <small>· 历史 {{ meta.bestWave }} 波</small></span></div>
    <div ref="surface" class="defense-stage" :class="`defense-region-${region.id}`" tabindex="0" data-game-focus aria-label="单塔守卫战场，塔自动攻击，可用下方按钮升级">
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
        <circle cx="300" cy="200" :r="Math.min(stats.range,470)" fill="none" stroke="var(--defense-range)" opacity=".65" stroke-dasharray="5 7" stroke-width="1.2"/>
        <g v-if="stats.auraDPS" data-skill-aura :opacity="paused||waiting ? .35 : .8">
          <circle cx="300" cy="200" :r="Math.min(360,stats.auraRadius)" fill="#eab06a" fill-opacity=".07" stroke="#f3c780" stroke-width="1.4" stroke-dasharray="8 5"/>
          <g :transform="`rotate(${state.time*24} 300 200)`"><path v-for="i in [0,1,2,3,4,5]" :key="i" d="M-7 0 0-10 7 0 0 5Z" :transform="`translate(${300+Math.cos(i*Math.PI/3)*Math.min(360,stats.auraRadius)} ${200+Math.sin(i*Math.PI/3)*Math.min(360,stats.auraRadius)})`" fill="#f4c17a" opacity=".65"/></g>
        </g>
        <g v-for="e in state.enemies" :key="e.id" :data-enemy="e.kind" :data-enemy-id="e.id">
          <g :transform="enemyPose(e)">
            <circle v-if="(e.charge||0)>0" r="23" cy="-3" fill="none" stroke="#f7be86" stroke-width="1.8" stroke-dasharray="5 3"/><circle v-if="e.boss==='bulwark'" r="31" cy="-3" fill="none" stroke="#a5ddf5" stroke-width="1.4"/><path v-if="e.boss==='summoner'" d="M-22-9-33-17-28-2m50-7 11-8-5 15" fill="none" stroke="#d9a7ed" stroke-width="3"/><use :href="`#defense-${e.kind}`"/>
            <circle v-if="e.shield>0" r="18" cy="-2" fill="none" stroke="#c2eef4" stroke-width="1.3" opacity=".8"/>
            <g v-if="e.freeze>0"><path d="M-17-9-8-21H9L18-8 12 13H-11Z" fill="#c5f0fa" fill-opacity=".4" stroke="#dafaff" stroke-width="1.3"/><path d="M-8-19-4-5 9 11M16-7-4-5-15 7M-4-5 6-19" fill="none" stroke="#f0ffff" stroke-width="1" opacity=".8"/></g>
            <g v-else-if="e.slow>0"><ellipse cy="10" rx="19" ry="7" fill="#a6dfe8" fill-opacity=".2" stroke="#d1f4f5" stroke-width="1.1"/><path d="M-15 9h7m15 4h7M-6 6l3 5-3 4m12-9-3 5 3 4" fill="none" stroke="#c7f5fa" stroke-width="1"/></g>
            <g v-if="e.burnTime>0"><path d="M-12 7Q-15 0-10-8L-9 0-4-4Q0 5-4 11m7-1Q-2 1 6-10L7-1 12-5Q17 7 7 12Z" fill="#ec9b63" stroke="#fbc788" stroke-width=".6"/><path d="M-9 7-8 1-5 7m9 1 3-8 2 8Z" fill="#ffe1a0"/></g>
          </g>
          <g :transform="`translate(${e.x} ${e.y-(e.kind==='boss'?40:23)})`"><rect :x="e.kind==='boss'?-27:-12" :width="e.kind==='boss'?54:24" height="3" rx="1.5" fill="#344b50"/><rect :x="e.kind==='boss'?-27:-12" :width="(e.kind==='boss'?54:24)*Math.max(0,e.hp/e.maxHP)" height="3" rx="1.5" fill="#e58d83"/></g>
        </g>
        <DefenseTower :core="features.core" :tier="tier" :recoil="recoil" :hurt="towerHurt" :direction="direction" :shield="state.shield" :max-shield="stats.maxShield" :evolved="evolved" :evolutions="features.evolutions" :skills="visualSkills" :paused="paused||waiting||state.phase!=='playing'" />
        <DefenseEffects :effects="effects" :tier="tier" :skills="visualSkills" />
      </svg>
      <div class="defense-hud" aria-label="塔身与护盾"><div><span>♥ {{ defenseNumber(Math.ceil(state.hp)) }} / {{ defenseNumber(Math.round(stats.maxHP)) }}</span><i><em :style="{width:`${Math.max(0,state.hp/stats.maxHP)*100}%`}"/></i></div><div class="defense-shield"><span>◇ {{ defenseNumber(Math.ceil(state.shield)) }} / {{ defenseNumber(stats.maxShield) }}</span><i><em :style="{width:`${state.shield/stats.maxShield*100}%`}"/></i></div><span v-if="boss" class="defense-boss-tag" :title="bossInfo?.description">{{ bossInfo?.name||'区域首领' }}</span></div>
      <div v-if="features.evolutions.volley" class="defense-volley-status" :class="{'volley-active':volleyFlash>0}" aria-label="穿云弹幕蓄能" role="status"><strong>{{ volleyFlash>0?'穿云弹幕 · 齐射释放！':'穿云弹幕 · 蓄能 '+volleyCharge+'/12' }}</strong><i><em :style="{width:`${volleyFlash>0?100:volleyCharge/12*100}%`}"/></i><small>已释放 {{ defenseNumber(features.volleyBursts||0) }} 次</small></div>
      <span v-if="state.phase==='playing'&&!waiting" class="defense-wave-detail">场上 {{ state.enemies.length }} · 待入场 {{ state.spawnLeft }} · 击败 {{ state.kills }}</span>
      <div v-if="state.phase==='ready'||waiting||state.phase==='won'||state.phase==='lost'" class="defense-overlay">
        <h3>{{ waiting?'守卫进度已保存':state.phase==='ready'?'今晚，只守这一座塔':state.phase==='won'?'守住三十波！':'这次已经很棒了' }}</h3>
        <template v-if="state.phase==='ready'">
          <div class="game-actions defense-modes" aria-label="选择守卫模式"><button v-for="mode in [{id:'campaign',name:'30波闯关'},{id:'endless',name:'无尽守卫'}] as const" :key="mode.id" class="button" :class="{primary:state.mode===mode.id}" :aria-pressed="state.mode===mode.id" :disabled="paused" @click="chooseMode(mode.id)">{{ mode.name }}</button></div>
          <div class="defense-core-choices" aria-label="选择炮塔核心"><button v-for="entry in DEFENSE_CORES" :key="entry.id" class="button defense-core-card" :class="{primary:features.core===entry.id}" :aria-pressed="features.core===entry.id" :disabled="paused" @click="chooseCore(entry.id)"><svg class="defense-core-preview" viewBox="230 112 145 128" aria-hidden="true"><DefenseTower :core="entry.id" :tier="3" :recoil="0" :hurt="0" :direction="-25" :shield="0" :max-shield="45" :evolved="false" :paused="true" :skills="{fire:0,ice:0,chain:0,blast:0}" /></svg><strong :style="{color:entry.color}">{{ entry.name }}</strong><small>{{ entry.description }}</small></button></div>
          <div class="game-actions"><button v-for="entry in [{id:'power',name:'火力 +10%'},{id:'guard',name:'守护 +30生命'},{id:'wealth',name:'补给 +25金币'}] as const" :key="entry.id" class="button" :class="{primary:state.perk===entry.id}" :disabled="paused" @click="state.perk=entry.id;persist()">{{ entry.name }}</button><button class="button primary" :disabled="paused" @click="begin">开始守卫</button></div>
          <p>{{ state.mode==='endless'?'不限波数与技能等级 · 阶段战场持续变化':'30波逐步加强 · 每10波特色首领' }} · 强化可累计、搜索与连续选择</p>
        </template>
        <button v-else-if="waiting" class="button primary" :disabled="paused" @click="waiting=false;focusSurface()">继续守卫</button>
        <template v-else><p>第 {{ state.wave }} 波 · {{ defenseNumber(state.score) }} 分 · 研究碎片 {{ meta.shards }}</p><p v-if="state.phase==='lost'" class="defense-loss-review">主要受伤来源：{{ defenseRunReview(state).top }}。{{ defenseRunReview(state).suggestion }}</p><button v-if="state.phase==='won'" class="button primary" :disabled="paused" @click="endless">继续无尽挑战</button><button v-else class="button primary" :disabled="paused" @click="restart">再试一种组合</button></template>
      </div>
      <DefenseUpgradePanel v-if="state.phase==='choice'" :state="state" :paused="paused" @pick="pick" @refresh="openUpgrades(true)" @close="deferUpgrades" @preference="preference" />
    </div>
    <div class="defense-battle-caption"><span>{{ core?.name||'旧版标准塔芯' }} · 外观阶段 {{ tier }}/5</span><span>{{ region.name }} · {{ defenseWaveTheme(state) }}</span></div>
    <div class="defense-upgrades"><button v-for="(upgrade,i) in DEFENSE_UPGRADES" :key="upgrade.id" class="button" :disabled="paused||waiting||!quotes[upgrade.id]?.levels||['won','lost'].includes(state.phase)" :title="upgrade.text+'；余额不足时购买可负担的等级'" @click="improve(upgrade.id)"><svg viewBox="0 0 24 24" aria-hidden="true"><path :d="upgradeIcons[i]" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/></svg><span>{{ upgrade.name }} <small>{{ state.upgrades[upgrade.id] }}级</small><b v-if="quotes[upgrade.id]?.levels">{{ defenseNumber(quotes[upgrade.id].cost) }} ◈ · +{{ quotes[upgrade.id].levels }}级</b><b v-else>需 {{ defenseNumber(defenseUpgradeCost(state,upgrade.id)) }} ◈</b></span></button></div>
    <div class="defense-automation">
      <label><input type="checkbox" aria-label="自动购买基础升级" :checked="features.auto.enabled" :disabled="paused||waiting" @change="changeAuto('enabled',$event)" />自动升级</label>
      <label>优先 <select class="input" aria-label="自动升级优先属性" :value="features.auto.priority" :disabled="paused||waiting" @change="changeAuto('priority',$event)"><option value="balanced">均衡轮换</option><option v-for="u in DEFENSE_UPGRADES" :key="u.id" :value="u.id">{{ u.name }}</option></select></label>
      <label>每次购买 <select class="input" aria-label="批量升级数量" :value="features.auto.batch" :disabled="paused||waiting" @change="changeAuto('batch',$event)"><option v-for="n in [1,5,10]" :key="n" :value="n">{{ n }}级</option></select></label>
      <label title="仅自动购买保留这部分金币，手动购买仍可自由使用">预留金币 <input class="input defense-reserve" type="number" min="0" step="1" aria-label="自动升级预留金币" :value="features.auto.reserve" :disabled="paused||waiting" @change="changeAuto('reserve',$event)" /></label>
    </div>
    <div v-if="features.core==='classic'&&state.phase!=='ready'" class="defense-legacy-core"><span>旧局进度完整保留，可选择一次新塔芯：</span><button v-for="entry in DEFENSE_CORES" :key="entry.id" class="button" :disabled="paused||waiting" @click="chooseCore(entry.id)">{{ entry.name }}</button></div>
    <DefenseDetails :state="state" :revision="revision" />
    <div v-if="state.phase==='ready'||state.phase==='lost'||state.phase==='won'" class="defense-research"><span>研究碎片 {{ meta.shards }} · 天赋永久解锁</span><button v-for="id in ['power','guard','wealth'] as const" :key="id" class="button" :disabled="paused||meta.talents[id]>=5||meta.shards<3+meta.talents[id]*3" @click="talent(id)">{{ {power:'火力',guard:'守护',wealth:'补给'}[id] }} {{ meta.talents[id] }}/5 · {{ 3+meta.talents[id]*3 }}碎片</button></div>
    <p class="game-help defense-help" role="status">{{ state.last }} <span>波次自动继续 · 强化可累计 · 选卡暂停</span></p>
  </div>
</template>
<style scoped>
.defense-topline{display:flex;align-items:center;justify-content:space-between;gap:9px;flex-wrap:wrap;margin-bottom:7px;font-size:11px;color:var(--muted);font-variant-numeric:tabular-nums}.defense-topline b{color:var(--text)}.defense-topline small{font-size:10px}.defense-coins{color:var(--warning);font-weight:600}.defense-topline .defense-endless-link{margin-left:7px;padding:3px 7px;font-size:10px}.defense-strengthen{padding:5px 9px;font-size:11px;white-space:nowrap}
.defense-stage{--defense-ground:#91af98;--defense-edge:#769d87;--defense-path:#c1c5a0;--defense-tree:#3d7467;--defense-grass:#6c9b77;--defense-stone:#b4bda1;--defense-range:#d9e4c4;position:relative;border:1px solid var(--line);border-radius:12px;overflow:hidden;outline-offset:3px}.defense-surface{display:block;width:100%;aspect-ratio:16/9}.defense-region-dusk{--defense-ground:#a8a29a;--defense-edge:#8b8c8c;--defense-path:#c8baa0;--defense-tree:#666780;--defense-grass:#87857d}.defense-region-frost{--defense-ground:#a0bac0;--defense-edge:#8ba5ae;--defense-path:#c9d5ce;--defense-tree:#4e7b85;--defense-grass:#86a5ac}.defense-region-ember{--defense-ground:#b39c88;--defense-edge:#99856f;--defense-path:#d4b99a;--defense-tree:#796963;--defense-grass:#a48c71}
.defense-volley-status{position:absolute;left:12px;top:66px;width:194px;max-width:calc(100% - 24px);padding:7px 10px;border:1px solid #b79c56;border-radius:8px;background:#273b43ed;color:#f4df9e;pointer-events:none;font-size:11px;line-height:1.6;overflow-wrap:anywhere}.defense-volley-status strong{font-weight:600}.defense-volley-status i{display:block;height:4px;margin-top:3px;background:#6c664433;border-radius:3px;overflow:hidden}.defense-volley-status em{display:block;height:100%;background:#efd589;transition:width .08s}.defense-volley-status.volley-active{border-color:#fff2af;background:#594928ed;color:#fff5c4;box-shadow:0 0 0 2px #efce7920}.defense-hud{position:absolute;inset:10px 12px auto;display:flex;align-items:flex-start;gap:9px;pointer-events:none;font-size:10px;color:#f2f5ec}.defense-hud>div{background:#28464add;padding:6px 9px;border-radius:7px;min-width:120px}.defense-hud span{font-variant-numeric:tabular-nums}.defense-hud i{display:block;height:4px;background:#193d42;border-radius:5px;margin-top:4px;overflow:hidden}.defense-hud em{display:block;height:100%;background:#86d4a5;transition:width .15s}.defense-shield em{background:#94d7e4}.defense-boss-tag{margin-left:auto;background:#7c4e59e8;border:1px solid #e7a3a2;padding:6px 9px;border-radius:7px}.defense-wave-detail{position:absolute;left:12px;bottom:9px;color:#e1ebd9;font-size:10px;border-radius:5px;background:#304f4ac9;padding:4px 7px;pointer-events:none}
.defense-overlay{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:safe center;gap:10px;padding:16px;overflow-y:auto;text-align:center;background:#1b3139e3;color:#f2f3e9}.defense-overlay h3{font-size:20px;margin:0}.defense-overlay p{font-size:11px;color:#d7e2db;margin:0;line-height:1.7}.defense-stage .game-actions{margin:0}.defense-overlay .button{font-size:11px}.defense-core-choices{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px;width:100%;max-width:850px}.defense-core-card{display:flex;flex-direction:column;align-items:flex-start;justify-content:flex-start;gap:7px;text-align:left;min-width:0;height:auto;white-space:normal;overflow-wrap:anywhere;line-height:1.5;padding:10px;background:#28424e;color:#e3ebe7;border:1px solid #657c81}.defense-core-card .defense-core-preview{width:100%;height:78px;flex:0 0 78px;align-self:center}.defense-core-card.primary{border-color:#edc98e;background:#344e5b}.defense-core-card strong{font-size:12px}.defense-core-card small{font-size:10px;line-height:1.6;color:#d9e3dd}.defense-loss-review{max-width:560px}
.defense-battle-caption{display:flex;justify-content:space-between;gap:8px;font-size:10px;color:var(--muted);margin:7px 0}.defense-upgrades{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:7px;margin-top:8px}.defense-upgrades .button{display:flex;align-items:center;justify-content:center;gap:8px;padding:8px 5px;min-width:0;height:auto;min-height:45px;line-height:1.35;font-size:12px}.defense-upgrades svg{width:25px;height:25px;flex:none;color:var(--primary)}.defense-upgrades span{text-align:left;white-space:normal;overflow-wrap:anywhere}.defense-upgrades small{font-size:10px;color:var(--muted)}.defense-upgrades b{display:block;color:var(--warning);font-size:10px;font-weight:500;margin-top:2px}
.defense-automation{display:flex;align-items:center;flex-wrap:wrap;gap:9px;margin-top:8px;padding:8px;border:1px solid var(--line);border-radius:8px;color:var(--muted);font-size:10px}.defense-automation label{display:flex;gap:5px;align-items:center}.defense-automation .input{min-height:27px;font-size:10px;background:var(--surface);color:var(--text);padding:4px 6px;border:1px solid var(--line);border-radius:5px}.defense-reserve{width:83px;min-width:0}.defense-legacy-core{display:flex;flex-wrap:wrap;gap:5px;align-items:center;font-size:10px;color:var(--muted);margin-top:8px}.defense-legacy-core .button{padding:5px 7px;font-size:10px}
.defense-research{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin-top:8px;font-size:10px;color:var(--muted)}.defense-research .button{font-size:10px;padding:5px 7px}.defense-game .defense-help{display:flex;justify-content:space-between;gap:10px;margin:7px 0 0;font-size:10px;text-align:left}
:global(:root[data-theme="warm"] .defense-stage){--defense-ground:#b6b495;--defense-edge:#9eaa83;--defense-path:#d6caab;--defense-tree:#738666;--defense-stone:#d0c5a5;--defense-range:#f1e7cb}:global(:root[data-theme="warm"] .defense-region-dusk){--defense-ground:#c2b29b;--defense-edge:#a89a8c;--defense-tree:#8a7b7d}:global(:root[data-theme="warm"] .defense-region-frost){--defense-ground:#c1d0c6;--defense-edge:#a3b6ac;--defense-tree:#7b9b91}:global(:root[data-theme="warm"] .defense-region-ember){--defense-ground:#cbb18d;--defense-edge:#b39879;--defense-tree:#928167}
@media(max-width:850px){.defense-upgrades{grid-template-columns:repeat(3,minmax(0,1fr))}.defense-upgrades .button{padding:6px}.defense-topline small{display:none}.defense-hud>div{min-width:90px}.defense-core-choices{grid-template-columns:repeat(2,minmax(0,1fr))}.defense-overlay h3{font-size:17px}.defense-overlay{padding:12px;gap:8px}.defense-core-card{padding:8px}.defense-game .defense-help{display:block}}
.defense-volley-status small{display:block;margin-top:3px;font-size:10px;line-height:1.3;font-weight:400}
@media(prefers-reduced-motion:reduce){.defense-hud em{transition:none}}
</style>
