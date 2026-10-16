<script setup lang="ts">
import type { RestGameId } from "../../utils/rest/catalog";
import GameDialog from "./GameDialog.vue";
import WiringGame from "./WiringAdventure.vue";
import BreakoutGame from "./BreakoutAdventure.vue";
import TowerGame from "./TowerAdventure.vue";
import MotaVersions from "./MotaVersions.vue";
import SokobanGame from "./SokobanGame.vue";
import GomokuGame from "./GomokuGame.vue";
import LegionGame from "./LegionGame.vue";
import DefenseGame from "./DefenseGame.vue";
import "../../styles/rest.css";

defineProps<{ game: RestGameId }>();
defineEmits<{ close: [] }>();
const components = { wiring: WiringGame, breakout: BreakoutGame, tower: TowerGame, mota: MotaVersions, sokoban: SokobanGame, gomoku: GomokuGame, legion:LegionGame, defense:DefenseGame };
</script>

<template>
  <GameDialog :game="game" :component="components[game]" @close="$emit('close')" />
</template>
