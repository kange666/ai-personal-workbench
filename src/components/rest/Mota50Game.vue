<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { commandMota50, discardMota50Frame, getMota50Frame, hideMota50Frame, mota50Channel, positionMota50Frame } from "../../services/mota50Frame";
const props = defineProps<{ paused: boolean }>();
const surface = ref<HTMLElement>();
const ready = ref(false), error = ref("");
let frame: HTMLIFrameElement | null = null;
let disposed = false;
let observer: ResizeObserver | null = null;
let timeout: ReturnType<typeof setTimeout>;
function position() {
  if (ready.value && !props.paused && surface.value) {
    positionMota50Frame(surface.value.getBoundingClientRect(), surface.value.closest(".rest-dialog")?.getBoundingClientRect());
  }
}
function focus() { if (!disposed && ready.value && !props.paused) frame?.focus(); }
function sync() {
  if (disposed || !ready.value) return;
  if (props.paused) hideMota50Frame();
  else { commandMota50("resume"); position(); focus(); }
}
function onMessage(event: MessageEvent) {
  if (disposed || event.source !== frame?.contentWindow || event.origin !== window.location.origin || event.data?.channel !== mota50Channel) return;
  if (event.data.type === "ready") {
    clearTimeout(timeout); ready.value = true; error.value = ""; void nextTick(sync);
  } else if (event.data.type === "error") {
    ready.value = false; hideMota50Frame();
    error.value = typeof event.data.message === "string" ? event.data.message.slice(0, 200) : "游戏加载失败，请重试。";
  } else if (event.data.type === "escape" || event.data.type === "tab") {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: event.data.type === "escape" ? "Escape" : "Tab", shiftKey: event.data.shiftKey === true, cancelable: true }));
  }
}
function checkStatus() { commandMota50("status"); }
function open() {
  clearTimeout(timeout); ready.value = false; error.value = "";
  frame = getMota50Frame(); frame.addEventListener("load", checkStatus);
  checkStatus();
  timeout = setTimeout(() => { if (!ready.value) error.value = "加载超时，请重新加载。已有存档会保留。"; }, 30000);
}
function restart() {
  frame?.removeEventListener("load", checkStatus); discardMota50Frame(); open();
}
watch(() => props.paused, sync);
onMounted(() => {
  window.addEventListener("message", onMessage); window.addEventListener("resize", position); window.addEventListener("scroll", position, true);
  if (typeof ResizeObserver !== "undefined") { observer = new ResizeObserver(position); if (surface.value) observer.observe(surface.value); }
  open();
});
onBeforeUnmount(() => {
  disposed = true;
  clearTimeout(timeout); frame?.removeEventListener("load", checkStatus); hideMota50Frame(); observer?.disconnect();
  window.removeEventListener("message", onMessage); window.removeEventListener("resize", position); window.removeEventListener("scroll", position, true);
});
defineExpose({ restart });
</script>
<template>
  <div class="mota50-game">
    <div class="mota50-actions">
      <button class="button" :disabled="!ready || paused" @click="commandMota50('save')">存档</button>
      <button class="button" :disabled="!ready || paused" @click="commandMota50('load')">读档</button>
      <small>存档保存在本机，重开游戏也会保留。</small>
    </div>
    <div ref="surface" class="mota50-surface" tabindex="0" data-game-focus aria-label="50 层魔塔游戏区域" @focus="focus">
      <div v-if="error" class="mota50-loading" role="alert"><p>{{ error }}</p><button class="button" @click="restart">重新加载</button></div>
      <p v-else-if="!ready" class="mota50-loading" role="status">正在加载经典 50 层魔塔…</p>
    </div>
    <p class="game-help">方向键移动 · 空格确认 · 退出工作台前请存档，再次游玩点击读档</p>
  </div>
</template>
<style scoped>
.mota50-surface{position:relative;width:100%;max-width:calc((100dvh - 280px)*1.333333);margin-inline:auto;aspect-ratio:4/3;overflow:hidden;background:#000;border-radius:8px;outline-offset:3px}
.mota50-actions{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-bottom:10px}
.mota50-actions small{font-size:11px;color:var(--muted)}
.mota50-actions .button{min-height:30px}
.mota50-loading{position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;color:#ddd;margin:0;gap:12px;text-align:center;padding:14px}
</style>
