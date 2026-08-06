<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import { RouterLink, RouterView, useRoute } from "vue-router";
import { useWorkbenchStore } from "../stores/workbench";
import { getCodexQuota, isTauriRuntime, type CodexQuotaSnapshot, type CodexQuotaWindow } from "../services/backend";
import ThemeSwitch from "./ThemeSwitch.vue";
import TaskEditor from "./TaskEditor.vue";
import WorkspaceSearch from "./WorkspaceSearch.vue";

const store = useWorkbenchStore();
const route = useRoute();
const editorOpen = ref(false);
const searchOpen = ref(false);
const refreshing = ref(false);
const quotaOpen = ref(false);
const quotaLoading = ref(false);
const quota = ref<CodexQuotaSnapshot>({ available:false });
const navItems = [
  ["/", "⌂", "工作台"], ["/work-records", "◫", "工作记录"], ["/projects", "◆", "项目资产"], ["/tasks", "✓", "任务中心"], ["/calendar", "▦", "日历"],
  ["/reports", "▤", "报告中心"], ["/testing", "◎", "测试中心"], ["/tokens", "◔", "Token 分析"],
  ["/knowledge", "◇", "知识库"], ["/content", "✦", "内容工坊"], ["/videos", "▶", "视频中心"],
];
const dateText = computed(() => new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "long", day: "numeric", weekday: "short" }).format(new Date()));
function taskOrder(status: string) { return ({overdue:0,blocked:1,doing:2,todo:3} as Record<string,number>)[status] ?? 4; }
const nextTask = computed(() => store.tasks.filter(item => !["done","cancelled","draft"].includes(item.status)).sort((a,b) => taskOrder(a.status)-taskOrder(b.status))[0]);
const draftTasks = computed(() => store.tasks.filter(item=>item.status==="draft"));
const quotaWindows = computed(() => [quota.value.primary, quota.value.secondary].filter((item): item is CodexQuotaWindow => Boolean(item)));
const primaryQuota = computed(() => quota.value.primary || quota.value.secondary);
const remainingText = computed(() => primaryQuota.value ? `${Math.round(primaryQuota.value.remainingPercent)}%` : "--");

watchEffect(() => document.documentElement.dataset.theme = store.theme);
let statusTimer = 0;
let quotaTimer = 0;
function openSearch() { searchOpen.value = true; }
function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); openSearch(); }
  if (event.key === "Escape") quotaOpen.value=false;
}
function quotaPeriod(minutes:number) {
  if (minutes >= 10_080 && minutes % 10_080 === 0) return `${minutes / 10_080}周`;
  if (minutes >= 1_440 && minutes % 1_440 === 0) return `${minutes / 1_440}天`;
  if (minutes >= 60) return `${Math.round(minutes / 60)}小时`;
  return `${minutes}分钟`;
}
function quotaReset(timestamp:number) {
  if (!timestamp) return "重置时间未知";
  return `${new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(new Date(timestamp * 1000))} 重置`;
}
function capturedText(value?:string) {
  if (!value) return "暂无快照时间";
  return `${new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(new Date(value))} 更新`;
}
async function loadQuota() {
  if (!isTauriRuntime()) return;
  quotaLoading.value=true;
  try { quota.value=await getCodexQuota(); }
  catch { quota.value={ available:false }; }
  finally { quotaLoading.value=false; }
}
function closeQuotaOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) quotaOpen.value=false;
}
async function refreshLocalData() { refreshing.value = true; await Promise.all([store.hydrate(), loadQuota()]); refreshing.value = false; }
onMounted(() => {
  void loadQuota();
  statusTimer = window.setInterval(store.refreshTaskStatuses, 60 * 60 * 1000);
  quotaTimer = window.setInterval(loadQuota, 5 * 60 * 1000);
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("open-workbench-search", openSearch);
});
onBeforeUnmount(() => {
  window.clearInterval(statusTimer);
  window.clearInterval(quotaTimer);
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("open-workbench-search", openSearch);
});
</script>

<template>
  <div class="app-shell">
    <aside class="icon-sidebar">
      <RouterLink class="app-mark" to="/">AI</RouterLink>
      <nav><RouterLink v-for="[path, icon, label] in navItems" :key="path" :to="path" :title="label"><span>{{ icon }}</span><em>{{ label }}</em></RouterLink></nav>
      <div class="side-footer"><i></i><RouterLink class="settings-link" to="/settings" title="设置">⚙</RouterLink></div>
    </aside>
    <header class="app-topbar">
      <button class="top-search" title="搜索全部本地数据" @click="openSearch">⌕<span>搜索任务、对话、报告、知识与内容</span><kbd>Ctrl K</kbd></button>
      <span class="top-date">{{ dateText }}</span>
      <div class="top-quota" @focusout="closeQuotaOnBlur">
        <button class="top-quota-trigger" :class="{loading:quotaLoading}" title="查看 Codex 剩余用量" @click="quotaOpen=!quotaOpen">
          <i>◔</i><span><small>剩余用量</small><b>{{ remainingText }}</b></span><em>⌄</em>
        </button>
        <section v-if="quotaOpen" class="quota-popover panel">
          <header><div><b>Codex 剩余用量</b><small>来自最近一次本地额度快照</small></div><button class="icon-button" title="关闭" @click="quotaOpen=false">×</button></header>
          <div v-if="quotaWindows.length" class="quota-window-list">
            <article v-for="(item,index) in quotaWindows" :key="index">
              <div><b>{{ quotaPeriod(item.windowMinutes) }}</b><span>{{ Math.round(item.remainingPercent) }}%</span><small>{{ quotaReset(item.resetsAt) }}</small></div>
              <div class="quota-progress"><i :style="{width:`${item.remainingPercent}%`}"></i></div>
            </article>
          </div>
          <p v-else class="quota-empty">暂无额度快照。使用一次 Codex 后再刷新即可读取。</p>
          <footer><span>{{ capturedText(quota.capturedAt) }}</span><button class="text-button" :disabled="quotaLoading" @click="loadQuota">{{ quotaLoading ? '读取中…' : '刷新' }}</button></footer>
        </section>
      </div>
      <button class="icon-button" title="刷新本地任务和额度数据" :disabled="refreshing" @click="refreshLocalData">{{ refreshing ? '↻' : '◌' }}</button><ThemeSwitch /><RouterLink class="avatar" to="/settings" title="打开本地用户设置">L</RouterLink>
    </header>
    <main class="page-area"><RouterView @new-task="editorOpen = true" /></main>
    <aside class="ai-rail">
      <div class="ai-rail-title"><RouterLink to="/knowledge" title="打开知识库">✦</RouterLink><div><b>AI 助手</b><small>实时工作建议</small></div><i>3</i></div>
      <section><small>下一步建议</small><b>{{ nextTask?.title || '同步今天的 Codex 工作数据' }}</b><p>{{ nextTask ? `${nextTask.project} · ${nextTask.status === 'overdue' ? '已逾期，建议优先处理或顺延' : '来自当前未完成事项'}` : '扫描会话和 Git 后，工作台会生成可确认的下一步建议。' }}</p><RouterLink class="button primary small link-button" :to="nextTask ? `/tasks?task=${nextTask.id}` : '/tokens'">{{ nextTask ? '查看任务' : '前往数据同步' }}</RouterLink></section>
      <section><small>今日内容候选</small><b>5 个小众科技选题</b><p>完整口播、分镜和 AI 画面提示词已准备，可直接选择。</p><RouterLink class="button secondary small link-button" to="/content">打开内容工坊</RouterLink></section>
      <section><small>待确认</small><div v-for="item in draftTasks.slice(0,3)" :key="item.id" class="rail-task"><i></i><span><b>{{ item.title }}</b><small>{{ item.source === 'test' ? '测试问题' : item.source === 'report' ? '报告建议' : 'Codex 建议' }}</small></span></div><p v-if="!draftTasks.length">暂无待确认建议。</p><RouterLink v-if="draftTasks.length" class="button secondary small link-button" to="/tasks">确认任务建议</RouterLink></section>
      <section><small>任务概览</small><div class="focus-box"><b>{{ store.pendingCount }}</b><span>未完成 · {{ store.completedCount }} 已完成</span></div></section>
    </aside>
    <TaskEditor :open="editorOpen" @close="editorOpen = false" @save="store.addTask" />
    <WorkspaceSearch :open="searchOpen" @close="searchOpen = false" />
  </div>
</template>
