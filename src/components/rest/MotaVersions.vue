<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import MotaGame from "./MotaGame.vue";
import Mota50Game from "./Mota50Game.vue";
defineProps<{ paused: boolean }>();
const selected = ref<"21" | "50" | null>(null);
const instance = ref<{ restart(): void }>();
const component = computed(() => selected.value === "21" ? MotaGame : Mota50Game);
const selection = ref<HTMLElement>();
async function back() { selected.value = null; await nextTick(); selection.value?.querySelector<HTMLElement>("button")?.focus(); }
defineExpose({ restart: () => instance.value?.restart() });
</script>
<template>
  <div class="mota-versions">
    <div v-if="!selected" ref="selection" class="mota-version-grid" aria-label="选择魔塔版本">
      <button class="mota-version" data-game-focus @click="selected = '21'"><span>21</span><strong>经典 21 层</strong><small>原版 Flash · 关闭弹窗可续玩</small></button>
      <button class="mota-version" @click="selected = '50'"><span>50</span><strong>经典 50 层</strong><small>完整楼层 · 支持本机存档与读档</small></button>
    </div>
    <template v-else>
      <div class="mota-version-bar"><strong>经典 {{ selected }} 层</strong><button class="button" @click="back">选择版本</button></div>
      <component :is="component" ref="instance" :paused="paused" />
    </template>
  </div>
</template>
<style scoped>
.mota-version-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px;padding:12px 0}
.mota-version{font:inherit;cursor:pointer;color:var(--text);background:var(--surface);border:1px solid var(--line);border-radius:14px;padding:24px 12px;display:flex;flex-direction:column;align-items:center;gap:12px}
.mota-version:hover{border-color:var(--primary);background:color-mix(in srgb,var(--primary) 6%,var(--surface))}
.mota-version span{font-size:36px;line-height:1;font-weight:650;color:var(--primary);font-variant-numeric:tabular-nums}
.mota-version strong{font-size:15px}.mota-version small{font-size:11px;color:var(--muted);line-height:1.6;text-align:center}
.mota-version-bar{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-bottom:12px;font-size:13px}
.mota-version-bar .button{min-height:30px}
@media(max-width:480px){.mota-version-grid{gap:8px}.mota-version{padding:20px 8px}}
</style>
