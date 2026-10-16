<script setup lang="ts">
import { computed, useId } from 'vue';
import { legionRegion, legionStats, type LegionActor, type LegionState } from '../../utils/rest/legion';
import { LEGION_ENEMIES, LEGION_UNITS, type LegionEnemyKind, type LegionUnitKind } from '../../utils/rest/legionContent';
import LegionFigure from './LegionFigure.vue';
import LegionEffects from './LegionEffects.vue';
const props = defineProps<{ state: LegionState }>();
const sceneId = useId().replaceAll(':','');
const region = computed(() => legionRegion(props.state));
const regionIndex = computed(() => Math.floor((props.state.wave-1)/10)%4);
const cold = computed(() => regionIndex.value===2), lava = computed(() => regionIndex.value===3);
const actors = computed(() => [...props.state.allies, ...props.state.enemies].sort((a, b) => a.y - b.y));
const boss = computed(() => props.state.enemies.find(a => ['warlord','behemoth','necromancer'].includes(a.kind)));
function motion(a:LegionActor) {
  if(!a.attack)return 0;
  const rate=a.side==='ally'?legionStats(props.state,a.kind as LegionUnitKind).rate:LEGION_ENEMIES[a.kind as LegionEnemyKind].rate;
  return Math.min(1,Math.max(0,(a.cooldown-(1/rate-.23))/.23));
}
const livingSlots = computed(() => LEGION_UNITS.reduce((sum,u)=>sum+props.state.formation[u.id],0));
</script>
<template>
  <div class="legion-battlefield" :style="{ '--legion-sky': region.sky, '--legion-floor': region.floor }">
    <svg viewBox="0 0 960 440" preserveAspectRatio="xMidYMid slice" role="img" aria-label="军团自动远征战场，蓝色军团向右推进，红色敌军迎战" class="legion-scene">
      <defs>
        <linearGradient :id="`${sceneId}-sky`" x2="0" y2="1"><stop stop-color="var(--legion-sky)"/><stop offset="1" :stop-color="cold?'#c6d8df':lava?'#cc997b':'#c5c6aa'"/></linearGradient>
        <linearGradient :id="`${sceneId}-ground`" x2="0" y2="1"><stop stop-color="var(--legion-floor)"/><stop offset="1" :stop-color="cold?'#455d69':lava?'#3c3e4b':'#344c43'"/></linearGradient>
        <g :id="`${sceneId}-tree`"><path d="M-3-45H4V8H-3Z" fill="#3b4842"/><path d="M0-84-22-46h10L-31-15h24l-11 17h36L7-15h25L13-46h10Z" fill="currentColor"/><path d="m-2-79-17 32h9L-26-20h21" fill="#c4d5b9" opacity=".13"/></g>
      </defs>
      <path :fill="`url(#${sceneId}-sky)`" d="M0 0h960v440H0Z"/>
      <circle cx="748" cy="67" r="28" :fill="lava?'#e7ac86':regionIndex===1?'#e8d7b8':'#dce3c8'" opacity=".7"/><circle cx="748" cy="67" r="39" fill="#f0dfc1" opacity=".09"/>
      <g fill="#e6e6d4" opacity=".13"><path d="M36 67q64-23 125 1l96-5q-49 22-126 14Z"/><path d="M385 37q64-17 125 2l99-2q-68 15-149 13Z"/></g>
      <path d="M0 159 80 78l90 52 100-88 119 110 125-67 80 51 104-102 89 83 173-70v204H0Z" fill="#3d5e6f" opacity=".43"/>
      <path d="m270 42-32 58 34-13 34 21-15-43m407-31-36 51 35-13 33 27" :fill="cold?'#dbe9e8':'#b6c8b4'" :opacity="cold ? .65 : .13"/>
      <path d="m0 190 95-48 127 48 113-46 119 45 124-60 129 52 106-54 147 47v96H0Z" :fill="lava?'#584d59':'#345b60'" opacity=".65"/>
      <g v-if="regionIndex!==0" :fill="cold?'#708e9d':'#536273'" opacity=".65"><path d="M546 169v-44h12v-13h16v13h12v-24h18v25h12v42Zm81 3v-26h15v-28h13v-12h16v65Zm-137 2v-19h17v-31h17v52Z"/><path d="M552 137h6v7h-6m39-7h6v7h-6m61-9h6v8h-6" fill="#d5c9ac" opacity=".45"/></g>
      <path d="M0 210q210-34 450-10t510-13v253H0Z" :fill="`url(#${sceneId}-ground)`"/>
      <path d="M0 263q310-48 960-17v114Q470 310 0 380Z" :fill="cold?'#cfdfdb':lava?'#b18d7d':'#c8b88c'" opacity=".19"/>
      <path d="M0 289q350-42 960-12M0 342q380-38 960-19" fill="none" stroke="#e1d7b5" stroke-width="1" opacity=".12"/>
      <g v-if="!lava" :color="cold?'#4f7070':'#315748'"><use v-for="(x,i) in [17,74,135,200,740,811,895,950]" :key="x" :href="`#${sceneId}-tree`" :transform="`translate(${x} ${190+i%3*10}) scale(${.6+i%2*.25})`"/></g>
      <g v-if="cold" fill="#c3dbdc" opacity=".55"><path v-for="(x,i) in [17,135,200,811,950]" :key="x" :d="`M${x} ${141+i%3*10}l-10 17 10-5 10 6Z`"/></g>
      <g v-if="lava" fill="none" stroke="#e1a073" stroke-width="2" opacity=".65"><path d="m24 205 62 6 24-10 37 8m608-4 30 8 23-11 51 8 66-8M60 374l62-8 16 11 61-3m528-10 29 12 23-14 48 13 90-10"/></g>
      <g :fill="cold?'#c0d8d5':'#b6bf93'" opacity=".24"><path v-for="i in 24" :key="i" :d="`M${i*47%960} ${239+i*37%177}l4-6 1 6 5-3`"/></g>
      <g v-for="(x,i) in [43,224,733,909]" :key="x" :transform="`translate(${x} ${375+i%2*21})`"><path d="m-18 0 7-9 19-2L19 0Z" :fill="cold?'#77959d':'#65736b'"/><path d="m-11-9 19-2 3 5-15 1Z" :fill="cold?'#bdd8db':'#92a089'"/></g>
      <g transform="translate(48 259)" stroke="#d0c5a0" stroke-width="2"><path d="M0 0v-76"/><path d="M1-73h28l-4 14 4 14H1Z" fill="#3b7b87"/><path d="m10-66 4-4 4 4-4 7Z" fill="#d9c382" stroke="none"/></g>
      <g v-for="a in actors" :key="a.id" :data-legion-unit="a.kind" :data-side="a.side" :transform="`translate(${a.x.toFixed(1)} ${a.y.toFixed(1)})`">
        <ellipse v-if="a.kind==='guard'&&state.cards.bulwark" cy="1" rx="25" ry="8" stroke="#92d9df" stroke-width="1.2" fill="#9fe3e316"/>
        <LegionFigure :kind="a.kind" :enemy="a.side==='enemy'" :motion="motion(a)" :walking="state.phase==='battle'&&a.cooldown<=.01&&state.enemies.length>0" :clock="state.elapsed+a.id*.3" :tier="a.side==='ally' ? Math.min(5,1+Math.floor(state.training[a.kind as LegionUnitKind]/5)) : boss?.id===a.id?4:1"/>
        <rect x="-17" y="-74" width="34" height="4" rx="2" fill="#20343ddd"/><rect x="-16" y="-73" :width="32*Math.max(0,a.hp/a.maxHP)" height="2" rx="1" :fill="a.side==='ally'?'#a2e6c3':'#f0a18a'"/>
        <g v-if="a.burnTime>0" fill="#efab72" opacity=".85"><path d="m-8-8-3-8 5 2 3-8 3 9 6-4-2 9Z"/><circle cx="-8" cy="-30" r="1.5"/><circle cx="7" cy="-20" r="1"/></g>
        <g v-if="a.slow>0" stroke="#c4f0ff" stroke-width="1.3" opacity=".8"><path d="M-8-7h16M0-15V1m-6-14L6-1m-12 0L6-13"/><ellipse cy="2" rx="17" ry="5" fill="#b1dbe826"/></g>
      </g>
      <LegionEffects :effects="state.effects" :cards="state.cards"/>
      <g v-if="state.phase!=='battle'"><rect x="286" y="148" width="388" height="82" rx="14" fill="#192e3edc"/><text x="480" y="180" text-anchor="middle" fill="#f1e7cc" font-size="19">{{state.phase==='march'?'战线已突破 · 自动前进':'远征结算 · 全军整备'}}</text><text x="480" y="207" text-anchor="middle" fill="#bfced0" font-size="13">{{state.phase==='recover'&&!state.options.retry?'自动续战已关闭，可在右侧开启':`${Math.max(0,Math.ceil(state.phaseTime))} 秒后继续 · 永久训练保留`}}</text></g>
    </svg>
    <div class="legion-field-status"><span>{{state.paused?'已暂停':state.phase==='battle'?'自动推进中':state.phase==='march'?'前往下一战线':'正在整备'}}</span><small>军团 {{ state.allies.length }}/{{livingSlots}} 人 · 敌军 {{state.enemies.length}} 人</small></div>
    <div v-if="boss" class="legion-boss"><strong>{{ LEGION_ENEMIES[boss.kind as LegionEnemyKind].name }}</strong><i><b :style="{width:`${boss.hp/boss.maxHP*100}%`}"/></i></div>
    <div class="legion-field-toast">{{state.log[0]}}</div>
  </div>
</template>
<style scoped>
.legion-battlefield{position:relative;overflow:hidden;min-width:0;border:1px solid var(--line);border-radius:12px;background:var(--legion-floor)}.legion-scene{display:block;width:100%;height:clamp(270px,calc(100dvh - 335px),470px)}.legion-field-status{position:absolute;inset:10px 12px auto;display:flex;justify-content:space-between;gap:8px;flex-wrap:wrap;color:#d7f1de;font-size:11px;pointer-events:none}.legion-field-status span,.legion-field-status small{padding:5px 8px;border-radius:7px;background:#18362bd9;font-size:11px}.legion-field-toast{position:absolute;inset:auto 10px 10px;padding:6px 9px;background:#172e3bd9;border:1px solid #9cbc9b55;border-radius:8px;color:#e1e9c9;font-size:11px;overflow-wrap:anywhere;pointer-events:none}.legion-boss{position:absolute;top:48px;left:26%;width:48%;font-size:12px;color:#ffebd0;text-align:center;pointer-events:none}.legion-boss strong{font-weight:600}.legion-boss i{display:block;overflow:hidden;height:6px;margin-top:5px;background:#263b46;border:1px solid #182b32;border-radius:4px}.legion-boss b{display:block;height:100%;background:#df7e6d}
</style>
