<script setup lang="ts">
import type { RestGameId } from "../../utils/rest/catalog";
import GameDialog from "./GameDialog.vue";
import WiringGame from "./WiringGame.vue";
import BreakoutGame from "./BreakoutGame.vue";
import TowerGame from "./TowerGame.vue";
import MotaGame from "./MotaGame.vue";
import SokobanGame from "./SokobanGame.vue";
import GomokuGame from "./GomokuGame.vue";
import "../../styles/rest.css";

defineProps<{ game: RestGameId }>();
defineEmits<{ close: [] }>();
const components = { wiring: WiringGame, breakout: BreakoutGame, tower: TowerGame, mota: MotaGame, sokoban: SokobanGame, gomoku: GomokuGame };
</script>

<template>
  <GameDialog :game="game" :component="components[game]" @close="$emit('close')" />
</template>
