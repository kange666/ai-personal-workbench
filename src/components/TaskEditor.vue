<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import { useRouter } from "vue-router";
import type { TaskDraft, WorkTask } from "../types/workbench";

const props = defineProps<{ open: boolean; task?: WorkTask | null }>();
const router = useRouter();
const emit = defineEmits<{ close: []; save: [draft: TaskDraft]; update: [id: string, patch: Partial<WorkTask>]; remove: [id: string] }>();

const blank = (): TaskDraft => ({
  title: "",
  project: "星枢工作台",
  scope: "day",
  priority: "P1",
  plannedDate: "2026-08-03",
  weekStart: "2026-08-03",
  startDate: "2026-08-03",
  endDate: "2026-08-07",
  progress: 0,
  note: "",
});
const form = reactive<TaskDraft>(blank());
const isEdit = computed(() => Boolean(props.task));
const sourceLabels = { manual: "人工创建", conversation: "从 Codex 对话提取", test: "从测试问题生成", report: "从日报或周报生成", ai: "从 Codex 对话提取", inbox: "从统一待办箱创建" } as const;

watch(() => [props.open, props.task] as const, () => {
  const task = props.task;
  Object.assign(form, task ? {
    title: task.title,
    project: task.project,
    scope: task.scope,
    priority: task.priority,
    plannedDate: task.plannedDate ?? "2026-08-03",
    weekStart: task.weekStart ?? "2026-08-03",
    startDate: task.startDate ?? "2026-08-03",
    endDate: task.endDate ?? "2026-08-07",
    progress: task.progress ?? 0,
    note: task.note,
  } : blank());
}, { immediate: true });

function submit() {
  if (!form.title.trim()) return;
  if (props.task) emit("update", props.task.id, { ...form, plannedDate: form.scope === "day" ? form.plannedDate : undefined, weekStart: form.scope === "week" ? form.weekStart : undefined, startDate: form.scope === "project" ? form.startDate : undefined, endDate: form.scope === "project" ? form.endDate : undefined });
  else emit("save", { ...form });
  emit("close");
}
function openSource() {
  if (!props.task?.sourceId) return;
  if (["conversation","ai"].includes(props.task.source)) void router.push(`/tokens?conversation=${props.task.sourceId}`);
  else if (props.task.source === "test") void router.push("/testing");
  else if (props.task.source === "report") void router.push(`/reports?report=${props.task.sourceId}`);
  emit("close");
}
</script>

<template>
  <div v-if="open" class="editor-backdrop" @click.self="emit('close')">
    <aside class="task-editor">
      <header><div><h2>{{ isEdit ? "任务详情" : "新增任务" }}</h2><p>{{ isEdit ? "修改后会同步到首页与日历" : "创建每日、每周或项目任务" }}</p></div><button class="icon-button" @click="emit('close')">×</button></header>
      <label>任务名称<input v-model="form.title" autofocus placeholder="输入任务名称" /></label>
      <div class="form-grid">
        <label>任务类型<select v-model="form.scope"><option value="day">每日任务</option><option value="week">每周任务</option><option value="project">项目任务</option></select></label>
        <label>优先级<select v-model="form.priority"><option>P0</option><option>P1</option><option>P2</option></select></label>
      </div>
      <label>所属项目<input v-model="form.project" /></label>
      <label v-if="form.scope === 'day'">计划日期<input v-model="form.plannedDate" type="date" /></label>
      <label v-if="form.scope === 'week'">所属周（周一）<input v-model="form.weekStart" type="date" /></label>
      <div v-if="form.scope === 'project'" class="form-grid"><label>开始日期<input v-model="form.startDate" type="date" /></label><label>结束日期<input v-model="form.endDate" type="date" /></label></div>
      <label v-if="form.scope === 'project'">当前进度：{{ form.progress }}%<input v-model.number="form.progress" type="range" min="0" max="100" step="5" /></label>
      <label>备注<textarea v-model="form.note" rows="5" placeholder="补充任务说明"></textarea></label>
      <div v-if="task" class="source-panel"><span>◎</span><div><b>任务来源</b><small>{{ sourceLabels[task.source] }}{{ task.status === 'draft' ? '，确认后进入正式计划' : '' }}</small></div><button v-if="task.sourceId" class="button secondary small" @click="openSource">查看来源</button></div>
      <footer><button v-if="task" class="button danger-button" @click="emit('remove', task.id); emit('close')">删除</button><span></span><button class="button secondary" @click="emit('close')">取消</button><button class="button primary" @click="submit">{{ isEdit ? "保存修改" : "创建任务" }}</button></footer>
    </aside>
  </div>
</template>
