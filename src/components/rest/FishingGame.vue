<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { useGameSave } from "../../composables/useGameSave";
import { useGameLoop } from "../../composables/useGameLoop";
import { actFishing, advanceFishing, createFishing, FISH_SPECIES, isFishingState, reelPointer, reelTarget } from "../../utils/rest/fishing";

const props = defineProps<{ paused: boolean }>();
const { state, best, save, recordScore, restart: resetSave, storageError } = useGameSave("fishing", createFishing, isFishingState);
const surface = ref<HTMLElement>();
const uid = useId().replace(/:/g, "");
const species = computed(() => state.value.fish ? FISH_SPECIES[state.value.fish.species] : null);
const pointer = computed(() => reelPointer(state.value));
const target = computed(() => reelTarget(state.value));
const active = computed(() => ["waiting", "bite", "reeling"].includes(state.value.phase));
// 恢复进行中的鱼竿时先等玩家准备，避免刚打开弹框就错过咬钩。
const resumeReady = ref(active.value);
const bobY = computed(() => 192 + (state.value.phase === "bite" ? 5 : Math.sin(state.value.elapsed * 3) * 2));
const status = computed(() => resumeReady.value ? "鱼竿和进度都在，准备好再继续。" : ({
  idle: "湖面很静，抛一竿试试。",
  waiting: "浮漂轻晃，耐心等鱼上钩…",
  bite: "鱼咬钩了！快提竿！",
  reeling: `指针进入绿色区间时收线 · ${state.value.reelHits} / 3`,
  caught: `${species.value?.name} ${state.value.fish?.weight.toFixed(2)} kg · +${state.value.lastPoints} 分`,
  escaped: state.value.escapeReason === "miss" ? "收线时机偏了，鱼儿游走了。" : "鱼儿等不及，游回了湖中。",
}[state.value.phase]));
const actionLabel = computed(() => resumeReady.value ? "准备好了，继续" : ({ idle: "抛竿", waiting: "等待咬钩", bite: "立即提竿", reeling: "收线", caught: "继续垂钓", escaped: "再抛一竿" }[state.value.phase]));
let saveElapsed = 0;

useGameLoop(() => props.paused || resumeReady.value, dt => {
  if (!active.value) return;
  const changed = advanceFishing(state.value, dt);
  saveElapsed += dt;
  if (changed || saveElapsed >= 1) { saveElapsed = 0; save(); }
});

function act() {
  if (props.paused) return;
  if (resumeReady.value) { resumeReady.value = false; surface.value?.focus({ preventScroll: true }); return; }
  const previousScore = state.value.score;
  if (!actFishing(state.value)) return;
  if (state.value.score !== previousScore) recordScore(state.value.score);
  else save();
  surface.value?.focus({ preventScroll: true });
}

function keydown(event: KeyboardEvent) {
  if (event.target instanceof HTMLButtonElement || ![" ", "Enter"].includes(event.key)) return;
  event.preventDefault();
  if (!event.repeat) act();
}

function restart() { if (props.paused) return; resetSave(); resumeReady.value = false; saveElapsed = 0; surface.value?.focus({ preventScroll: true }); }
defineExpose({ restart });
</script>

<template>
  <div class="fishing-game">
    <div class="game-stats">
      <span>本次鱼获 <strong>{{ state.caught }} 尾</strong></span>
      <span>累计得分 <strong>{{ state.score }}</strong></span>
      <span>最高分 <strong>{{ best }}</strong></span>
    </div>
    <div ref="surface" class="game-surface fishing-surface" tabindex="0" data-game-focus aria-label="休闲钓鱼，空格或回车抛竿、提竿和收线" @keydown="keydown">
      <svg class="fishing-scene" viewBox="0 0 480 290" aria-hidden="true">
        <defs>
          <linearGradient :id="`${uid}-sky`" x2="0" y2="1"><stop stop-color="var(--lake-sky)"/><stop offset="1" stop-color="var(--lake-horizon)"/></linearGradient>
          <linearGradient :id="`${uid}-water`" x2="0" y2="1"><stop stop-color="var(--lake-water)"/><stop offset="1" stop-color="var(--lake-deep)"/></linearGradient>
          <radialGradient :id="`${uid}-glow`"><stop stop-color="#f1e1b1" stop-opacity=".22"/><stop offset="1" stop-color="#f1e1b1" stop-opacity="0"/></radialGradient>
        </defs>
        <rect width="480" height="290" :fill="`url(#${uid}-sky)`"/>
        <circle cx="351" cy="63" r="60" :fill="`url(#${uid}-glow)`"/>
        <circle cx="351" cy="63" r="19" fill="#f1e1b1"/>
        <g fill="var(--lake-star)" opacity=".7"><circle cx="69" cy="42" r="1"/><circle cx="199" cy="29" r="1.2"/><circle cx="275" cy="71" r="1"/><circle cx="424" cy="39" r="1.1"/></g>
        <path d="M0 151L57 94 113 143 176 103 247 155 325 121 392 158 458 103 480 117V200H0Z" fill="var(--lake-mountain)"/>
        <path d="M0 164Q55 133 121 159T244 159T360 157T480 162V200H0Z" fill="var(--lake-bank)"/>
        <rect y="166" width="480" height="124" :fill="`url(#${uid}-water)`"/>
        <g stroke="#f1e1b1" stroke-linecap="round" opacity=".22"><path d="M334 181H367M326 191H374M338 202H359M317 214H378M336 226H365"/></g>
        <g fill="none" stroke="var(--lake-ripple)" opacity=".38"><path d="M38 195H98M101 241H174M260 266H338M391 230H448M26 274H70"/><ellipse cx="274" cy="198" :rx="state.phase === 'bite' ? 24 : 16" ry="4"/><ellipse cx="274" cy="198" rx="30" ry="7" opacity=".4"/></g>
        <path d="M0 241L84 229 104 247 0 267Z" fill="#8d7054"/><path d="M0 250L104 247V256L0 278Z" fill="#5d4b3b"/>
        <g stroke="#604d3d" stroke-width="2"><path d="M22 239L31 258M47 236L59 254M72 232L88 251"/></g>
        <path d="M57 226Q124 132 168 124" stroke="#cab082" stroke-width="4" fill="none" stroke-linecap="round"/>
        <path v-if="state.phase !== 'idle'" :d="`M168 124Q235 121 274 ${bobY}`" stroke="#c1d8df" stroke-opacity=".65" fill="none"/>
        <g v-if="state.phase !== 'idle'" :transform="`translate(274,${bobY})`"><path d="M0-11V4" stroke="#f4e6c7" stroke-width="2"/><ellipse cy="-3" rx="3" ry="6" :fill="state.phase === 'bite' ? '#ffbe72' : '#e88b7f'"/></g>
        <text v-if="state.phase === 'bite'" x="274" y="169" text-anchor="middle" fill="#ffce8a" font-size="25" font-weight="700">!</text>
      </svg>
      <div v-if="state.phase === 'caught'" class="catch-card">
        <svg viewBox="0 0 80 46" aria-hidden="true" :style="{ color: species?.color }"><path d="M22 23C36 3 65 7 72 23 65 39 36 43 22 23L7 10V36Z" fill="currentColor"/><circle cx="61" cy="20" r="2.3" fill="#25394a"/><path d="M44 9L34 2 33 15M41 35L32 43 31 29" fill="currentColor" opacity=".7"/></svg>
        <strong>{{ species?.name }}</strong><span>{{ state.fish?.weight.toFixed(2) }} kg · +{{ state.lastPoints }} 分</span>
      </div>
      <div v-if="resumeReady" class="fishing-resume-card"><strong>继续湖边的时光</strong><span>计时已暂停，等你准备好</span></div>
      <div v-if="state.phase === 'reeling'" class="reel-panel">
        <div class="reel-heading"><span>保持节奏，收线三次</span><span>{{ Math.max(0, 9 - state.reelElapsed).toFixed(1) }}s</span></div>
        <div class="reel-track"><i :style="{ left: `${target.start * 100}%`, width: `${target.width * 100}%` }"/><b :style="{ left: `${pointer * 100}%` }"/></div>
        <div class="reel-steps"><i v-for="step in 3" :key="step" :class="{ complete: state.reelHits >= step }"/></div>
      </div>
      <div v-if="state.phase === 'bite'" class="bite-meter" aria-hidden="true"><i :style="{ transform: `scaleX(${Math.max(0, 1 - state.elapsed / 2.4)})` }"/></div>
    </div>
    <p class="fishing-status" :class="{ urgent: state.phase === 'bite', success: state.phase === 'caught' }" role="status">{{ status }}</p>
    <div class="game-actions"><button class="button primary" :disabled="paused || (!resumeReady && state.phase === 'waiting')" @click="act">{{ actionLabel }}</button><span class="fishing-shortcut">空格 / 回车</span></div>
    <div class="fish-collection" aria-label="本次鱼获图鉴"><span v-for="(fish, index) in FISH_SPECIES" :key="fish.name" :class="{ discovered: state.collection[index] }" :title="`${fish.name}：${state.collection[index]} 尾`"><i :style="{ background: fish.color }"/>{{ fish.name }}<b>{{ state.collection[index] }}</b></span></div>
    <p class="game-help">等咬钩时提竿，再在绿色区间收线 3 次。累计鱼获分自动续存，重新开始会清空本次鱼获，保留最高分。</p>
    <p v-if="storageError" class="game-help" role="alert">存档空间不可用，本次进度暂时无法保存。</p>
  </div>
</template>

<style scoped>
.fishing-game{--lake-sky:#172c47;--lake-horizon:#627c88;--lake-water:#335a72;--lake-deep:#1a354d;--lake-mountain:#344c63;--lake-bank:#233f4b;--lake-ripple:#aacbd0;--lake-star:#e3e4e8}
:global(:root[data-theme="warm"]) .fishing-game{--lake-sky:#9ec4c7;--lake-horizon:#f3d6b0;--lake-water:#80afb5;--lake-deep:#497e89;--lake-mountain:#92a9a0;--lake-bank:#648e7f;--lake-ripple:#e0eadd;--lake-star:#f8ecd7}
.fishing-surface{position:relative;max-width:480px;width:100%;margin:0 auto;overflow:hidden;border-radius:14px;outline-offset:3px;aspect-ratio:48/29}
.fishing-scene{display:block;width:100%;height:100%}.fishing-status{min-height:21px;margin:12px 0 9px;text-align:center;color:var(--text);font-size:12px}.fishing-status.urgent{color:var(--warning);font-weight:700}.fishing-status.success{color:var(--success)}
.fishing-shortcut{font-size:10px;color:var(--muted);align-self:center}.catch-card{position:absolute;left:50%;top:22px;transform:translateX(-50%);display:flex;flex-direction:column;align-items:center;gap:4px;min-width:160px;padding:11px 18px;border:1px solid #eff5ee33;background:#173144db;color:#f0eee3;border-radius:14px;box-shadow:0 8px 25px #001c3433}.catch-card svg{width:70px;height:40px}.catch-card strong{font-size:15px}.catch-card span{font-size:11px;color:#d8e4de}
.reel-panel{position:absolute;left:10%;right:10%;bottom:17px;padding:11px 14px;background:#142b3fe8;border:1px solid #dde8f026;border-radius:11px;color:#e7efed}.reel-heading{display:flex;justify-content:space-between;margin-bottom:10px;font-size:10px}.reel-track{position:relative;height:14px;background:#b7c8d529;border-radius:7px}.reel-track i{position:absolute;height:100%;background:#59c99e;border-radius:5px;box-shadow:0 0 12px #59c99e33}.reel-track b{position:absolute;top:-4px;width:4px;height:22px;border-radius:2px;background:#fff9e9;transform:translateX(-50%);box-shadow:0 0 5px #ffffdd66}.reel-steps{display:flex;justify-content:center;gap:5px;margin-top:10px}.reel-steps i{width:20px;height:3px;border-radius:2px;background:#ffffff30}.reel-steps i.complete{background:#59c99e}
.bite-meter{position:absolute;height:4px;left:25%;right:25%;bottom:17px;background:#142b3f99;border-radius:3px;overflow:hidden}.bite-meter i{display:block;width:100%;height:100%;transform-origin:left;background:#ffcc85}
.fishing-resume-card{position:absolute;left:50%;top:28px;transform:translateX(-50%);display:flex;flex-direction:column;align-items:center;gap:7px;white-space:nowrap;padding:14px 20px;border:1px solid #eff5ee33;background:#173144e8;color:#f0eee3;border-radius:12px}.fishing-resume-card strong{font-size:14px}.fishing-resume-card span{font-size:10px;color:#d8e4de}
.fish-collection{display:flex;justify-content:center;gap:7px;flex-wrap:wrap;margin:15px 0 8px}.fish-collection span{display:flex;align-items:center;gap:4px;font-size:9px;color:var(--muted);opacity:.62}.fish-collection span.discovered{opacity:1;color:var(--text)}.fish-collection i{width:7px;height:7px;border-radius:50%}.fish-collection b{font-weight:500;color:var(--muted)}
</style>
