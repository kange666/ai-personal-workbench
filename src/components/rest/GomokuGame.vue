<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { useGameLoop } from "../../composables/useGameLoop";
import { getGomokuDeepSeekMove } from "../../services/backend";
import { searchGomokuDeepSeek } from "../../utils/rest/gomokuExpert";
import { chooseGomokuMove, createGomoku, GOMOKU_DIFFICULTIES, placeGomoku, undoGomoku, winningLine, type GomokuDifficulty } from "../../utils/rest/gomoku";
import { createGomokuPuzzle, createGomokuSession, GOMOKU_PUZZLES, gomokuPuzzleContinuation, gomokuPuzzleTurns, migrateGomoku, validGomokuSession } from "../../utils/rest/gomokuCampaign";
import { finishLevel } from "../../utils/rest/progression";
import CampaignToolbar from "./CampaignToolbar.vue";

const props = defineProps<{ paused: boolean }>();
const { state: session, best, save, recordScore, storageError } = useGameSave("gomoku", createGomokuSession, validGomokuSession, migrateGomoku);
const state = computed(()=>session.value.run);
const human = computed(()=>state.value.human || 1);
const displayedScore = computed(()=>session.value.mode==='puzzle' ? (session.value.done ? 300 : 0) : state.value.score);
const playerMoves = computed(()=>session.value.mode==='puzzle' ? gomokuPuzzleTurns(session.value.level,state.value) : state.value.moves.filter(move=>move.player===human.value).length);
const puzzle = computed(()=>GOMOKU_PUZZLES[session.value.level]!);
const puzzleFailed = computed(()=>session.value.mode==='puzzle'&&!session.value.done&&(state.value.status==='lost'||(playerMoves.value>=puzzle.value.turns&&state.value.status==='playing')));
const review = ref(-1), hint = ref(-1), feedback=ref("");
const shownBoard = computed(()=>review.value<0?state.value.board:state.value.moves.slice(0,review.value).reduce((board,move)=>{board[move.index]=move.player;return board;},Array<number>(225).fill(0)));
const cursor = ref(112);
const surface = ref<HTMLElement>();
const aiTurn = computed(() => !session.value.done && !puzzleFailed.value && state.value.status === "playing" && state.value.moves.length % 2 + 1 !== human.value);
const deepSeekMode = computed(() => session.value.mode==='endless' && session.value.difficulty==='deepseek');
const deepSeekEnabled = ref(false), networkPending = ref(false), aiNotice=ref('');
const lastMove = computed(() => (review.value < 0 ? state.value.moves.at(-1) : state.value.moves[review.value-1])?.index ?? -1);
const line = computed(() => (review.value<0||review.value===state.value.moves.length)&&(state.value.status === "won" || state.value.status === "lost") ? winningLine(state.value.board, lastMove.value) : []);
const message = computed(() => feedback.value || (session.value.done ? "解题成功！可以进入下一题。" : puzzleFailed.value ? '未完成限定步数内的杀棋，可以悔棋或重新挑战。' : { playing: deepSeekMode.value&&!deepSeekEnabled.value ? '点击“开始 DeepSeek 对弈”后继续。' : aiTurn.value ? (deepSeekMode.value ? "DeepSeek 正在思考…" : "电脑正在思考…") : `轮到你了 · 你执${human.value===1?'黑':'白'}`, won: `五子连珠，你赢了！+${state.value.score} 分`, lost: "这局电脑获胜，试试入门难度或落点提示。", draw: "棋盘已满，平局。+50 分" }[state.value.status]));
let aiElapsed = 0;
const thinking = ref(false);
let worker:Worker|null=null, request=0, stopSearch:(()=>void)|null=null;
function cancelThinking() { request++;stopSearch?.();stopSearch=null;worker?.terminate();worker=null;thinking.value=false; }
function expertMove(side:1|2, consume:(index:number|null)=>void) {
  cancelThinking();const ticket=request,board=[...state.value.board];thinking.value=true;
  const accept=(index:number|null)=>{if(ticket!==request||props.paused)return;worker?.terminate();worker=null;thinking.value=false;consume(index);};
  if(typeof Worker==='undefined'){accept(chooseGomokuMove(board,'expert',()=>.5,side));return;}
  try {
    worker=new Worker(new URL('../../utils/rest/gomokuExpert.worker.ts',import.meta.url),{type:'module'});
    worker.onmessage=event=>accept(event.data.index);
    worker.onerror=()=>{cancelThinking();feedback.value='思考暂时中断，请点击重试思考。';};
    worker.postMessage({board,side});
  } catch {cancelThinking();feedback.value='思考暂时中断，请点击重试思考。';}
}
async function deepSeekMove(side:1|2) {
  cancelThinking();const ticket=request,board=[...state.value.board];
  thinking.value=true;networkPending.value=true;aiNotice.value='';
  try {
    const result=await getGomokuDeepSeekMove(board,side);
    if(ticket!==request||props.paused)return;
    if(result.index!==null&&(!Number.isInteger(result.index)||state.value.board[result.index]!==0)) {
      feedback.value='DeepSeek 返回无效落点，本回合未落子，请重试。';return;
    }
    const checked=new Promise<number|null>((resolve,reject)=>{
      stopSearch=()=>resolve(null);
      if(typeof Worker==='undefined'){resolve(searchGomokuDeepSeek(board,side,300,result.index??undefined).index);return;}
      worker=new Worker(new URL('../../utils/rest/gomokuExpert.worker.ts',import.meta.url),{type:'module'});
      worker.onmessage=event=>resolve(event.data.index);
      worker.onerror=()=>reject(new Error('search'));
      worker.postMessage({board,side,strongest:true,preferred:result.index??undefined});
    });
    const safeIndex=await checked;
    if(ticket!==request||props.paused)return;
    // 联网建议必须通过最高档搜索；弱于校验落点时修正，防止漏挡和随意送棋。
    const index=safeIndex;
    if(index===null||!Number.isInteger(index)||!placeGomoku(state.value,index,side)){
      feedback.value='攻防校验未完成，本回合未落子，请重试。';return;
    }
    aiNotice.value=result.notice??'';commit();
  } catch(error) {
    if(ticket===request&&!props.paused)feedback.value=typeof error==='string'?error:'DeepSeek 请求失败，请重试。';
  } finally {
    networkPending.value=false;
    if(ticket===request){stopSearch?.();stopSearch=null;worker?.terminate();worker=null;thinking.value=false;}
  }
}
watch(()=>[props.paused,session.value.run,session.value.difficulty,review.value],()=>{cancelThinking();aiNotice.value='';aiElapsed=0;}, {flush:'sync'});
onBeforeUnmount(cancelThinking);

useGameLoop(() => props.paused, dt => {
  if (!aiTurn.value || review.value>=0 || thinking.value || feedback.value || (deepSeekMode.value&&(networkPending.value||!deepSeekEnabled.value))) { aiElapsed = 0; return; }
  aiElapsed += dt;
  if (aiElapsed < 0.32) return;
  aiElapsed = 0;
  const side=human.value===1?2:1;
  const apply=(index:number|null)=>{if(index!==null&&placeGomoku(state.value,index,side))commit();};
  const reference=session.value.mode==='puzzle'?gomokuPuzzleContinuation(session.value.level,state.value):null;
  if(deepSeekMode.value)void deepSeekMove(side);
  else if(reference!==null)apply(reference);
  else if(session.value.difficulty==='expert'||session.value.mode==='puzzle')expertMove(side,apply);
  else apply(chooseGomokuMove([...state.value.board],session.value.difficulty==='deepseek'?'hard':session.value.difficulty,Math.random,side));
});

function commit() {
  if(session.value.mode==='puzzle'){
    if(state.value.status==='won'&&!session.value.done){session.value.done=true;finishLevel(session.value.progress,session.value.level,session.value.hintUsed?2:3,playerMoves.value);recordScore(300);}
    else save();
    return;
  }
  if (state.value.status !== "playing") recordScore(state.value.score);
  else save();
}
function place(index: number) {
  if (props.paused || aiTurn.value || review.value>=0 || session.value.done || puzzleFailed.value || (deepSeekMode.value&&!deepSeekEnabled.value)) return;
  cursor.value = index;
  if (state.value.board[index]) return;
  cancelThinking();
  if (placeGomoku(state.value, index, human.value)) { hint.value=-1;feedback.value="";aiElapsed = 0; commit(); }
}
function clickBoard(event: MouseEvent) {
  if (props.paused) return;
  const rect = (event.currentTarget as SVGElement).getBoundingClientRect();
  if (!rect.width || !rect.height) return;
  const x = Math.round(((event.clientX - rect.left) / rect.width * 360 - 19) / 23);
  const y = Math.round(((event.clientY - rect.top) / rect.height * 360 - 19) / 23);
  if (x >= 0 && x < 15 && y >= 0 && y < 15) place(y * 15 + x);
  surface.value?.focus();
}
function keydown(event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", " ", "Enter"].includes(event.key)) return;
  event.preventDefault();
  if (props.paused) return;
  const x = cursor.value % 15, y = Math.floor(cursor.value / 15);
  if (event.key === "ArrowLeft") cursor.value = y * 15 + Math.max(0, x - 1);
  if (event.key === "ArrowRight") cursor.value = y * 15 + Math.min(14, x + 1);
  if (event.key === "ArrowUp") cursor.value = Math.max(0, y - 1) * 15 + x;
  if (event.key === "ArrowDown") cursor.value = Math.min(14, y + 1) * 15 + x;
  if ([" ", "Enter"].includes(event.key) && !event.repeat) place(cursor.value);
}
function undo() { if(props.paused||session.value.done||(session.value.mode==='puzzle'&&playerMoves.value===0))return;cancelThinking();if(undoGomoku(state.value)){feedback.value='';hint.value=-1;aiElapsed=0;save();surface.value?.focus({preventScroll:true});} }
function restart() { if (props.paused) return; deepSeekEnabled.value=false;session.value.run=session.value.mode==="puzzle"?createGomokuPuzzle(session.value.level).run:createGomoku(human.value);session.value.done=false;session.value.hintUsed=false;feedback.value="";hint.value=-1;review.value=-1;save();aiElapsed = 0; cursor.value = 112; surface.value?.focus({ preventScroll: true }); }
function mode(value:string){if(props.paused)return;session.value.mode=value==="puzzle"?"puzzle":"endless";restart();}
function level(value:number){if(props.paused||value>session.value.progress.unlocked)return;session.value.level=value;restart();}
function difficulty(event:Event){if(props.paused)return;deepSeekEnabled.value=false;feedback.value='';session.value.difficulty=(event.target as HTMLSelectElement).value as GomokuDifficulty;save();}
function showHint(){if(props.paused||aiTurn.value||session.value.done||puzzleFailed.value)return;session.value.hintUsed=true;save();const reference=session.value.mode==='puzzle'?gomokuPuzzleContinuation(session.value.level,state.value):null;if(reference!==null)hint.value=reference;else expertMove(human.value,index=>{hint.value=index??-1;});}
function color(event:Event){if(props.paused)return;deepSeekEnabled.value=false;feedback.value='';session.value.run=createGomoku(Number((event.target as HTMLSelectElement).value)===2?2:1);review.value=-1;hint.value=-1;save();}
defineExpose({ restart });
</script>

<template>
  <div class="gomoku-game">
    <CampaignToolbar :mode="session.mode" :level="session.level" :levels="GOMOKU_PUZZLES" :stars="session.progress.stars" :unlocked="session.progress.unlocked" :disabled="paused" :modes="[{id:'endless',label:'自由对弈'},{id:'puzzle',label:'实战残局 · 30题'}]" @mode="mode" @select="level" />
    <p v-if="session.mode==='puzzle'" class="game-help">{{ puzzle.chapter }} · 执{{ human===1?'黑':'白' }}先行，在 {{ puzzle.turns }} 手内取胜 · 已用 {{ playerMoves }} 手。电脑会实际应对，连成五子才通关。</p>
    <div v-if="session.mode==='endless'" class="gomoku-options"><label>电脑 <select aria-label="电脑难度" :value="session.difficulty" :disabled="paused" @change="difficulty"><option v-for="entry in GOMOKU_DIFFICULTIES" :key="entry.id" :value="entry.id">{{ entry.name }}</option></select></label><label>新棋局执子 <select :value="human" :disabled="paused" @change="color"><option :value="1">黑棋 · 先手</option><option :value="2">白棋 · 后手</option></select></label></div>
    <p v-if="deepSeekMode" class="game-help gomoku-network">最高难度：DeepSeek 联网分析＋本地深搜校验，只发送棋盘和执子，会消耗 API 额度。<button v-if="!deepSeekEnabled && state.status==='playing'" class="button" :disabled="paused || networkPending" @click="deepSeekEnabled=true;feedback=''">开始 DeepSeek 对弈</button><span v-else>已启用 · 深搜最多约8秒</span></p>
    <div class="game-stats"><span>你的步数 <strong>{{ playerMoves }}</strong></span><span>本局得分 <strong>{{ displayedScore }}</strong></span><span>最高分 <strong>{{ best }}</strong></span></div>
    <div class="gomoku-players"><span><i :class="human===1?'black':'white'"/>你 · {{ human===1?'黑':'白' }}棋</span><b>VS</b><span><i :class="human===1?'white':'black'"/>{{ deepSeekMode?'DeepSeek':'电脑' }} · {{ human===1?'白':'黑' }}棋</span></div>
    <div ref="surface" class="game-surface gomoku-surface" tabindex="0" data-game-focus :aria-label="`五子棋棋盘，光标第 ${Math.floor(cursor / 15) + 1} 行第 ${cursor % 15 + 1} 列。方向键移动，空格或回车落子。${message}`" @keydown="keydown">
      <svg viewBox="0 0 360 360" class="gomoku-board" aria-hidden="true" @click="clickBoard">
        <defs><radialGradient id="gomoku-black" cx="30%" cy="25%"><stop stop-color="#68717b"/><stop offset=".65" stop-color="#252e39"/><stop offset="1" stop-color="#131c27"/></radialGradient><radialGradient id="gomoku-white" cx="30%" cy="25%"><stop stop-color="#fffefa"/><stop offset=".7" stop-color="#ebe8de"/><stop offset="1" stop-color="#c7c9c4"/></radialGradient></defs>
        <rect width="360" height="360" rx="12" fill="var(--board-bg)"/>
        <g stroke="var(--board-line)" stroke-width=".65"><template v-for="n in 15" :key="n"><path :d="`M19 ${19 + (n - 1) * 23}H341`"/><path :d="`M${19 + (n - 1) * 23} 19V341`"/></template></g>
        <g fill="var(--board-line)"><circle v-for="point in [48, 56, 112, 168, 176]" :key="point" :cx="19 + point % 15 * 23" :cy="19 + Math.floor(point / 15) * 23" r="2.4"/></g>
        <g v-for="(stone, index) in shownBoard" :key="index">
          <template v-if="stone"><circle :cx="20 + index % 15 * 23" :cy="21 + Math.floor(index / 15) * 23" r="10" fill="#0c17272b"/><circle :cx="19 + index % 15 * 23" :cy="19 + Math.floor(index / 15) * 23" r="9.7" :fill="stone === 1 ? 'url(#gomoku-black)' : 'url(#gomoku-white)'" :stroke="stone === 1 ? '#161f2c' : '#bcbdba'" stroke-width=".5"/></template>
          <circle v-if="line.includes(index)" :cx="19 + index % 15 * 23" :cy="19 + Math.floor(index / 15) * 23" r="11.2" fill="none" stroke="#de9553" stroke-width="1.8"/>
        </g>
        <rect v-if="lastMove >= 0" :x="16 + lastMove % 15 * 23" :y="16 + Math.floor(lastMove / 15) * 23" width="6" height="6" rx="1" fill="none" :stroke="state.board[lastMove] === 1 ? '#e1bd80' : '#536b80'" stroke-width="1.1"/>
        <rect class="gomoku-cursor" :x="8 + cursor % 15 * 23" :y="8 + Math.floor(cursor / 15) * 23" width="22" height="22" rx="4" fill="none" stroke="var(--primary)" stroke-width="1.5"/>
        <circle v-if="hint>=0" :cx="19+hint%15*23" :cy="19+Math.floor(hint/15)*23" r="7" fill="none" stroke="var(--success)" stroke-width="2.5"/>
      </svg>
    </div>
    <p class="gomoku-status" role="status" :class="{ won: state.status === 'won' }">{{ message }}</p>
    <p v-if="deepSeekMode&&aiNotice" class="game-help" role="status">{{ aiNotice }}</p>
    <div class="game-actions"><button class="button" :disabled="paused || !state.moves.length || state.status !== 'playing' || session.done || (session.mode==='puzzle'&&playerMoves===0)" @click="undo">悔一步</button><button class="button" :disabled="paused || aiTurn || session.done || thinking || puzzleFailed || review>=0" @click="showHint">落点提示</button><button class="button" :disabled="paused" @click="review=review<0?state.moves.length:-1">{{ review<0?'复盘':'返回对局' }}</button><button v-if="feedback" class="button" :disabled="paused" @click="feedback=''">重试思考</button><button v-if="session.done && session.level<29" class="button primary" :disabled="paused" @click="level(session.level+1)">下一题</button><button v-if="state.status !== 'playing'||puzzleFailed" class="button primary" :disabled="paused" @click="restart">重新挑战</button></div>
    <label v-if="review>=0" class="game-help gomoku-review">复盘第 {{ review }} 步 <input v-model.number="review" type="range" min="0" :max="state.moves.length" :disabled="paused"></label>
    <p class="game-help">点击交叉点落子，或方向键移动、空格落子。{{ session.mode==='puzzle' ? '完成整段杀法得300分；提示会随局面更新，使用提示后最高两星。' : '无禁手，先连成五子获胜。高手每步最多思考约2秒。胜局500−你的步数×10分（最低50）；平局50，败局0。' }}</p>
    <p v-if="storageError" class="game-help" role="alert">存档空间不可用，本局进度暂时无法保存。</p>
  </div>
</template>

<style scoped>
.gomoku-game{--board-bg:#c8b48e;--board-line:#73664d}:global(:root[data-theme="warm"]) .gomoku-game{--board-bg:#e0cda8;--board-line:#9a835d}
.gomoku-network{display:flex;flex-wrap:wrap;align-items:center;justify-content:center;gap:6px;white-space:normal;overflow-wrap:anywhere}.gomoku-network .button{font-size:11px;padding:5px 9px}.gomoku-network span{color:var(--success)}
.gomoku-options{display:flex;gap:12px;justify-content:center;margin-bottom:12px;font-size:11px;color:var(--muted)}.gomoku-options select{background:var(--surface-2);border:1px solid var(--line);border-radius:6px;padding:5px;color:var(--text);max-width:150px}.gomoku-review{display:flex;align-items:center;justify-content:center;gap:8px}
.gomoku-players{display:flex;align-items:center;justify-content:center;gap:20px;margin:-2px 0 10px;font-size:10px;color:var(--muted)}.gomoku-players span{display:flex;align-items:center;gap:6px}.gomoku-players b{font-weight:400;font-size:9px;opacity:.6}.gomoku-players i{width:13px;height:13px;border-radius:50%;box-shadow:0 1px 3px #0003}.gomoku-players .black{background:linear-gradient(135deg,#64707d,#17212d);border:1px solid #171f2b}.gomoku-players .white{background:linear-gradient(135deg,#fffdf6,#d7d5cc);border:1px solid #bebfb7}
.gomoku-surface{width:min(100%,350px);aspect-ratio:1;margin:0 auto;outline-offset:3px;border-radius:12px;box-shadow:inset 0 0 0 1px #fff2,0 8px 22px #0002}.gomoku-board{display:block;width:100%;height:100%;cursor:crosshair;touch-action:manipulation;border-radius:12px}.gomoku-cursor{opacity:0}.gomoku-surface:focus-visible .gomoku-cursor{opacity:1}.gomoku-status{text-align:center;min-height:20px;font-size:12px;color:var(--text);margin:11px 0 8px}.gomoku-status.won{color:var(--success)}
</style>
