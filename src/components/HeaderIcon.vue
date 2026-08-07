<script setup lang="ts">
defineProps<{ name: "quota" | "notification" | "refresh" }>();
</script>

<template>
  <svg class="header-icon" viewBox="0 0 24 24" aria-hidden="true">
    <template v-if="name === 'quota'">
      <circle cx="12" cy="12" r="8.25" />
      <path d="M12 7.25v4.9l3.2 1.85" />
    </template>
    <template v-else-if="name === 'notification'">
      <path d="M6.5 10.25a5.5 5.5 0 0 1 11 0c0 5.25 2.25 5.25 2.25 6.5H4.25c0-1.25 2.25-1.25 2.25-6.5Z" />
      <path d="M9.75 19a2.5 2.5 0 0 0 4.5 0" />
    </template>
    <template v-else>
      <path d="M19 8.25V4.5l-2 2A7.5 7.5 0 1 0 19.2 15" />
      <path d="M19 4.5h-4" />
    </template>
  </svg>
</template>
