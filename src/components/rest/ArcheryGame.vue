<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { useGameLoop } from "../../composables/useGameLoop";
import { ARCHERY_LEVELS, ARROWS, archerPosition, archeryTrajectory, createArchery, obstacleAt, shootArchery, stepArchery, suggestArcheryAim, validArchery } from "../../utils/rest/archery";
import { clamp, createProgress, finishLevel, validProgress } from "../../utils/rest/progression";
import CampaignToolbar from "./CampaignToolbar.vue";
const props=defineProps<{paused:boolean}>();
const {state,best,save,recordScore}=useGameSave("archery",()=>createArchery(),validArchery);
const {state:progress,save:saveProgress}=useGameSave("archery-progress",()=>createProgress(30),(v):v is ReturnType<typeof createProgress>=>validProgress(v,30));
const mode=computed({get:()=>state.value.mode,set:(v)=>{state.value.mode=v as "campaign"|"endless";}}),surface=ref<SVGSVGElement>();const awaiting=ref(state.value.phase==="flight"||state.value.phase==="ai");
const positions=computed(()=>[archerPosition(state.value.level,"player"),archerPosition(state.value.level,"enemy")]);
const preview=computed(()=>state.value.phase==="aim"&&state.value.assist?archeryTrajectory(state.value).points:[]);
const path=(points:{x:number;y:number}[])=>points.map((p,i)=>`${i?'L':'M'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(" ");
let saved=0,drag:{x:number;y:number;id:number}|null=null;
useGameLoop(()=>props.paused||awaiting.value,dt=>{const phase=state.value.phase;stepArchery(state.value,dt);saved+=dt;if(state.value.phase==="won"&&phase!=="won"){if(mode.value==="campaign"){finishLevel(progress.value,state.value.level,state.value.playerHP>=80?3:state.value.playerHP>=35?2:1,state.value.shots);saveProgress();}recordScore(state.value.score);}if(saved>=1||phase!==state.value.phase){save();saved=0;}});
function start(level=state.value.level){if(props.paused)return;const selected=mode.value;state.value=createArchery(level);state.value.mode=selected;awaiting.value=false;save();}
function fire(){if(!props.paused&&!awaiting.value&&shootArchery(state.value))save();}
function aim(){if(props.paused||state.value.phase!=="aim")return;const solution=suggestArcheryAim(state.value,"player",state.value.kind);state.value.angle=solution.angle;state.value.power=solution.power;save();}
function pointer(event:PointerEvent){const bounds=surface.value!.getBoundingClientRect();if(!bounds.width||!bounds.height)return{x:0,y:0};return{x:(event.clientX-bounds.left)/bounds.width*640,y:(event.clientY-bounds.top)/bounds.height*360};}
function down(event:PointerEvent){if(props.paused||state.value.phase!=="aim"||awaiting.value||event.button!==0)return;const p=pointer(event);drag={...p,id:event.pointerId};surface.value?.setPointerCapture?.(event.pointerId);}
function move(event:PointerEvent){if(!drag||props.paused)return;const p=pointer(event),dx=drag.x-p.x,dy=p.y-drag.y;state.value.angle=clamp(Math.atan2(Math.max(0,dy),Math.max(1,dx))*180/Math.PI,15,80);state.value.power=clamp(Math.hypot(dx,dy)*2.5+180,180,450);}
function up(event:PointerEvent){if(!drag||event.pointerId!==drag.id)return;move(event);drag=null;surface.value?.releasePointerCapture?.(event.pointerId);fire();}
defineExpose({restart:()=>start()});
</script>
<template>
  <div class="archery-game">
    <CampaignToolbar :mode="mode" :level="state.level" :levels="ARCHERY_LEVELS" :stars="progress.stars" :unlocked="progress.unlocked" :disabled="paused" :modes="[{id:'campaign',label:'章节闯关'},{id:'endless',label:'随机遭遇'}]" @mode="value=>{mode=value==='endless'?'endless':'campaign';start(value==='endless'?Math.floor(Math.random()*30):Math.min(state.level,progress.unlocked))}" @select="level=>{if(level<=progress.unlocked)start(level)}" />
    <div class="game-stats"><span>你的生命 <b>{{ state.playerHP }}/130</b></span><span>电脑 <b>{{ state.enemyHP }}/{{ state.enemyMax }}</b></span><span>风 {{ state.wind===0?'平静':state.wind>0?'向右 →':'← 向左' }} {{ Math.abs(state.wind) }}</span><span>最高 {{ best }}</span></div>
    <svg ref="surface" class="archery-surface" viewBox="0 0 640 360" tabindex="0" data-game-focus aria-label="火柴人射箭战场，向左下拖动拉弓，松开射击，空格也可射击" @pointerdown="down" @pointermove="move" @pointerup="up" @pointercancel="drag=null" @keydown.space.prevent="event=>{if(!event.repeat)fire()}">
      <defs><linearGradient id="archer-sky" x2="0" y2="1"><stop stop-color="var(--surface-2)"/><stop offset="1" stop-color="var(--primary-soft)"/></linearGradient></defs>
      <rect width="640" height="360" rx="12" fill="url(#archer-sky)"/><path d="M0 337H640M0 355H640" stroke="var(--line)"/>
      <circle cx="480" cy="60" r="24" fill="var(--warning)" opacity=".12"/><path d="M0 260 80 210 170 270 260 220 340 270 460 190 640 270V337H0Z" fill="var(--primary)" opacity=".035"/>
      <template v-for="(p,i) in positions" :key="i"><rect :x="p.x-37" :y="p.y+49" width="74" :height="337-p.y-49" rx="4" fill="var(--line)"/>
        <g :transform="`translate(${p.x} ${p.y})`" :stroke="i?'var(--danger)':'var(--primary)'" fill="none" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round"><circle r="8"/><path d="M0 8V29M0 29l-12 19M0 29l13 19"/><path :d="i?'M0 14l-17 5M0 18l-9 9':'M0 14l17 5M0 18l9 9'"/><path :d="i?'M-20 6q-13 14 0 28L-20 6':'M20 6q13 14 0 28L20 6'" stroke-width="2"/></g>
      </template>
      <g v-for="(obstacle,i) in state.obstacles" :key="i"><template v-if="obstacle.hp>0"><rect :x="obstacle.x" :y="obstacleAt(obstacle,state.time+(state.shot?.age||0)).y" :width="obstacle.width" :height="obstacle.height" rx="3" :fill="obstacle.kind==='wood'?'#a87f52':obstacle.kind==='reflect'?'var(--success)':obstacle.kind==='moving'?'var(--warning)':'var(--muted)'" opacity=".65"/><path v-if="obstacle.kind==='wood'" :d="`M${obstacle.x} ${obstacle.y+15}l${obstacle.width} 15m-${obstacle.width} 15l${obstacle.width} 15`" stroke="var(--surface)" opacity=".5"/></template></g>
      <path v-if="preview.length" :d="path(preview)" fill="none" stroke="var(--primary)" stroke-width="1.7" stroke-dasharray="3 7" opacity=".55"/>
      <g v-if="state.shot"><path :d="path(state.shot.trail)" fill="none" stroke="var(--primary)" opacity=".3" stroke-width="2"/><g :transform="`translate(${state.shot.x} ${state.shot.y}) rotate(${Math.atan2(state.shot.vy,state.shot.vx)*180/Math.PI})`" stroke="var(--text)" stroke-width="2"><path d="M-15 0H6m-5-4 6 4-6 4"/></g></g>
      <g v-if="awaiting || state.phase==='won' || state.phase==='lost'"><rect x="170" y="110" width="300" height="120" rx="14" fill="var(--surface)" stroke="var(--line)" opacity=".96"/><text x="320" y="154" text-anchor="middle" fill="var(--text)" font-size="21">{{ awaiting?'本次对决已保存':state.phase==='won'?'漂亮，赢下这一局！':'再试一次，你可以的' }}</text><text x="320" y="187" text-anchor="middle" fill="var(--muted)" font-size="12">{{ awaiting?'点击下方继续':'修正弧线、利用障碍，特殊箭也能破局' }}</text></g>
    </svg>
    <div class="archery-controls"><label>角度 {{ Math.round(state.angle) }}° <input v-model.number="state.angle" type="range" min="15" max="80" :disabled="paused||state.phase!=='aim'"></label><label>力度 {{ Math.round(state.power) }} <input v-model.number="state.power" type="range" min="180" max="450" :disabled="paused||state.phase!=='aim'"></label></div>
    <div class="game-actions"><select v-model="state.kind" aria-label="箭种" :disabled="paused||state.phase!=='aim'"><option v-for="arrow in ARROWS" :key="arrow.id" :value="arrow.id" :disabled="arrow.id!=='normal'&&state.ammo[arrow.id]<=0">{{ arrow.name }} {{ arrow.id==='normal'?'∞':state.ammo[arrow.id] }}</option></select><button class="button" :disabled="paused||state.phase!=='aim'" @click="aim">瞄准辅助</button><button class="button primary" :disabled="paused||state.phase!=='aim'||awaiting" @click="fire">放箭</button><button v-if="awaiting" class="button primary" :disabled="paused" @click="awaiting=false">继续对决</button><button v-if="state.phase==='won'&&state.level<29&&mode==='campaign'" class="button primary" :disabled="paused" @click="start(state.level+1)">下一关</button><button v-if="state.phase==='lost'||(state.phase==='won'&&(mode==='endless'||state.level===29))" class="button" :disabled="paused" @click="start(mode==='endless'?Math.floor(Math.random()*30):state.level)">再战一局</button></div>
    <p class="game-help" role="status">{{ state.last }}<br>向左下拖动拉弓，或调整滑块后放箭。你先手、生命更充足，没有回合倒计时。</p>
  </div>
</template>
<style scoped>
.archery-surface{display:block;width:100%;aspect-ratio:16/9;border:1px solid var(--line);border-radius:12px;outline-offset:3px;touch-action:none;user-select:none;cursor:crosshair}.archery-controls{display:flex;gap:16px;margin-top:10px;font-size:11px;color:var(--muted)}.archery-controls label{display:flex;flex:1;align-items:center;gap:5px;min-width:0}.archery-controls input{width:100%;min-width:35px;accent-color:var(--primary)}.archery-game select{border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:7px;color:var(--text);font-size:11px}
</style>
