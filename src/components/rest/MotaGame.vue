<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { discardMotaPlayer, getMotaPlayer, hideMotaPlayer, positionMotaPlayer, type MotaPlayer } from "../../services/motaPlayer";

const props = defineProps<{ paused: boolean }>();
const container = ref<HTMLElement>();
const loading = ref(true);
const error = ref("");
let player: MotaPlayer | null = null;
let disposed = false;
let generation = 0;
let resizeObserver: ResizeObserver | null = null;

function positionPlayer() {
  if (player && !props.paused && container.value) {
    const bounds = container.value.closest(".rest-dialog")?.getBoundingClientRect();
    positionMotaPlayer(container.value.getBoundingClientRect(), bounds);
  }
}
function focusPlayer() { if (!props.paused) player?.focus(); }

async function open() {
  const current = ++generation;
  loading.value = true; error.value = "";
  try {
    const loaded = await getMotaPlayer();
    if (disposed || current !== generation) return;
    player = loaded;
    loading.value = false;
    props.paused ? player.ruffle().suspend() : player.ruffle().resume();
    await nextTick();
    if (!disposed && current === generation) { positionPlayer(); focusPlayer(); }
  } catch {
    if (!disposed && current === generation) { loading.value = false; error.value = "魔塔加载失败，请重试。"; }
  }
}
function restart() {
  if (props.paused) return;
  player?.ruffle().suspend(); hideMotaPlayer(); player = null;
  discardMotaPlayer(); void open();
}
watch(() => props.paused, paused => {
  if (!player) return;
  if (paused) { player.ruffle().suspend(); hideMotaPlayer(); }
  else { player.ruffle().resume(); positionPlayer(); focusPlayer(); }
});
onMounted(() => {
  if (typeof ResizeObserver !== "undefined") { resizeObserver = new ResizeObserver(positionPlayer); if (container.value) resizeObserver.observe(container.value); }
  window.addEventListener("resize", positionPlayer);
  window.addEventListener("scroll", positionPlayer, true);
  void open();
});
onBeforeUnmount(() => {
  disposed = true; generation++; player?.ruffle().suspend(); hideMotaPlayer();
  resizeObserver?.disconnect(); window.removeEventListener("resize", positionPlayer); window.removeEventListener("scroll", positionPlayer, true);
});
defineExpose({ restart });
</script>

<template>
  <div class="mota-game">
    <div ref="container" class="mota-surface" tabindex="0" data-game-focus aria-label="魔塔游戏区域" @focus="focusPlayer">
      <p v-if="loading" class="mota-loading" role="status">正在加载经典魔塔…</p>
      <div v-if="error" class="mota-loading" role="alert"><p>{{ error }}</p><button class="button" @click="open">重新加载</button></div>
    </div>
    <p class="game-help">方向键移动 · 空格确认对话 · 点击游戏画面后操作</p>
    <p class="game-help">原版没有持久存盘。关闭弹窗后可续玩，退出工作台后需重新开始。</p>
  </div>
</template>

<style scoped>
.mota-surface{position:relative;width:100%;max-width:calc((100dvh - 250px)*1.390244);margin-inline:auto;aspect-ratio:57/41;overflow:hidden;background:#000;border-radius:8px;outline-offset:3px}
.mota-surface :deep(.mota-player){display:block;width:100%;height:100%}
.mota-loading{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#ddd;margin:0;gap:12px}
.game-help{margin:10px 0 0}
</style>
