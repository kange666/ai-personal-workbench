<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Component } from "vue";
import { restGames, type RestGameId } from "../../utils/rest/catalog";
import { restStorageWarning } from "../../composables/useGameSave";
import GameIcon from "./GameIcon.vue";
const props = defineProps<{ game: RestGameId; component: Component }>();
const emit = defineEmits<{ close: [] }>();
const info = computed(() => restGames.find(game => game.id === props.game)!);
const panel = ref<HTMLElement>();
const instance = ref<{ restart: () => void }>();
const paused = ref(false);
const resetting = ref(false);
let wasPaused = false;
let previousFocus: HTMLElement | null = null;
let appElement: HTMLElement | null = null;
let previousInert = false;
const focusGame = () => nextTick(() => panel.value?.querySelector<HTMLElement>("[data-game-focus]")?.focus());
function resume() { paused.value = false; void focusGame(); }
function requestRestart() { wasPaused = paused.value; paused.value = true; resetting.value = true; }
function cancelRestart() { resetting.value = false; paused.value = wasPaused; void focusGame(); }
async function restart() { resetting.value = false; paused.value = false; await nextTick(); instance.value?.restart(); void focusGame(); }
const pause = () => { paused.value = true; };
const visibility = () => { if (document.hidden) pause(); };
function keydown(event: KeyboardEvent) {
  // 游戏弹框内不让工作台全局快捷键打开背景弹层。
  if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "k" || (event.shiftKey && event.code === "Space"))) {
    event.preventDefault(); event.stopImmediatePropagation(); return;
  }
  if (event.key === "Escape") {
    event.preventDefault(); event.stopPropagation();
    if (resetting.value) cancelRestart(); else emit("close");
  }
  if (event.key !== "Tab") return;
  const focusable = [...panel.value!.querySelectorAll<HTMLElement>('button:not(:disabled), [tabindex="0"]')].filter(element => element.tabIndex >= 0 && !element.closest("[inert]"));
  const first = focusable[0], last = focusable.at(-1);
  if (event.shiftKey && (document.activeElement === first || !panel.value?.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
  else if (!event.shiftKey && (document.activeElement === last || !panel.value?.contains(document.activeElement))) { event.preventDefault(); first?.focus(); }
}
onMounted(() => {
  previousFocus = document.activeElement as HTMLElement;
  appElement = document.getElementById("app");
  previousInert = appElement?.inert ?? false;
  if (appElement) appElement.inert = true;
  window.addEventListener("keydown", keydown, true);
  window.addEventListener("blur", pause);
  document.addEventListener("visibilitychange", visibility);
  void focusGame();
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", keydown, true);
  window.removeEventListener("blur", pause);
  document.removeEventListener("visibilitychange", visibility);
  if (appElement) appElement.inert = previousInert;
  previousFocus?.focus();
});
</script>
<template>
  <Teleport to="body">
    <div class="rest-backdrop" @click.self="emit('close')">
      <section ref="panel" class="rest-dialog" role="dialog" aria-modal="true" :aria-labelledby="`rest-title-${game}`">
        <header class="rest-dialog-header">
          <span class="rest-dialog-mark" :class="`rest-color-${info.color}`"><GameIcon :name="game" /></span>
          <div><h2 :id="`rest-title-${game}`">{{ info.name }}</h2><small>进度自动保存 · 随时回来</small></div>
          <button class="icon-button" :title="paused ? '继续游戏' : '暂停游戏'" :aria-label="paused ? '继续游戏' : '暂停游戏'" :disabled="resetting" @click="paused ? resume() : pause()">{{ paused ? '▷' : 'Ⅱ' }}</button>
          <button class="icon-button" title="重新开始" aria-label="重新开始" @click="requestRestart">↻</button>
          <button class="icon-button" title="保存并关闭（Esc）" aria-label="保存并关闭" @click="emit('close')">×</button>
        </header>
        <p v-if="restStorageWarning" class="rest-storage-warning" role="status">{{ restStorageWarning }}</p>
        <div class="rest-game-body">
          <div :inert="paused ? true : undefined"><component :is="component" ref="instance" :paused="paused" /></div>
          <div v-if="paused" class="rest-pause-cover">
            <template v-if="resetting"><span class="rest-pause-symbol">↻</span><h3>重新开始这一轮？</h3><p>当前进度会重置，最高分保留。</p><div class="game-actions"><button class="button" @click="cancelRestart">继续当前进度</button><button class="button primary" @click="restart">确认重新开始</button></div></template>
            <template v-else><span class="rest-pause-symbol">Ⅱ</span><h3>休息一下也没关系</h3><p>游戏已暂停，进度留在这里。</p><button class="button primary" @click="resume">继续游戏</button></template>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
