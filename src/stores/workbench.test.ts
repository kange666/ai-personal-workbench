import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { useWorkbenchStore } from "./workbench";

describe("workbench store", () => {
  beforeEach(() => {
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
