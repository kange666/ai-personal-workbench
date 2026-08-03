<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import TaskEditor from "../components/TaskEditor.vue";
import TaskRow from "../components/TaskRow.vue";
import { isTauriRuntime, syncTaskSuggestions } from "../services/backend";
import { useWorkbenchStore } from "../stores/workbench";
import type { TaskScope, TaskSource, WorkTask } from "../types/workbench";

const emit = defineEmits<{ "new-task": [] }>();
const store = useWorkbenchStore();
const route = useRoute();
const scope = ref<TaskScope>("day");
const status = ref("all");
const selectedTask = ref<WorkTask | null>(null);
const editorOpen = ref(false);
const searchOpen = ref(false);
const searchQuery = ref("");
const sortMode = ref<"priority" | "date" | "title">("priority");
const sourceFilter = ref<"all" | TaskSource>("all");
const syncing = ref(false);
const message = ref("");
const scopeTabs: Array<[TaskScope, string]> = [["day", "今日任务"], ["week", "本周任务"], ["project", "项目任务"]];
const filteredTasks = computed(() => {
  const priorityOrder = { P0: 0, P1: 1, P2: 2 };
  return store.tasksByScope(scope.value)
    .filter((task) => status.value === "all" || task.status === status.value)
    .filter((task) => sourceFilter.value === "all" || task.source === sourceFilter.value || (sourceFilter.value === "conversation" && task.source === "ai"))
    .filter((task) => !searchQuery.value.trim() || `${task.title} ${task.project} ${task.note}`.toLowerCase().includes(searchQuery.value.trim().toLowerCase()))
    .slice()
    .sort((a, b) => sortMode.value === "title" ? a.title.localeCompare(b.title, "zh-CN") : sortMode.value === "date" ? (b.plannedDate || b.weekStart || b.startDate || b.updatedAt).localeCompare(a.plannedDate || a.weekStart || a.startDate || a.updatedAt) : priorityOrder[a.priority] - priorityOrder[b.priority]);
});
const todayLabel = new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(new Date());
const weekEnd = new Date(); weekEnd.setDate(weekEnd.getDate() + (7 - ((weekEnd.getDay() + 6) % 7)) - 1);
const weekLabel = `${todayLabel}—${new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(weekEnd)}`;

function edit(task: WorkTask) { selectedTask.value = task; editorOpen.value = true; }
function close() { editorOpen.value = false; selectedTask.value = null; }
function cycleSort() { sortMode.value = sortMode.value === "priority" ? "date" : sortMode.value === "date" ? "title" : "priority"; }
async function syncSuggestions() {
  if (!isTauriRuntime()) { message.value = "桌面版会从真实 Codex、报告和测试记录提取建议。"; return; }
  syncing.value = true;
  try { const result=await syncTaskSuggestions(); await store.hydrate(); message.value=`已整理任务建议：Codex ${result.conversationSuggestions} 项、报告 ${result.reportSuggestions} 项、测试 ${result.testSuggestions} 项。`; status.value="draft"; scope.value="project"; }
  catch (cause) { message.value=String(cause); }
  finally { syncing.value=false; }
}
watch([() => route.query.task, () => store.tasks.length], () => { const id = String(route.query.task || ""); const task = store.tasks.find(item => item.id === id); if (task) edit(task); }, { immediate: true });
</script>

<template>
  <div class="view tasks-view">
    <header class="page-header"><div><h1>任务中心</h1><p>统一管理每日、每周与项目任务；自动建议需确认后才进入正式计划</p></div><div><button class="button secondary" :disabled="syncing" @click="syncSuggestions">{{ syncing ? '整理中…' : '↻ 整理建议' }}</button><button class="button secondary" @click="searchOpen = !searchOpen">⌕ 搜索</button><button class="button primary" @click="selectedTask = null; editorOpen = true">＋ 新增任务</button></div></header>
    <div v-if="message" class="scan-message">{{ message }}</div>
    <div v-if="searchOpen" class="inline-search"><span>⌕</span><input v-model="searchQuery" autofocus placeholder="搜索任务标题、项目或备注"><button class="icon-button" @click="searchQuery = ''; searchOpen = false">×</button></div>
    <nav class="scope-tabs"><button v-for="[value,label] in scopeTabs" :key="value" :class="{ active: scope === value && status !== 'draft' }" @click="scope = value; status='all'">{{ label }} <b>{{ store.tasksByScope(value).length }}</b></button><button :class="{ active: status === 'draft' }" @click="scope = 'project'; status = 'draft'">任务建议 <b>{{ store.tasks.filter(t => t.status === 'draft').length }}</b></button></nav>
    <section class="task-workspace panel"><div class="task-toolbar"><div><button v-for="item in [['all','全部'],['todo','待办'],['doing','进行中'],['done','已完成'],['overdue','逾期']]" :key="item[0]" class="filter-chip" :class="{ active: status === item[0] }" @click="status = item[0]">{{ item[1] }}</button></div><div><button class="button secondary small" @click="cycleSort">⇅ 排序：{{ sortMode === 'priority' ? '优先级' : sortMode === 'date' ? '日期' : '标题' }}</button><select v-model="sourceFilter" class="button secondary small"><option value="all">来源：全部</option><option value="manual">人工创建</option><option value="conversation">Codex 对话</option><option value="test">测试问题</option><option value="report">日报/周报</option></select></div></div><div class="task-section-head"><b>{{ status === 'draft' ? '待确认任务建议' : scope === 'day' ? `今天 · ${todayLabel}` : scope === 'week' ? `本周 · ${weekLabel}` : '全部项目任务' }}</b><span>{{ status === 'draft' ? '确认后才进入正式计划' : `${filteredTasks.filter(t => t.status === 'done').length} / ${filteredTasks.length} 已完成` }}</span></div><TaskRow v-for="task in filteredTasks" :key="task.id" :task="task" @toggle="store.toggleTask" @confirm="store.confirmTask" @postpone="store.postponeTask" @edit="edit" /><div v-if="!filteredTasks.length" class="empty-state"><b>当前没有任务</b><p>可以新建任务，或切换其他筛选条件。</p><button class="button primary" @click="editorOpen = true">＋ 新增任务</button></div></section>
    <TaskEditor :open="editorOpen" :task="selectedTask" @close="close" @save="store.addTask" @update="store.updateTask" @remove="store.removeTask" />
  </div>
</template>
