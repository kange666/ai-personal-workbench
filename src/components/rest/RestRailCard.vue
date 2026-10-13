<script setup lang="ts">
import { defineAsyncComponent, ref } from "vue";
import { restGames, type RestGameId } from "../../utils/rest/catalog";
import GameIcon from "./GameIcon.vue";

// 点击后才加载游戏和弹窗，避免增加工作台启动时的开销。
const RestGameDialog = defineAsyncComponent(() => import("./RestGameDialog.vue").then(module => module.default));
const active = ref<RestGameId | null>(null);
</script>

<template>
  <div class="rail-rest-card" role="group" aria-label="休息站">
    <button v-for="game in restGames" :key="game.id" type="button"
      class="icon-button rail-rest-game" :class="`rail-rest-${game.id}`"
      :title="game.name" :aria-label="`打开${game.name}`" @click="active = game.id">
      <GameIcon :name="game.id" />
    </button>
  </div>
  <RestGameDialog v-if="active" :game="active" @close="active = null" />
</template>
