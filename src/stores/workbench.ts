import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import { deleteTask as deleteTaskFromBackend, isTauriRuntime, listTasks, saveTask } from "../services/backend";
import type { TaskDraft, TaskScope, ThemeMode, WorkTask } from "../types/workbench";

const TASKS_KEY = "ai-workbench.tasks.v1";
const THEME_KEY = "ai-workbench.theme.v1";

const demoTasks: WorkTask[] = [
  { id: "task-1", title: "完成工作台正式版基础界面", project: "星枢 ASTRION", scope: "day", status: "doing", priority: "P0", plannedDate: "2026-08-03", progress: 60, note: "完成 B 布局、双主题和任务基础链路。", source: "manual", createdAt: "2026-08-03T09:20:00+08:00", updatedAt: "2026-08-03T10:24:00+08:00" },
  { id: "task-2", title: "整理本周开发总结", project: "客户端升级", scope: "week", status: "todo", priority: "P1", weekStart: "2026-08-03", progress: 20, note: "汇总本周完成事项与遗留问题。", source: "manual", createdAt: "2026-08-03T09:40:00+08:00", updatedAt: "2026-08-03T09:40:00+08:00" },
  { id: "task-3", title: "补充部署异常处理说明", project: "自动部署", scope: "day", status: "overdue", priority: "P1", plannedDate: "2026-08-02", progress: 30, note: "保留原计划日期，等待确认是否顺延。", source: "manual", createdAt: "2026-08-02T16:20:00+08:00", updatedAt: "2026-08-02T16:20:00+08:00" },
  { id: "task-5", title: "核对 DeepSeek 接口字段", project: "星枢 ASTRION", scope: "day", status: "done", priority: "P1", plannedDate: "2026-08-03", progress: 100, note: "确认兼容 Chat Completions 和 JSON 输出。", source: "manual", createdAt: "2026-08-03T08:30:00+08:00", updatedAt: "2026-08-03T10:30:00+08:00", completedAt: "2026-08-03T10:30:00+08:00" },
];

function readTasks(): WorkTask[] {
  try {
    const value = localStorage.getItem(TASKS_KEY);
    const stored = value ? JSON.parse(value) as WorkTask[] : structuredClone(demoTasks);
    return stored.filter((task) => task.id !== "task-4").map((task) => ({ ...task, progress: task.progress ?? (task.status === "done" ? 100 : 0) }));
  } catch {
    return structuredClone(demoTasks).filter((task) => task.id !== "task-4");
  }
}

export const useWorkbenchStore = defineStore("workbench", () => {
  // 桌面端以数据库为唯一来源，不把浏览器演示数据写进正式库。
  const tasks = ref<WorkTask[]>(isTauriRuntime() ? [] : readTasks());
  const loadError = ref("");
  const hydrating = ref(false);
  const pendingTaskIds = ref<string[]>([]);
  let mutationRevision = 0;
  const changedTaskRevisions = new Map<string, number>();
  let hydratePending: Promise<void> | undefined;
  const theme = ref<ThemeMode>((localStorage.getItem(THEME_KEY) as ThemeMode | null) ?? "command");

  const todayText = () => { const now = new Date(); return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`; };
  const currentWeekStart = () => { const now = new Date(); now.setHours(0, 0, 0, 0); now.setDate(now.getDate() - ((now.getDay() + 6) % 7)); return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`; };
  const todayTasks = computed(() => tasks.value.filter((task) => task.scope === "day" && task.plannedDate === todayText()));
  const weekTasks = computed(() => tasks.value.filter((task) => task.scope === "week" && task.weekStart === currentWeekStart()));
  const projectTasks = computed(() => tasks.value.filter((task) => task.scope === "project"));
  const pendingCount = computed(() => tasks.value.filter((task) => task.status !== "done" && task.status !== "cancelled").length);
  const completedCount = computed(() => tasks.value.filter((task) => task.status === "done").length);

  function deadline(task: WorkTask) {
    if (task.scope === "day") return task.plannedDate;
    if (task.scope === "project") return task.endDate;
    if (!task.weekStart) return undefined;
    const end = new Date(`${task.weekStart}T00:00:00`); end.setDate(end.getDate() + 6);
    return `${end.getFullYear()}-${String(end.getMonth() + 1).padStart(2, "0")}-${String(end.getDate()).padStart(2, "0")}`;
  }

  function normalizedTask(task: WorkTask): WorkTask {
    if (["done", "cancelled", "draft", "blocked"].includes(task.status)) return task;
    const due = deadline(task);
    const overdue = Boolean(due && due < todayText());
    return { ...task, status: overdue ? "overdue" : task.status === "overdue" ? "todo" : task.status };
  }

  async function persistTask(task: WorkTask) {
    const next = normalizedTask(task);
    if (pendingTaskIds.value.includes(next.id)) throw new Error("该任务正在保存，请稍后重试。");
    pendingTaskIds.value.push(next.id);
    try {
      if (isTauriRuntime()) await saveTask(next);
      mutationRevision += 1;
      changedTaskRevisions.set(next.id, mutationRevision);
      const index = tasks.value.findIndex(item => item.id === next.id);
      if (index < 0) tasks.value.unshift(next);
      else tasks.value[index] = next;
    } finally { pendingTaskIds.value = pendingTaskIds.value.filter(id => id !== next.id); }
  }

  async function refreshTaskStatuses() {
    const updates = tasks.value.filter(task => normalizedTask(task).status !== task.status && !pendingTaskIds.value.includes(task.id));
    const results = await Promise.allSettled(updates.map(task => persistTask({ ...task, updatedAt: new Date().toISOString() })));
    if (results.some(result => result.status === "rejected")) loadError.value = "部分任务状态未保存，已保留原状态，请重试读取。";
  }

  function hydrate(): Promise<void> {
    if (!isTauriRuntime()) return Promise.resolve();
    if (hydratePending) return hydratePending;
    hydrating.value = true;
    hydratePending = (async () => { try {
      const revision = mutationRevision;
      const persisted = await listTasks();
      // 较旧的查询仍提供其他任务，但不能覆盖期间已保存/删除的任务。
      const merged = new Map(persisted.map(task => [task.id, task]));
      for (const [id, changedAt] of changedTaskRevisions) if (changedAt > revision) {
        const task = tasks.value.find(item => item.id === id);
        if (task) merged.set(id, task); else merged.delete(id);
      }
      tasks.value = [...merged.values()];
      loadError.value = "";
      await refreshTaskStatuses();
    } catch (error) {
      loadError.value = `任务读取失败，已保留上次成功数据：${String(error)}`;
      throw error;
    } })().finally(() => { hydrating.value = false; hydratePending = undefined; });
    return hydratePending;
  }

  function setTheme(value: ThemeMode) {
    theme.value = value;
  }

  async function addTask(draft: TaskDraft) {
    const now = new Date().toISOString();
    const task: WorkTask = {
      id: crypto.randomUUID(),
      title: draft.title.trim(),
      project: draft.project.trim() || "未归类项目",
      scope: draft.scope,
      status: "todo",
      priority: draft.priority,
      plannedDate: draft.scope === "day" ? draft.plannedDate : undefined,
      weekStart: draft.scope === "week" ? draft.weekStart : undefined,
      startDate: draft.scope === "project" ? draft.startDate : undefined,
      endDate: draft.scope === "project" ? draft.endDate : undefined,
      progress: draft.progress,
      note: draft.note.trim(),
      source: "manual",
      createdAt: now,
      updatedAt: now,
    };
    await persistTask(task);
  }

  async function updateTask(id: string, patch: Partial<WorkTask>) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) throw new Error("任务已不存在，请重新读取。");
    await persistTask({ ...task, ...patch, id, updatedAt: new Date().toISOString() });
  }

  async function toggleTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) return;
    const done = task.status === "done";
    await updateTask(id, { status: done ? "todo" : "done", progress: done ? Math.min(task.progress, 95) : 100, completedAt: done ? undefined : new Date().toISOString() });
  }

  async function confirmTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task || task.status !== "draft") return;
    await updateTask(id, { status: "todo", source: task.source === "ai" ? "conversation" : task.source });
  }

  async function postponeTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) return;
    const shift = (value: string | undefined, days: number) => { if (!value) return value; const date=new Date(`${value}T00:00:00`); date.setDate(date.getDate()+days); return date.toLocaleDateString("sv-SE"); };
    const patch: Partial<WorkTask> = { status: "todo" };
    if (task.scope === "day") patch.plannedDate = shift(todayText(), 1);
    else if (task.scope === "week") patch.weekStart = shift(task.weekStart || currentWeekStart(), 7);
    else { patch.startDate=shift(task.startDate,7); patch.endDate=shift(task.endDate,7); }
    await updateTask(id, patch);
  }

  async function removeTask(id: string) {
    if (pendingTaskIds.value.includes(id)) throw new Error("该任务正在保存，请稍后重试。");
    pendingTaskIds.value.push(id);
    try {
      if (isTauriRuntime()) await deleteTaskFromBackend(id);
      mutationRevision += 1;
      changedTaskRevisions.set(id, mutationRevision);
      tasks.value = tasks.value.filter((item) => item.id !== id);
    } finally { pendingTaskIds.value = pendingTaskIds.value.filter(item => item !== id); }
  }

  function tasksByScope(scope: TaskScope) {
    return tasks.value.filter((task) => task.scope === scope);
  }

  watch(tasks, (value) => { if (!isTauriRuntime()) localStorage.setItem(TASKS_KEY, JSON.stringify(value)); }, { deep: true });
  watch(theme, (value) => localStorage.setItem(THEME_KEY, value));

  if (!isTauriRuntime()) void refreshTaskStatuses();

  return { tasks, loadError, hydrating, pendingTaskIds, theme, todayTasks, weekTasks, projectTasks, pendingCount, completedCount, hydrate, setTheme, addTask, updateTask, toggleTask, confirmTask, postponeTask, removeTask, tasksByScope, refreshTaskStatuses };
});
