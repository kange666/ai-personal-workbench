<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watchEffect } from "vue";
import { RouterLink, RouterView, useRouter } from "vue-router";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useWorkbenchStore } from "../stores/workbench";
import { getCodexQuota, getEmailNotificationStatus, getVipStatus, isTauriRuntime, listNotifications, markAllNotificationsRead, markNotificationRead, retryFailedEmails, setCodexEmailEnabled, syncCodexNotifications, type CodexQuotaSnapshot, type CodexQuotaWindow, type EmailNotificationStatus, type VipStatus, type WorkbenchNotification } from "../services/backend";
import { getAlmanac } from "../utils/almanac";
import appLogo from "../assets/app-logo.png";
import HeaderIcon from "./HeaderIcon.vue";
import NavIcon from "./NavIcon.vue";
import WindowControlIcon from "./WindowControlIcon.vue";
import ThemeSwitch from "./ThemeSwitch.vue";
import TaskEditor from "./TaskEditor.vue";
import WorkspaceSearch from "./WorkspaceSearch.vue";
import NotificationDrawer from "./NotificationDrawer.vue";

const store = useWorkbenchStore();
const router = useRouter();
const editorOpen = ref(false);
const searchOpen = ref(false);
const refreshing = ref(false);
const quotaOpen = ref(false);
const quotaLoading = ref(false);
const quota = ref<CodexQuotaSnapshot>({ available:false });
const notificationOpen = ref(false);
const notificationLoading = ref(false);
const notifications = ref<WorkbenchNotification[]>([]);
const notificationToast = ref<WorkbenchNotification | null>(null);
const selectedNotification = ref<WorkbenchNotification | null>(null);
const emailStatus = ref<EmailNotificationStatus>({ configured:false, enabled:false, state:"unconfigured", maskedEmail:"", afterTime:"17:40", lastError:"", retryingCount:0, failedCount:0 });
const emailLoading = ref(false);
const vipStatus = ref<VipStatus>({ active:false });
const windowMaximized = ref(false);
const navItems = [
  { path:"/", icon:"home", label:"工作台" },
  { path:"/work-records", icon:"records", label:"工作记录" },
  { path:"/projects", icon:"projects", label:"项目资产" },
  { path:"/calendar", icon:"calendar", label:"工作日历" },
  { path:"/reports", icon:"reports", label:"报告中心" },
  { path:"/testing", icon:"testing", label:"测试中心" },
  { path:"/tokens", icon:"tokens", label:"Token 分析" },
  { path:"/tapd", icon:"tapd", label:"TAPD 工作" },
  { path:"/content", icon:"content", label:"内容工坊", vip:true },
  { path:"/videos", icon:"videos", label:"视频中心", vip:true },
  { path:"/knowledge", icon:"knowledge", label:"知识库" },
];
const visibleNavItems = computed(() => navItems.filter(item => !item.vip || vipStatus.value.active));
const dateText = computed(() => new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "long", day: "numeric", weekday: "short" }).format(new Date()));
const todayIso = new Date().toLocaleDateString("sv-SE");
const todayDay = new Date().getDate();
const todayAlmanac = computed(() => getAlmanac(todayIso));
function taskOrder(status: string) { return ({overdue:0,blocked:1,doing:2,todo:3} as Record<string,number>)[status] ?? 4; }
const nextTask = computed(() => store.tasks.filter(item => !["done","cancelled","draft"].includes(item.status)).sort((a,b) => taskOrder(a.status)-taskOrder(b.status))[0]);
const draftTasks = computed(() => store.tasks.filter(item=>item.status==="draft"));
const quotaWindows = computed(() => [quota.value.primary, quota.value.secondary].filter((item): item is CodexQuotaWindow => Boolean(item)));
const primaryQuota = computed(() => quota.value.primary || quota.value.secondary);
const remainingText = computed(() => primaryQuota.value ? `${Math.round(primaryQuota.value.remainingPercent)}%` : "--");
const unreadCount = computed(() => notifications.value.filter(item => !item.isRead).length);
const emailButtonText = computed(() => emailStatus.value.state === "error" ? "异常" : emailStatus.value.enabled ? "开" : emailStatus.value.state === "unverified" ? "待验" : emailStatus.value.state === "unconfigured" ? "未配" : "关");
const emailTooltip = computed(() => [
  "Codex完成邮件通知",
  `每天${emailStatus.value.afterTime}后生效`,
  `收件人：${emailStatus.value.maskedEmail || "尚未配置"}`,
  emailStatus.value.lastError || "",
].filter(Boolean).join("\n"));

watchEffect(() => document.documentElement.dataset.theme = store.theme);
let statusTimer = 0;
let quotaTimer = 0;
let notificationTimer = 0;
let notificationToastTimer = 0;
let emailTimer = 0;
let emailUnlisten: UnlistenFn | undefined;
let vipUnlisten: UnlistenFn | undefined;
let windowResizeUnlisten: UnlistenFn | undefined;
function openSearch() { searchOpen.value = true; }
function handleKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") { event.preventDefault(); openSearch(); }
  if (event.key === "Escape") { quotaOpen.value=false; notificationOpen.value=false; selectedNotification.value=null; }
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
async function loadNotifications(sync = true) {
  if (!isTauriRuntime() || notificationLoading.value) return;
  notificationLoading.value=true;
  try {
    if (sync) await syncCodexNotifications();
    const knownIds = new Set(notifications.value.map(item => item.id));
    const latest = await listNotifications();
    const newestUnread = latest.find(item => !item.isRead && !knownIds.has(item.id));
    notifications.value=latest;
    if (newestUnread) {
      notificationToast.value=newestUnread;
      window.clearTimeout(notificationToastTimer);
      notificationToastTimer=window.setTimeout(() => notificationToast.value=null, 8000);
    }
  }
  catch (error) { console.error("读取 Codex 完成提醒失败", error); }
  finally { notificationLoading.value=false; }
}
async function loadEmailStatus() {
  if (!isTauriRuntime()) return;
  try { emailStatus.value=await getEmailNotificationStatus(); }
  catch (cause) { console.error("读取 Codex 邮件通知状态失败", cause); }
}
async function loadVipStatus() {
  if (!isTauriRuntime()) return;
  try { vipStatus.value=await getVipStatus(); }
  catch (cause) { console.error("读取 VIP 状态失败", cause); }
}
async function syncWindowState() {
  if (isTauriRuntime()) windowMaximized.value=await getCurrentWindow().isMaximized();
}
async function minimizeWindow() { if (isTauriRuntime()) await getCurrentWindow().minimize(); }
async function toggleMaximizeWindow() {
  if (!isTauriRuntime()) return;
  await getCurrentWindow().toggleMaximize();
  await syncWindowState();
}
async function closeWindow() { if (isTauriRuntime()) await getCurrentWindow().close(); }
async function startWindowDragging(event:MouseEvent) {
  if (!isTauriRuntime() || event.button !== 0) return;
  const target=event.target as HTMLElement;
  if (target.closest("button,a,input,select,textarea,[role='button'],.quota-popover,.notification-popover")) return;
  await getCurrentWindow().startDragging();
}
async function toggleEmailNotification() {
  if (emailLoading.value) return;
  if (["unconfigured","unverified"].includes(emailStatus.value.state)) {
    window.alert(emailStatus.value.state === "unconfigured" ? "请先在设置中保存 QQ 邮箱和 SMTP 授权码。" : "请先在设置中发送测试邮件，验证 QQ 邮箱和 SMTP 授权码。");
    await router.push("/settings");
    return;
  }
  if (emailStatus.value.state === "error") {
    const detail=emailStatus.value.lastError || "邮件发送失败，请检查网络或 SMTP 授权码。";
    if (window.confirm(`${detail}\n\n是否立即重试失败邮件？`)) {
      emailLoading.value=true;
      try { emailStatus.value=await retryFailedEmails(); }
      catch (cause) { window.alert(String(cause)); }
      finally { emailLoading.value=false; }
    }
    return;
  }
  emailLoading.value=true;
  try { emailStatus.value=await setCodexEmailEnabled(!emailStatus.value.enabled); }
  catch (cause) { window.alert(String(cause)); await router.push("/settings"); }
  finally { emailLoading.value=false; }
}
function notificationTime(value:string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(date);
}
async function openNotification(item:WorkbenchNotification) {
  if (!item.isRead) {
    await markNotificationRead(item.id);
    item.isRead=true;
    item.readAt=new Date().toISOString();
  }
  notificationToast.value=null;
  window.clearTimeout(notificationToastTimer);
  notificationOpen.value=false;
  selectedNotification.value=item;
}
function openNotificationFromPage(event:Event) {
  const item = (event as CustomEvent<WorkbenchNotification>).detail;
  if (item) void openNotification(item);
}
async function readAllNotifications() {
  if (!unreadCount.value) return;
  await markAllNotificationsRead();
  const readAt = new Date().toISOString();
  notifications.value.forEach(item => { item.isRead=true; item.readAt=readAt; });
}
function toggleNotifications() {
  notificationOpen.value=!notificationOpen.value;
  if (notificationOpen.value) void loadNotifications(false);
}
function closeQuotaOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) quotaOpen.value=false;
}
function closeNotificationOnBlur(event:FocusEvent) {
  const next = event.relatedTarget as Node | null;
  if (!(event.currentTarget as HTMLElement).contains(next)) notificationOpen.value=false;
}
async function refreshLocalData() { refreshing.value = true; await Promise.all([store.hydrate(), loadQuota(), loadNotifications(true), loadEmailStatus(), loadVipStatus()]); refreshing.value = false; }
onMounted(() => {
  void loadQuota();
  void loadNotifications(true);
  void loadEmailStatus();
  void loadVipStatus();
  if (isTauriRuntime()) void listen<EmailNotificationStatus>("codex-email-status-changed", event => { emailStatus.value=event.payload; }).then(unlisten => { emailUnlisten=unlisten; });
  if (isTauriRuntime()) void listen<VipStatus>("vip-status-changed", event => { vipStatus.value=event.payload; }).then(unlisten => { vipUnlisten=unlisten; });
  if (isTauriRuntime()) {
    void syncWindowState();
    void getCurrentWindow().onResized(() => void syncWindowState()).then(unlisten => { windowResizeUnlisten=unlisten; });
  }
  statusTimer = window.setInterval(store.refreshTaskStatuses, 60 * 60 * 1000);
  quotaTimer = window.setInterval(loadQuota, 5 * 60 * 1000);
  notificationTimer = window.setInterval(() => void loadNotifications(true), 15 * 1000);
  emailTimer = window.setInterval(() => void loadEmailStatus(), 15 * 1000);
  window.addEventListener("keydown", handleKeydown);
  window.addEventListener("open-workbench-search", openSearch);
  window.addEventListener("open-workbench-notification", openNotificationFromPage);
});
onBeforeUnmount(() => {
  window.clearInterval(statusTimer);
  window.clearInterval(quotaTimer);
  window.clearInterval(notificationTimer);
  window.clearInterval(emailTimer);
  window.clearTimeout(notificationToastTimer);
  emailUnlisten?.();
  vipUnlisten?.();
  windowResizeUnlisten?.();
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("open-workbench-search", openSearch);
  window.removeEventListener("open-workbench-notification", openNotificationFromPage);
});
</script>

<template>
  <div class="app-shell">
    <aside class="icon-sidebar">
      <RouterLink class="app-mark" to="/"><img :src="appLogo" alt="AI 个人工作台"></RouterLink>
      <nav><RouterLink v-for="item in visibleNavItems" :key="item.path" :to="item.path" :title="item.label"><NavIcon :name="item.icon" /><em>{{ item.label }}</em><b v-if="item.vip" class="vip-badge">VIP</b></RouterLink></nav>
      <div class="side-footer"><RouterLink class="settings-link" to="/settings" title="设置"><NavIcon name="settings" /><em>设置</em></RouterLink></div>
    </aside>
    <header class="app-topbar" data-tauri-drag-region @mousedown="startWindowDragging">
      <button class="top-search" title="搜索全部本地数据" @click="openSearch">⌕<span>搜索任务、对话、报告、知识与内容</span><kbd>Ctrl K</kbd></button>
      <span class="topbar-drag-zone" data-tauri-drag-region aria-hidden="true"></span>
      <span class="top-date" data-tauri-drag-region>{{ dateText }}</span>
      <span class="top-runtime-status" data-tauri-drag-region title="AI 个人工作台正在本机运行"><i data-tauri-drag-region></i><span data-tauri-drag-region>本地运行</span></span>
      <div class="top-quota" @focusout="closeQuotaOnBlur">
        <button class="top-quota-trigger" :class="{loading:quotaLoading}" title="查看 Codex 剩余用量" @click="quotaOpen=!quotaOpen">
          <i class="header-icon-box"><HeaderIcon name="quota" /></i><span><small>剩余用量</small><b>{{ remainingText }}</b></span><em>⌄</em>
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
      <div class="top-notifications" @focusout="closeNotificationOnBlur">
        <button class="icon-button header-action-button notification-trigger" :class="{ active:notificationOpen }" title="查看 Codex 完成提醒" @click="toggleNotifications"><HeaderIcon name="notification" /><b v-if="unreadCount">{{ unreadCount > 99 ? '99+' : unreadCount }}</b></button>
        <section v-if="notificationOpen" class="notification-popover panel">
          <header><div><b>Codex 完成提醒</b><small>{{ unreadCount ? `${unreadCount} 条未读` : '全部已读' }}</small></div><button class="text-button" :disabled="!unreadCount" @click="readAllNotifications">全部已读</button></header>
          <div class="notification-list">
            <button v-for="item in notifications" :key="item.id" class="notification-item" :class="{ unread:!item.isRead }" @click="openNotification(item)"><i></i><span><b>{{ item.title }}</b><p>{{ item.body }}</p><small>{{ notificationTime(item.createdAt) }} · {{ item.isRead ? '已读' : '未读' }}</small></span></button>
            <p v-if="!notifications.length && !notificationLoading" class="notification-empty">暂无完成提醒。Codex 对话任务完成后会显示在这里。</p>
            <p v-if="notificationLoading" class="notification-empty">正在读取完成提醒…</p>
          </div>
        </section>
      </div>
      <button class="top-email-toggle" :class="[emailStatus.state,{loading:emailLoading}]" :title="emailTooltip" :disabled="emailLoading" @click="toggleEmailNotification">✉ <span>邮件</span><b>{{ emailButtonText }}</b></button>
      <button class="icon-button header-action-button refresh-action" :class="{ loading:refreshing }" title="刷新本地任务和额度数据" :disabled="refreshing" @click="refreshLocalData"><HeaderIcon name="refresh" /></button><ThemeSwitch />
      <div class="window-controls" aria-label="窗口控制">
        <button type="button" title="最小化" aria-label="最小化" @click="minimizeWindow"><WindowControlIcon name="minimize" /></button>
        <button type="button" :title="windowMaximized ? '还原' : '最大化'" :aria-label="windowMaximized ? '还原' : '最大化'" @click="toggleMaximizeWindow"><WindowControlIcon :name="windowMaximized ? 'restore' : 'maximize'" /></button>
        <button type="button" class="window-close" title="关闭到系统托盘" aria-label="关闭到系统托盘" @click="closeWindow"><WindowControlIcon name="close" /></button>
      </div>
    </header>
    <main class="page-area"><RouterView @new-task="editorOpen = true" /></main>
    <aside class="ai-rail">
      <div class="ai-rail-title"><RouterLink to="/knowledge" title="打开知识库">✦</RouterLink><div><b>AI 助手</b><small>实时工作建议</small></div><i>3</i></div>
      <section class="rail-almanac"><small>今日黄历 · {{ todayAlmanac.lunarDate }}</small><div class="rail-almanac-main"><b>{{ todayDay }}</b><span><strong>{{ todayAlmanac.duty }} · {{ todayAlmanac.luck }}</strong><em>{{ todayAlmanac.ganZhi }}</em></span></div><p>{{ todayAlmanac.heavenlyGod }}值日 · {{ todayAlmanac.clash }} · {{ todayAlmanac.sha }}</p><div class="rail-almanac-yiji"><span><i>宜</i>{{ todayAlmanac.yi.slice(0,3).join(' · ') || '诸事不宜' }}</span><span><i>忌</i>{{ todayAlmanac.ji.slice(0,3).join(' · ') || '诸事不忌' }}</span></div><RouterLink class="button secondary small link-button" :to="`/calendar?date=${todayIso}`">查看详细黄历</RouterLink></section>
      <section><small>下一步建议</small><b>{{ nextTask?.title || '同步今天的 Codex 工作数据' }}</b><p>{{ nextTask ? `${nextTask.project} · ${nextTask.status === 'overdue' ? '已逾期，建议优先处理或顺延' : '来自当前未完成事项'}` : '扫描会话和 Git 后，工作台会生成可确认的下一步建议。' }}</p><RouterLink class="button primary small link-button" :to="nextTask ? `/calendar?tab=tasks&task=${nextTask.id}` : '/tokens'">{{ nextTask ? '查看任务' : '前往数据同步' }}</RouterLink></section>
      <section><small>待确认</small><div v-for="item in draftTasks.slice(0,3)" :key="item.id" class="rail-task"><i></i><span><b>{{ item.title }}</b><small>{{ item.source === 'test' ? '测试问题' : item.source === 'report' ? '报告建议' : 'Codex 建议' }}</small></span></div><p v-if="!draftTasks.length">暂无待确认建议。</p><RouterLink v-if="draftTasks.length" class="button secondary small link-button" to="/calendar?tab=tasks">确认任务建议</RouterLink></section>
      <section><small>任务概览</small><div class="focus-box"><b>{{ store.pendingCount }}</b><span>未完成 · {{ store.completedCount }} 已完成</span></div></section>
    </aside>
    <TaskEditor :open="editorOpen" @close="editorOpen = false" @save="store.addTask" />
    <WorkspaceSearch :open="searchOpen" @close="searchOpen = false" />
    <NotificationDrawer :notification="selectedNotification" @close="selectedNotification=null" />
    <button v-if="notificationToast" class="notification-toast panel" @click="openNotification(notificationToast)"><i></i><span><small>Codex 任务完成</small><b>{{ notificationToast.title.replace(/^Codex 任务已完成：/, '') }}</b><p>{{ notificationToast.body }}</p></span><em>查看</em></button>
  </div>
</template>
