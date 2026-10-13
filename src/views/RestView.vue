<script setup lang="ts">
import { computed, ref } from "vue";
import { restGames, type RestGameId } from "../utils/rest/catalog";
import { gameBest, restStorageWarning } from "../composables/useGameSave";
import GameIcon from "../components/rest/GameIcon.vue";
import RestGameDialog from "../components/rest/RestGameDialog.vue";
import "../styles/rest.css";
const active = ref<RestGameId | null>(null);
const revision = ref(0);
const bests = computed(() => { void revision.value; return Object.fromEntries(restGames.map(game => [game.id, gameBest(game.id)])); });
function close() { active.value = null; queueMicrotask(() => revision.value++); }
</script>
<template>
  <div class="view rest-view">
    <header class="page-header"><div><h1>休息站</h1><p>把忙碌放一放，玩一小会儿。</p></div><span class="rest-local-badge"><i></i> 离线可玩</span></header>
    <div class="rest-selection">
      <div class="rest-grid" aria-label="六款小游戏">
        <button v-for="game in restGames" :key="game.id" class="rest-card" :class="`rest-color-${game.color}`" :aria-label="`打开${game.name}`" @click="active = game.id">
          <span class="rest-icon-tile"><GameIcon :name="game.id" /></span><strong>{{ game.name }}</strong><small>{{ game.hint }}</small>
          <span class="rest-best">{{ bests[game.id] ? `最高分 ${bests[game.id]}` : '开始游玩' }}<span aria-hidden="true">↗</span></span>
        </button>
      </div>
      <p class="rest-footnote">自动续玩 · 本机记录 · 随时暂停</p>
      <p v-if="restStorageWarning" class="rest-storage-warning" role="status">{{ restStorageWarning }}</p>
    </div>
    <RestGameDialog v-if="active" :game="active" @close="close" />
  </div>
</template>
