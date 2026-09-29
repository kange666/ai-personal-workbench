<script setup lang="ts">
import type { AsyncSectionState } from "../composables/useAsyncSection";
defineProps<{ state: AsyncSectionState }>();
defineEmits<{ retry: [] }>();
function time(value: string) { return new Date(value).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" }); }
</script>
<template>
  <div class="async-section-status" :class="{ error: Boolean(state.error) }" :role="state.error ? 'alert' : 'status'" @click.stop>
    <span v-if="state.loading">{{ state.updatedAt ? '正在更新，当前保留上次数据…' : '正在读取…' }}</span>
    <span v-else-if="state.error">{{ state.error }}</span>
    <small v-if="state.updatedAt">上次成功 {{ time(state.updatedAt) }}</small>
    <button v-if="state.error" class="button secondary small" :disabled="state.loading" @click.stop="$emit('retry')">重试</button>
  </div>
</template>
<style scoped>
.async-section-status{display:flex;align-items:center;flex-wrap:wrap;gap:6px;padding:4px 12px;color:var(--muted);font-size:10px;overflow-wrap:anywhere}.async-section-status.error{color:var(--danger)}.async-section-status small{color:var(--muted)}
</style>
