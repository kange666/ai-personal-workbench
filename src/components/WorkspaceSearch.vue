<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { isTauriRuntime, searchWorkspace, type WorkspaceSearchResult } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();
const router = useRouter();
const store = useWorkbenchStore();
const input = ref<HTMLInputElement | null>(null);
const query = ref("");
const results = ref<WorkspaceSearchResult[]>([]);
const loading = ref(false);
const error = ref("");

watch(() => props.open, async (open) => {
  if (!open) return;
  query.value = "";
  results.value = [];
  error.value = "";
  await nextTick();
  input.value?.focus();
});

async function search() {
  const value = query.value.trim();
  if (!value) { results.value = []; return; }
  loading.value = true;
  error.value = "";
  try {
    if (isTauriRuntime()) results.value = await searchWorkspace(value);
    else results.value = store.tasks.filter(task => `${task.title} ${task.project} ${task.note}`.includes(value)).map(task => ({ id: task.id, kind: "任务", title: task.title, subtitle: task.project, date: task.updatedAt.slice(0, 10), route: `/tasks?task=${task.id}` }));
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function openResult(result: WorkspaceSearchResult) {
  await router.push(result.route);
  emit("close");
}
</script>

<template>
  <div v-if="open" class="search-backdrop" @click.self="emit('close')" @keydown.esc="emit('close')">
    <section class="workspace-search panel">
      <header><b>⌕ 全局搜索</b><button class="icon-button" @click="emit('close')">×</button></header>
      <label><span>⌕</span><input ref="input" v-model="query" placeholder="搜索任务、Codex 对话、报告、知识和内容" @keyup.enter="search"><button class="button primary" :disabled="loading" @click="search">{{ loading ? '搜索中…' : '搜索' }}</button></label>
      <p v-if="error" class="search-error">{{ error }}</p>
      <div class="workspace-search-results">
        <button v-for="result in results" :key="`${result.kind}-${result.id}`" @click="openResult(result)"><i>{{ result.kind === '任务' ? '✓' : result.kind === 'Codex 对话' ? '◔' : result.kind === '报告' ? '▤' : result.kind === '内容' ? '✦' : '◇' }}</i><span><b>{{ result.title }}</b><small>{{ result.kind }} · {{ result.subtitle }}<template v-if="result.date"> · {{ result.date }}</template></small></span><em>›</em></button>
        <p v-if="query.trim() && !loading && !results.length && !error" class="panel-empty">没有找到匹配内容。</p>
        <p v-if="!query.trim()" class="search-hint">输入关键词后按 Enter，所有结果都可点击进入对应数据页。</p>
      </div>
    </section>
  </div>
</template>
