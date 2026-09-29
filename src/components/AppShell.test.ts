import { flushPromises, shallowMount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emptySystemPerformanceSnapshot } from "../utils/systemPerformance";

const backend = vi.hoisted(() => ({
  listRepositoryAssets: vi.fn(), listRunningRepositoryProjects: vi.fn(),
  openRepositoryRuntimeUrl: vi.fn(), stopRepositoryProject: vi.fn(),
  getCodexQuota: vi.fn(), listBackgroundTaskStatus: vi.fn(), listNotifications: vi.fn(),
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
  Object.defineProperty(document,"visibilityState",{value:"visible",configurable:true});
  backend.getCodexQuota.mockResolvedValue({available:false,freshness:"",selectionReason:"",resetCreditsAvailable:null,resetCreditsStatus:"unavailable"});
  backend.listBackgroundTaskStatus.mockResolvedValue([]);
  backend.listNotifications.mockResolvedValue([]);
  backend.listRepositoryAssets.mockResolvedValue([{ path: projectPath, name: "运行示例", runtimeStatus: "failed", runtimeError: process.errorMessage, behindCount: 0 }]);
  backend.listRunningRepositoryProjects.mockResolvedValue([process]);
  backend.stopRepositoryProject.mockResolvedValue({ message: "已停止" });
});
afterEach(() => { wrapper?.unmount(); vi.useRealTimers(); });

describe("侧边栏项目运行", () => {
  async function render() {
    const router=createRouter({ history:createMemoryHistory(),routes:[{path:"/:pathMatch(.*)*",component:{template:"<div/>"}}] });
    await router.push("/projects");
    wrapper=shallowMount(AppShell,{global:{plugins:[router],stubs:{RouterLink:false,CodexQuotaPopover:false}}});
    await flushPromises(); return router;
  }
  it.each([
    [0,"available","0 次"],
    [null,"unavailable","官方未提供该数据"],
    [null,"error","读取失败，请刷新重试"],
  ])("重置次数 %s / %s 显示准确分支", async (count,status,label) => {
    backend.getCodexQuota.mockResolvedValue({available:true,freshness:"fresh",selectionReason:"",resetCreditsAvailable:count,resetCreditsStatus:status});
    await render(); await wrapper.get(".top-quota-trigger").trigger("click");
    expect(wrapper.get(".quota-reset-credit").text()).toContain(label);
  });
  it("慢页面可先查看，已关闭遮罩不被轮询状态重新打开", async () => {
    const router=await render(); await vi.advanceTimersByTimeAsync(1200);
    await router.push("/reports");
    window.dispatchEvent(new CustomEvent("workbench-backend-loading",{detail:{active:1}}));
    await vi.advanceTimersByTimeAsync(3600);
    expect(wrapper.get(".workbench-page-loader").text()).toContain("先查看页面");
    await wrapper.get(".workbench-page-loader button").trigger("click");
    window.dispatchEvent(new CustomEvent("workbench-backend-loading",{detail:{active:1}}));
    await flushPromises(); expect(wrapper.find(".workbench-page-loader").exists()).toBe(false);
  });
  it.each(["Codex 账户接口", "anonymous-session.jsonl"])("额度来源 %s 不重复官方来源，日志回退仍显示文件", async (sourceFile) => {
    backend.getCodexQuota.mockResolvedValue({available:true,freshness:"fresh",selectionReason:"",sourceFile});
    await render(); await wrapper.get(".top-quota-trigger").trigger("click");
    expect(wrapper.find(".quota-source").exists()).toBe(sourceFile !== "Codex 账户接口");
    if (sourceFile === "Codex 账户接口") expect(wrapper.get(".quota-popover").text().match(/Codex 账户接口/g)).toHaveLength(1);
  });
  it("后台错误只进入异常列表，全部异常可展开且低频更新", async () => {
    backend.listBackgroundTaskStatus.mockResolvedValue([{id:"one",name:"同步",errorMessage:"连接失败"},{id:"two",name:"扫描",errorMessage:"读取失败"}]);
    await render();
    expect(wrapper.get(".rail-issues").text()).toContain("查看全部 3 项异常");
    await wrapper.get(".rail-issues .button").trigger("click");
    expect(wrapper.findAll(".rail-issue")).toHaveLength(3);
    await vi.advanceTimersByTimeAsync(5000); expect(backend.listBackgroundTaskStatus).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(25000); expect(backend.listBackgroundTaskStatus).toHaveBeenCalledTimes(2);
  });
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
    await vi.advanceTimersByTimeAsync(5000);
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
