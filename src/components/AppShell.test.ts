import { flushPromises, shallowMount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emptySystemPerformanceSnapshot } from "../utils/systemPerformance";

const backend = vi.hoisted(() => ({
  listRepositoryAssets: vi.fn(), listRunningRepositoryProjects: vi.fn(),
  openRepositoryRuntimeUrl: vi.fn(), stopRepositoryProject: vi.fn(),
}));
vi.mock("../services/backend", async (importOriginal) => {
  const original = await importOriginal<typeof import("../services/backend")>();
  return { ...Object.fromEntries(Object.entries(original).map(([name, value]) => [name,
    typeof value === "function" ? vi.fn(async () => name.startsWith("list") ? [] : {}) : value,
  ])), ...backend, isTauriRuntime: () => true,
  getSystemPerformanceSnapshot: vi.fn(async () => emptySystemPerformanceSnapshot()),
  getWorkSummary: vi.fn(async () => ({ totalMinutes: 0, byProject: [], byType: [], daily: [] })),
  };
});
vi.mock("../stores/workbench", () => ({ useWorkbenchStore: () => ({
  theme: "command", tasks: [], hydrate: vi.fn(), refreshTaskStatuses: vi.fn(),
}) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => vi.fn()) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({
  isMaximized: vi.fn(async () => false), onResized: vi.fn(async () => vi.fn()),
}) }));
vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  isRegistered: vi.fn(async () => true), register: vi.fn(), unregister: vi.fn(),
}));
import AppShell from "./AppShell.vue";

let wrapper: VueWrapper;
const projectPath = "C:/projects/runtime-example";
const process = { projectPath, projectName: "运行示例", processId: 123, status: "failed", errorMessage: "[vite] Internal server error", localUrl: "http://localhost:82/", startedAt: "2026-09-14T01:00:00Z" };
beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  backend.listRepositoryAssets.mockResolvedValue([{ path: projectPath, name: "运行示例", runtimeStatus: "failed", runtimeError: process.errorMessage, behindCount: 0 }]);
  backend.listRunningRepositoryProjects.mockResolvedValue([process]);
  backend.stopRepositoryProject.mockResolvedValue({ message: "已停止" });
});
afterEach(() => { wrapper?.unmount(); vi.useRealTimers(); });

describe("侧边栏项目运行", () => {
  it("编译异常的存活进程仍可打开和停止，恢复时旧异常同步消失", async () => {
    const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/:pathMatch(.*)*", component: { template: "<div/>" } }] });
    await router.push("/projects");
    wrapper = shallowMount(AppShell, { global: { plugins: [router], stubs: { RouterLink: false } } });
    await flushPromises();
    expect(wrapper.get(".rail-projects").text()).toContain("运行示例");
    expect(wrapper.get(".rail-projects").text()).toContain("编译异常");
    expect(wrapper.get(".rail-issues").text()).toContain("运行示例 编译异常");
    await wrapper.get(".rail-project-url").trigger("click");
    expect(backend.openRepositoryRuntimeUrl).toHaveBeenCalledWith("http://localhost:82/");
    backend.listRunningRepositoryProjects.mockResolvedValue([{ ...process, status: "running", errorMessage: "" }]);
    await vi.advanceTimersByTimeAsync(3000);
    await flushPromises();
    expect(wrapper.get(".rail-projects").text()).toContain("运行中");
    expect(wrapper.find(".rail-issues").exists()).toBe(false);
    backend.listRunningRepositoryProjects.mockResolvedValue([]);
    backend.listRepositoryAssets.mockResolvedValue([]);
    await wrapper.get(".rail-project-stop").trigger("click");
    await flushPromises();
    expect(backend.stopRepositoryProject).toHaveBeenCalledWith(projectPath);
    expect(wrapper.find(".rail-projects").exists()).toBe(false);
  });
});
