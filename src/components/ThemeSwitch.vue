<script setup lang="ts">
import { useWorkbenchStore } from "../stores/workbench";

const store = useWorkbenchStore();
</script>

<template>
  <div class="theme-switch" aria-label="切换主题颜色">
    <button :class="{ active: store.theme === 'command' }" @click="store.setTheme('command')">深色</button>
    <button :class="{ active: store.theme === 'warm' }" @click="store.setTheme('warm')">暖色</button>
  </div>
</template>
