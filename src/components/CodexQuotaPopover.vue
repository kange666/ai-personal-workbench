<script setup lang="ts">
import { computed } from "vue";
import type { CodexQuotaSnapshot, CodexQuotaWindow } from "../services/backend";

const props = defineProps<{ quota: CodexQuotaSnapshot; loading: boolean }>();
defineEmits<{ close: []; refresh: [] }>();
const quotaWindows = computed(() => [props.quota.primary, props.quota.secondary].filter((item): item is CodexQuotaWindow => Boolean(item)));
const quotaFreshnessText = computed(() => ({ fresh:"刚刚更新", recent:"近期快照", stale:"快照较旧" } as Record<string,string>)[props.quota.freshness] || "暂无有效快照");
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
</script>

<template>
        <section class="quota-popover panel">
          <header><div><b>Codex 剩余用量</b><small>来源：{{ quota.sourceFile === 'Codex 账户接口' ? 'Codex 账户接口' : '本地 Codex 额度记录' }}</small></div><button class="icon-button" title="关闭" @click="$emit('close')">×</button></header>
          <div v-if="quotaWindows.length" class="quota-window-list">
            <article v-for="(item,index) in quotaWindows" :key="index">
              <div><b>{{ quotaPeriod(item.windowMinutes) }}</b><span>{{ Math.round(item.remainingPercent) }}%</span><small>{{ quotaReset(item.resetsAt) }}</small></div>
              <div class="quota-progress"><i :style="{width:`${item.remainingPercent}%`}"></i></div>
            </article>
          </div>
          <p v-else class="quota-empty">当前未读取到有效额度数据，可刷新重试；工作台不会将旧周期额度当作当前额度。</p>
          <div class="quota-reset-credit">
            <span><b>可用额度重置</b></span>
            <strong v-if="quota.resetCreditsAvailable != null">{{ quota.resetCreditsAvailable }} 次</strong>
            <em v-else>{{ quota.resetCreditsStatus === 'error' ? '读取失败，请刷新重试' : quota.resetCreditsStatus === 'unavailable' ? '官方未提供该数据' : loading ? '正在读取…' : '暂未读取到' }}</em>
          </div>
          <p v-if="quota.available && quota.sourceFile !== 'Codex 账户接口'" class="quota-source" :title="quota.selectionReason">来源：{{ quota.sourceFile || '本地 Codex 日志' }}</p>
          <footer><span>{{ capturedText(quota.capturedAt) }} · {{ quotaFreshnessText }}</span><button class="text-button" :disabled="loading" @click="$emit('refresh')">{{ loading ? '读取中…' : '刷新' }}</button></footer>
        </section>
</template>
