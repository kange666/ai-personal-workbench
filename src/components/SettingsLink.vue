<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { RouterLink } from "vue-router";
import { isTauriRuntime } from "../services/backend";
import { latestUpdateStatus, refreshUpdateStatus } from "../services/updateStatus";
import NavIcon from "./NavIcon.vue";

const hasUpdate = computed(() => latestUpdateStatus.value?.updateAvailable === true);
const label = computed(() => hasUpdate.value
  ? `设置 · 发现新版本 V${latestUpdateStatus.value!.latestVersion.replace(/^[vV]/, "")}，点击查看更新`
  : "设置");
let checkTimer: number | undefined;

function checkInBackground() {
  // 后台检查失败保持静默，详细错误仍由设置页展示。
  void refreshUpdateStatus().catch(() => {});
}
onMounted(() => {
  if (!isTauriRuntime()) return;
  checkInBackground();
  // 工作台常驻托盘时，每小时检查一次；不下载、不安装更新。
  checkTimer = window.setInterval(checkInBackground, 60 * 60 * 1000);
});
onBeforeUnmount(() => window.clearInterval(checkTimer));
</script>

<template>
  <RouterLink class="settings-link" to="/settings" :title="label" :aria-label="label">
    <NavIcon name="settings" /><em>设置</em>
    <svg v-if="hasUpdate" class="settings-update-badge" viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="10" r="9" />
      <path d="M10 14V6m-3 3 3-3 3 3" />
    </svg>
  </RouterLink>
</template>

<style scoped>
.settings-update-badge {
  width: 18px;
  height: 18px;
  flex: 0 0 18px;
  margin-left: auto;
  color: var(--primary);
}
.settings-update-badge circle { fill: var(--primary-soft); stroke: currentColor; stroke-width: 1; }
.settings-update-badge path { fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
</style>
