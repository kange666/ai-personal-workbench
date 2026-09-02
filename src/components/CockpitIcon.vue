<script setup lang="ts">
defineProps<{ name: string }>();
</script>

<template>
  <svg class="cockpit-icon" viewBox="0 0 24 24" aria-hidden="true">
    <template v-if="name === 'project'">
      <path d="M3.5 7.5h6l2-2h9v13h-17z" />
      <path d="M3.5 9.5h17" />
    </template>
    <template v-else-if="name === 'clock'">
      <circle cx="12" cy="12" r="8.5" />
      <path d="M12 7.5v5l3.5 2" />
    </template>
    <template v-else-if="name === 'chat'">
      <path d="M4 5.5h16v11H9l-4.5 3v-3H4z" />
      <path d="M8 10.8h.01M12 10.8h.01M16 10.8h.01" />
    </template>
    <template v-else-if="name === 'token'">
      <circle cx="12" cy="12" r="8.5" />
      <circle cx="12" cy="12" r="4" />
      <path d="M12 3.5v4M20.5 12h-4M12 20.5v-4M3.5 12h4" />
    </template>
    <template v-else-if="name === 'test'">
      <path d="M9 3.5h6M10 3.5v5l-5.5 9a2 2 0 0 0 1.7 3h11.6a2 2 0 0 0 1.7-3l-5.5-9v-5" />
      <path d="m8.5 15 2 2 4.5-5" />
    </template>
    <template v-else-if="name === 'report'">
      <path d="M6 3.5h8l4 4v13H6z" />
      <path d="M14 3.5v4h4M9 12h6M9 15.5h6" />
    </template>
    <template v-else-if="name === 'warning'">
      <path d="M12 3.5 21 20H3z" />
      <path d="M12 9v5M12 17.2h.01" />
    </template>
    <template v-else-if="name === 'deploy'">
      <path d="M12 20.5v-12M7.5 13 12 8.5l4.5 4.5" />
      <path d="M5 4.5h14" />
    </template>
    <template v-else>
      <circle cx="12" cy="12" r="8.5" />
      <path d="m8 12 2.6 2.6L16.5 9" />
    </template>
  </svg>
</template>

<style scoped>
.cockpit-icon {
  width: 1em;
  height: 1em;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
