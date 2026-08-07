<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import type { WorkbenchNotification } from "../services/backend";

const props = defineProps<{ notification: WorkbenchNotification | null }>();
const emit = defineEmits<{ close: [] }>();
const router = useRouter();

const completedAt = computed(() => {
  const value = props.notification?.createdAt;
  if (!value) return "-";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat("zh-CN", {
    year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit",
  }).format(date);
});

async function openSource() {
  const route = props.notification?.route;
  emit("close");
  if (route) await router.push(route);
}
</script>

<template>
  <div v-if="notification" class="activity-backdrop" @click.self="emit('close')">
    <aside class="activity-drawer panel notification-detail-drawer">
      <header>
        <div><h2>{{ notification.title }}</h2><p>Codex 对话任务完成详情</p></div>
        <button class="icon-button" title="关闭" @click="emit('close')">×</button>
      </header>
      <section class="notification-detail-status">
        <span class="notification-read-state">✓ 已完成</span>
        <span>{{ notification.isRead ? "已读" : "未读" }}</span>
      </section>
      <dl>
        <div><dt>完成时间</dt><dd>{{ completedAt }}</dd></div>
        <div><dt>来源</dt><dd>Codex 对话</dd></div>
        <div><dt>会话 ID</dt><dd class="notification-source-id">{{ notification.sourceId || "未关联" }}</dd></div>
      </dl>
      <section class="notification-detail-section">
        <h3>完成摘要</h3>
        <p>{{ notification.body }}</p>
      </section>
      <section class="notification-detail-section output-section">
        <h3>Codex 输出</h3>
        <pre>{{ notification.output || notification.body || "Codex 未返回文本输出。" }}</pre>
      </section>
      <footer class="activity-actions">
        <button class="button secondary" @click="openSource">查看会话数据</button>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>
    </aside>
  </div>
</template>

<style scoped>
.notification-detail-drawer>header>div{min-width:0}.notification-detail-drawer>header h2{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.notification-detail-drawer{width:620px}.notification-detail-status{display:flex;gap:8px;padding:14px 16px}.notification-detail-status span{padding:6px 9px;border-radius:999px;background:var(--surface-2);color:var(--muted);font-size:10px}.notification-detail-status .notification-read-state{background:color-mix(in srgb,var(--success) 14%,transparent);color:var(--success)}.notification-source-id{max-width:330px;overflow-wrap:anywhere}.notification-detail-section{margin:16px;padding:15px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2)}.notification-detail-section h3{margin:0 0 10px;font-size:13px}.notification-detail-section p{margin:0;color:var(--muted);line-height:1.7;white-space:pre-wrap}.notification-detail-section pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.75;color:var(--text)}.output-section{background:color-mix(in srgb,var(--primary) 4%,var(--surface-2))}.notification-detail-drawer>footer{justify-content:flex-end}
</style>
