import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { Router } from "vue-router";
import { backendLoadingEvent, beginBackendPageScope } from "../services/backendRequest";

/** 页面反馈独立于壳层轮询；超时只让用户先查看，不假装请求成功。 */
export function usePageLoading(router: Router) {
  const pageLoading = ref(true);
  const pageLoadingSlow = ref(false);
  const pageLoadingTitle = computed(() => String(router.currentRoute.value.meta.title || "工作数据"));
  let active = 0;
  let startedAt = Date.now();
  let finishTimer = 0;
  let slowTimer = 0;
  const clearTimers = () => { window.clearTimeout(finishTimer); window.clearTimeout(slowTimer); };
  function revealSlowPage() { clearTimers(); pageLoading.value = false; pageLoadingSlow.value = false; }
  function finishWhenReady() {
    window.clearTimeout(finishTimer);
    if (active > 0) return;
    finishTimer = window.setTimeout(() => { if (!active) revealSlowPage(); }, Math.max(160, 620 - (Date.now() - startedAt)));
  }
  function onLoading(event: Event) {
    active = Math.max(0, Number((event as CustomEvent<{ active: number }>).detail?.active || 0));
    if (!pageLoading.value) return;
    if (active) window.clearTimeout(finishTimer); else finishWhenReady();
  }
  window.addEventListener(backendLoadingEvent, onLoading);
  watch(() => router.currentRoute.value.fullPath, (next, previous) => {
    if (previous && router.currentRoute.value.path === "/api-docs" && next.split("?")[0] === previous.split("?")[0]) return;
    clearTimers();
    startedAt = Date.now(); pageLoading.value = true; pageLoadingSlow.value = false;
    beginBackendPageScope();
    slowTimer = window.setTimeout(() => { pageLoadingSlow.value = true; }, 3500);
    finishWhenReady();
  }, { immediate: true, flush: "sync" });
  onBeforeUnmount(() => { clearTimers(); window.removeEventListener(backendLoadingEvent, onLoading); });
  return { pageLoading, pageLoadingSlow, pageLoadingTitle, revealSlowPage };
}
