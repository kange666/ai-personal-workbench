<script setup lang="ts">
import { computed, ref } from 'vue';
import { useGameSave, gameBest } from '../../composables/useGameSave';
import { useGameLoop } from '../../composables/useGameLoop';
import { createBreakoutAdventure, validBreakoutAdventure, startBreakoutAdventure, stepBreakoutAdventure, nextBreakoutAdventure, breakoutPaddleWidth, breakoutBrickX, BREAKOUT_LEVELS, BREAKOUT_POWER_NAMES } from '../../utils/rest/breakoutCampaign';
import { createProgress, validProgress, finishLevel, clamp } from '../../utils/rest/progression';
import CampaignToolbar from './CampaignToolbar.vue';
import BreakoutGame from './BreakoutGame.vue';
const props=defineProps<{paused:boolean}>();
const saved=useGameSave('breakout-v2',()=>createBreakoutAdventure(),validBreakoutAdventure),state=saved.state;
const progress=useGameSave('breakout-progress',()=>createProgress(40),v=>validProgress(v,40));
const legacy=ref(false),oldGame=ref<InstanceType<typeof BreakoutGame>|null>(null),waiting=ref(state.value.phase==='playing');
const surface=ref<HTMLElement|null>(null);
function focusGame(){surface.value?.focus({preventScroll:true});}
const best=computed(()=>Math.max(saved.best.value,gameBest('breakout'))),width=computed(()=>breakoutPaddleWidth(state.value));
const activeEffects=computed(()=>Object.entries(state.value.effects).filter(([k,v])=>v>0&&k!=='multi').map(([k,v])=>`${BREAKOUT_POWER_NAMES[k as keyof typeof BREAKOUT_POWER_NAMES]} ${k==='shield'?`×${v}`:`${Math.ceil(v)}秒`}`).join(' · '));
let ticks=0;
useGameLoop(()=>props.paused||legacy.value||waiting.value,dt=>{
  const before=state.value.phase;stepBreakoutAdventure(state.value,dt);ticks+=dt;
  if(before!==state.value.phase){saved.recordScore(state.value.score);if(state.value.phase==='won'&&state.value.mode==='campaign'){finishLevel(progress.state.value,state.value.level,state.value.lostBalls===0?3:state.value.lostBalls<=2?2:1,state.value.lostBalls+1);progress.save();}saved.save();}
  if(ticks>=1){saved.save();ticks=0;}
});
function select(level:number){if(!props.paused&&level<=progress.state.value.unlocked){state.value=createBreakoutAdventure(level);waiting.value=false;saved.save();}}
function mode(value:string){if(props.paused)return;legacy.value=value==='legacy';if(!legacy.value){state.value=createBreakoutAdventure(0,value==='endless'?'endless':'campaign');waiting.value=false;saved.save();}}
function launch(){if(props.paused)return;waiting.value=false;startBreakoutAdventure(state.value);saved.save();focusGame();}
function pointer(event:PointerEvent){if(props.paused||waiting.value)return;const bounds=(event.currentTarget as HTMLElement).getBoundingClientRect();if(bounds.width>0)state.value.paddleX=clamp((event.clientX-bounds.left)/bounds.width*480,width.value/2,480-width.value/2);if(state.value.phase==='ready')state.value.balls[0]!.x=state.value.paddleX;}
function key(event:KeyboardEvent){if(props.paused||waiting.value)return;if(['ArrowLeft','ArrowRight','a','d',' '].includes(event.key)){event.preventDefault();event.stopPropagation();if(event.key===' ')launch();else{state.value.paddleX=clamp(state.value.paddleX+(['ArrowLeft','a'].includes(event.key)?-24:24),width.value/2,480-width.value/2);if(state.value.phase==='ready')state.value.balls[0]!.x=state.value.paddleX;}}}
function next(){if(props.paused)return;state.value=nextBreakoutAdventure(state.value);saved.save();}
function restart(){if(props.paused)return;if(legacy.value)oldGame.value?.restart();else{state.value=createBreakoutAdventure(state.value.level,state.value.mode);waiting.value=false;saved.save();}}
defineExpose({restart});
</script>
<template>
  <div class="breakout-adventure">
    <CampaignToolbar :mode="legacy?'legacy':state.mode" :modes="[{id:'campaign',name:'五章远征'},{id:'endless',name:'无尽阵列'},{id:'legacy',name:'经典 · 旧进度'}]" :level="Math.min(39,state.level)" :levels="BREAKOUT_LEVELS" :stars="progress.state.value.stars" :unlocked="progress.state.value.unlocked" :disabled="paused" @mode="mode" @select="select"/>
    <BreakoutGame v-if="legacy" ref="oldGame" :paused="paused"/>
    <template v-else>
      <div class="game-stats"><span>生命 <b>{{ '♥'.repeat(state.lives) }}</b></span><span>关卡 <b>{{ state.level+1 }}</b></span><span>得分 <b>{{ state.score }}</b> / {{ best }}</span></div>
      <div ref="surface" class="game-surface" data-game-focus tabindex="0" @keydown="key" @pointermove="pointer" @pointerdown="pointer($event); focusGame()">
        <svg class="breakout-scene" viewBox="0 0 480 330" aria-label="弹球砖阵，移动鼠标控制挡板，点击发球" @click="launch">
          <rect width="480" height="330" fill="var(--surface-2)"/>
          <g v-for="b in state.bricks.filter(b=>b.hp>0)" :key="b.id"><rect :x="breakoutBrickX(state,b)" :y="b.y" :width="b.w" :height="b.h" rx="3" :fill="b.kind==='metal'?'var(--muted)':b.kind==='boss'?'var(--danger)':b.kind==='bomb'?'var(--warning)':b.kind==='switch'?'var(--success)':'var(--primary)'" :opacity="b.kind==='boss'?.9:.55+b.hp/b.maxHP*.4"/><text v-if="b.kind!=='normal'" :x="breakoutBrickX(state,b)+b.w/2" :y="b.y+b.h/2+4" text-anchor="middle" font-size="10" fill="var(--surface)">{{ b.kind==='metal'?'锁':b.kind==='switch'?'开':b.kind==='bomb'?'✦':b.kind==='moving'?'↔':b.hp }}</text></g>
          <rect :x="state.paddleX-width/2" y="288" :width="width" height="9" rx="4" fill="var(--text)"/>
          <circle v-for="(ball,i) in state.balls" :key="i" :cx="ball.x" :cy="ball.y" r="6" :fill="state.effects.pierce>0?'var(--warning)':'var(--primary)'"/>
          <g v-for="(item,i) in state.items" :key="i"><rect :x="item.x-14" :y="item.y-8" width="28" height="16" rx="5" fill="var(--warning)"/><text :x="item.x" :y="item.y+3" text-anchor="middle" font-size="8" fill="var(--surface)">{{ BREAKOUT_POWER_NAMES[item.kind] }}</text></g>
          <path v-if="state.effects.shield>0" d="M8 317h464" stroke="var(--success)" stroke-width="3"/>
          <g v-if="waiting||state.phase!=='playing'"><rect width="480" height="330" fill="var(--surface)" opacity=".72"/><text x="240" y="174" text-anchor="middle" fill="var(--text)" font-size="20">{{ waiting?'进度已保留':state.phase==='ready'?'点击发球':state.phase==='won'?'阵列清空！':'本轮结束' }}</text></g>
        </svg>
      </div>
      <div class="breakout-actions"><button v-if="waiting||state.phase==='ready'" class="button primary" :disabled="paused" @click="launch">{{ waiting?'继续游玩':'发球' }}</button><button v-if="state.phase==='won'&&(state.mode==='endless'||state.level<39)" class="button primary" :disabled="paused" @click="next">下一关 · 回复一条生命</button><button class="button" :disabled="paused" @click="restart">重开本关</button></div>
      <p class="game-help">{{ activeEffects || state.last }} · 鼠标 / 触屏移动挡板，方向键微调，空格发球。首领需先清除护卫；开关可解除锁砖。</p>
      <p v-if="saved.storageError.value||progress.storageError.value" class="game-help">本机存档暂不可用，当前仍可游玩。</p>
    </template>
  </div>
</template>
<style scoped>.breakout-adventure{display:grid;gap:10px}.breakout-adventure>.game-surface{width:min(100%,480px)}.breakout-scene{width:100%;display:block;touch-action:none;cursor:crosshair}.breakout-actions{display:flex;flex-wrap:wrap;gap:8px}.game-help{margin:0;line-height:1.6}</style>
