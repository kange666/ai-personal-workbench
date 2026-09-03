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
  const tasks = ref<WorkTask[]>(readTasks());
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

  function refreshTaskStatuses() {
    const now = new Date();
    const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
    for (const task of tasks.value) {
      if (["done", "cancelled", "draft", "blocked"].includes(task.status)) continue;
      const due = deadline(task);
      if (due && due < today && task.status !== "overdue") {
        task.status = "overdue";
        task.updatedAt = new Date().toISOString();
        if (isTauriRuntime()) void saveTask(task);
      } else if ((!due || due >= today) && task.status === "overdue") {
        task.status = "todo";
        task.updatedAt = new Date().toISOString();
        if (isTauriRuntime()) void saveTask(task);
      }
    }
  }

  async function hydrate() {
    if (!isTauriRuntime()) return;
    try {
      const persisted = await listTasks();
      if (persisted.length) tasks.value = persisted;
      else await Promise.all(tasks.value.map((task) => saveTask(task)));
      refreshTaskStatuses();
    } catch (error) {
      console.error("读取本地数据库失败，暂时使用浏览器存储。", error);
    }
  }

  function setTheme(value: ThemeMode) {
    theme.value = value;
  }

  function addTask(draft: TaskDraft) {
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
    tasks.value.unshift(task);
    refreshTaskStatuses();
    if (isTauriRuntime()) void saveTask(task);
  }

  function updateTask(id: string, patch: Partial<WorkTask>) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) return;
    Object.assign(task, patch, { updatedAt: new Date().toISOString() });
    refreshTaskStatuses();
    if (isTauriRuntime()) void saveTask(task);
  }

  function toggleTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) return;
    const done = task.status === "done";
    task.status = done ? "todo" : "done";
    task.progress = done ? Math.min(task.progress, 95) : 100;
    task.completedAt = done ? undefined : new Date().toISOString();
    task.updatedAt = new Date().toISOString();
    refreshTaskStatuses();
    if (isTauriRuntime()) void saveTask(task);
  }

  function confirmTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task || task.status !== "draft") return;
    task.status = "todo";
    task.updatedAt = new Date().toISOString();
    if (task.source === "ai") task.source = "conversation";
    if (isTauriRuntime()) void saveTask(task);
  }

  function postponeTask(id: string) {
    const task = tasks.value.find((item) => item.id === id);
    if (!task) return;
    const shift = (value: string | undefined, days: number) => { if (!value) return value; const date=new Date(`${value}T00:00:00`); date.setDate(date.getDate()+days); return date.toLocaleDateString("sv-SE"); };
    if (task.scope === "day") task.plannedDate = shift(todayText(), 1);
    else if (task.scope === "week") task.weekStart = shift(task.weekStart || currentWeekStart(), 7);
    else { task.startDate=shift(task.startDate,7); task.endDate=shift(task.endDate,7); }
    task.status="todo"; task.updatedAt=new Date().toISOString();
    if (isTauriRuntime()) void saveTask(task);
  }

  function removeTask(id: string) {
    tasks.value = tasks.value.filter((item) => item.id !== id);
    if (isTauriRuntime()) void deleteTaskFromBackend(id);
  }

  function tasksByScope(scope: TaskScope) {
    return tasks.value.filter((task) => task.scope === scope);
  }

  watch(tasks, (value) => localStorage.setItem(TASKS_KEY, JSON.stringify(value)), { deep: true });
  watch(theme, (value) => localStorage.setItem(THEME_KEY, value));

  refreshTaskStatuses();

  return { tasks, theme, todayTasks, weekTasks, projectTasks, pendingCount, completedCount, hydrate, setTheme, addTask, updateTask, toggleTask, confirmTask, postponeTask, removeTask, tasksByScope, refreshTaskStatuses };
});
