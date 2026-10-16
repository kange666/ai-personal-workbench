<script setup lang="ts">
import { computed } from 'vue';
import { DEFENSE_CORES, defenseBarrelLength, type DefenseCoreId } from '../../utils/rest/defenseContent';
const props=defineProps<{core:DefenseCoreId;tier:number;recoil:number;hurt:number;direction:number;shield:number;maxShield:number;evolved:boolean;paused:boolean;skills:{fire:number;ice:number;chain:number;blast:number};evolutions?:Partial<Record<string,number>>}>();
const color=computed(()=>DEFENSE_CORES.find(c=>c.id===props.core)?.color||'#d5bf9a');
const barrel=computed(()=>defenseBarrelLength(props.core,props.tier));
const muzzleColor=computed(()=>props.skills.fire?'#ffbc6c':props.skills.ice?'#b7f3ff':color.value);
</script>
<template>
  <g data-central-tower :data-core="core" :data-tower-tier="tier" transform="translate(300 200)" :style="{'--tower-tone':color}" :class="{'tower-still':paused}">
    <ellipse cy="23" :rx="46+tier*2" ry="14" fill="#1d3c42" opacity=".28"/>
    <g v-if="tier>=4||evolved" class="tower-ground-energy"><ellipse cy="17" :rx="43+tier*2" :ry="22+tier" fill="none" :stroke="color" stroke-width="1.4" stroke-dasharray="18 6 3 6"/><path v-for="angle in [0,90,180,270]" :key="angle" d="M47 13h9m-3-3 3 3-3 3" :transform="`rotate(${angle} 0 13)`" :stroke="color" fill="none" stroke-width="1.3"/></g>
    <g v-if="tier>=3" data-tower-structure="stabilizers" fill="#607984" stroke="#344b5b" stroke-width="1.6"><path d="M-26 0-42 13-43 24-30 25-17 9Z"/><path d="M26 0 42 13 43 24 30 25 17 9Z"/><path d="M-37 15-30 18m67-3-7 3" :stroke="color" stroke-width="3"/></g>
    <path d="M-35-5-39 9-24 24H24L39 9 35-5 20-17H-20Z" fill="#526a78" stroke="#304958" stroke-width="2"/><path d="M-35-5-19-16H19L35-5 20 10H-20Z" fill="#9daea4" stroke="#d3ddc4" stroke-width="1.5"/><path d="M-36 10-21 21H21L36 10" fill="none" stroke="#839b9c" stroke-width="3"/>
    <path v-if="tier>=2" data-tower-structure="armor" d="M-36-3-26-9-19 2-23 14-35 8Zm72 0L26-9 19 2l4 12 12-6Z" :fill="color" stroke="#3d5664" stroke-width="1.5"/>
    <g :transform="`rotate(${hurt*3} 0 10)`">
      <path d="M-25-25V3Q0 19 25 3V-25Z" fill="#c1cbb4" stroke="#405b6a" stroke-width="2"/><path d="M8-26V11L24 3V-24Z" fill="#7e969a"/><path d="M-24-10 0-2 24-10M-13-20v8M-7 1v10M15-15V4" fill="none" stroke="#718787" stroke-width="1.5"/>
      <path d="M-18-12-8-8V3L-18-1Zm18 7 7 1V8L0 10Z" fill="#365464"/><path d="M-16-10-10-8M2-1H5" :stroke="color" stroke-width="2"/>
      <ellipse v-if="core!=='bastion'" cy="-26" rx="28" ry="13" fill="#69838b" stroke="#344e60" stroke-width="2"/><ellipse v-if="core!=='bastion'" cy="-28" rx="24" ry="10" fill="#a6b9aa" stroke="#d1ddc7" stroke-width="1.4"/>
      <g v-if="core==='bastion'" data-tower-silhouette="fortress"><path d="M-34-33V-9L-22 2H22L34-9V-33L20-43H-20Z" fill="#849d91" stroke="#3c5965" stroke-width="2"/><path d="M-34-33-21-38v9h10v-12H0v12h11v-12h10v12h13v-4L20-43H-20Z" :fill="color" stroke="#3f5e63" stroke-width="1.4"/><path d="M-29-25v12l9 7V-20m49-5v12l-9 7V-20" fill="#c1d4aa" stroke="#526f6c" stroke-width="1.5"/><path d="M-8-25h16V-7L0-2-8-7Z" fill="#3e6369" stroke="#c0e4ad" stroke-width="1.4"/><path d="m0-22 4 7-4 6-4-6Z" fill="#cbe8a9"/></g>
      <g v-if="tier>=4" data-tower-structure="reactor" :stroke="color" fill="none"><path d="M-22 4Q0 17 22 4" stroke-width="3"/><path d="M-18 9v5m9-2v5m9-2v4m9-7v5m9-8v5" stroke-width="1.3"/></g>
      <g v-if="tier>=5" data-tower-structure="crown" :fill="color" stroke="#49626b" stroke-width="1.2"><path d="M-35-10-43-19-39-34-31-29-28-15Zm70 0 8-9-4-15-8 5-3 14Z"/><path d="M-39-30-36-20m75-10-3 10" stroke="#ecf3d7"/></g>
    </g>
    <g v-if="core==='tesla'" data-tower-barrel :transform="`translate(0 -30) rotate(${direction})`">
      <g data-tower-silhouette="reactor"><path d="M-24-14-29-5-20-2m-4 16-5-9 9-3M24-14l5 9-9 3m4 16 5-9-9-3" fill="none" stroke="#517888" stroke-width="5"/><path d="M-26-11-22-6m-4 17 4-5m48-17-4 5m4 17-4-5" stroke="#b4eef4" stroke-width="2.5"/><ellipse :rx="18+tier" :ry="12+tier*.5" fill="#49678a" stroke="#a9e6e8" stroke-width="2"/><path d="M-12 0-3-14 7-8 13 0 3 13-7 8Z" fill="#94dcdf" stroke="#e4ffff" stroke-width="1.7"/><path d="M-3-11 0 0 9-1M0 0 3 10" fill="none" stroke="#e9ffff" stroke-width="2"/><path v-if="recoil>.15" d="M-18-8-7-4-13 3 0 0 9 6 6-4 19 4 26-1" fill="none" stroke="#f1ffff" stroke-width="2" :opacity="recoil"/><circle r="10" fill="#d2ffff" :opacity="recoil*.45"/></g>
      <g v-if="tier>=3" data-tower-weapon="capacitors"><path d="M-15-21H15m-30 42H15" stroke="#779eac" stroke-width="5"/><path d="M-12-23V-19m8-4v4m8-4v4m8-4v4M-12 19v4m8-4v4m8-4v4m8-4v4" stroke="#cff9f4" stroke-width="2"/></g>
    </g>
    <g v-else data-tower-barrel :transform="`translate(0 -30) rotate(${direction})`">
      <g :transform="`translate(${-recoil*(core==='cannon'?9:core==='rapid'?4:5)} 0)`">
        <g v-if="core==='rapid'" data-tower-silhouette="gatling">
          <path d="M-19-16H7L17-9V9L7 16H-19l-5-8V-8Z" fill="#69838c" stroke="#344e60" stroke-width="2"/><path d="M-18-12H4v24H-18Z" :fill="color" stroke="#536d75" stroke-width="1.4"/><path d="M-22-5h11m-11 5h11m-11 5h11" stroke="#455e6a" stroke-width="2"/>
          <path v-if="tier>=3" d="M-6-22H12V-13H-6Zm0 35H12V22H-6Z" fill="#a1afa2" stroke="#3d596b" stroke-width="1.5"/>
          <g v-for="y in (tier>=3?[-11,-3,5]:[-7,5])" :key="y"><rect x="3" :y="y-2" :width="barrel-3" height="5" rx="1.5" fill="#405a6a" stroke="#a6bbb3" stroke-width="1.1"/><path :d="`M16 ${y-1}H${barrel-4}`" stroke="#d5dfc1" stroke-width="1"/></g>
          <path :d="`M${barrel-9}-17h7v32h-7Z`" fill="#758b8e" stroke="#334f60" stroke-width="1.4"/><path :d="`M${barrel-6}-12v22`" :stroke="color" stroke-width="2.4"/>
          <path v-if="tier>=5" d="M-24-8-31-15-31 15-24 8Z" fill="#d6b57a" stroke="#566971" stroke-width="1.5"/>
        </g>
        <g v-else-if="core==='cannon'" data-tower-silhouette="howitzer">
          <path d="M-22-17 5-21 22-12V12L5 21-22 17-28 8V-8Z" fill="#7d8b8b" stroke="#354d5f" stroke-width="2"/><path d="M-23-13 2-16 13-9V9L2 16-23 13Z" :fill="color" stroke="#5a6c70" stroke-width="1.5"/><path d="M-17-10v20m7-22v24" stroke="#ead5a4" stroke-width="2"/>
          <path :d="`M7-9H${barrel-12}l8 4V5l-8 4H7Z`" fill="#536c7b" stroke="#314b5e" stroke-width="1.8"/><path :d="`M22-9V9m12-18V9m12-18V9`" stroke="#b6bda6" stroke-width="3"/><path :d="`M${barrel-8}-12h8v24h-8Z`" fill="#70858c" stroke="#365367" stroke-width="1.8"/><path :d="`M${barrel}-8V8`" :stroke="muzzleColor" stroke-width="2"/>
          <path v-if="tier>=3" :d="`M0-16H${barrel-15}m${15-barrel} 32H${barrel-15}`" fill="none" stroke="#466372" stroke-width="4"/><path v-if="tier>=4" d="M-28-8-33-16h15m-10 24-5 8h15" fill="none" stroke="#e5c694" stroke-width="3"/>
        </g>
        <g v-else data-tower-silhouette="guard-cannon"><path d="M-14-12H13L23-7V7L13 12H-14Z" :fill="color" stroke="#375463" stroke-width="2"/><path d="M4-8H35V8H4Z" fill="#58727b" stroke="#355260" stroke-width="1.5"/><path d="M16-8V8m12-16V8" stroke="#b9d1b1" stroke-width="3"/><path d="M34-11h5v22h-5Z" fill="#82998f" stroke="#3a5864" stroke-width="1.5"/></g>
      </g>
      <g v-if="recoil>.15" data-tower-muzzle :transform="`translate(${barrel-recoil*3} 0)`" :opacity="recoil">
        <ellipse rx="8" :ry="core==='cannon'?12:7" :fill="muzzleColor" opacity=".4"/><path d="M-2-3 9-6 5-12 16-5 26-2 17 1 23 7 12 5 5 11 7 3-2 3Z" :fill="muzzleColor"/><path d="M0-1 12-2 19 0 8 2H0Z" fill="#fff7da"/>
      </g>
    </g>
    <g v-if="skills.fire" data-tower-module="fire"><path d="M-37-4-43-12-40-24H-27l5 12-4 8Z" fill="#745d59" stroke="#394e59" stroke-width="1.5"/><path d="M-36-7-39-15-34-24-32-16-28-21-26-12-30-6Z" fill="#ef9664"/><path d="M-34-8-35-14-31-19-30-10Z" fill="#ffe7a1"/><path v-if="skills.fire>=3" d="M-41-17-44-26-39-33-37-26-34-29-35-22Z" fill="#ffb46a"/><path d="M-42-3h16" stroke="#b5aa86" stroke-width="2"/></g>
    <g v-if="skills.ice" data-tower-module="ice"><path d="M24-5 23-19 30-28 35-19 39-8 33-3Z" fill="#6f9eaa" stroke="#3d657c" stroke-width="1.5"/><path d="M26-12 29-39 36-30 34-7Z" fill="#b3e8eb" stroke="#e1ffff" stroke-width="1.2"/><path d="M29-35 30-13 33-24" fill="none" stroke="#78b9d2" stroke-width="1.5"/><path v-if="skills.ice>=3" d="M36-10 38-30 43-24 41-8Z" fill="#c9eff5" stroke="#5a9cb8" stroke-width="1.1"/></g>
    <g v-if="skills.chain" data-tower-module="chain"><path d="M-23-29-27-44m50 15 4-15" stroke="#456277" stroke-width="4"/><path d="M-30-35h9m-10-5h9m39 5h9m-8-5h9" stroke="#accde3" stroke-width="2.4"/><circle cx="-27" cy="-45" r="4" fill="#c6b6ff" stroke="#f1eaff" stroke-width="1.2"/><circle cx="27" cy="-45" r="4" fill="#c6b6ff" stroke="#f1eaff" stroke-width="1.2"/><path v-if="skills.chain>=3" class="tower-electric-bridge" d="M-26-46-16-51-9-45-1-50 9-46 17-51 26-46" fill="none" stroke="#d9cdff" stroke-width="1.3"/></g>
    <g v-if="skills.blast" data-tower-module="blast"><g v-for="x in [-23,23]" :key="x" :transform="`translate(${x} 15) rotate(${x<0?-25:25})`"><path d="M-7-4-8 7 0 12 8 7 7-4Z" fill="#887e70" stroke="#405966" stroke-width="1.4"/><path d="M-5-4-3-12H3L5-4Z" fill="#e3b27c" stroke="#695d59" stroke-width="1.1"/><path d="M-3-3H3V5H-3Z" fill="#f0c880"/><path v-if="skills.blast>=3" d="M-7 7H7" stroke="#ffe4a6" stroke-width="2"/></g></g>
    <g v-if="evolutions?.firestorm" data-tower-evolution="firestorm" class="tower-evolution-flame"><path d="M-43 10-47 0-42-12-40-3-36-8-36 6m79 4 4-10-5-12-2 9-4-5v14" fill="#f3a76f" stroke="#ffe4a5" stroke-width="1"/></g>
    <g v-if="evolutions?.froststorm" data-tower-evolution="froststorm"><path d="M-16-52H16M0-63V-41m-9-19 18 16m-18 0 18-16" stroke="#c3f1f7" stroke-width="1.8"/><circle cy="-52" r="12" fill="none" stroke="#99dce6" stroke-width="1"/></g>
    <g v-if="evolutions?.volley" data-tower-evolution="volley" :stroke="color" fill="#64808a" stroke-width="1.6"><path d="M-36-9-47-25-40-33-30-16m66 7 11-16-7-8-10 17"/><path d="M-47-25-51-30m98 5 4-5" stroke-width="3"/></g>
    <path v-if="shield>0" data-tower-shield d="M-43-15-29-39H29L43-15 38 12 0 29-38 12Z" fill="#c4f6ef" :fill-opacity="evolutions?.bulwark ? .05 : 0" stroke="#b5e6e9" :stroke-width="evolutions?.bulwark?2.2:1.2" :opacity=".2+Math.min(1,shield/Math.max(1,maxShield))*.35" :stroke-dasharray="evolutions?.bulwark?undefined:'18 6'"/>
    <ellipse v-if="hurt>0" cy="3" rx="44" ry="31" fill="#ffd7a4" :fill-opacity="hurt*.12" stroke="#ffcd9d" stroke-width="2.5" :opacity="hurt"/>
  </g>
</template>
<style scoped>
.tower-ground-energy{animation:tower-energy 4s ease-in-out infinite}.tower-electric-bridge{animation:tower-electric .7s steps(2,end) infinite}.tower-evolution-flame{animation:tower-energy 1s ease-in-out infinite}.tower-still *{animation-play-state:paused!important}@keyframes tower-energy{50%{opacity:.38}}@keyframes tower-electric{50%{opacity:.35}}@media(prefers-reduced-motion:reduce){.tower-ground-energy,.tower-electric-bridge,.tower-evolution-flame{animation:none}}
</style>
