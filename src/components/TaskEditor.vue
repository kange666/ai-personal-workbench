<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useRouter } from "vue-router";
import type { TaskDraft, WorkTask } from "../types/workbench";
import { APP_BRAND } from "../utils/brand";
import { useWorkbenchStore } from "../stores/workbench";

const props = defineProps<{ open: boolean; task?: WorkTask | null }>();
const router = useRouter();
const emit = defineEmits<{ close: []; saved: [] }>();
const store = useWorkbenchStore();
const saving = ref(false);
const error = ref("");
const today = () => new Date().toLocaleDateString("sv-SE");

const blank = (): TaskDraft => ({
  title: "",
  project: APP_BRAND.name,
  scope: "day",
  priority: "P1",
  plannedDate: today(),
  weekStart: (() => { const date = new Date(); date.setDate(date.getDate() - ((date.getDay() + 6) % 7)); return date.toLocaleDateString("sv-SE"); })(),
  startDate: today(),
  endDate: today(),
  progress: 0,
  note: "",
});
const form = reactive<TaskDraft>(blank());
const isEdit = computed(() => Boolean(props.task));
const sourceLabels = { manual: "人工创建", conversation: "从 Codex 对话提取", test: "从测试问题生成", report: "从日报或周报生成", ai: "从 Codex 对话提取", inbox: "从统一待办箱创建" } as const;

watch(() => [props.open, props.task] as const, () => {
  if (saving.value) return;
  error.value = "";
  const task = props.task;
  const defaults = blank();
  Object.assign(form, task ? {
    title: task.title,
    project: task.project,
    scope: task.scope,
    priority: task.priority,
    plannedDate: task.plannedDate ?? defaults.plannedDate,
    weekStart: task.weekStart ?? defaults.weekStart,
    startDate: task.startDate ?? defaults.startDate,
    endDate: task.endDate ?? defaults.endDate,
    progress: task.progress ?? 0,
    note: task.note,
  } : blank());
}, { immediate: true });

function close() { if (!saving.value) emit("close"); }
async function submit() {
  if (!form.title.trim() || saving.value) return;
  saving.value = true; error.value = "";
  try {
    if (props.task) await store.updateTask(props.task.id, { ...form, plannedDate: form.scope === "day" ? form.plannedDate : undefined, weekStart: form.scope === "week" ? form.weekStart : undefined, startDate: form.scope === "project" ? form.startDate : undefined, endDate: form.scope === "project" ? form.endDate : undefined });
    else await store.addTask({ ...form });
    emit("saved"); emit("close");
  } catch (cause) { error.value = `保存失败，输入已保留，可重试：${String(cause)}`; }
  finally { saving.value = false; }
}
async function remove() {
  if (!props.task || saving.value) return;
  saving.value = true; error.value = "";
  try { await store.removeTask(props.task.id); emit("saved"); emit("close"); }
  catch (cause) { error.value = `删除失败，任务未移除：${String(cause)}`; }
  finally { saving.value = false; }
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
  <div v-if="open" class="editor-backdrop" @click.self="close">
    <aside class="task-editor">
      <header><div><h2>{{ isEdit ? "任务详情" : "新增任务" }}</h2><p>{{ isEdit ? "修改后会同步到首页与日历" : "创建每日、每周或项目任务" }}</p></div><button class="icon-button" :disabled="saving" @click="close">×</button></header>
      <p v-if="error" class="scan-message error" role="alert">{{ error }}</p>
      <label>任务名称<input :disabled="saving" v-model="form.title" autofocus placeholder="输入任务名称" /></label>
      <div class="form-grid">
        <label>任务类型<select :disabled="saving" v-model="form.scope"><option value="day">每日任务</option><option value="week">每周任务</option><option value="project">项目任务</option></select></label>
        <label>优先级<select :disabled="saving" v-model="form.priority"><option>P0</option><option>P1</option><option>P2</option></select></label>
      </div>
      <label>所属项目<input :disabled="saving" v-model="form.project" /></label>
      <label v-if="form.scope === 'day'">计划日期<input :disabled="saving" v-model="form.plannedDate" type="date" /></label>
      <label v-if="form.scope === 'week'">所属周（周一）<input :disabled="saving" v-model="form.weekStart" type="date" /></label>
      <div v-if="form.scope === 'project'" class="form-grid"><label>开始日期<input :disabled="saving" v-model="form.startDate" type="date" /></label><label>结束日期<input :disabled="saving" v-model="form.endDate" type="date" /></label></div>
      <label v-if="form.scope === 'project'">当前进度：{{ form.progress }}%<input :disabled="saving" v-model.number="form.progress" type="range" min="0" max="100" step="5" /></label>
      <label>备注<textarea :disabled="saving" v-model="form.note" rows="5" placeholder="补充任务说明"></textarea></label>
      <div v-if="task" class="source-panel"><span>◎</span><div><b>任务来源</b><small>{{ sourceLabels[task.source] }}{{ task.status === 'draft' ? '，确认后进入正式计划' : '' }}</small></div><button v-if="task.sourceId" class="button secondary small" :disabled="saving" @click="openSource">查看来源</button></div>
      <footer><button v-if="task" class="button danger-button" :disabled="saving" @click="remove">删除</button><span></span><button class="button secondary" :disabled="saving" @click="close">取消</button><button class="button primary" :disabled="saving" @click="submit">{{ saving ? '保存中…' : isEdit ? "保存修改" : "创建任务" }}</button></footer>
    </aside>
  </div>
</template>
