<script setup lang="ts">
import { computed, ref } from "vue";
import { useGameSave, gameBest } from "../../composables/useGameSave";
import { circuitStars, circuitStatus, createCircuit, hintCircuit, turnCircuit, undoCircuit, validCircuit, WIRING_LEVELS } from "../../utils/rest/wiringCampaign";
import { createProgress, finishLevel, validProgress } from "../../utils/rest/progression";
import CampaignToolbar from "./CampaignToolbar.vue";
import WiringGame from "./WiringGame.vue";
const props=defineProps<{paused:boolean}>();
const {state,best,save,recordScore}=useGameSave("wiring-v2",()=>createCircuit(0),validCircuit);
best.value=Math.max(best.value,gameBest("wiring"));
const {state:progress,save:saveProgress}=useGameSave("wiring-progress",()=>createProgress(60),(v):v is ReturnType<typeof createProgress>=>validProgress(v,60));
const legacyMode=ref(false),mode=computed(()=>legacyMode.value?"legacy":state.value.mode),legacy=ref<{restart():void}>(),cursor=ref(0);
const status=computed(()=>circuitStatus(state.value));
const lines=[{bit:1,x:20,y:0},{bit:2,x:40,y:20},{bit:4,x:20,y:40},{bit:8,x:0,y:20}];
function commit(){if(state.value.won){if(mode.value==="campaign"){finishLevel(progress.value,state.value.level,circuitStars(state.value),state.value.moves);saveProgress();}recordScore(1000-state.value.hints*20+state.value.level*50);}save();}
function turn(index:number){if(props.paused)return;turnCircuit(state.value,index);cursor.value=index;commit();}
function select(level:number){if(props.paused||level>progress.value.unlocked)return;state.value=createCircuit(level);save();}
function next(){if(props.paused)return;const currentMode=mode.value;state.value=mode.value==="endless"?createCircuit(Math.floor(Math.random()*60),Date.now()>>>0):createCircuit(Math.min(59,state.value.level+1));state.value.mode=currentMode==="endless"?"endless":"campaign";save();}
function restart(){if(props.paused)return;if(mode.value==="legacy")legacy.value?.restart();else {const currentMode=state.value.mode;state.value=createCircuit(state.value.level,state.value.seed);state.value.mode=currentMode;save();}}
function key(event:KeyboardEvent){if(!["ArrowUp","ArrowDown","ArrowLeft","ArrowRight"," ","Enter"].includes(event.key))return;event.preventDefault();if(props.paused)return;const x=cursor.value%state.value.size,y=Math.floor(cursor.value/state.value.size);if(event.key==="ArrowUp")cursor.value=Math.max(0,y-1)*state.value.size+x;if(event.key==="ArrowDown")cursor.value=Math.min(state.value.size-1,y+1)*state.value.size+x;if(event.key==="ArrowLeft")cursor.value=y*state.value.size+Math.max(0,x-1);if(event.key==="ArrowRight")cursor.value=y*state.value.size+Math.min(state.value.size-1,x+1);if([" ","Enter"].includes(event.key))turn(cursor.value);}
defineExpose({restart});
</script>
<template>
  <div><CampaignToolbar :mode="mode" :level="state.level" :levels="WIRING_LEVELS" :stars="progress.stars" :unlocked="progress.unlocked" :disabled="paused" :modes="[{id:'campaign',label:'章节闯关 · 60关'},{id:'endless',label:'随机挑战'},{id:'legacy',label:'经典随机 · 旧进度'}]" @select="select" @mode="value=>{if(paused)return;legacyMode=value==='legacy';if(value!=='legacy'){state=createCircuit(value==='endless'?Math.floor(Math.random()*60):Math.min(state.level,progress.unlocked),value==='endless'?Date.now()>>>0:undefined);state.mode=value as 'campaign'|'endless';save();}}" />
    <WiringGame v-if="mode==='legacy'" ref="legacy" :paused="paused" />
    <template v-else><div class="game-stats"><span>{{ status.powered }}/{{ state.tiles.length }} 点亮</span><span>{{ state.moves }} 步 · 提示 {{ state.hints }} 次</span><span>最高 {{ best }}</span></div>
      <div class="circuit-board" :style="{gridTemplateColumns:`repeat(${state.size},minmax(0,1fr))`}" tabindex="0" data-game-focus aria-label="电路棋盘，方向键选择节点，空格旋转" @keydown="key"><button v-for="(mask,i) in state.tiles" :key="i" class="circuit-tile" :class="{selected:cursor===i,locked:state.locked[i]}" :disabled="paused||state.won||state.locked[i]" tabindex="-1" :aria-label="`旋转第${Math.floor(i/state.size)+1}行第${i%state.size+1}列`" @click="turn(i)"><svg viewBox="0 0 40 40" aria-hidden="true"><template v-for="line in lines" :key="line.bit"><line v-if="mask&line.bit" x1="20" y1="20" :x2="line.x" :y2="line.y" :stroke="status.colors[i*2+(state.bridges[i]&&[2,8].includes(line.bit)?1:0)]===2?'#e5b56f':status.colors[i*2]?'var(--primary)':'var(--muted)'" stroke-width="4" stroke-linecap="round"/></template><circle cx="20" cy="20" :r="state.bridges[i]?5:3" fill="var(--surface)" :stroke="state.locked[i]?'var(--warning)':'var(--line)'" stroke-width="1.5"/><circle v-for="source in state.sources.filter(s=>s.index===i)" :key="source.color" cx="20" cy="20" r="6" :fill="source.color===1?'var(--primary)':'#e5b56f'"/></svg></button></div>
      <p class="game-help" role="status">{{ state.won?'所有线路接通！'+ '★'.repeat(circuitStars(state)):status.mixed?'两种电源混接了，请分开连接。':`还有 ${status.leaks} 个断口。金色边点固定，交叉桥上下与左右互不连通。` }}</p>
      <div class="game-actions"><button class="button" :disabled="paused||!state.history.length" @click="undoCircuit(state);save()">撤销</button><button class="button" :disabled="paused||state.won" @click="hintCircuit(state);commit()">单步提示</button><button v-if="state.won&&(mode==='endless'||state.level<59)" class="button primary" :disabled="paused" @click="next">下一关</button></div>
    </template>
  </div>
</template>
<style scoped>
.circuit-board{display:grid;gap:3px;width:min(100%,370px);margin:auto;padding:8px;border:1px solid var(--line);border-radius:12px;background:var(--surface-2);outline-offset:3px}.circuit-tile{position:relative;aspect-ratio:1;min-width:0;padding:0;border:1px solid transparent;border-radius:6px;background:var(--surface);cursor:pointer}.circuit-tile svg{position:absolute;inset:0;width:100%;height:100%}.circuit-tile.selected{border-color:var(--primary)}.circuit-tile.locked{background:color-mix(in srgb,var(--warning) 5%,var(--surface));cursor:default}.circuit-tile:disabled{opacity:1}
</style>
