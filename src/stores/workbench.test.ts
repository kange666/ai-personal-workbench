import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { useWorkbenchStore } from "./workbench";
import * as backend from "../services/backend";
import type { TaskDraft, WorkTask } from "../types/workbench";

vi.mock("../services/backend", () => ({ isTauriRuntime: vi.fn(() => false), saveTask: vi.fn(), listTasks: vi.fn(), deleteTask: vi.fn() }));

describe("workbench store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(backend.isTauriRuntime).mockReturnValue(false);
    vi.mocked(backend.saveTask).mockResolvedValue(undefined);
    vi.mocked(backend.deleteTask).mockResolvedValue(undefined);
    localStorage.clear();
    setActivePinia(createPinia());
  });

  it("creates a daily task with a concrete date", () => {
    const store = useWorkbenchStore();
    store.addTask({ title: "新增每日任务", project: "星枢工作台", scope: "day", priority: "P1", plannedDate: "2026-08-04", weekStart: "2026-08-03", startDate: "2026-08-03", endDate: "2026-08-07", progress: 0, note: "测试" });
    expect(store.tasks[0]).toMatchObject({ title: "新增每日任务", scope: "day", plannedDate: "2026-08-04", weekStart: undefined });
  });

  it("creates a weekly task without forcing a day", () => {
    const store = useWorkbenchStore();
    store.addTask({ title: "新增每周任务", project: "星枢工作台", scope: "week", priority: "P1", plannedDate: "2026-08-04", weekStart: "2026-08-03", startDate: "2026-08-03", endDate: "2026-08-07", progress: 0, note: "整周管理" });
    expect(store.tasks[0]).toMatchObject({ title: "新增每周任务", scope: "week", plannedDate: undefined, weekStart: "2026-08-03" });
  });

  it("keeps project start, end and progress for the gantt view", () => {
    const store = useWorkbenchStore();
    store.addTask({ title: "甘特项目任务", project: "星枢工作台", scope: "project", priority: "P0", plannedDate: "2026-08-04", weekStart: "2026-08-03", startDate: "2026-08-05", endDate: "2026-08-12", progress: 35, note: "用于甘特联动" });
    expect(store.tasks[0]).toMatchObject({ scope: "project", plannedDate: undefined, weekStart: undefined, startDate: "2026-08-05", endDate: "2026-08-12", progress: 35 });
  });

  it("toggles completion and persists the selected theme", async () => {
    const store = useWorkbenchStore();
    const id = store.tasks[0].id;
    store.toggleTask(id);
    expect(store.tasks[0].status).toBe("done");
    expect(store.tasks[0].completedAt).toBeTruthy();
    store.setTheme("warm");
    await nextTick();
    expect(localStorage.getItem("ai-workbench.theme.v1")).toBe("warm");
  });

  it("marks an unfinished task overdue after its deadline", () => {
    const store = useWorkbenchStore();
    store.addTask({ title: "已过期任务", project: "测试", scope: "day", priority: "P1", plannedDate: "2000-01-01", weekStart: "2000-01-03", startDate: "2000-01-01", endDate: "2000-01-02", progress: 0, note: "" });
    expect(store.tasks[0].status).toBe("overdue");
  });
});

describe("桌面任务持久化", () => {
  const draft: TaskDraft = { title:"测试任务",project:"匿名项目",scope:"day",priority:"P1",plannedDate:"2099-01-01",weekStart:"2099-01-01",startDate:"2099-01-01",endDate:"2099-01-02",progress:0,note:"保留输入" };
  const persisted: WorkTask = { ...draft,id:"persisted",status:"todo",source:"manual",createdAt:"2026-01-01",updatedAt:"2026-01-01" };
  beforeEach(() => {
    localStorage.clear(); vi.clearAllMocks(); setActivePinia(createPinia());
    vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
    vi.mocked(backend.listTasks).mockResolvedValue([persisted]);
    vi.mocked(backend.saveTask).mockResolvedValue(undefined);
    vi.mocked(backend.deleteTask).mockResolvedValue(undefined);
  });
  it("正式空库不读浏览器演示任务、更不灌入", async () => {
    localStorage.setItem("ai-workbench.tasks.v1", JSON.stringify([persisted]));
    vi.mocked(backend.listTasks).mockResolvedValue([]);
    const store = useWorkbenchStore();
    expect(store.tasks).toEqual([]);
    await store.hydrate();
    expect(store.tasks).toEqual([]);
    expect(backend.saveTask).not.toHaveBeenCalled();
  });
  it("新增等待保存成功；失败不插入列表，可以原样重试", async () => {
    const store = useWorkbenchStore();
    vi.mocked(backend.saveTask).mockRejectedValueOnce(new Error("busy"));
    await expect(store.addTask(draft)).rejects.toThrow("busy");
    expect(store.tasks).toEqual([]);
    let finish!: () => void;
    vi.mocked(backend.saveTask).mockReturnValueOnce(new Promise<void>(resolve => { finish=resolve; }));
    const pending=store.addTask(draft);
    expect(store.tasks).toEqual([]);
    finish(); await pending;
    expect(store.tasks[0].title).toBe(draft.title);
  });
  it("编辑、完成和删除失败不改变当前任务", async () => {
    const store = useWorkbenchStore(); await store.hydrate();
    vi.mocked(backend.saveTask).mockRejectedValue(new Error("readonly"));
    await expect(store.updateTask(persisted.id,{title:"未保存"})).rejects.toThrow();
    await expect(store.toggleTask(persisted.id)).rejects.toThrow();
    await expect(store.postponeTask(persisted.id)).rejects.toThrow();
    vi.mocked(backend.deleteTask).mockRejectedValue(new Error("busy"));
    await expect(store.removeTask(persisted.id)).rejects.toThrow();
    expect(store.tasks[0]).toEqual(persisted);
  });
  it("确认建议保存失败时仍保持草稿和原来源", async () => {
    const task={...persisted,status:"draft",source:"ai"} as WorkTask;
    vi.mocked(backend.listTasks).mockResolvedValue([task]);
    const store=useWorkbenchStore(); await store.hydrate();
    vi.mocked(backend.saveTask).mockRejectedValueOnce(new Error("busy"));
    await expect(store.confirmTask(task.id)).rejects.toThrow();
    expect(store.tasks[0]).toEqual(task);
  });
  it("读取失败保留原列表并给出可见错误", async () => {
    const store = useWorkbenchStore(); await store.hydrate();
    vi.mocked(backend.listTasks).mockRejectedValue(new Error("busy"));
    await expect(store.hydrate()).rejects.toThrow();
    expect(store.tasks[0]).toEqual(persisted); expect(store.loadError).toContain("读取失败");
  });
  it("较早发起的读取不能覆盖刚保存成功的新任务", async () => {
    const store = useWorkbenchStore();
    let finish!: (tasks: WorkTask[]) => void;
    vi.mocked(backend.listTasks).mockReturnValueOnce(new Promise(resolve => { finish=resolve; }));
    const read=store.hydrate(); await store.addTask(draft); finish([persisted]); await read;
    expect(store.tasks).toHaveLength(2);
    expect(store.tasks.some(task=>task.id===persisted.id)).toBe(true);
  });
});
