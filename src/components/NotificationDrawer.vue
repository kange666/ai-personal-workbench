<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { reviewNotification, type WorkbenchNotification } from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";

const props = defineProps<{ notification: WorkbenchNotification | null }>();
const emit = defineEmits<{ close: []; reviewed: [id:string, decision:"accepted"|"follow_up", note:string] }>();
const router = useRouter();
const reviewNote = ref("");
const loading = ref(false);
const isTapdItem = computed(() => props.notification?.kind === "tapd_item");
const isJenkinsPublish = computed(() => props.notification?.kind === "jenkins_publish");
const detailSubtitle = computed(() => isTapdItem.value ? "TAPD 工作项变更详情" : "Codex 对话任务完成详情");
const sourceLabel = computed(() => isTapdItem.value ? "TAPD · 安全生产管理" : "Codex 对话");
const detailTitle = computed(() => compactDetailTitle(props.notification?.title || "消息详情"));
watch(()=>props.notification?.id,()=>{ reviewNote.value=props.notification?.reviewNote || ""; });

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
async function review(decision:"accepted"|"follow_up") {
  if (!props.notification || loading.value) return;
  loading.value=true;
  try { await reviewNotification(props.notification.id,decision,reviewNote.value); emit("reviewed",props.notification.id,decision,reviewNote.value); }
  finally { loading.value=false; }
}
</script>

<template>
  <div v-if="notification" class="activity-backdrop" @click.self="emit('close')">
    <aside class="activity-drawer panel notification-detail-drawer">
      <header>
        <div><h2 :title="notification.title">{{ detailTitle }}</h2><p>{{ detailSubtitle }}</p></div>
        <button class="icon-button" title="关闭" @click="emit('close')">×</button>
      </header>
      <section class="notification-detail-status">
        <span class="notification-read-state">{{ isTapdItem ? "TAPD 更新" : isJenkinsPublish ? "Jenkins 发布" : "✓ 已完成" }}</span>
        <span>{{ notification.isRead ? "已读" : "未读" }}</span>
      </section>
      <dl>
        <div><dt>{{ isTapdItem ? "变更时间" : "完成时间" }}</dt><dd>{{ completedAt }}</dd></div>
        <div><dt>来源</dt><dd>{{ sourceLabel }}</dd></div>
        <div><dt>{{ isTapdItem ? "工作项 ID" : isJenkinsPublish ? "发布记录 ID" : "会话 ID" }}</dt><dd class="notification-source-id">{{ notification.sourceId || "未关联" }}</dd></div>
      </dl>
      <section class="notification-detail-section">
        <h3>{{ isTapdItem ? "变更摘要" : "完成摘要" }}</h3>
        <p>{{ notification.body }}</p>
      </section>
      <section class="notification-detail-section output-section">
        <h3>{{ isTapdItem ? "工作项详情" : "Codex 输出" }}</h3>
        <pre>{{ notification.output || notification.body || (isTapdItem ? "TAPD 未返回工作项详情。" : "Codex 未返回文本输出。") }}</pre>
      </section>
      <section v-if="!isTapdItem && !isJenkinsPublish" class="notification-detail-section notification-review-section">
        <h3>结果处理</h3>
        <p>这里只记录你是否认可结果，不会自动创建每日任务，也不会自动提交代码。</p>
        <textarea v-model="reviewNote" rows="3" placeholder="可选：记录需要继续处理的地方"></textarea>
        <div><button class="button secondary" :disabled="loading" @click="review('follow_up')">需要继续处理</button><button class="button primary" :disabled="loading" @click="review('accepted')">结果可用</button></div>
        <small v-if="notification.reviewStatus!=='pending'">当前结论：{{ notification.reviewStatus==='accepted'?'结果可用':'需要继续处理' }}</small>
      </section>
      <footer class="activity-actions">
        <button class="button secondary" @click="openSource">{{ isTapdItem ? "查看 TAPD 工作项" : "查看会话数据" }}</button>
        <button class="button primary" @click="emit('close')">关闭</button>
      </footer>
    </aside>
  </div>
</template>

<style scoped>
.notification-detail-drawer>header>div{min-width:0}.notification-detail-drawer>header h2{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.notification-detail-drawer{width:620px}.notification-detail-status{display:flex;gap:8px;padding:14px 16px}.notification-detail-status span{padding:6px 9px;border-radius:999px;background:var(--surface-2);color:var(--muted);font-size:10px}.notification-detail-status .notification-read-state{background:color-mix(in srgb,var(--success) 14%,transparent);color:var(--success)}.notification-source-id{max-width:330px;overflow-wrap:anywhere}.notification-detail-section{margin:16px;padding:15px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2)}.notification-detail-section h3{margin:0 0 10px;font-size:13px}.notification-detail-section p{margin:0;color:var(--muted);line-height:1.7;white-space:pre-wrap}.notification-detail-section pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;line-height:1.75;color:var(--text)}.output-section{background:color-mix(in srgb,var(--primary) 4%,var(--surface-2))}.notification-detail-drawer>footer{justify-content:flex-end}
.notification-review-section textarea{width:100%;margin:10px 0;border:1px solid var(--line);border-radius:8px;background:var(--surface-3);color:var(--text);padding:9px;resize:vertical}.notification-review-section>div{display:flex;justify-content:flex-end;gap:8px}.notification-review-section small{display:block;margin-top:9px;color:var(--muted)}
</style>
