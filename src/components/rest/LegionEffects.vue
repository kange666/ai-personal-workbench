<script setup lang="ts">
import type { LegionEffect, LegionState } from '../../utils/rest/legion';
defineProps<{ effects: LegionEffect[]; cards: LegionState['cards'] }>();
const progress = (e:LegionEffect) => Math.max(0,Math.min(1,1-e.life/(e.kind==='hit'?.7:.32)));
function point(e:LegionEffect, t:number) {
  const bend=Math.min(e.y,e.ty)-Math.min(38,Math.abs(e.tx-e.x)*.15);
  return {x:(1-t)*(1-t)*e.x+2*(1-t)*t*(e.x+e.tx)/2+t*t*e.tx,y:(1-t)*(1-t)*e.y+2*(1-t)*t*bend+t*t*e.ty};
}
function projectile(e:LegionEffect) {
  const t=progress(e),p=point(e,t),bend=Math.min(e.y,e.ty)-Math.min(38,Math.abs(e.tx-e.x)*.15);
  return `translate(${p.x} ${p.y}) rotate(${Math.atan2(2*(1-t)*(bend-e.y)+2*t*(e.ty-bend),e.tx-e.x)*180/Math.PI})`;
}
function trail(e:LegionEffect) { const p=point(e,progress(e)), last=point(e,Math.max(0,progress(e)-.2));return `M${last.x} ${last.y}Q${(last.x+p.x)/2} ${last.y-2} ${p.x} ${p.y}`; }
function bolt(e:LegionEffect) { const dx=e.tx-e.x,dy=e.ty-e.y;return `M${e.x} ${e.y}l${dx*.28} ${dy*.28-9} ${dx*.12} ${dy*.12+17} ${dx*.22} ${dy*.22-18} ${dx*.12} ${dy*.12+15} ${dx*.26} ${dy*.26-5}`; }
const enemy = (e:LegionEffect) => ['skeleton','shaman','necromancer','behemoth','warlord','goblin','swarm','runner','armored'].includes(e.source??'');
</script>
<template>
  <g aria-hidden="true" pointer-events="none">
    <g v-for="e in effects" :key="e.id" data-legion-effect :data-effect-kind="e.kind" :opacity="Math.min(1,e.life*7)" fill="none" stroke-linecap="round" stroke-linejoin="round">
      <template v-if="e.kind==='arrow'">
        <path :d="trail(e)" :stroke="enemy(e)?'#f1a992':'#efd79a'" stroke-width="3" opacity=".3"/>
        <g :transform="projectile(e)" data-legion-projectile :stroke="enemy(e)?'#f5b89f':'#ffedb7'" stroke-width="1.7"><path d="M-17 0H3m-16 0-4-3m4 3-4 3"/><path d="m2-3 7 3-7 3Z" fill="currentColor" stroke-width="1" :style="{color:enemy(e)?'#f2aa93':'#ffe4a2'}"/><path v-if="e.source==='archer'&&cards.double" d="M-22 6H1m-3-2 4 2-4 2" opacity=".75"/></g>
        <circle v-if="e.source==='archer'&&cards.explosive&&progress(e)>.65" :cx="e.tx" :cy="e.ty" :r="8+progress(e)*17" stroke="#ffc27f" stroke-width="2" opacity=".6"/>
      </template>
      <template v-else-if="e.kind==='lightning'">
        <path :d="bolt(e)" stroke="#8e9bff" stroke-width="8" opacity=".2"/><path :d="bolt(e)" stroke="#a7ddff" stroke-width="3.2"/><path :d="bolt(e)" stroke="#f0faff" stroke-width="1.1"/>
        <path :d="`M${e.tx-8} ${e.ty-10}l8 6 8-6-5 10 7 6-10-2-5 10-2-11-10-2Z`" fill="#c5edff" opacity=".8"/>
      </template>
      <template v-else-if="e.kind==='fire'">
        <template v-if="e.source==='behemoth'"><ellipse :cx="e.tx" :cy="e.ty+22" :rx="12+progress(e)*38" :ry="5+progress(e)*12" stroke="#f9c18b" stroke-width="2.5"/><ellipse :cx="e.tx" :cy="e.ty+22" :rx="5+progress(e)*25" :ry="2+progress(e)*8" stroke="#d58a71" stroke-width="2"/></template>
        <template v-else><path :d="trail(e)" :stroke="cards.frost?'#91d8f6':'#ffc078'" stroke-width="7" opacity=".35"/><g :transform="projectile(e)" data-legion-projectile><path d="m-22-5 17-5 12 10-12 10-17-5 11-5Z" :fill="cards.frost?'#8fcfe7':'#ec9365'" opacity=".85"/><circle r="6" :fill="cards.frost?'#e1f9ff':'#ffe8ae'"/><path d="M-4-3 3 0-4 3" stroke="#fff7df" stroke-width="2"/></g><circle v-if="progress(e)>.65" :cx="e.tx" :cy="e.ty" :r="8+progress(e)*16" :stroke="cards.frost?'#c5f3ff':'#ffd597'" stroke-width="2" opacity=".7"/></template>
      </template>
      <template v-else-if="e.kind==='slash'">
        <template v-if="e.source==='giant'"><ellipse :cx="e.tx" :cy="e.ty+21" :rx="10+progress(e)*(cards.stomp?43:25)" :ry="4+progress(e)*10" stroke="#e6c993" stroke-width="2.4"/><path :d="`M${e.tx-14} ${e.ty+14}l-5-8m32 9 6-9m-17 6v-10`" stroke="#ecd7ad" stroke-width="2"/></template>
        <g v-else :transform="`translate(${e.tx} ${e.ty}) rotate(${enemy(e)?-20:15}) scale(${.7+progress(e)*.5})`"><path d="M-20-24Q27-23 29 25Q15-2-20-24Z" :fill="enemy(e)?'#f2aa91':'#f3d9a1'" opacity=".85"/><path d="M-17-22Q24-19 29 22" stroke="#fff9e1" stroke-width="2"/><path d="M-13 9-21 14m31-30 2-9m4 39 6 6" stroke="#f7d59b" stroke-width="1.6"/></g>
      </template>
      <template v-else-if="e.kind==='heal'"><path :d="`M${e.x} ${e.y}Q${(e.x+e.tx)/2} ${Math.min(e.y,e.ty)-23} ${e.tx} ${e.ty}`" stroke="#b0efb9" stroke-width="1.4" opacity=".5"/><ellipse :cx="e.tx" :cy="e.ty+20" :rx="11+progress(e)*10" ry="5" stroke="#b6f0be" stroke-width="1.8"/><path :d="`M${e.tx-5} ${e.ty-8}h10m-5-5v10`" stroke="#d4f9bc" stroke-width="3"/><text :x="e.tx+9" :y="e.ty-14-progress(e)*15" fill="#c8f5b8" stroke="#284c40" stroke-width=".5" font-size="12" font-weight="700">{{e.text}}</text></template>
      <text v-else :x="e.tx" :y="e.ty-14-progress(e)*22" text-anchor="middle" fill="#fff1d0" stroke="#38454c" stroke-width=".6" font-size="12" font-weight="700">{{e.text}}</text>
    </g>
  </g>
</template>
