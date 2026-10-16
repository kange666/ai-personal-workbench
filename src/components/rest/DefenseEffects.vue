<script setup lang="ts">
import { computed } from 'vue';
import { effectProgress, type BattleEffect } from '../../utils/rest/battleEffects';
const props=defineProps<{effects:BattleEffect[];tier:number;skills?:{fire:number;ice:number;chain:number;blast:number}}>();
// 强化增加形状与层次，粒子数量保持固定；不用模糊滤镜或逐弹节点。
const fire=computed(()=>Math.min(3,Math.log2(1+(props.skills?.fire||0))));
const ice=computed(()=>Math.min(3,Math.log2(1+(props.skills?.ice||0))));
const electric=computed(()=>Math.min(3,Math.log2(1+(props.skills?.chain||0))));
function position(e:BattleEffect){const t=effectProgress(e),x=e.fromX??300,y=e.fromY??170;return{x:x+(e.x-x)*t,y:y+(e.y-y)*t,angle:Math.atan2(e.y-y,e.x-x)*180/Math.PI};}
function trail(e:BattleEffect){const t=effectProgress(e),tail=Math.max(0,t-.36),x=e.fromX??300,y=e.fromY??170;return 'M'+(x+(e.x-x)*tail)+' '+(y+(e.y-y)*tail)+'L'+(x+(e.x-x)*t)+' '+(y+(e.y-y)*t);}
function chain(e:BattleEffect){const x=e.fromX??300,y=e.fromY??170,dx=e.x-x,dy=e.y-y,w=(Math.floor(effectProgress(e)*6)+e.id)%2?1:-1;return 'M'+x+' '+y+'l'+[dx*.2-dy*.07*w,dy*.2+dx*.07*w,dx*.2+dy*.13*w,dy*.2-dx*.13*w,dx*.2-dy*.1*w,dy*.2+dx*.1*w,dx*.2+dy*.08*w,dy*.2-dx*.08*w,dx*.2-dy*.04*w,dy*.2+dx*.04*w].join(' ');}
function ring(e:BattleEffect){return Math.max(4,Math.min(360,e.radius||44)*(.12+effectProgress(e)*.88));}
function scatter(e:BattleEffect,i:number){const angle=i*Math.PI/3+e.id*.7,r=ring(e)*.65;return 'translate('+(e.x+Math.cos(angle)*r)+' '+(e.y+Math.sin(angle)*r)+') rotate('+(angle*180/Math.PI)+') scale('+(1-effectProgress(e)*.65)+')';}
</script>
<template>
  <g v-for="fx in effects" :key="fx.id" data-battle-effect :data-effect-kind="fx.kind" :data-effect-variant="fx.variant" :opacity="fx.delay?0:Math.min(1,fx.life*5)" pointer-events="none">
    <template v-if="fx.kind==='shot'">
      <path :d="trail(fx)" fill="none" :stroke="fx.color" :stroke-width="fx.variant==='volley'?9:fx.variant==='fire'?6:fx.variant==='shell'?5:4" stroke-linecap="round" opacity=".18"/>
      <path :d="trail(fx)" fill="none" :stroke="fx.color" :stroke-width="fx.variant==='volley'?2.5:fx.variant==='fire'?2.6:1.4" stroke-linecap="round" opacity=".85"/>
      <g :transform="'translate('+position(fx).x+' '+position(fx).y+') rotate('+position(fx).angle+')'">
        <g v-if="fx.variant==='shell'" data-projectile-body="shell">
          <circle v-for="i in [1,2,3]" :key="i" :cx="-8-i*6" :cy="i%2?2:-2" :r="2+i*.5" fill="#e5d5b3" :opacity=".33-i*.06"/>
          <path d="M7 0 2-4H-6V4H2Z" fill="#647585" stroke="#e9d6a7" stroke-width="1.1"/><path d="M-5-4V4m4-8V4" stroke="#e3b97f" stroke-width="1.5"/>
          <path d="M-7-2-17-4-13 0-19 3-7 2Z" fill="#e9a363"/><path d="M-7-1-13 0-7 1Z" fill="#fff3c5"/>
        </g>
        <g v-else-if="fx.variant==='fire'" data-projectile-body="fire" :transform="'scale('+(1+fire*.12)+')'">
          <path d="M8 0Q1-10-9-7L-5-3-23-6-15 0-27 4-10 4-12 8Q1 9 8 0Z" fill="#dc7953" opacity=".8"/>
          <path d="M7 0Q-1-6-14-3L-8 0-18 3Q-2 7 7 0Z" fill="#ffc179"/><path d="M5 0Q-1-3-9 0-1 3 5 0Z" fill="#fff1c0"/>
          <path d="M-19-7h-4m-2 15h-5m18 1-3 3" stroke="#ffda94" stroke-width="1.4" stroke-linecap="round"/>
        </g>
        <g v-else-if="fx.variant==='ice'" data-projectile-body="ice" :transform="'scale('+(1+ice*.1)+')'">
          <path d="M11 0-2-5-9 0-2 5Z" fill="#c7f3fa" stroke="#70bed6" stroke-width="1.2"/><path d="M8 0H-6L-2-3Z" fill="#f3ffff"/>
          <path d="M-10-4-17-7m7 11-7 3M-13 0H-24" stroke="#c4f1f8" stroke-width="1.2"/>
          <path v-for="y in [-7,7]" :key="y" :d="'M-10 '+y+'l-4-2-3 2 3 2Z'" fill="#c1eaf4" opacity=".8"/>
        </g>
        <g v-else-if="fx.variant==='plasma'" data-projectile-body="plasma"><ellipse rx="8" ry="6" fill="#b5b5f7" opacity=".25"/><circle r="4" fill="#e6faff" stroke="#bcacfb" stroke-width="1.7"/><path d="M-2-5-7-3-4 0-12 2-7 5-15 7M3-5 7-8 10-5m-6 9 6 3" fill="none" stroke="#d1c7ff" stroke-width="1.5"/></g>
        <g v-else-if="fx.variant==='volley'" data-projectile-body="volley"><path d="M-32-9 15 0-32 9-8 0Z" fill="#fae6a6" opacity=".2"/><path d="M-22-6 8 0-22 6M-32 0H16" fill="none" stroke="#fff1b7" stroke-width="2"/><path d="M-5-2H13L19 0 13 2H-5Z" fill="#fff8d8"/></g>
        <g v-else data-projectile-body="kinetic"><path :d="'M'+(-12-tier*2)+'-2H5L10 0 5 2H'+(-12-tier*2)+'Z'" fill="#fff0b5"/><path d="M-17 0H4" stroke="#fffce4" stroke-width="1.2"/><path v-if="tier>=3" d="M-14-4H-3M-14 4H-3" stroke="#f0c479" stroke-width="1.1"/></g>
        <g v-if="ice&&fx.variant==='fire'" data-projectile-layer="frost"><path d="M-10-10-6-14-1-10-6-7m-6 16 5-4 5 4-5 3" fill="#c2f0fb" stroke="#79c7df" stroke-width=".8"/></g>
      </g>
      <g v-if="effectProgress(fx)>.7" :transform="'translate('+fx.x+' '+fx.y+') scale('+((effectProgress(fx)-.7)*3+.4)+')'" :opacity="(effectProgress(fx)-.7)*3"><path d="M-9 0-3-2-1-8 2-2 9 0 2 2 0 8-2 2Z" fill="#fff6d9"/></g>
      <text v-if="(fx.amount||0)>4" :x="position(fx).x" :y="position(fx).y-10" fill="#f5e5be" font-size="8">×{{ fx.amount }}</text>
    </template>
    <g v-else-if="fx.kind==='chain'" data-skill-effect="lightning">
      <path :d="chain(fx)" fill="none" :stroke="fx.color" :stroke-width="5+electric" opacity=".16"/><path :d="chain(fx)" fill="none" :stroke="fx.color" :stroke-width="2.2+electric*.3"/><path :d="chain(fx)" fill="none" stroke="#f2efff" stroke-width=".9"/>
      <circle :cx="fx.x" :cy="fx.y" :r="4+effectProgress(fx)*9" fill="none" stroke="#d0c4ff" stroke-width="1"/><path :transform="'translate('+fx.x+' '+fx.y+')'" d="M-9-5-3-2-5 5 1 1 8 6M-3-9 1-4 6-7" fill="none" stroke="#efe9ff" stroke-width="1.2"/>
    </g>
    <g v-else-if="fx.kind==='coin'">
      <g v-if="effectProgress(fx)<.3" :transform="'translate('+fx.x+' '+fx.y+') scale('+(.5+effectProgress(fx)*3)+')'" :opacity="1-effectProgress(fx)*3"><circle r="10" fill="none" stroke="#f0d49a" stroke-width="1"/><path d="M-10-7-14-10m24 3 4-3m-14 21v5m10-9 4 3" stroke="#e7c68d" stroke-width="1.3"/></g>
      <g :transform="'translate('+(fx.x+(300-fx.x)*effectProgress(fx))+' '+(fx.y+(200-fx.y)*effectProgress(fx)-Math.sin(effectProgress(fx)*Math.PI)*24)+')'"><circle r="4.5" fill="#efc477" stroke="#fff0bc"/><path d="M0-2V2" stroke="#b28a4c" stroke-width="1.5"/></g>
    </g>
    <g v-else-if="fx.kind==='hit'" :transform="'translate('+fx.x+' '+(fx.y-effectProgress(fx)*15)+')'"><path v-if="effectProgress(fx)<.25" d="M-6 14-2 12 0 7 2 12 7 14 2 16 0 21-2 16Z" fill="#fff6ce" :opacity="1-effectProgress(fx)*4"/><text text-anchor="middle" :fill="fx.color" font-size="11" font-weight="700" stroke="#4a5a50" stroke-width=".8" paint-order="stroke">{{ fx.text }}</text></g>
    <g v-else-if="fx.kind==='upgrade'" data-skill-effect="upgrade"><ellipse :cx="fx.x" :cy="fx.y+5" :rx="32+effectProgress(fx)*35" :ry="16+effectProgress(fx)*17" fill="none" stroke="#e2c997" stroke-width="2"/><path v-for="i in [0,1,2,3]" :key="i" d="M-2 0 0-5 2 0 7 2 2 4 0 9-2 4-7 2Z" :transform="'translate('+(fx.x+Math.cos(i*1.57)*37)+' '+(fx.y-15-effectProgress(fx)*45+Math.sin(i*1.57)*12)+')'" fill="#f6e8b6"/><text :x="fx.x" :y="fx.y-62-effectProgress(fx)*14" text-anchor="middle" fill="#f2d8ff" stroke="#4c6261" stroke-width=".9" paint-order="stroke" font-size="11" font-weight="700">{{ fx.text }}</text></g>
    <g v-else :data-skill-effect="fx.variant||'burst'">
      <template v-if="fx.variant==='shell'||fx.variant==='fire'">
        <circle :cx="fx.x" :cy="fx.y" :r="ring(fx)" fill="#f3b87d" :fill-opacity="(1-effectProgress(fx))*.16" :stroke="fx.color" stroke-width="1.7"/><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)*.68" fill="none" stroke="#f9dbab" stroke-width=".9" :stroke-dasharray="fx.variant==='fire'?'9 7':undefined"/>
        <g v-for="i in [0,1,2,3,4,5]" :key="i" :transform="scatter(fx,i)"><path v-if="fx.variant==='fire'" d="M-5 2Q-6-4 0-12L1-5 5-9Q10 3 2 7Z" fill="#f3a466" stroke="#ffdda0" stroke-width=".7"/><path v-else d="M-4-2 5-4 10 0 5 3-4 2Z" fill="#e6c49b" opacity=".8"/></g>
        <path v-if="effectProgress(fx)<.35" :transform="'translate('+fx.x+' '+fx.y+') scale('+(.8+effectProgress(fx)*4)+')'" d="M-15 0-6-4-8-11 0-6 7-12 6-4 17 0 6 4 8 12 0 6-7 11-6 4Z" fill="#fff1be" :opacity="1-effectProgress(fx)*2.5"/>
      </template>
      <template v-else-if="fx.variant==='ice'">
        <circle :cx="fx.x" :cy="fx.y" :r="ring(fx)" fill="#c5eff8" :fill-opacity="(1-effectProgress(fx))*.13" stroke="#b0e8f4" stroke-width="1.4" stroke-dasharray="7 6"/>
        <g v-for="i in [0,1,2,3,4,5]" :key="i" :transform="scatter(fx,i)"><path d="M-9 0 0-4 10 0 0 4Z" fill="#d9faff" stroke="#7ac5da" stroke-width="1"/><path d="M-4 0H7" stroke="#f5ffff" stroke-width="1"/></g>
        <path :transform="'translate('+fx.x+' '+fx.y+') scale('+(1+effectProgress(fx)*2)+')'" d="M-12 0H12M0-12V12m-9-9 18 18m-18 0 18-18M-8-3-5 0-8 3m16-6-3 3 3 3" fill="none" stroke="#ddfaff" stroke-width="1.3"/>
      </template>
      <template v-else-if="fx.variant==='shield'">
        <g :transform="'translate('+fx.x+' '+fx.y+') scale('+(ring(fx)/32)+')'"><path d="M0-30 26-15V15L0 30-26 15V-15Z" fill="#c5edf0" :fill-opacity="(1-effectProgress(fx))*.1" :stroke="fx.color" stroke-width="1.3"/><path d="M0-22 19-11V11L0 22-19 11V-11Z" fill="none" :stroke="fx.color" stroke-width=".5"/></g><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)*1.05" fill="none" :stroke="fx.color" stroke-width="1" stroke-dasharray="12 12"/>
      </template>
      <template v-else-if="fx.variant==='nova'">
        <circle :cx="fx.x" :cy="fx.y" :r="ring(fx)" fill="none" stroke="#eadbff" stroke-width="2.4"/><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)*.9" fill="none" stroke="#bda2ed" stroke-width="5" opacity=".2"/><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)*.7" fill="none" stroke="#d4c1fb" stroke-width="1" stroke-dasharray="20 8 4 8"/><path v-for="i in [0,1,2,3,4,5]" :key="i" d="M-6-3 0 0-6 3m-4-3H3" :transform="scatter(fx,i)" stroke="#f3e7ff" stroke-width="1.3" fill="none"/>
      </template>
      <template v-else><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)" fill="none" :stroke="fx.color" stroke-width="1.7"/><circle :cx="fx.x" :cy="fx.y" :r="ring(fx)*.65" fill="none" :stroke="fx.color" stroke-width=".8" stroke-dasharray="8 6"/><path v-for="i in [0,1,2]" :key="i" d="M-4-4 0-9 4-4 0 1Z" :transform="scatter(fx,i*2)" :fill="fx.color"/></template>
    </g>
  </g>
</template>
