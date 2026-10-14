<script setup lang="ts">
import type { RestGameId } from "../../utils/rest/catalog";
defineProps<{ name: RestGameId }>();
</script>
<template>
  <svg class="rest-game-icon" viewBox="0 0 64 64" fill="none" aria-hidden="true">
    <g v-if="name === 'wiring'" stroke="currentColor" stroke-width="3.5" stroke-linecap="round" stroke-linejoin="round">
      <path d="M14 16h18v32h18M14 48h8M42 16h8" opacity=".3"/><path d="M14 16h18v16h18"/>
      <circle cx="14" cy="16" r="5" fill="var(--surface)"/><circle cx="50" cy="32" r="5" fill="currentColor"/><circle cx="50" cy="48" r="4"/><circle cx="14" cy="48" r="4"/>
    </g>
    <g v-else-if="name === 'breakout'" fill="currentColor">
      <rect x="8" y="12" width="14" height="8" rx="2" opacity=".45"/><rect x="25" y="12" width="14" height="8" rx="2"/><rect x="42" y="12" width="14" height="8" rx="2" opacity=".65"/><rect x="8" y="23" width="14" height="8" rx="2"/><rect x="42" y="23" width="14" height="8" rx="2" opacity=".4"/>
      <circle cx="34" cy="38" r="4"/><path d="m36 33 4-7" stroke="currentColor" stroke-width="2" stroke-linecap="round" opacity=".5"/><rect x="19" y="50" width="26" height="5" rx="2.5"/>
    </g>
    <g v-else-if="name === 'tower'" fill="currentColor" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round">
      <path d="m9 43 25-9 21 9-25 9z" opacity=".35"/><path d="M9 43v7l21 8 25-9v-6L30 52Z" opacity=".6"/><path d="m14 30 20-8 17 7-21 8z" opacity=".7"/><path d="M14 30v7l16 7 21-8v-7l-21 8Z" opacity=".45"/><path d="m20 14 16-6 13 5-16 7z"/><path d="M20 14v6l13 6 16-7v-6l-16 7Z" opacity=".7"/>
    </g>
    <g v-else-if="name === 'mota'" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <path d="M15 55V24h34v31M21 24V13h6v6h10v-6h6v11M10 55h44M26 55V42a6 6 0 0 1 12 0v13" />
      <path d="M25 29v4M39 29v4"/><path d="m32 5 2 4 4 2-4 2-2 4-2-4-4-2 4-2z" fill="currentColor" stroke-width="1" opacity=".6"/>
    </g>
    <g v-else-if="name === 'sokoban'" stroke="currentColor" stroke-width="3" stroke-linejoin="round">
      <path d="m16 18 17-8 18 8-18 9z" fill="currentColor" opacity=".3"/><path d="M16 18v25l17 10 18-10V18L33 27Z"/><path d="M33 27v26M24 15l17 9v9"/><path d="M6 32h16m-5-5 5 5-5 5" stroke-linecap="round"/>
    </g>
    <g v-else-if="name==='archery'" stroke="currentColor" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="16" cy="14" r="5"/><path d="M16 19v18m0-10 14-4M16 37l-7 17m7-17 10 17M34 9q22 20 0 40L34 9M27 29h30m-6-5 6 5-6 5"/><path d="M41 13q9 12 2 23" opacity=".3"/></g>
    <g v-else-if="name==='defense'" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"><circle cx="32" cy="33" r="24" stroke-dasharray="3 5" opacity=".35"/><path d="m32 13 13 9-4 30H23l-4-30Z" fill="currentColor" opacity=".3"/><path d="m32 13 13 9-4 30H23l-4-30Z"/><circle cx="32" cy="31" r="6"/><path d="M32 5v7M55 33h-7M16 33H9M32 54v7" stroke-linecap="round"/></g>
    <g v-else stroke="currentColor" stroke-width="1.5"><path d="M12 12h40M12 25h40M12 38h40M12 51h40M12 12v40M25 12v40M38 12v40M51 12v40" opacity=".25"/><circle cx="25" cy="25" r="6" fill="currentColor"/><circle cx="38" cy="38" r="6" fill="currentColor"/><circle cx="25" cy="38" r="6" fill="var(--surface)"/><circle cx="38" cy="25" r="6" fill="var(--surface)"/><circle cx="51" cy="51" r="6" fill="currentColor"/></g>
  </svg>
</template>
