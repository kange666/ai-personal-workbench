<script setup lang="ts">
defineProps<{ name: "minimize" | "maximize" | "restore" | "close" }>();
</script>

<template>
  <svg class="window-control-icon" viewBox="0 0 16 16" aria-hidden="true">
    <path v-if="name === 'minimize'" d="M3 8.5h10" />
    <rect v-else-if="name === 'maximize'" x="3.5" y="3.5" width="9" height="9" rx="0.75" />
    <template v-else-if="name === 'restore'">
      <path d="M5.5 5.5V3.75h6.75v6.75H10.5" />
      <rect x="3.5" y="5.5" width="7" height="7" rx="0.75" />
    </template>
    <template v-else>
      <path d="m4 4 8 8" />
      <path d="m12 4-8 8" />
    </template>
  </svg>
</template>
