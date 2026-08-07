<script setup lang="ts">
defineProps<{ name: string }>();
</script>

<template>
  <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
    <template v-if="name === 'home'"><path d="M3.5 10.5 12 3.8l8.5 6.7"/><path d="M5.8 9.3v10.2h12.4V9.3"/><path d="M9.5 19.5v-6h5v6"/></template>
    <template v-else-if="name === 'records'"><rect x="4" y="3.5" width="16" height="17" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/></template>
    <template v-else-if="name === 'projects'"><path d="M4 7.5h6l1.7 2H20v8.8a2.2 2.2 0 0 1-2.2 2.2H6.2A2.2 2.2 0 0 1 4 18.3z"/><path d="M4 7.5V6.2A2.2 2.2 0 0 1 6.2 4h3.2l1.7 2H18"/></template>
    <template v-else-if="name === 'calendar'"><rect x="3.5" y="5" width="17" height="15.5" rx="2"/><path d="M7.5 3v4M16.5 3v4M3.5 9h17M8 13h.01M12 13h.01M16 13h.01M8 17h.01M12 17h.01"/></template>
    <template v-else-if="name === 'reports'"><path d="M6 3.5h9l3 3v14H6z"/><path d="M14.5 3.5V7H18M9 11h6M9 15h6M9 18h4"/></template>
    <template v-else-if="name === 'testing'"><path d="M9 3.5h6M10 3.5v5l-5.3 8.3A2.4 2.4 0 0 0 6.7 20h10.6a2.4 2.4 0 0 0 2-3.2L14 8.5v-5"/><path d="M7.5 15h9"/></template>
    <template v-else-if="name === 'tokens'"><circle cx="12" cy="12" r="8.5"/><path d="M12 7.5v9M8.8 10c0-1.4 1.3-2.3 3.2-2.3s3.2.9 3.2 2.3-1.1 2-3.2 2-3.2.7-3.2 2 1.3 2.3 3.2 2.3 3.2-.9 3.2-2.3"/></template>
    <template v-else-if="name === 'tapd'"><path d="M4 5h16M12 5v14M7.5 19h9"/></template>
    <template v-else-if="name === 'content'"><path d="m12 3 1.5 4.3L18 9l-4.5 1.7L12 15l-1.5-4.3L6 9l4.5-1.7z"/><path d="m18.5 15 .8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z"/></template>
    <template v-else-if="name === 'videos'"><rect x="3.5" y="5" width="17" height="14" rx="2"/><path d="m10 9 5 3-5 3z"/></template>
    <template v-else-if="name === 'knowledge'"><path d="M4 5.5A3.5 3.5 0 0 1 7.5 2H12v18H7.5A3.5 3.5 0 0 0 4 23z"/><path d="M20 5.5A3.5 3.5 0 0 0 16.5 2H12v18h4.5A3.5 3.5 0 0 1 20 23z"/></template>
    <template v-else-if="name === 'settings'"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1z"/></template>
  </svg>
</template>
